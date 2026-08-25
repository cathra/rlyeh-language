//! 跨模块路径表达式集成测试。
//!
//! 覆盖：无参/带参变体构造、模块常量、模块函数、嵌套模块、
//! 跨模块泛型变体 + match、裸变体回归、compile-fail 场景。

use rlyeh_driver::run_source;

const MODULE_VARIANT_NO_ARG: &str = r#"
mod shape {
    enum Kind {
        None,
    }
}

fn main() {
    let a = shape::Kind::None;
    println(1);
}
"#;

const MODULE_VARIANT_WITH_ARG: &str = r#"
mod shape {
    enum Kind {
        Pair(i64, i64),
    }
}

fn main() {
    let b = shape::Kind::Pair(3, 4);
    println(2);
}
"#;

const MODULE_CONSTANT: &str = r#"
mod shape {
    const ORIGIN: i64 = 7;
}

fn main() {
    let c = shape::ORIGIN;
    println(c);
}
"#;

const MODULE_CONSTANT_EXPR: &str = r#"
mod lib {
    const SCALE: i64 = 10;
}

fn main() {
    let c = lib::SCALE * 3;
    println(c);
}
"#;

const MODULE_FUNCTION: &str = r#"
mod shape {
    fn make(x: i64) -> i64 { x * 2 }
}

fn main() {
    let d = shape::make(5);
    println(d);
}
"#;

const NESTED_MODULE_CONSTANT: &str = r#"
mod lib {
    mod inner {
        const DEPTH: i64 = 3;
    }
}

fn main() {
    println(lib::inner::DEPTH);
}
"#;

const MODULE_GENERIC_MATCH: &str = r#"
mod lib {
    enum Option {
        Some(i64),
        None,
    }
}

fn main() {
    let v = lib::Option::Some(5);
    let mut r = 0;
    match v {
        lib::Option::Some(x) => { r = x * 2; }
        lib::Option::None => { r = -1; }
    }
    println(r);
}
"#;

const MODULE_GENERIC_MATCH_NONE: &str = r#"
mod lib {
    enum Option {
        Some(i64),
        None,
    }
}

fn main() {
    let v2 = lib::Option::None;
    let mut r2 = 0;
    match v2 {
        lib::Option::Some(x) => { r2 = x; }
        lib::Option::None => { r2 = 42; }
    }
    println(r2);
}
"#;

const BARE_VARIANT_REGRESSION: &str = r#"
mod lib {
    enum Option {
        Some(i64),
        None,
    }
}

fn main() {
    // 裸变体构造 + 裸模式（跨模块 enum 注册后不破坏原有路径）
    let v3 = Some(7);
    let mut r3 = 0;
    match v3 {
        Some(x) => { r3 = x + 1; }
        None => { r3 = 0; }
    }
    println(r3);
}
"#;

const MODULE_RESULT_ERR: &str = r#"
mod lib {
    enum Result {
        Ok(i64),
        Err,
    }
}

fn main() {
    let e = lib::Result::Err;
    println(99);
}
"#;

const MODULE_CONSTANT_NOT_FOUND: &str = r#"
mod lib {
    const SCALE: i64 = 10;
}

fn main() {
    let c = lib::MISSING;
    println(c);
}
"#;

#[test]
fn module_variant_no_arg() {
    assert_eq!(run_source(MODULE_VARIANT_NO_ARG).expect("运行失败"), "1\n");
}

#[test]
fn module_variant_with_arg() {
    assert_eq!(run_source(MODULE_VARIANT_WITH_ARG).expect("运行失败"), "2\n");
}

#[test]
fn module_constant() {
    assert_eq!(run_source(MODULE_CONSTANT).expect("运行失败"), "7\n");
}

#[test]
fn module_constant_in_expr() {
    assert_eq!(run_source(MODULE_CONSTANT_EXPR).expect("运行失败"), "30\n");
}

#[test]
fn module_function() {
    assert_eq!(run_source(MODULE_FUNCTION).expect("运行失败"), "10\n");
}

#[test]
fn nested_module_constant() {
    assert_eq!(
        run_source(NESTED_MODULE_CONSTANT).expect("运行失败"),
        "3\n"
    );
}

#[test]
fn module_generic_variant_match() {
    assert_eq!(run_source(MODULE_GENERIC_MATCH).expect("运行失败"), "10\n");
}

#[test]
fn module_generic_variant_match_none() {
    assert_eq!(
        run_source(MODULE_GENERIC_MATCH_NONE).expect("运行失败"),
        "42\n"
    );
}

#[test]
fn bare_variant_regression() {
    assert_eq!(
        run_source(BARE_VARIANT_REGRESSION).expect("运行失败"),
        "8\n"
    );
}

#[test]
fn module_result_err() {
    assert_eq!(run_source(MODULE_RESULT_ERR).expect("运行失败"), "99\n");
}

#[test]
fn module_constant_not_found() {
    let err = run_source(MODULE_CONSTANT_NOT_FOUND).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("MISSING") || msg.contains("未定义") || msg.contains("typecheck"),
        "意外错误: {msg}"
    );
}
