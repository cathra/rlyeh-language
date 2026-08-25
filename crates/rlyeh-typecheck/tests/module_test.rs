//! zeta-typecheck 模块系统集成测试：嵌套 `mod`、`use` 导入、模块路径调用。

use zeta_hir::HirItemKind;
use zeta_parser::parse;
use zeta_typecheck::{typecheck, TypeError};

/// 对源码执行类型检查。
fn check(source: &str) -> Result<zeta_hir::HirProgram, TypeError> {
    let program = parse(source).expect("parse should succeed");
    typecheck(&program)
}

/// 提取 HIR 项名集合（验证模块扁平化命名）。
fn item_names(program: &zeta_hir::HirProgram) -> Vec<String> {
    let mut names: Vec<String> = program.items.iter().map(|i| i.name.clone()).collect();
    names.sort();
    names
}

#[test]
fn test_nested_module_call() {
    let source = r#"
mod math {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }
}

fn main() {
    let x = math::add(1, 2);
    let _ = x;
}
"#;
    let program = check(source).expect("should typecheck");
    // 模块内函数扁平化为 `math::add`
    let names = item_names(&program);
    assert!(
        names.contains(&"math::add".to_string()),
        "expected math::add in {names:?}"
    );
}

#[test]
fn test_use_import() {
    let source = r#"
mod math {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }
}

use math::add;

fn main() {
    let x = add(1, 2);
    let _ = x;
}
"#;
    check(source).expect("should typecheck with use import");
}

#[test]
fn test_use_alias() {
    let source = r#"
mod math {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }
}

use math::add as madd;

fn main() {
    let x = madd(1, 2);
    let _ = x;
}
"#;
    check(source).expect("should typecheck with use alias");
}

#[test]
fn test_nested_mod_deep() {
    let source = r#"
mod a {
    mod b {
        mod c {
            fn deep() -> i64 {
                42
            }
        }
    }
}

fn main() {
    let x = a::b::c::deep();
    let _ = x;
}
"#;
    let program = check(source).expect("should typecheck");
    let names = item_names(&program);
    assert!(
        names.contains(&"a::b::c::deep".to_string()),
        "expected a::b::c::deep in {names:?}"
    );
}

#[test]
fn test_module_struct_type() {
    // zeta 的 `let` 必须有初始化，故用函数签名验证模块路径类型解析
    let source = r#"
mod geo {
    struct Point {
        x: i64,
        y: i64,
    }
}

fn area(p: geo::Point) -> i64 {
    0
}
"#;
    check(source).expect("should typecheck module struct type");
}

#[test]
fn test_use_import_struct() {
    let source = r#"
mod geo {
    struct Point {
        x: i64,
        y: i64,
    }
}

use geo::Point;

fn area(p: Point) -> i64 {
    0
}
"#;
    check(source).expect("should typecheck use-imported struct type");
}

#[test]
fn test_module_fn_type_mismatch() {
    let source = r#"
mod math {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }
}

fn main() {
    let x = math::add(1, "not a number");
    let _ = x;
}
"#;
    let err = check(source).expect_err("argument type mismatch should fail");
    assert!(matches!(err, TypeError::ArgumentTypeMismatch { .. }));
}

#[test]
fn test_glob_import_unsupported() {
    let source = r#"
mod math {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }
}

use math::*;

fn main() {
    let x = add(1, 2);
    let _ = x;
}
"#;
    let err = check(source).expect_err("glob import is unsupported in MVP");
    assert!(matches!(err, TypeError::Unsupported { .. }));
}

#[test]
fn test_use_unknown_symbol() {
    let source = r#"
use nonexistent::foo;

fn main() {
    foo();
}
"#;
    // use 目标本身不做存在性检查（MVP 宽松），调用时才会报函数不存在
    let err = check(source).expect_err("calling unknown function should fail");
    assert!(matches!(err, TypeError::FunctionNotFound { .. }));
}

#[test]
fn test_module_in_interface_hash() {
    // 接口哈希应包含模块内函数（带前缀符号名）
    let source = r#"
mod math {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }
}
"#;
    let program = parse(source).expect("parse");
    let sigs = zeta_typecheck::collect_fn_signatures(&program).expect("collect signatures");
    assert!(sigs.iter().any(|(name, _)| name == "math::add"));
}

#[test]
fn test_hir_item_kind() {
    let source = r#"
mod m {
    const K: i64 = 7;
    fn f() -> i64 {
        K
    }
}
"#;
    let program = check(source).expect("should typecheck");
    let names: Vec<&str> = program
        .items
        .iter()
        .map(|i| i.name.as_str())
        .filter(|n| n.contains("::"))
        .collect();
    assert!(
        names.contains(&"m::f") && names.contains(&"m::K"),
        "expected module-prefixed items in {names:?}"
    );
    let kind = program
        .items
        .iter()
        .find(|i| i.name == "m::f")
        .map(|i| &i.kind)
        .unwrap();
    assert!(matches!(kind, HirItemKind::Fn(_)));
}
