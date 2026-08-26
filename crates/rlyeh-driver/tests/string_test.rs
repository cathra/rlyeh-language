//! `String` 标准库集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/core.rl`）。
//!
//! 覆盖：`String::from` / `new` / `with_capacity` 构造、`println(String)`、
//! `len` / `is_empty`、按字节 get / push_byte + 翻倍扩容、`s[i]` 索引。
//!
//! 需要系统 clang（与 driver_test.rs / vec_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-string-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("String 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 基础：`String::from("字面量")` 构造 + `println(String)` + len。
#[test]
fn string_from_print_len() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello");
    println(s);
    println(s.len());
    println(s.is_empty());
}
"#,
    );
    assert_eq!(out, "hello\n5\nfalse\n");
}

/// `String::new()` 空串 + push_byte 追加（触发 grow）+ 打印。
#[test]
fn string_new_push_grow() {
    let out = run(
        r#"
fn main() {
    let mut s = String::new();
    s.push_byte(72); // 'H'
    s.push_byte(105); // 'i'
    println(s);
    println(s.len());
    println(s.cap());
    let mut t = String::new();
    for i in 0..<3 {
        t.push_byte(65 + i); // 'A' 'B' 'C'
    }
    println(t);
}
"#,
    );
    assert_eq!(out, "Hi\n2\n8\nABC\n");
}

/// `String::with_capacity(n)` 预留容量 + get 按字节读取。
#[test]
fn string_with_capacity_get() {
    let out = run(
        r#"
fn main() {
    let s = String::from("rlyeh");
    println(s.get(0));
    println(s.get(3));
    let t = String::with_capacity(16);
    println(t.cap());
    println(t.len());
}
"#,
    );
    assert_eq!(out, "114\n101\n16\n0\n");
}

/// `s[i]` 直接索引（步长 1 字节）。
#[test]
fn string_index() {
    let out = run(
        r#"
fn main() {
    let s = String::from("abc");
    let sum = s[0] + s[1] + s[2];
    println(sum);
}
"#,
    );
    assert_eq!(out, "294\n");
}

/// 非 ASCII 内容：UTF-8 字节缓冲（len 为字节数，索引按字节）。
#[test]
fn string_utf8_bytes() {
    let out = run(
        r#"
fn main() {
    let s = String::from("中文");
    println(s.len()); // "中文" = 6 字节
    println(s.get(0)); // 0xe4 = 228
}
"#,
    );
    assert_eq!(out, "6\n228\n");
}

/// 函数间传递 String（值拷贝对象指针 + len/cap，data 缓冲共享）。
#[test]
fn string_passing() {
    let out = run(
        r#"
fn describe(s: String) -> i64 {
    s.len()
}
fn main() {
    let s = String::from("rlyeh");
    println(describe(s));
    println(s.len());
}
"#,
    );
    assert_eq!(out, "5\n5\n");
}
