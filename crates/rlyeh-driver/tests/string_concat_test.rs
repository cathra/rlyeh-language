//! `String` 拼接集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/core.rl`）。
//!
//! 覆盖：`a + b` 运算符拼接、链式拼接 `a + b + c`、`push_str` 方法直接调用、
//! 拼接后 len / 索引 / 内容相等比较、多次拼接触发扩容。
//!
//! 前置特性：`String::from` 字面量构造 + `println(String)` + `s[i]` 索引
//! + `s1 == s2` 内容相等比较。
//!
//! 需要系统 clang（与 driver_test.rs / vec_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-scat-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("String 拼接测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 基本拼接：`a + b` → 内容拼接，println 输出。
#[test]
fn concat_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("Hello, ") + String::from("World!");
    println(s);
    println(s.len());
}
"#,
    );
    assert_eq!(out, "Hello, World!\n13\n");
}

/// 链式拼接：`a + b + c`（左结合，递归 desugar）。
#[test]
fn concat_chain() {
    let out = run(
        r#"
fn main() {
    let a = String::from("foo");
    let b = String::from("bar");
    let c = String::from("baz");
    let s = a + b + c;
    println(s);
    println(s.len());
}
"#,
    );
    assert_eq!(out, "foobarbaz\n9\n");
}

/// 拼接结果索引：按字节读取（步长 1）。
#[test]
fn concat_index() {
    let out = run(
        r#"
fn main() {
    let s = String::from("ab") + String::from("cd");
    println(s[0]);
    println(s[2]);
    println(s[3]);
}
"#,
    );
    // 'a'=97 'c'=99 'd'=100
    assert_eq!(out, "97\n99\n100\n");
}

/// 拼接结果内容相等比较：`==`/`!=` 对拼接产物生效。
#[test]
fn concat_eq() {
    let out = run(
        r#"
fn main() {
    let s = String::from("ab") + String::from("cd");
    println(s == String::from("abcd"));
    println(s == String::from("abce"));
    println(s != String::from("ab"));
}
"#,
    );
    assert_eq!(out, "true\nfalse\ntrue\n");
}

/// `push_str` 方法直接调用：追加到已有字符串。
#[test]
fn push_str_direct() {
    let out = run(
        r#"
fn main() {
    let mut s = String::from("a");
    s.push_str(String::from("b"));
    s.push_str(String::from("c"));
    println(s);
    println(s.len());
    println(s.is_empty());
}
"#,
    );
    assert_eq!(out, "abc\n3\nfalse\n");
}

/// 多次拼接触发扩容：cap 0 → 8 → 16（长字符串 + 反复追加）。
#[test]
fn concat_grow() {
    let out = run(
        r#"
fn main() {
    let mut s = String::from("start:");
    let mut i = 0;
    while i < 10 {
        s = s + String::from("x");
        i = i + 1;
    }
    println(s.len());
    println(s.cap());
    println(s);
}
"#,
    );
    // "start:" 6 字节 + 10 个 'x' = 16；
    // A3 后为拷贝语义：每轮 `s = s.clone() + "x"`，clone 从 cap 0 开始逐字节
    // push_byte（0 → 8 → 16 翻倍）：第 3 轮（len 8 → 9）grow 到 16，此后 16 足够
    // → 最终 len 16 / cap 16（旧共享语义为 from cap 6 → 12 → 24）
    assert_eq!(out, "16\n16\nstart:xxxxxxxxxx\n");
}

/// A3 回归：拼接结果为深拷贝——修改拼接结果不影响左操作数。
#[test]
fn concat_alias_result_isolation() {
    let out = run(
        r#"
fn main() {
    let a = String::from("hello");
    let s = a + String::from("!");
    s.push_str(String::from("!!"));
    println(a);
    println(s);
}
"#,
    );
    // 共享缓冲 bug：s.push_str 会污染 a（输出 hello!!!/hello!!!）
    assert_eq!(out, "hello\nhello!!!\n");
}

/// A3 回归：拼接结果为深拷贝——左操作数扩容（realloc）不影响拼接结果。
#[test]
fn concat_alias_lhs_isolation() {
    let out = run(
        r#"
fn main() {
    let a = String::from("hello");
    let s = a + String::from("!");
    a.push_str(String::from(" world"));
    println(s);
    println(s.len());
    println(a);
}
"#,
    );
    // 共享缓冲 bug：a 扩容后 s 内容被污染（hello! world / 12）
    assert_eq!(out, "hello!\n6\nhello world\n");
}

/// `String::clone` 深拷贝直接调用：内容相等、互不影响。
#[test]
fn clone_direct() {
    let out = run(
        r#"
fn main() {
    let a = String::from("clone me");
    let b = a.clone();
    a.push_str(String::from("!"));
    println(b);
    println(b == String::from("clone me"));
    println(a);
}
"#,
    );
    assert_eq!(out, "clone me\ntrue\nclone me!\n");
}
