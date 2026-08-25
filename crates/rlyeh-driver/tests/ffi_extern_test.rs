//! 测试 `extern fn` 外部函数声明（FFI）：声明无函数体的外部符号，
//! codegen 生成 LLVM `declare`，链接器解析符号（libc 提供）。
//!
//! 覆盖：标量参数/返回（i64/f64）、void 返回、嵌套调用、循环内调用、
//! 结果参与运算与比较。
//!
//! 需要系统 clang（与 std_test.rs / agg_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-ffi-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("extern FFI 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 基础：i64 参数/返回的 libc extern 调用。
#[test]
fn extern_basic_i64() {
    let out = run(
        r#"
extern fn labs(x: i64) -> i64;

fn main() {
    let a = labs(-42);
    println(a);          // 42
    println(labs(7));    // 7
}
"#,
    );
    assert_eq!(out, "42\n7\n");
}

/// f64 参数/返回与 void 返回。
#[test]
fn extern_f64_and_void() {
    let out = run(
        r#"
extern fn fabs(x: f64) -> f64;
extern fn srand(seed: i64) -> ();

fn main() {
    let x = fabs(-3.5);
    println(x);                 // 3.500000（f64 打印 6 位小数）
    println(fabs(-2.0) + 1.0);  // 3.000000
    srand(42);                  // void 返回调用
    println(1);
}
"#,
    );
    assert_eq!(out, "3.500000\n3.000000\n1\n");
}

/// 嵌套调用、循环、运算比较。
#[test]
fn extern_nested_loop_arith() {
    let out = run(
        r#"
extern fn labs(x: i64) -> i64;
extern fn llabs(x: i64) -> i64;

fn main() {
    // 嵌套 extern 调用
    println(labs(llabs(-9)));      // 9
    // 循环内调用
    let mut i = 0;
    while i < 3 {
        println(labs(-i));
        i = i + 1;
    }
    // 结果参与运算与比较
    let v = labs(-5) * 10 + 2;
    println(v);                    // 52
    println(v > 50);               // true
    // 返回值作参数
    println(labs(labs(-100) - 90)); // 10
}
"#,
    );
    assert_eq!(out, "9\n0\n1\n2\n52\ntrue\n10\n");
}
