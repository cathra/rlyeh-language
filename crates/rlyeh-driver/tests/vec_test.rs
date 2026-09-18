//! `Vec<T>` 标准库集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/`）。
//!
//! 覆盖：`Vec::with_capacity` / `Vec::new` 构造、push 翻倍扩容、get/set、
//! pop 返回 Option、泛型多类型实例化（i64/f64）、函数间别名共享。
//!
//! 需要系统 clang（与 driver_test.rs / std_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-vec-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("Vec 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 基础：with_capacity 构造 + push + len + get。
#[test]
fn vec_push_grow_get() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::with_capacity(2);
    v.push(10);
    v.push(20);
    v.push(30); // 触发扩容（cap 2 -> 4）
    v.push(40);
    println(v.len());
    let sum = v.get(0) + v.get(1) + v.get(2) + v.get(3);
    println(sum);
    println(v.cap());
}
"#,
    );
    assert_eq!(out, "4\n100\n4\n");
}

/// `Vec::new()` 默认容量 4，无需类型注解（由 push 参数推断）。
#[test]
fn vec_new_no_annotation() {
    let out = run(
        r#"
fn main() {
    let mut v = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    v.push(4);
    v.push(5); // 触发扩容（cap 4 -> 8）
    println(v.len());
    println(v.get(4));
}
"#,
    );
    assert_eq!(out, "5\n5\n");
}

/// set 修改既有槽位。
#[test]
fn vec_set() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::with_capacity(3);
    v.push(1);
    v.push(2);
    v.push(3);
    v.set(1, 99);
    println(v.get(0));
    println(v.get(1));
    println(v.get(2));
}
"#,
    );
    assert_eq!(out, "1\n99\n3\n");
}

/// pop 返回 Option：有值 / 空 Vec。
#[test]
fn vec_pop() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::with_capacity(2);
    v.push(7);
    v.push(8);
    let a = v.pop();
    println(a.is_some());
    println(a.unwrap());
    println(v.len());
    let b = v.pop();
    let c = v.pop();
    println(b.unwrap());
    println(c.is_none());
}
"#,
    );
    assert_eq!(out, "1\n8\n1\n7\n1\n");
}

/// 泛型多类型实例化：`Vec<f64>` 与 `Vec<i64>` 同时使用。
#[test]
fn vec_multiple_type_params() {
    let out = run(
        r#"
fn main() {
    let mut vi: Vec<i64> = Vec::with_capacity(2);
    vi.push(1);
    vi.push(2);
    let mut vf: Vec<f64> = Vec::with_capacity(2);
    vf.push(1.5);
    vf.push(2.5);
    let fsum = vf.get(0) + vf.get(1);
    println(fsum);
    println(vi.len());
}
"#,
    );
    // f64 经 `printf("%f")` 固定 6 位小数（语言现状）
    assert_eq!(out, "4.000000\n2\n");
}

/// 函数间别名共享：Vec 对象按指针传递，函数内读取共享的底层槽区。
#[test]
fn vec_alias_share() {
    let out = run(
        r#"
fn peek(v: Vec<i64>) -> i64 {
    v.len()
}

fn main() {
    let mut v: Vec<i64> = Vec::with_capacity(4);
    let mut i = 0;
    while i < 6 {
        v.push(i * 10);
        i = i + 1;
    }
    println(peek(v)); // 值传递共享同一对象
    println(v.len()); // 原变量仍可见
    println(v.get(5));
}
"#,
    );
    assert_eq!(out, "6\n6\n50\n");
}
