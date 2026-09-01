//! 比较链（`0 < x < 10`）方向检查与展开。

use rlyeh_ast::{AstExpr, CompareOp};
use rlyeh_hir::{
    FieldScalar, HirAssignOp, HirBinaryOp, HirBlock, HirExpr, HirStmt, HirUnaryOp,
};
use rlyeh_lexer::Span;

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
        let op = operators[0];
        // String 对象字典序比较 `<` / `>` / `<=` / `>=`：跳过数值类型检查
        // （`check_comparison` 仅放行数值与字符），直接 desugar 为
        // 前缀 memcmp（`bytes_cmp` 内建）+ 长度兜底：
        //   `s1 < s2`  → `__c < 0 || (__c == 0 && __la < __lb)`
        //   `s1 > s2`  → `s2 < s1`（交换操作数）
        //   `s1 <= s2` → `!(s2 < s1)`，`s1 >= s2` → `!(s1 < s2)`
        // `&str` 视图与 String 同内容语义（G2），`str` 值（字面量绑定）经
        // 升级为 String 对象后同样纳入字符串比较（内容比较，非指针比较）
        let lhs_str = is_string_type(ctx, &items[0].2)
            || is_str_view(&items[0].2)
            || is_str_value(&items[0].2);
        let rhs_str = is_string_type(ctx, &items[1].2)
            || is_str_view(&items[1].2)
            || is_str_value(&items[1].2);
        if matches!(
            op,
            CompareOp::Lt | CompareOp::Le | CompareOp::Gt | CompareOp::Ge
        ) && lhs_str
            && rhs_str
        {
            // `str` 值操作数升级为 String 对象（编译期长度展开，见
            // `check_string_from`），使 `string_lt_hir` 可读 len/data 槽
            if is_str_value(&items[0].2) {
                let (h, t) =
                    check_expr::check_string_from(ctx, std::slice::from_ref(&items[0].0), span)?;
                items[0].1 = h;
                items[0].2 = t;
            }
            if is_str_value(&items[1].2) {
                let (h, t) =
                    check_expr::check_string_from(ctx, std::slice::from_ref(&items[1].0), span)?;
                items[1].1 = h;
                items[1].2 = t;
            }
            let (lhs, rhs) = (&items[0].1, &items[1].1);
            let hir = match op {
                CompareOp::Lt => string_lt_hir(ctx, lhs, rhs),
                CompareOp::Gt => string_lt_hir(ctx, rhs, lhs),
                CompareOp::Le => {
                    HirExpr::Unary(HirUnaryOp::Not, Box::new(string_lt_hir(ctx, rhs, lhs)))
                }
                CompareOp::Ge => {
                    HirExpr::Unary(HirUnaryOp::Not, Box::new(string_lt_hir(ctx, lhs, rhs)))
                }
                _ => unreachable!(),
            };
            return Ok((hir, Type::Bool));
        }
        // 字符串组合（Str 值 / `&str` 视图 / String）跳过类型兼容检查
        // （`check_comparison` 仅放行数值与字符；字符串内容比较在下方统一处理）
        if !(lhs_str && rhs_str) {
            check_comparison(&items[0].2, &items[1].2, op, span)?;
        }
        // String 对象相等 / 不等：内容比较 desugar
        // `s1 == s2` → `s1.len == s2.len && bytes_eq(s1.data, s2.data, s1.len)`
        if matches!(op, CompareOp::Eq | CompareOp::Ne) && lhs_str && rhs_str {
            // `str` 值操作数升级为 String 对象（同字典序分支）
            if is_str_value(&items[0].2) {
                let (h, t) =
                    check_expr::check_string_from(ctx, std::slice::from_ref(&items[0].0), span)?;
                items[0].1 = h;
                items[0].2 = t;
            }
            if is_str_value(&items[1].2) {
                let (h, t) =
                    check_expr::check_string_from(ctx, std::slice::from_ref(&items[1].0), span)?;
                items[1].1 = h;
                items[1].2 = t;
            }
            let eq = string_eq_hir(ctx, &items[0].1, &items[1].1);
            let hir = if op == CompareOp::Ne {
                HirExpr::Unary(HirUnaryOp::Not, Box::new(eq))
            } else {
                eq
            };
            return Ok((hir, Type::Bool));
        }
        // 其余聚合对象（结构体 / Vec 等）的 `==` / `!=`：MVP 仅 String 支持内容比较
        if matches!(op, CompareOp::Eq | CompareOp::Ne) && is_struct_object(ctx, &items[0].2) {
            return Err(TypeError::Unsupported {
                what: format!(
                    "{} 对象的 == / !=（MVP 阶段仅 String 支持内容相等比较）",
                    items[0].2
                ),
                span,
            });
        }
        let hir = compare_hir(&items[0].1, op, &items[1].1);
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
    // U3 核心项（2026-08-30）：比较对称——标量枚举值即 tag，可与整数双向比较
    // （`c == 1` 与 `1 == c` 均允许）。注意赋值仍由 `check_stmt` 单向判定
    // （`let c: Color = 1` 禁止），此处对称性不影响之。
    if !left.compatible_with(right) && !right.compatible_with(left) {
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

/// 判断类型是否为 `String` 对象（Named 类型，解析后全名 == "String"）。
pub(crate) fn is_string_type(ctx: &TypeContext, ty: &Type) -> bool {
    if let Type::Named(n, _) = ty {
        let full = ctx.resolve_full_name(n).unwrap_or_else(|| n.clone());
        return full == "String" && ctx.lookup_struct(&full).is_some();
    }
    false
}

/// 是否为 `&str` 视图（对字符串字面量类型的引用；G2）。
///
/// MVP 中 `&str` 是 String 对象的只读借用（瘦指针），其内容操作
/// （比较 / 方法 / 索引 / 切片）与 String 一致。
pub(crate) fn is_str_view(ty: &Type) -> bool {
    matches!(ty, Type::Ref(inner, _) if matches!(**inner, Type::Str))
}

/// 是否为 `str` 值（字符串字面量 / 绑定字面量的变量；非 `&str` 引用）。
///
/// MVP 中 `str` 值的运行期表示是 `i8*` 数据指针（指向静态字面量数据），
/// 无 String 对象的 len/cap 槽；参与字符串操作（方法 / 拼接 / 比较）前
/// 需升级为 String 对象（编译期长度展开，见 `check_string_from`）。
pub(crate) fn is_str_value(ty: &Type) -> bool {
    matches!(ty, Type::Str)
}

/// 判断类型是否为已定义的结构体对象（含 `Vec` / `HashMap` 等动态集合）。
fn is_struct_object(ctx: &TypeContext, ty: &Type) -> bool {
    if let Type::Named(n, _) = ty {
        let full = ctx.resolve_full_name(n).unwrap_or_else(|| n.clone());
        return ctx.lookup_struct(&full).is_some();
    }
    false
}

/// 生成 String 内容相等的比较 HIR：`s1 == s2` →
/// `s1.len == s2.len && bytes_eq(s1.data, s2.data, s1.len)`。
///
/// 操作数绑定到唯一临时变量（防止重复求值）；String 槽布局：
/// 槽 0 = data 指针、槽 1 = len、槽 2 = cap。`bytes_eq` 为内建
/// （`memcmp(a, b, n) == 0`），经 MIR/LIR 透传至代码生成。
pub(crate) fn string_eq_hir(ctx: &mut TypeContext, lhs: &HirExpr, rhs: &HirExpr) -> HirExpr {
    let a = ctx.fresh_temp();
    let b = ctx.fresh_temp();

    let a_var = HirExpr::Variable(a.clone());
    let b_var = HirExpr::Variable(b.clone());
    let a_data = HirExpr::FieldGet {
        base: Box::new(a_var.clone()),
        index: 0, // 槽 0 = data 指针
        ty: FieldScalar::Ptr,
    };
    let b_data = HirExpr::FieldGet {
        base: Box::new(b_var.clone()),
        index: 0,
        ty: FieldScalar::Ptr,
    };
    let a_len = HirExpr::FieldGet {
        base: Box::new(a_var),
        index: 1, // 槽 1 = len
        ty: FieldScalar::Int,
    };
    let b_len = HirExpr::FieldGet {
        base: Box::new(b_var),
        index: 1,
        ty: FieldScalar::Int,
    };

    let stmts = vec![
        HirStmt::Let {
            name: a.clone(),
            init: lhs.clone(),
            mutable: false,
        },
        HirStmt::Let {
            name: b.clone(),
            init: rhs.clone(),
            mutable: false,
        },
    ];

    let len_eq = HirExpr::Binary(HirBinaryOp::Eq, Box::new(a_len.clone()), Box::new(b_len));
    let content_eq = HirExpr::Call {
        callee: "bytes_eq".to_string(),
        args: vec![a_data, b_data, a_len],
    };
    let cmp = HirExpr::Binary(HirBinaryOp::And, Box::new(len_eq), Box::new(content_eq));
    HirExpr::Block(Box::new(HirBlock {
        stmts,
        final_expr: Some(cmp),
    }))
}

/// 生成 String 字典序 `<` 比较 HIR：`s1 < s2` →
/// 前缀 memcmp（`bytes_cmp` 内建，负/零/正 → 小于/等于/大于）+ 长度兜底：
///
/// ```text
/// let __a = <lhs>;
/// let __b = <rhs>;
/// let __la = __a.len;       // 槽 1 长度
/// let __lb = __b.len;
/// let mut __n = __la;       // n = min(la, lb)
/// if __la >= __lb { __n = __lb; }
/// let __c = bytes_cmp(__a.data, __b.data, __n);
/// __c < 0 || (__c == 0 && __la < __lb)
/// ```
///
/// 前缀字节相同且长度相等 → 相等（false）；前缀相同但长度不等 → 短者更小；
/// 前缀第一个不同字节即定大小（memcmp 返回其差值符号）。操作数绑定唯一
/// 临时变量防重复求值。`>` / `<=` / `>=` 由调用方交换操作数或取反获得。
fn string_lt_hir(ctx: &mut TypeContext, lhs: &HirExpr, rhs: &HirExpr) -> HirExpr {
    let a = ctx.fresh_temp();
    let b = ctx.fresh_temp();
    let la = ctx.fresh_temp();
    let lb = ctx.fresh_temp();
    let n = ctx.fresh_temp();
    let c = ctx.fresh_temp();

    let a_var = HirExpr::Variable(a.clone());
    let b_var = HirExpr::Variable(b.clone());
    let a_len = HirExpr::Variable(la.clone());
    let b_len = HirExpr::Variable(lb.clone());
    let a_data = HirExpr::FieldGet {
        base: Box::new(a_var.clone()),
        index: 0, // 槽 0 = data 指针
        ty: FieldScalar::Ptr,
    };
    let a_len_field = HirExpr::FieldGet {
        base: Box::new(a_var),
        index: 1, // 槽 1 = len
        ty: FieldScalar::Int,
    };
    let b_data = HirExpr::FieldGet {
        base: Box::new(b_var.clone()),
        index: 0,
        ty: FieldScalar::Ptr,
    };
    let b_len_field = HirExpr::FieldGet {
        base: Box::new(b_var),
        index: 1,
        ty: FieldScalar::Int,
    };

    let stmts = vec![
        HirStmt::Let {
            name: a,
            init: lhs.clone(),
            mutable: false,
        },
        HirStmt::Let {
            name: b,
            init: rhs.clone(),
            mutable: false,
        },
        HirStmt::Let {
            name: la,
            init: a_len_field,
            mutable: false,
        },
        HirStmt::Let {
            name: lb,
            init: b_len_field,
            mutable: false,
        },
        // n = min(la, lb)：先取 la，la >= lb 时覆盖为 lb（if 仅做控制流，
        // 与既有 `check_for_range` desugar 模式一致，避免 block 值语义）
        HirStmt::Let {
            name: n.clone(),
            init: a_len.clone(),
            mutable: true,
        },
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Ge,
                Box::new(a_len.clone()),
                Box::new(b_len.clone()),
            )),
            then_block: Box::new(HirBlock {
                stmts: vec![HirStmt::Expr(HirExpr::Assign {
                    target: n.clone(),
                    op: HirAssignOp::Assign,
                    value: Box::new(b_len.clone()),
                })],
                final_expr: None,
            }),
            else_block: None,
        }),
        HirStmt::Let {
            name: c.clone(),
            init: HirExpr::Call {
                callee: "bytes_cmp".to_string(),
                args: vec![a_data, b_data, HirExpr::Variable(n)],
            },
            mutable: false,
        },
    ];

    let prefix_lt = HirExpr::Binary(
        HirBinaryOp::Lt,
        Box::new(HirExpr::Variable(c.clone())),
        Box::new(HirExpr::IntLiteral(0)),
    );
    let eq_zero = HirExpr::Binary(
        HirBinaryOp::Eq,
        Box::new(HirExpr::Variable(c)),
        Box::new(HirExpr::IntLiteral(0)),
    );
    let len_lt = HirExpr::Binary(
        HirBinaryOp::Lt,
        Box::new(a_len.clone()),
        Box::new(b_len),
    );
    let result = HirExpr::Binary(
        HirBinaryOp::Or,
        Box::new(prefix_lt),
        Box::new(HirExpr::Binary(
            HirBinaryOp::And,
            Box::new(eq_zero),
            Box::new(len_lt),
        )),
    );

    HirExpr::Block(Box::new(HirBlock {
        stmts,
        final_expr: Some(result),
    }))
}
