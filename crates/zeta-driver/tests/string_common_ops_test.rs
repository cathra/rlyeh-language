//! `String` 常用操作集成测试（`starts_with` / `ends_with` / `replace`）。
//!
//! 覆盖：前缀判断（命中/不命中/空前缀/长于自身/全等/大小写敏感）、后缀判断
//! （命中/不命中/空后缀/长于自身/全等）、前缀后缀与 `substring`/`find` 配合、
//! 替换（单次/多次/未命中原样/替换串更长/空 old/空 new 删除/old == new）、
//! 链式组合（trim + replace + 大小写 + 前缀后缀 + 子串 + find + contains）。
//!
//! 实现：core.zeta 纯 Zeta 方法（`starts_with`/`ends_with` 逐字节比较 +
//! 标志变量 + result 模式，`replace` 滑动窗口扫描 + 空 old 特判防死循环）。
//!
//! 需要系统 clang（与 string_case_trim_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-string-common-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("String 常用操作测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 前缀判断基本：命中/不命中/空前缀/长于自身/全等/大小写敏感。
#[test]
fn starts_with_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello world");
    println(s.starts_with(String::from("he")));      // true
    println(s.starts_with(String::from("wo")));      // false（不在开头）
    println(s.starts_with(String::from("")));        // true（空前缀）
    println(s.starts_with(String::from("hello world!"))); // false（长于自身）
    println(s.starts_with(s));                       // true（与自身全等）
    println(s.starts_with(String::from("Hello")));   // false（大小写敏感）
}
"#,
    );
    assert_eq!(out, "true\nfalse\ntrue\nfalse\ntrue\nfalse\n");
}

/// 后缀判断基本：命中/不命中/空后缀/长于自身/全等/大小写敏感。
#[test]
fn ends_with_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello world");
    println(s.ends_with(String::from("world")));     // true
    println(s.ends_with(String::from("hello")));     // false（不在结尾）
    println(s.ends_with(String::from("")));          // true（空后缀）
    println(s.ends_with(String::from("a hello world"))); // false（长于自身）
    println(s.ends_with(s));                         // true（与自身全等）
    println(s.ends_with(String::from("World")));     // false（大小写敏感）
}
"#,
    );
    assert_eq!(out, "true\nfalse\ntrue\nfalse\ntrue\nfalse\n");
}

/// 前缀/后缀与子串、查找配合：嵌套 if 组合判断、substring 提取协议、find 定位。
#[test]
fn prefix_suffix_combo() {
    let out = run(
        r#"
fn main() {
    let s = String::from("https://zeta.dev/docs");
    println(s.starts_with(String::from("https")));   // true
    // 嵌套 if 组合：https 前缀 && docs 后缀
    let mut verdict = 0;
    if s.starts_with(String::from("https")) {
        if s.ends_with(String::from("docs")) {
            verdict = 1;
        }
    }
    println(verdict);                                // 1
    // starts_with 判断后 substring 提取协议名
    let proto = s.substring(0, 5);
    println(proto);                                  // https
    // find 定位 "//" 后取路径（跳过两个 '/'），再判断前后缀
    let slash = s.find(String::from("/"));
    let path = s.substring(slash + 2, s.len);
    println(path);                                   // zeta.dev/docs
    println(path.ends_with(String::from("docs")));   // true
    println(path.starts_with(String::from("zeta"))); // true
}
"#,
    );
    assert_eq!(
        out,
        "true\n1\nhttps\nzeta.dev/docs\ntrue\ntrue\n"
    );
}

/// 替换基本：单次命中/未命中原样/多次替换/内容相等断言/原串不受影响。
#[test]
fn replace_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("foo bar foo baz");
    let r1 = s.replace(String::from("foo"), String::from("qux"));
    println(r1);                                     // qux bar qux baz
    let r2 = s.replace(String::from("xyz"), String::from("nope"));
    println(r2);                                     // foo bar foo baz（未命中）
    println(r1 == String::from("qux bar qux baz"));  // true
    println(s.len);                                  // 15（原串不受影响）
    println(s.starts_with(String::from("foo")));     // true（原串仍可继续用）
}
"#,
    );
    assert_eq!(out, "qux bar qux baz\nfoo bar foo baz\ntrue\n15\ntrue\n");
}

/// 替换边界：空 old 原样拷贝/空 new 删除所有 old/old == new/替换串更长。
#[test]
fn replace_edge() {
    let out = run(
        r#"
fn main() {
    let s = String::from("ababab");
    // 空 old：不替换，返回自身拷贝
    let r1 = s.replace(String::from(""), String::from("X"));
    println(r1);                                     // ababab
    // 空 new：删除所有 old
    let r2 = s.replace(String::from("ab"), String::from(""));
    println(r2.len);                                 // 0（已全部删除）
    println(r2.is_empty());                          // true
    // old == new：原样返回
    let r3 = s.replace(String::from("ab"), String::from("ab"));
    println(r3);                                     // ababab
    // 替换串更长：命中处跳过 old.len 不回溯
    let t = String::from("aaba");
    let r4 = t.replace(String::from("aa"), String::from("YYY"));
    println(r4);                                     // YYYba
    println(r4.len);                                 // 5
}
"#,
    );
    assert_eq!(out, "ababab\n0\ntrue\nababab\nYYYba\n5\n");
}

/// 链式组合：trim + replace + 大小写 + 前缀后缀 + 子串 + find + contains。
#[test]
fn chain_combinations() {
    let out = run(
        r#"
fn main() {
    let raw = String::from("  Hello, Zeta!  ");
    let clean = raw.trim();
    let greeting = clean.replace(String::from("Zeta"), String::from("World"));
    println(greeting);                               // Hello, World!
    println(greeting.starts_with(String::from("Hello")));  // true
    println(greeting.ends_with(String::from("!")));        // true
    println(greeting.ends_with(String::from("?")));        // false
    // replace + find + substring + contains 组合
    let msg = String::from("a-b-c-d");
    let parts = msg.replace(String::from("-"), String::from(","));
    println(parts);                                  // a,b,c,d
    println(parts.contains(String::from(",")));      // true
    let first = parts.substring(0, parts.find(String::from(",")));
    println(first);                                  // a
    // replace 后继续前缀判断（http -> https）
    let url = String::from("http://zeta.dev");
    let https = url.replace(String::from("http"), String::from("https"));
    println(https.starts_with(String::from("https")));  // true
    println(https.ends_with(String::from(".dev")));     // true
}
"#,
    );
    assert_eq!(
        out,
        "Hello, World!\ntrue\ntrue\nfalse\na,b,c,d\ntrue\na\ntrue\ntrue\n"
    );
}
