//! `in` 表达式检查与展开。
//!
//! 区分两种语义（与 AST 节点一一对应）：
//!
//! - [`crate::in_expr::check_in_expression`]：`in` 右侧为**括号集合**
//!   （`InSet`），成员判断。集合内范围元素展开为离散成员：
//!   `x in (0..<10)` ≡ `x == 0 || x == 1 || ... || x == 9`。
//! - [`crate::in_expr::check_in_range_expression`]：`in` 右侧为**裸范围**
//!   （`InRange`），区间判断：`x in 0..<10` ≡ `x >= 0 && x < 10`。

use rlyeh_ast::{AstExpr, ExprKind};
use rlyeh_hir::{HirBinaryOp, HirExpr};
use rlyeh_lexer::Span;

use crate::check_expr;
use crate::context::TypeContext;
use crate::error::TypeError;
use crate::types::Type;

/// 集合离散展开的最大成员数（超过则报错，防止内存爆炸）。
const MAX_SET_MEMBERS: i128 = 1_000_000;

/// 展开为离散链的最小成员数（少于该值直接生成 `==` / `!=` 链）。
const OR_CHAIN_THRESHOLD: usize = 5;

/// 检查并展开集合成员判断（`x in (1, 3, 5)` / `x in (0..<10)` / `x not in (...)`）。
///
/// 集合内范围元素要求上下界为编译期整数常量（时间字面量归一化为分钟值），
/// 展开为离散成员；成员数较少时生成 `==`（或 `!=`）链，较多时生成
/// [`HirExpr::SetLookup`]。
pub(crate) fn check_in_expression(
    ctx: &mut TypeContext,
    value: AstExpr,
    set: Vec<AstExpr>,
    negated: bool,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let (v_hir, v_ty) = check_expr::infer_expr(ctx, &value)?;

    let mut members: Vec<HirExpr> = Vec::new();
    for element in set {
        match &*element.kind {
            ExprKind::Range {
                lower,
                upper,
                lower_inclusive,
                upper_inclusive,
            } => {
                // 范围元素：离散展开（要求编译期整数常量）
                // P8：集合字面量内的范围须显式边界（省略 `..` 仅切片支持）
                let lower = lower.as_ref().ok_or_else(|| TypeError::Unsupported {
                    what: "集合内范围缺少下界（省略边界 `..` 仅切片 `v[..]` 支持）".to_string(),
                    span,
                })?;
                let upper = upper.as_ref().ok_or_else(|| TypeError::Unsupported {
                    what: "集合内范围缺少上界（省略边界 `..` 仅切片 `v[..]` 支持）".to_string(),
                    span,
                })?;
                let lo = const_eval_int(lower, span)?;
                let hi = const_eval_int(upper, span)?;
                let vals = expand_range(lo, hi, *lower_inclusive, *upper_inclusive, span)?;
                for v in vals {
                    check_member_type(&v_ty, &Type::I64, span)?;
                    members.push(HirExpr::IntLiteral(v));
                }
            }
            _ => {
                // 单值元素
                let (m_hir, m_ty) = check_expr::infer_expr(ctx, &element)?;
                check_member_type(&v_ty, &m_ty, span)?;
                members.push(m_hir);
            }
        }
    }

    let hir = optimize_in_set(v_hir, members, negated);
    Ok((hir, Type::Bool))
}

/// 检查并展开裸范围区间判断（`x in 0..<10` / `x not in 0...10`）。
///
/// 端点不要求为常量（运行时求值），按开闭标志生成
/// [`HirExpr::RangeCheck`]。
pub(crate) fn check_in_range_expression(
    ctx: &mut TypeContext,
    value: AstExpr,
    range: AstExpr,
    negated: bool,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let ExprKind::Range {
        lower,
        upper,
        lower_inclusive,
        upper_inclusive,
    } = &*range.kind
    else {
        return Err(TypeError::Unsupported {
            what: "`in` 右侧应为范围表达式 `a..<b` / `a...b` / `a<..b`".to_string(),
            span,
        });
    };

    // P8：`in` 范围判断须显式边界（省略 `..` 仅切片 `v[..]` 支持）
    let lower = lower.as_ref().ok_or_else(|| TypeError::Unsupported {
        what: "`in` 范围缺少下界（省略边界 `..` 仅切片 `v[..]` 支持）".to_string(),
        span,
    })?;
    let upper = upper.as_ref().ok_or_else(|| TypeError::Unsupported {
        what: "`in` 范围缺少上界（省略边界 `..` 仅切片 `v[..]` 支持）".to_string(),
        span,
    })?;

    let (v_hir, v_ty) = check_expr::infer_expr(ctx, &value)?;
    let (lo_hir, lo_ty) = check_expr::infer_expr(ctx, lower)?;
    let (hi_hir, hi_ty) = check_expr::infer_expr(ctx, upper)?;

    if !v_ty.compatible_with(&lo_ty) || !v_ty.compatible_with(&hi_ty) {
        return Err(TypeError::InSetTypeMismatch {
            value_type: v_ty.to_string(),
            element_type: lo_ty.to_string(),
            span,
        });
    }
    if v_ty == Type::Str {
        return Err(TypeError::MissingPartialOrd {
            type_: v_ty.to_string(),
            span,
        });
    }

    let hir = HirExpr::RangeCheck {
        value: Box::new(v_hir),
        lower: Some(Box::new(lo_hir)),
        upper: Some(Box::new(hi_hir)),
        lower_inclusive: *lower_inclusive,
        upper_inclusive: *upper_inclusive,
        negated,
    };
    Ok((hir, Type::Bool))
}

/// 检查集合成员类型与值类型是否兼容。
fn check_member_type(value_type: &Type, member_type: &Type, span: Span) -> Result<(), TypeError> {
    if !value_type.compatible_with(member_type) {
        return Err(TypeError::InSetTypeMismatch {
            value_type: value_type.to_string(),
            element_type: member_type.to_string(),
            span,
        });
    }
    Ok(())
}

/// 对编译期整型表达式求值。
///
/// 支持整数字面量、时间字面量（归一化为分钟值）与一元取负。
fn const_eval_int(expr: &AstExpr, span: Span) -> Result<i128, TypeError> {
    match &*expr.kind {
        ExprKind::IntLiteral(n) => Ok(*n),
        ExprKind::TimeLiteral { hour, minute, .. } => {
            Ok(i128::from(*hour) * 60 + i128::from(*minute))
        }
        ExprKind::Unary {
            op: rlyeh_ast::UnaryOp::Neg,
            operand,
        } => Ok(-const_eval_int(operand, span)?),
        _ => Err(TypeError::NonConstantBound { span }),
    }
}

/// 将开闭区间展开为离散整数成员。
fn expand_range(
    lower: i128,
    upper: i128,
    lower_inclusive: bool,
    upper_inclusive: bool,
    span: Span,
) -> Result<Vec<i128>, TypeError> {
    let start = if lower_inclusive {
        lower
    } else {
        lower.saturating_add(1)
    };
    let end = if upper_inclusive {
        upper
    } else {
        upper.saturating_sub(1)
    };
    if start > end {
        // 空集合：`x in ()` 恒为 false
        return Ok(Vec::new());
    }
    let len = end.saturating_sub(start).saturating_add(1);
    if len > MAX_SET_MEMBERS {
        return Err(TypeError::NonConstantBound { span });
    }
    Ok((start..=end).collect())
}

/// 集合成员判断优化：
///
/// - 成员数 ≤ 5：展开为 `==`（`not in` 为 `!=`）链；
/// - 成员数 > 5：保留为 [`HirExpr::SetLookup`]（编译器生成查找结构）；
/// - 空集合：`in` 恒 `false`，`not in` 恒 `true`。
fn optimize_in_set(value: HirExpr, members: Vec<HirExpr>, negated: bool) -> HirExpr {
    if members.is_empty() {
        return HirExpr::BoolLiteral(negated);
    }
    if members.len() <= OR_CHAIN_THRESHOLD {
        let op = if negated {
            HirBinaryOp::Ne
        } else {
            HirBinaryOp::Eq
        };
        let join = if negated {
            HirBinaryOp::And
        } else {
            HirBinaryOp::Or
        };
        let mut acc = HirExpr::Binary(op, Box::new(value.clone()), Box::new(members[0].clone()));
        for m in &members[1..] {
            acc = HirExpr::Binary(
                join,
                Box::new(acc),
                Box::new(HirExpr::Binary(
                    op,
                    Box::new(value.clone()),
                    Box::new(m.clone()),
                )),
            );
        }
        acc
    } else {
        HirExpr::SetLookup {
            value: Box::new(value),
            members,
            negated,
        }
    }
}
