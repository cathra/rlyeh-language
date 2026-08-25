//! zeta-typecheck 集成测试：比较链与 `in` 表达式语义。

use zeta_hir::{HirBinaryOp, HirBlock, HirExpr, HirItemKind};
use zeta_parser::parse;
use zeta_typecheck::{typecheck, TypeError};

/// 对源码执行类型检查。
fn check(source: &str) -> Result<zeta_hir::HirProgram, TypeError> {
    let program = parse(source).expect("parse should succeed");
    typecheck(&program)
}

/// 提取第一个含函数体的函数体（跳过注入的 extern 内建声明）。
fn first_fn_body(program: &zeta_hir::HirProgram) -> &HirBlock {
    program
        .items
        .iter()
        .find_map(|item| match &item.kind {
            HirItemKind::Fn(f) => f.body.as_ref(),
            _ => None,
        })
        .expect("expected a function with body")
}

/// 提取函数体中最外层 if 表达式的条件。
///
/// `if` 作为块末尾表达式时位于 `final_expr`，否则位于某条语句中。
fn first_if_cond(program: &zeta_hir::HirProgram) -> &HirExpr {
    let block = first_fn_body(program);
    if let Some(e) = &block.final_expr {
        return if_cond(e);
    }
    for stmt in block.stmts.iter().rev() {
        match stmt {
            zeta_hir::HirStmt::Expr(e) | zeta_hir::HirStmt::Semi(e) => return if_cond(e),
            _ => {}
        }
    }
    panic!("no if expression found in function body");
}

/// 提取 if 表达式条件。
fn if_cond(expr: &HirExpr) -> &HirExpr {
    match expr {
        HirExpr::If { cond, .. } => cond,
        _ => panic!("expected an if expression, got {expr:?}"),
    }
}

// ---------- 比较链 ----------

#[test]
fn test_forward_chain() {
    // 正向链（区间内）：0 < x < 10
    assert!(check("fn main() { let x = 5; if 0 < x < 10 {} }").is_ok());
}

#[test]
fn test_backward_chain() {
    // 反向链（区间外）：0 > x > 10 → x < 0 || x > 10
    assert!(check("fn main() { let x = 5; if 0 > x > 10 {} }").is_ok());
}

#[test]
fn test_mixed_chain() {
    // 混合方向：非法
    let err = check("fn main() { let x = 5; if 0 < x > 10 {} }").unwrap_err();
    assert!(matches!(err, TypeError::InconsistentComparison { .. }));
}

#[test]
fn test_chain_type_mismatch() {
    // 字符串与整数混合比较：非法
    let err = check("fn main() { let x = 5; if \"hello\" < x < 10 {} }").unwrap_err();
    assert!(matches!(err, TypeError::ChainTypeMismatch { .. }));
}

#[test]
fn test_chain_short_circuit() {
    // 比较链与逻辑运算组合
    assert!(check("fn main() { let x = 5; if (0 < x < 10) && (x != 7) {} }").is_ok());
}

#[test]
fn test_chain_expression_sequence() {
    // 链元素为变量表达式
    assert!(check("fn main() { let a = 5; let b = 10; if a < b < 100 {} }").is_ok());
}

#[test]
fn test_chain_forward_expands_to_and() {
    // 正向链展开为 && 链：0 < x < 10 → (0 < x) && (x < 10)
    let program = check("fn main() { let x = 5; if 0 < x < 10 {} }").unwrap();
    let cond = first_if_cond(&program);
    assert!(
        matches!(cond, HirExpr::Binary(HirBinaryOp::And, _, _)),
        "正向链应展开为 && 链，got {cond:?}"
    );
}

#[test]
fn test_chain_backward_expands_to_or() {
    // 反向链展开为 ||：0 > x > 10 → (x < 0) || (x > 10)
    let program = check("fn main() { let x = 5; if 0 > x > 10 {} }").unwrap();
    let cond = first_if_cond(&program);
    assert!(
        matches!(cond, HirExpr::Binary(HirBinaryOp::Or, _, _)),
        "反向链应展开为 || 链，got {cond:?}"
    );
}

// ---------- in 集合（成员判断，离散展开） ----------

#[test]
fn test_in_set() {
    // 小集合：x == 1 || x == 3 || x == 5
    assert!(check("fn main() { let x = 5; if x in (1, 3, 5) {} }").is_ok());
}

#[test]
fn test_in_set_range() {
    // 集合内范围元素离散展开：x in (0..<10) → x == 0 || ... || x == 9
    assert!(check("fn main() { let x = 5; if x in (0..<10) {} }").is_ok());
}

#[test]
fn test_in_set_with_time() {
    // 时间字面量参与集合（归一化为分钟值）
    assert!(check("fn main() { let h = 14; if h in (9am...6pm) {} }").is_ok());
}

#[test]
fn test_in_mixed() {
    // 混合集合：范围元素 + 单值
    assert!(check("fn main() { let x = 5; if x in (1..<10, 20, 30) {} }").is_ok());
}

#[test]
fn test_in_not_in() {
    // not in：取反
    assert!(check("fn main() { let x = 5; if x not in (1, 3, 5) {} }").is_ok());
}

#[test]
fn test_in_type_mismatch() {
    // 集合元素类型与值不兼容
    let err = check("fn main() { let x = 5; if x in (\"a\", \"b\") {} }").unwrap_err();
    assert!(matches!(err, TypeError::InSetTypeMismatch { .. }));
}

#[test]
fn test_in_set_small_expands_to_or_chain() {
    // 小集合（≤5 成员）展开为 == 链
    let program = check("fn main() { let x = 5; if x in (1, 3, 5) {} }").unwrap();
    let cond = first_if_cond(&program);
    assert!(
        matches!(cond, HirExpr::Binary(HirBinaryOp::Or, _, _)),
        "小集合应展开为 || 链，got {cond:?}"
    );
}

#[test]
fn test_in_set_large_uses_set_lookup() {
    // 大集合（>5 成员）保留为 SetLookup
    let program = check("fn main() { let x = 5; if x in (0..<10) {} }").unwrap();
    let cond = first_if_cond(&program);
    let HirExpr::SetLookup {
        members, negated, ..
    } = cond
    else {
        panic!("大集合应为 SetLookup，got {cond:?}");
    };
    assert_eq!(members.len(), 10);
    assert!(!negated);
}

#[test]
fn test_not_in_set_large_uses_set_lookup_negated() {
    // not in 大集合：SetLookup + negated
    let program = check("fn main() { let x = 5; if x not in (0..<10) {} }").unwrap();
    let cond = first_if_cond(&program);
    let HirExpr::SetLookup { negated, .. } = cond else {
        panic!("大集合应为 SetLookup，got {cond:?}");
    };
    assert!(negated);
}

#[test]
fn test_in_set_open_bound_discrete() {
    // 左开右闭集合展开：x in (0<..9) → {1..=9}（9 个成员）
    let program = check("fn main() { let x = 5; if x in (0<..9) {} }").unwrap();
    let cond = first_if_cond(&program);
    let HirExpr::SetLookup { members, .. } = cond else {
        panic!("应为 SetLookup，got {cond:?}");
    };
    assert_eq!(members.len(), 9);
}

#[test]
fn test_in_set_negative_bound() {
    // 负下界
    assert!(check("fn main() { let x = 5; if x in (-5..<5) {} }").is_ok());
}

// ---------- in 裸范围（区间判断） ----------

#[test]
fn test_in_bare_range() {
    // 裸范围：x in 0..<10 → x >= 0 && x < 10
    assert!(check("fn main() { let x = 5; if x in 0..<10 {} }").is_ok());
}

#[test]
fn test_in_bare_range_forms() {
    // 三种范围运算符
    for src in ["0..<10", "0...10", "0<..10"] {
        let source = format!("fn main() {{ let x = 5; if x in {src} {{}} }}");
        assert!(check(&source).is_ok(), "裸范围 {src} 应通过");
    }
}

#[test]
fn test_not_in_bare_range() {
    // 区间补：x not in 0..<10 → x < 0 || x >= 10
    assert!(check("fn main() { let x = 5; if x not in 0..<10 {} }").is_ok());
}

#[test]
fn test_in_range_structural() {
    // RangeCheck 结构：0..<10 → lower=0(含), upper=10(不含), 非取反
    let program = check("fn main() { let x = 5; if x in 0..<10 {} }").unwrap();
    let cond = first_if_cond(&program);
    let HirExpr::RangeCheck {
        value,
        lower,
        upper,
        lower_inclusive,
        upper_inclusive,
        negated,
    } = cond
    else {
        panic!("裸范围应为 RangeCheck，got {cond:?}");
    };
    assert!(matches!(&**value, HirExpr::Variable(n) if n == "x"));
    assert!(matches!(lower.as_deref(), Some(HirExpr::IntLiteral(0))));
    assert!(matches!(upper.as_deref(), Some(HirExpr::IntLiteral(10))));
    assert!(*lower_inclusive);
    assert!(!*upper_inclusive);
    assert!(!*negated);
}

#[test]
fn test_in_range_not_in_structural() {
    // 区间补取反：x not in 0...10
    let program = check("fn main() { let x = 5; if x not in 0...10 {} }").unwrap();
    let cond = first_if_cond(&program);
    let HirExpr::RangeCheck {
        lower_inclusive,
        upper_inclusive,
        negated,
        ..
    } = cond
    else {
        panic!("裸范围应为 RangeCheck，got {cond:?}");
    };
    assert!(*lower_inclusive);
    assert!(*upper_inclusive);
    assert!(*negated);
}

#[test]
fn test_in_range_type_mismatch() {
    // 字符串与范围不兼容
    let err = check("fn main() { let s = \"hi\"; if s in 0..<10 {} }").unwrap_err();
    assert!(matches!(err, TypeError::InSetTypeMismatch { .. }));
}

#[test]
fn test_in_bare_range_with_time() {
    // 时间字面量裸范围
    assert!(check("fn main() { let h = 14; if h in 9am...6pm {} }").is_ok());
}

// ---------- 基础控制流 ----------

#[test]
fn test_if_requires_bool() {
    let err = check("fn main() { let x = 5; if x {} }").unwrap_err();
    assert!(matches!(err, TypeError::ExpectedBool { .. }));
}

#[test]
fn test_undefined_variable() {
    let err = check("fn main() { if y < 10 {} }").unwrap_err();
    assert!(matches!(err, TypeError::UndefinedVariable { .. }));
}

#[test]
fn test_function_call() {
    assert!(check("fn add(a: i64, b: i64) -> i64 { a + b } fn main() { add(1, 2); }").is_ok());
}

#[test]
fn test_function_call_arg_count() {
    let err =
        check("fn add(a: i64, b: i64) -> i64 { a + b } fn main() { add(1, 2, 3); }").unwrap_err();
    assert!(matches!(err, TypeError::UnexpectedArgumentCount { .. }));
}

#[test]
fn test_undefined_function() {
    let err = check("fn main() { foo(); }").unwrap_err();
    assert!(matches!(err, TypeError::FunctionNotFound { .. }));
}

#[test]
fn test_let_type_annotation_mismatch() {
    let err = check("fn main() { let x: i64 = \"hello\"; }").unwrap_err();
    assert!(matches!(err, TypeError::WrongType { .. }));
}

#[test]
fn test_typecheck_source() {
    // 便捷入口：源码 → HIR（items 含注入的 extern 内建，取含函数体的 main 断言）
    let program = zeta_typecheck::typecheck_source("fn main() { let x = 5; if x in 0..<10 {} }")
        .expect("should typecheck");
    assert_eq!(first_fn_body(&program).stmts.len(), 1);
}

#[test]
fn test_empty_set_constant() {
    // 空集合：x in () 恒 false / x not in () 恒 true
    let program = check("fn main() { let x = 5; if x in (4..<4) {} }").unwrap();
    let cond = first_if_cond(&program);
    assert!(matches!(cond, HirExpr::BoolLiteral(false)));
}
