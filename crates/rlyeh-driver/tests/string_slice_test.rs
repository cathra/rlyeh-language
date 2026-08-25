//! `String` 子串/查找集成测试（`substring` / `find` / `contains`）。
//!
//! 覆盖：字节区间截取、边界 clamp（负 start / 超长 end / start >= end）、
//! 朴素查找命中/未命中/空子串、包含判断、find+substring 组合提取、
//! 拼接 + 子串链式操作。
//!
//! 实现：core.rl 纯 Rlyeh 方法（`substring` 逐字节 push_byte 拷贝，
//! `find` 朴素滑动窗口匹配 + result 变量返回，`contains` = `find >= 0`）。
//!
//! 需要系统 clang（与 driver_test.rs / string_eq_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-string-slice-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("String 子串测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 基本截取：`substring(start, end)` 返回 [start, end) 字节区间的新缓冲，
/// 原字符串不受影响；空区间返回空串。
#[test]
fn substring_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello world");
    println(s.substring(0, 5));               // hello
    println(s.substring(6, 11));              // world
    println(s.substring(3, 8));               // lo wo
    println(s.substring(0, 0).len());         // 空区间
    println(s.substring(0, 5) == String::from("hello"));
    println(s.len());                         // 原串不受影响
}
"#,
    );
    assert_eq!(out, "hello\nworld\nlo wo\n0\ntrue\n11\n");
}

/// 边界 clamp：负 start 收敛到 0，超长 end 收敛到 len，
/// start >= end（含双向越界）返回空串。
#[test]
fn substring_clamp() {
    let out = run(
        r#"
fn main() {
    let s = String::from("abcdef");
    println(s.substring(-3, 2));              // ab
    println(s.substring(4, 99));              // ef
    println(s.substring(-10, 99));            // abcdef
    println(s.substring(3, 1).len());         // start >= end
    println(s.substring(6, 2).len());         // start >= end（反向）
    println(s.substring(6, 10).len());        // 空区间（越界后收敛）
}
"#,
    );
    assert_eq!(out, "ab\nef\nabcdef\n0\n0\n0\n");
}

/// 朴素查找：返回首次出现下标；未命中 -1；空子串 0；整串/单字节命中。
#[test]
fn find_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello world");
    println(s.find(String::from("world")));   // 6
    println(s.find(String::from("o")));       // 4（首个出现）
    println(s.find(String::from("xyz")));     // -1
    println(s.find(String::from("")));        // 空子串 = 0
    println(s.find(String::from("hello world"))); // 整串 = 0
    println(s.find(String::from("d")));       // 10
}
"#,
    );
    assert_eq!(out, "6\n4\n-1\n0\n0\n10\n");
}

/// 包含判断：`contains(sub)` = `find(sub) >= 0`；空子串恒真；
/// 自身包含自身；比自身长的子串恒假。
#[test]
fn contains_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello world");
    println(s.contains(String::from("world")));   // true
    println(s.contains(String::from("xyz")));     // false
    println(s.contains(String::from("")));        // true
    println(s.contains(String::from("h")));       // true
    println(s.contains(String::from("hello world!"))); // false
    println(s.contains(s));                       // 自身包含自身
}
"#,
    );
    assert_eq!(out, "true\nfalse\ntrue\ntrue\nfalse\ntrue\n");
}

/// 组合：find 返回的下标直接喂给 substring 提取子串；
/// 子串结果可继续 find。
#[test]
fn find_and_substring_combo() {
    let out = run(
        r#"
fn main() {
    let s = String::from("the quick brown fox");
    let p = s.find(String::from("brown"));        // 10
    println(p);
    println(s.substring(p, p + 5) == String::from("brown"));
    let sub = s.substring(0, 3);                  // "the"
    println(sub == String::from("the"));
    println(sub.find(String::from("h")));         // 1
    println(s.substring(4, 9) == String::from("quick"));
}
"#,
    );
    assert_eq!(out, "10\ntrue\ntrue\n1\ntrue\n");
}

/// 链式：拼接结果取子串、子串结果再拼接/查找，
/// 验证多次 3 槽缓冲搬运 + 扩容不破坏内容。
#[test]
fn concat_and_substring_chain() {
    let out = run(
        r#"
fn main() {
    let a = String::from("foo");
    let b = String::from("bar");
    let c = a + b;                                // "foobar"
    println(c);
    println(c.substring(0, 3) == String::from("foo"));
    println(c.substring(3, 6) == String::from("bar"));
    let d = c.substring(1, 5) + String::from("!"); // "ooba!"
    println(d);
    println(d.contains(String::from("oba")));
    println(d.find(String::from("!")));           // 4
}
"#,
    );
    assert_eq!(out, "foobar\ntrue\ntrue\nooba!\ntrue\n4\n");
}
