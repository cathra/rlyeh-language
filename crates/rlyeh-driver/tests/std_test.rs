//! rlyeh-driver 标准库核心类型集成测试：纯 Rlyeh 语言实现的
//! `Option<T>` / `Result<T, E>`（泛型 enum + 泛型 impl + match）。
//!
//! 注意：字符串 API（`run_source`）不注入标准库预置，因此这里显式内联
//! `core.rl` 副本；文件入口 API 的自动注入见 `std_prelude_test.rs`。
//! 需要系统 clang（与 driver_test.rs / agg_test.rs 相同）。

use rlyeh_driver::run_source;

/// 标准库核心类型源码（`rlyeh-std/rlyeh/core.rl` 的测试内联副本，需保持同步）。
const CORE_TYPES: &str = r#"
enum Option<T> {
    None,
    Some(T),
}

impl<T> Option<T> {
    fn is_some(self) -> i64 {
        match self {
            Option::Some(v) => 1,
            Option::None => 0,
        }
    }
    fn is_none(self) -> i64 {
        match self {
            Option::Some(v) => 0,
            Option::None => 1,
        }
    }
    fn unwrap(self) -> T {
        match self {
            Option::Some(v) => v,
            Option::None => loop {},
        }
    }
    fn unwrap_or(self, default: T) -> T {
        match self {
            Option::Some(v) => v,
            Option::None => default,
        }
    }
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> Result<T, E> {
    fn is_ok(self) -> i64 {
        match self {
            Result::Ok(v) => 1,
            Result::Err(e) => 0,
        }
    }
    fn unwrap(self) -> T {
        match self {
            Result::Ok(v) => v,
            Result::Err(e) => loop {},
        }
    }
}
"#;

/// Option 基本方法：is_some / is_none / unwrap / unwrap_or。
#[test]
fn option_basic() {
    let src = format!(
        "{CORE_TYPES}
fn main() {{
    let a = Option::Some(42);
    println(a.is_some());
    println(a.is_none());
    println(a.unwrap());

    let b = Option::None;
    println(b.is_some());
    println(b.is_none());
    println(b.unwrap_or(-1));
}}
"
    );
    let out = run_source(&src).expect("option_basic 编译运行失败");
    assert_eq!(out, "1\n0\n42\n0\n1\n-1\n");
}

/// Option<T> 多类型单态化：i64 与 str 两个实例并存。
#[test]
fn option_generic_monomorph() {
    let src = format!(
        "{CORE_TYPES}
fn main() {{
    let a = Option::Some(7);
    println(a.is_some());
    println(a.unwrap());

    let s = Option::Some(\"hi\");
    println(s.is_some());
}}
"
    );
    let out = run_source(&src).expect("option_generic_monomorph 编译运行失败");
    assert_eq!(out, "1\n7\n1\n");
}

/// Result 基本方法：is_ok / unwrap。
#[test]
fn result_basic() {
    let src = format!(
        "{CORE_TYPES}
fn main() {{
    let ok = Result::Ok(99);
    println(ok.is_ok());
    println(ok.unwrap());

    let err = Result::Err(-7);
    println(err.is_ok());
}}
"
    );
    let out = run_source(&src).expect("result_basic 编译运行失败");
    assert_eq!(out, "1\n99\n0\n");
}

/// 泛型参数为聚合类型：`Option<Result<i64, i64>>` 嵌套单态化。
#[test]
fn option_of_result() {
    let src = format!(
        "{CORE_TYPES}
fn main() {{
    let inner = Result::Ok(5);
    let outer = Option::Some(inner);
    println(outer.is_some());
}}
"
    );
    let out = run_source(&src).expect("option_of_result 编译运行失败");
    assert_eq!(out, "1\n");
}
