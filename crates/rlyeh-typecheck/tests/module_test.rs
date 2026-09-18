//! rlyeh-typecheck 模块系统集成测试：嵌套 `mod`、`use` 导入、模块路径调用。

use rlyeh_hir::HirItemKind;
use rlyeh_parser::parse;
use rlyeh_typecheck::{typecheck, TypeError};

/// 对源码执行类型检查。
fn check(source: &str) -> Result<rlyeh_hir::HirProgram, TypeError> {
    let program = parse(source).expect("parse should succeed");
    // B-1：`typecheck` 现返回 `(HirProgram, Vec<Warning>)`；本集成测试只关心 HIR，丢弃警告。
    typecheck(&program).map(|(hir, _warnings)| hir)
}

/// 提取 HIR 项名集合（验证模块扁平化命名）。
fn item_names(program: &rlyeh_hir::HirProgram) -> Vec<String> {
    let mut names: Vec<String> = program.items.iter().map(|i| i.name.clone()).collect();
    names.sort();
    names
}

#[test]
fn test_nested_module_call() {
    let source = r#"
module math {
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
module math {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }
}

import math::add;

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
module math {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }
}

import math::add as madd;

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
module a {
    module b {
        module c {
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
    // rlyeh 的 `let` 必须有初始化，故用函数签名验证模块路径类型解析
    let source = r#"
module geo {
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
module geo {
    struct Point {
        x: i64,
        y: i64,
    }
}

import geo::Point;

fn area(p: Point) -> i64 {
    0
}
"#;
    check(source).expect("should typecheck use-imported struct type");
}

#[test]
fn test_module_fn_type_mismatch() {
    let source = r#"
module math {
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
fn test_glob_import_supported() {
    let source = r#"
module math {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }
}

import math::*;

fn main() {
    let x = add(1, 2);
    let _ = x;
}
"#;
    // glob 导入（SH-P1-3，2026-09-02 实现）应将模块内全部可见符号注入当前作用域，
    // 故裸名 `add` 可解析（此前该测试断言 glob 不支持，与已落地行为矛盾，已更正）。
    check(source).expect("glob import should resolve math::add");
}

#[test]
fn test_use_unknown_symbol() {
    let source = r#"
import nonexistent::foo;

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
module math {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }
}
"#;
    let program = parse(source).expect("parse");
    let sigs = rlyeh_typecheck::collect_fn_signatures(&program).expect("collect signatures");
    assert!(sigs.iter().any(|(name, _)| name == "math::add"));
}

#[test]
fn test_hir_item_kind() {
    let source = r#"
module m {
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
