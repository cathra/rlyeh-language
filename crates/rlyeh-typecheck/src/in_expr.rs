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
use rlyeh_hir::{
    FieldScalar, HirAssignOp, HirBinaryOp, HirBlock, HirExpr, HirStmt, HirUnaryOp,
};
use rlyeh_lexer::Span;

use crate::check_expr;
use crate::check_expr::construct::check_string_from;
use crate::check_expr::util::substitute;
use crate::comparison;
use crate::context::TypeContext;
use crate::error::TypeError;
use crate::types::{field_scalar_of, Type};

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

/// 检查并展开容器成员判断（`x in arr` / `x in vec` / `x in [1,2,3]` /
/// `x in &slice` / `x not in ...`）。
///
/// 与 [`check_in_expression`]（集合 `InSet` 编译期离散展开）不同：容器长度
/// 运行时未知，typecheck 生成 **HIR 循环**在运行期逐元素相等判断：
///
/// ```text
/// let __v = <value>;            // 缓存值（避免重复求值）
/// let __c = <container>;        // 缓存容器
/// let mut __found = false;
/// let mut __i = 0;
/// let __len = <容器长度>;
/// loop {
///     if __i >= __len { break; }
///     let __e = <容器>[__i];     // 按下标读元素
///     if __e == __v { __found = true; break; }   // 命中
///     __i += 1;
/// }
/// <negated ? !__found : __found>
/// ```
///
/// 支持容器：数组 `[T; N]`、切片 `&[T]` / `&mut [T]`（元素类型须确定）、
/// `Vec<T>`。元素与值类型须兼容（字符串经 `string_eq_hir` 内容比较）。
pub(crate) fn check_in_container_expression(
    ctx: &mut TypeContext,
    value: AstExpr,
    container: AstExpr,
    negated: bool,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let (v_hir, v_ty) = check_expr::infer_expr(ctx, &value)?;
    let (c_hir, c_ty) = check_expr::infer_expr(ctx, &container)?;

    // 解析容器：剥离一层引用（&[T] / &[T; N]），匹配数组 / 切片 / Vec
    let inner = match &c_ty {
        Type::Ref(inner, _) => &**inner,
        other => other,
    };
    let (elem_ty, is_array, array_len, is_str) = match inner {
        Type::Array(elem, n) => {
            let et = substitute(elem, &ctx.generic_subst);
            if matches!(et, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "`in` 的数组容器要求元素类型确定（如 `let a: [i64; N] = ...`）"
                        .to_string(),
                    span,
                });
            }
            let is_str = matches!(et, Type::U8);
            (et, true, *n, is_str)
        }
        Type::Slice(elem) => {
            let et = substitute(elem, &ctx.generic_subst);
            if matches!(et, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "`in` 的切片容器要求元素类型确定（如 `let s: &[i64] = ...`）"
                        .to_string(),
                    span,
                });
            }
            (et, false, 0, false)
        }
        Type::Named(n, args) if n == "Vec" && ctx.lookup_struct("Vec").is_some() => {
            let et = substitute(args.first().unwrap_or(&Type::Infer), &ctx.generic_subst);
            if matches!(et, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "`in` 的 Vec 容器要求元素类型确定（如 `let v: Vec<i64> = ...`）"
                        .to_string(),
                    span,
                });
            }
            (et, false, 0, false)
        }
        _ => {
            return Err(TypeError::Unsupported {
                what: "`in` 右侧容器须为数组 / 切片 / Vec（`[T; N]` / `&[T]` / `Vec<T>`）".to_string(),
                span,
            })
        }
    };

    // 类型兼容检查（字符串容器须与字符串值配对）
    let elem_str = comparison::is_string_type(ctx, &elem_ty)
        || comparison::is_str_view(&elem_ty)
        || comparison::is_str_value(&elem_ty);
    let val_str = comparison::is_string_type(ctx, &v_ty)
        || comparison::is_str_view(&v_ty)
        || comparison::is_str_value(&v_ty);
    if elem_str != val_str {
        return Err(TypeError::InSetTypeMismatch {
            value_type: v_ty.to_string(),
            element_type: elem_ty.to_string(),
            span,
        });
    }
    if !elem_str && !v_ty.compatible_with(&elem_ty) {
        return Err(TypeError::InSetTypeMismatch {
            value_type: v_ty.to_string(),
            element_type: elem_ty.to_string(),
            span,
        });
    }

    // 唯一临时名（避免与用户变量冲突）
    let v_var = ctx.fresh_temp();
    let found = ctx.fresh_temp();
    let i_var = ctx.fresh_temp();
    let e_var = ctx.fresh_temp();
    let len_var = ctx.fresh_temp();

    // 值初始化：字符串字面量（`str` 值）升级为 String 对象，便于 `string_eq_hir`
    let v_init = if val_str && comparison::is_str_value(&v_ty) {
        check_string_from(ctx, &[value.clone()], span)?.0
    } else {
        v_hir
    };

    let elem_scalar = field_scalar_of(&elem_ty);
    let index_base = if is_array {
        // 数组：容器表达式本身即数据（按元素步长 GEP）
        c_hir.clone()
    } else {
        // Vec / 切片：槽 0 = data 指针。注意：直接复用容器表达式 `c_hir`，
        // 不要绑定到新的临时变量——切片为胖指针（2 槽 {data, len}），若经
        // `let __c = <容器>` 复制到单槽临时变量会截断长度信息，导致遍历越界崩溃。
        HirExpr::FieldGet {
            base: Box::new(c_hir.clone()),
            index: 0,
            ty: FieldScalar::Ptr,
        }
    };
    let len_hir = if is_array {
        HirExpr::IntLiteral(array_len as i128)
    } else {
        // Vec / 切片：槽 1 = 长度
        HirExpr::FieldGet {
            base: Box::new(c_hir.clone()),
            index: 1,
            ty: FieldScalar::Int,
        }
    };

    // 元素相等条件：`==` 对字符串走 `string_eq_hir`（内容比较），其余标量直接 `==`
    let cmp_cond = if elem_str {
        comparison::string_eq_hir(
            ctx,
            &HirExpr::Variable(e_var.clone()),
            &HirExpr::Variable(v_var.clone()),
        )
    } else {
        HirExpr::Binary(
            HirBinaryOp::Eq,
            Box::new(HirExpr::Variable(e_var.clone())),
            Box::new(HirExpr::Variable(v_var.clone())),
        )
    };

    let loop_body = HirBlock {
        stmts: vec![
            // if __i >= __len { break; }
            HirStmt::Expr(HirExpr::If {
                cond: Box::new(HirExpr::Binary(
                    HirBinaryOp::Ge,
                    Box::new(HirExpr::Variable(i_var.clone())),
                    Box::new(HirExpr::Variable(len_var.clone())),
                )),
                then_block: Box::new(HirBlock {
                    stmts: vec![HirStmt::Expr(HirExpr::Break(None))],
                    final_expr: None,
                }),
                else_block: None,
            }),
            // let __e = <容器>[__i];
            HirStmt::Let {
                name: e_var.clone(),
                init: HirExpr::Index {
                    base: Box::new(index_base),
                    index: Box::new(HirExpr::Variable(i_var.clone())),
                    elem: elem_scalar,
                    is_str,
                },
                mutable: false,
            },
            // if __e == __v { __found = true; break; }
            HirStmt::Expr(HirExpr::If {
                cond: Box::new(cmp_cond),
                then_block: Box::new(HirBlock {
                    stmts: vec![
                        HirStmt::Expr(HirExpr::Assign {
                            target: found.clone(),
                            op: HirAssignOp::Assign,
                            value: Box::new(HirExpr::BoolLiteral(true)),
                        }),
                        HirStmt::Expr(HirExpr::Break(None)),
                    ],
                    final_expr: None,
                }),
                else_block: None,
            }),
            // __i += 1;
            HirStmt::Expr(HirExpr::Assign {
                target: i_var.clone(),
                op: HirAssignOp::AddAssign,
                value: Box::new(HirExpr::IntLiteral(1)),
            }),
        ],
        final_expr: None,
    };

    let stmts = vec![
        HirStmt::Let {
            name: v_var.clone(),
            init: v_init,
            mutable: false,
        },
        HirStmt::Let {
            name: len_var.clone(),
            init: len_hir,
            mutable: false,
        },
        HirStmt::Let {
            name: found.clone(),
            init: HirExpr::BoolLiteral(false),
            mutable: true,
        },
        HirStmt::Let {
            name: i_var.clone(),
            init: HirExpr::IntLiteral(0),
            mutable: true,
        },
        HirStmt::Expr(HirExpr::Loop {
            body: Box::new(loop_body),
        }),
    ];

    let result = if negated {
        HirExpr::Unary(HirUnaryOp::Not, Box::new(HirExpr::Variable(found)))
    } else {
        HirExpr::Variable(found)
    };

    let hir = HirExpr::Block(Box::new(HirBlock {
        stmts,
        final_expr: Some(result),
    }));
    Ok((hir, Type::Bool))
}
