//! 比较链（`0 < x < 10`）方向检查与展开。

use zeta_ast::{AstExpr, CompareOp};
use zeta_hir::{HirBinaryOp, HirExpr};
use zeta_lexer::Span;

use crate::check_expr;
use crate::context::TypeContext;
use crate::error::TypeError;
use crate::types::Type;

/// 比较链方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChainDirection {
    /// 正向（全 `<` / `<=`）：区间内
    Forward,
    /// 反向（全 `>` / `>=`）：区间外
    Backward,
}

/// 检查并展开比较链。
///
/// 1. 推断所有元素类型；
/// 2. 检查方向一致性（正向 / 反向 / 混合报错）；
/// 3. 检查相邻元素类型兼容；
/// 4. 展开为低层比较运算。
///
/// - 正向 `0 < x < 10` → `(0 < x) && (x < 10)`
/// - 反向 `0 > x > 10` → `(x < 0) || (x > 10)`
pub(crate) fn check_comparison_chain(
    ctx: &mut TypeContext,
    elements: Vec<AstExpr>,
    operators: Vec<CompareOp>,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if elements.len() < 2 {
        return Err(TypeError::ChainTypeMismatch { span });
    }

    // 1. 推断所有元素（HIR + 类型一次完成，后续克隆复用）
    let mut items = Vec::with_capacity(elements.len());
    for e in &elements {
        let (hir, ty) = check_expr::infer_expr(ctx, e)?;
        items.push((e.clone(), hir, ty));
    }

    // 单个比较（如 `x != 7`）：直接展开，不参与链方向检查
    if operators.len() == 1 {
        check_comparison(&items[0].2, &items[1].2, operators[0], span)?;
        let hir = compare_hir(&items[0].1, operators[0], &items[1].1);
        return Ok((hir, Type::Bool));
    }

    // 2. 方向检查
    let direction = chain_direction(&operators, span)?;

    // 3. 相邻元素类型检查
    for (i, op) in operators.iter().enumerate() {
        check_comparison(&items[i].2, &items[i + 1].2, *op, span)?;
    }

    // 4. 展开
    let hir = match direction {
        ChainDirection::Forward => expand_forward(&items, &operators),
        ChainDirection::Backward => expand_backward(&items, &operators, span)?,
    };
    Ok((hir, Type::Bool))
}

/// 检查比较链方向一致性。
fn chain_direction(operators: &[CompareOp], span: Span) -> Result<ChainDirection, TypeError> {
    let forward = operators
        .iter()
        .all(|op| matches!(op, CompareOp::Lt | CompareOp::Le));
    let backward = operators
        .iter()
        .all(|op| matches!(op, CompareOp::Gt | CompareOp::Ge));
    if forward {
        Ok(ChainDirection::Forward)
    } else if backward {
        Ok(ChainDirection::Backward)
    } else {
        // 混合方向（或含 ==/!=）不是合法的比较链
        Err(TypeError::InconsistentComparison { span })
    }
}

/// 检查两个操作数在给定运算符下的类型兼容性。
fn check_comparison(left: &Type, right: &Type, op: CompareOp, span: Span) -> Result<(), TypeError> {
    if !left.compatible_with(right) {
        return Err(TypeError::ChainTypeMismatch { span });
    }
    match op {
        CompareOp::Eq | CompareOp::Ne => {
            // 大多数具体类型支持相等；占位类型除外
            if matches!(left, Type::Infer | Type::Never | Type::Unit) {
                return Err(TypeError::MissingPartialEq {
                    type_: left.to_string(),
                    span,
                });
            }
        }
        _ => {
            // 排序仅支持数值与字符
            if !left.is_numeric() && *left != Type::Char {
                return Err(TypeError::MissingPartialOrd {
                    type_: left.to_string(),
                    span,
                });
            }
        }
    }
    Ok(())
}

/// 正向链展开：`e0 op0 e1 && e1 op1 e2 && ...`
fn expand_forward(items: &[(AstExpr, HirExpr, Type)], operators: &[CompareOp]) -> HirExpr {
    let mut acc = compare_hir(&items[0].1, operators[0], &items[1].1);
    for (i, op) in operators.iter().enumerate().skip(1) {
        let cmp = compare_hir(&items[i].1, *op, &items[i + 1].1);
        acc = HirExpr::Binary(HirBinaryOp::And, Box::new(acc), Box::new(cmp));
    }
    acc
}

/// 反向链展开：`a > b > c` → `(b < a) || (b > c)`（b 在区间外）。
///
/// 反向链语义由 ADR-001 定义，仅支持 3 个元素（2 个比较运算符）。
fn expand_backward(
    items: &[(AstExpr, HirExpr, Type)],
    operators: &[CompareOp],
    span: Span,
) -> Result<HirExpr, TypeError> {
    if items.len() != 3 {
        return Err(TypeError::Unsupported {
            what: "反向比较链仅支持 3 个元素（2 个比较运算符）".to_string(),
            span,
        });
    }
    let rev = reverse_op(operators[0]);
    let left = compare_hir(&items[1].1, rev, &items[0].1);
    let right = compare_hir(&items[1].1, operators[1], &items[2].1);
    Ok(HirExpr::Binary(
        HirBinaryOp::Or,
        Box::new(left),
        Box::new(right),
    ))
}

/// 生成单个比较：`left op right`。
fn compare_hir(left: &HirExpr, op: CompareOp, right: &HirExpr) -> HirExpr {
    let hir_op = match op {
        CompareOp::Lt => HirBinaryOp::Lt,
        CompareOp::Le => HirBinaryOp::Le,
        CompareOp::Gt => HirBinaryOp::Gt,
        CompareOp::Ge => HirBinaryOp::Ge,
        CompareOp::Eq => HirBinaryOp::Eq,
        CompareOp::Ne => HirBinaryOp::Ne,
    };
    HirExpr::Binary(hir_op, Box::new(left.clone()), Box::new(right.clone()))
}

/// 取反方向的比较运算符（`>` ↔ `<`，`>=` ↔ `<=`）。
fn reverse_op(op: CompareOp) -> CompareOp {
    match op {
        CompareOp::Gt => CompareOp::Lt,
        CompareOp::Ge => CompareOp::Le,
        _ => op,
    }
}
