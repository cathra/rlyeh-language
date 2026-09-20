//! rlyeh-driver 聚合对象集成测试：enum + match + impl 方法 + 泛型单态化。
//!
//! 需要系统 clang（与 driver_test.rs 相同）。

use rlyeh_driver::{compile_to_llvm, run_source};

/// 枚举构造 + match 判别：tag 槽与字段槽的读写。
const ENUM_MATCH: &str = r#"
enum Option {
    None,
    Some(i64),
}

fn main() {
    let a = Option::Some(42);
    match a {
        Option::Some(v) => println(v),
        Option::None => println(-1),
    }
    let b = Option::None;
    match b {
        Option::Some(v) => println(v),
        Option::None => println(-1),
    }
}
"#;

/// impl 方法（`self` 参数）通过 match 读取字段。
const IMPL_METHOD: &str = r#"
enum Result {
    Ok(i64),
    Err(i64),
}

impl Result {
    fn unwrap(self) -> i64 {
        match self {
            Result::Ok(v) => v,
            Result::Err(e) => e,
        }
    }
}

fn main() {
    let r = Result::Ok(99);
    println(r.unwrap());
    let e = Result::Err(-7);
    println(e.unwrap());
}
"#;

/// 泛型函数单态化：同一模板按实参类型实例化。
const GENERIC_FN: &str = r#"
fn identity<T>(x: T) -> T {
    x
}

fn main() {
    let a = identity(10);
    println(a);
    let s = identity("hi");
    println(s);
}
"#;

/// match 作为表达式赋值（绑定字段参与运算）。
const MATCH_VALUE: &str = r#"
enum Opt {
    None,
    Some(i64),
}

fn main() {
    let v = match Opt::Some(7) {
        Opt::Some(n) => n * 10,
        Opt::None => -1,
    };
    println(v);
}
"#;

/// 泛型函数 + 泛型 impl 方法 + 枚举联合（P009 标准库 `Vec::push` 模式的缩影）。
const GENERIC_IMPL: &str = r#"
enum Option {
    None,
    Some(i64),
}

fn wrap<T>(x: T) -> T {
    x
}

impl Option {
    fn get(self) -> i64 {
        match self {
            Option::Some(v) => v,
            Option::None => -1,
        }
    }
}

fn main() {
    let o = Option::Some(5);
    let g = wrap(o.get());
    println(g);
    println(Option::None.get());
}
"#;

/// 多字段枚举：同一变体绑定多个字段参与运算。
const MULTI_FIELD_ENUM: &str = r#"
enum Shape {
    Empty,
    Point(i64, i64),
}

fn main() {
    let p = Shape::Point(3, 4);
    let d = match p {
        Shape::Point(x, y) => x * x + y * y,
        Shape::Empty => 0,
    };
    println(d);
}
"#;

/// f64 浮点字段：槽值 bitcast 转换（i64 <-> f64）。
const FLOAT_FIELD: &str = r#"
enum Num {
    I(i64),
    F(f64),
}

fn main() {
    let f = Num::F(3.5);
    let v = match f {
        Num::F(x) => x,
        _ => 0.0,
    };
    println(v);
    let i = Num::I(2);
    println(match i {
        Num::I(n) => n,
        _ => 0,
    });
}
"#;

/// 协议定义 + `impl Type: Protocol` + 协议方法调用。
const PROTOCOL_IMPL: &str = r#"
enum Option {
    None,
    Some(i64),
}

protocol Get {
    fn get(self) -> i64;
}

impl Option: Get {
    fn get(self) -> i64 {
        match self {
            Option::Some(v) => v,
            Option::None => -1,
        }
    }
}

fn main() {
    let o = Option::Some(77);
    println(o.get());
    println(Option::None.get());
}
"#;

/// 泛型多参数单态化：A/B 两个类型参数分别实例化。
const GENERIC_MULTI_PARAM: &str = r#"
fn first<A, B>(a: A, b: B) -> A {
    a
}

fn main() {
    let a = first(11, "ignored");
    println(a);
    let b = first("kept", 99);
    println(b);
}
"#;

#[test]
fn run_multi_field_enum() {
    let out = run_source(MULTI_FIELD_ENUM).expect("运行失败");
    assert_eq!(out, "25\n");
}

#[test]
fn run_float_field() {
    let out = run_source(FLOAT_FIELD).expect("运行失败");
    assert_eq!(out, "3.500000\n2\n");
}

#[test]
fn run_protocol_impl() {
    let out = run_source(PROTOCOL_IMPL).expect("运行失败");
    assert_eq!(out, "77\n-1\n");
}

#[test]
fn run_generic_multi_param() {
    let out = run_source(GENERIC_MULTI_PARAM).expect("运行失败");
    assert_eq!(out, "11\nkept\n");
}

#[test]
fn run_enum_match() {
    let out = run_source(ENUM_MATCH).expect("运行失败");
    assert_eq!(out, "42\n-1\n");
}

#[test]
fn run_impl_method() {
    let out = run_source(IMPL_METHOD).expect("运行失败");
    assert_eq!(out, "99\n-7\n");
}

#[test]
fn run_generic_fn() {
    let out = run_source(GENERIC_FN).expect("运行失败");
    assert_eq!(out, "10\nhi\n");
}

#[test]
fn run_match_value() {
    let out = run_source(MATCH_VALUE).expect("运行失败");
    assert_eq!(out, "70\n");
}

#[test]
fn run_generic_impl() {
    let out = run_source(GENERIC_IMPL).expect("运行失败");
    assert_eq!(out, "5\n-1\n");
}

/// 非标量聚合（>2 槽）构造：走堆分配（calloc）+ GEP 槽访问。
/// （≤2 槽标量聚合走 by_value 栈槽优化、免 calloc，见 `run_enum_match`。）
const LARGE_AGG: &str = r#"
struct Big {
    a: i64,
    b: i64,
    c: i64,
    d: i64,
}

fn main() {
    let p = Big { a: 1, b: 2, c: 3, d: 4 };
    println(p.a + p.b + p.c + p.d);
}
"#;

#[test]
fn compile_agg_llvm() {
    // 聚合对象代码生成：非标量聚合（>2 槽）calloc（清零分配）+ GEP 槽访问
    let ll = compile_to_llvm(LARGE_AGG).expect("编译失败");
    assert!(ll.contains("declare i8* @malloc(i64)"), "应声明 malloc");
    assert!(ll.contains("call i64 @calloc"), "应有堆分配（calloc 清零）");
    assert!(ll.contains("getelementptr"), "应有槽寻址 GEP");
}

#[test]
fn run_large_agg() {
    let out = run_source(LARGE_AGG).expect("运行失败");
    assert_eq!(out, "10\n");
}
