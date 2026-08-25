//! `String::from` 运行期内容集成测试（G2 块一：消除 §13 约束 4「非字面量长度表达未实现」）。
//!
//! 覆盖：`String::from(runtime_string)` 深拷贝（`≡ s.clone()`）、
//! 拷贝内容相等、嵌套 from、运行期拼接后的长度表达、副本独立性。
//!
//! 前置特性：`String::from` 字面量/变量构造 + `push_str` + `s[i]` 索引
//! + `s1 == s2` 内容相等比较 + `println(String)`。
//!
//! 需要系统 clang（与 string_concat_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-sfrom-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("String::from 运行期测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 运行期 String → 深拷贝：副本 push_str 不影响原串。
#[test]
fn from_runtime_deep_copy() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello");
    let t = String::from(s);          // 运行期深拷贝
    t.push_str(String::from(" world"));
    println(t.len());                 // 11
    println(s.len());                 // 5（原串不受影响）
    println(s);
}
"#,
    );
    assert_eq!(out, "11\n5\nhello\n");
}

/// 运行期拷贝后内容相等（== 逐字节比较）。
#[test]
fn from_runtime_equals() {
    let out = run(
        r#"
fn main() {
    let s = String::from("rlyeh");
    let t = String::from(s);
    println(t == s);                  // true
    t.push_str(String::from("!"));
    println(t == s);                  // false
}
"#,
    );
    assert_eq!(out, "true\nfalse\n");
}

/// 嵌套 from：`String::from(String::from("x"))`。
#[test]
fn from_runtime_nested() {
    let out = run(
        r#"
fn main() {
    let inner = String::from("nested");
    let outer = String::from(inner);
    let outer2 = String::from(outer);
    println(outer2.len());
    println(outer2);
}
"#,
    );
    assert_eq!(out, "6\nnested\n");
}

/// 运行期 String 经函数参数传递后再 from（内容完全运行期决定）。
#[test]
fn from_runtime_via_arg() {
    let out = run(
        r#"
fn echo(s: String) -> String {
    String::from(s)
}

fn main() {
    let s = String::from("abc") + String::from("def");  // 运行期拼接
    let t = echo(s);
    println(t.len());                 // 6
    println(t == String::from("abcdef"));
}
"#,
    );
    assert_eq!(out, "6\ntrue\n");
}

/// 运行期长度表达：内容由运行期拼接决定，from 读取 len 槽而非编译期字面量。
#[test]
fn from_runtime_length_expression() {
    let out = run(
        r#"
fn make(n: i64) -> String {
    let mut s = String::from("");
    let mut i = 0;
    while i < n {
        s.push_str(String::from("x"));
        i += 1;
    }
    String::from(s)
}

fn main() {
    let t = make(7);
    println(t.len());                 // 7（运行期长度，非字面量）
    println(t);
}
"#,
    );
    assert_eq!(out, "7\nxxxxxxx\n");
}

/// 副本扩容（cap 变化）不影响原串的数据缓冲。
#[test]
fn from_runtime_independent_buffer() {
    let out = run(
        r#"
fn main() {
    let s = String::from("ab");
    let t = String::from(s);
    let mut i = 0;
    while i < 10 {
        t.push_str(String::from("z"));
        i += 1;
    }
    println(t.len());                 // 12（触发多次扩容）
    println(s.len());                 // 2（原串缓冲独立）
    println(s);
}
"#,
    );
    assert_eq!(out, "12\n2\nab\n");
}
