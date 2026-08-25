//! `String` 前缀/后缀剥离与截断集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/core.rl`）。
//!
//! 覆盖：`strip_prefix`（命中/未命中/空串/长于自身/恰好相等）、`strip_suffix`
//! （命中/未命中/空串/长于自身/完全相等）、`truncate`（截断/0/负数/超长/
//! 不影响原串）、组合链式。
//!
//! 注意：`Option::None` 上 `unwrap` 会 `loop {}` 死循环，未命中路径一律用
//! `is_some`/`is_none`（返回 i64 标志）断言，命中后再 `unwrap` 取内容。
//!
//! 需要系统 clang（与 driver_test.rs / std_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-string-strip-trunc-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("String 剥离/截断测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// strip_prefix 基本：命中剥离 / 未命中 is_none / 命中内容 unwrap。
#[test]
fn string_strip_prefix_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello");
    let hit = s.strip_prefix(String::from("he"));
    println(hit.is_some());        // 1（命中）
    println(hit.unwrap());         // llo（剩余部分）
    let miss = s.strip_prefix(String::from("xx"));
    println(miss.is_none());       // 1（未命中）
    println(miss.is_some());       // 0
    let full = s.strip_prefix(String::from("hello"));
    println(full.is_some());       // 1（完全相等命中）
    println(full.unwrap().len());  // 0（剩余空串）
}
"#,
    );
    assert_eq!(out, "1\nllo\n1\n0\n1\n0\n");
}

/// strip_prefix 边界：空前缀整体拷贝 / 长于自身未命中 / 部分前缀未命中。
#[test]
fn string_strip_prefix_edge() {
    let out = run(
        r#"
fn main() {
    let s = String::from("banana");
    let empty = s.strip_prefix(String::from(""));
    println(empty.is_some());      // 1（空前缀恒命中）
    println(empty.unwrap());       // banana（整体拷贝）
    let long = s.strip_prefix(String::from("bananax"));
    println(long.is_none());       // 1（前缀长于自身）
    let partial = s.strip_prefix(String::from("ban"));
    println(partial.is_some());    // 1
    println(partial.unwrap());     // ana（去掉 ban）
    let wrong = s.strip_prefix(String::from("bax"));
    println(wrong.is_none());      // 1（首字节相同后续不同）
}
"#,
    );
    assert_eq!(out, "1\nbanana\n1\n1\nana\n1\n");
}

/// strip_suffix 基本：命中剥离 / 未命中 is_none / 剩余内容。
#[test]
fn string_strip_suffix_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello");
    let hit = s.strip_suffix(String::from("lo"));
    println(hit.is_some());        // 1（命中）
    println(hit.unwrap());         // hel（剩余部分）
    let miss = s.strip_suffix(String::from("xx"));
    println(miss.is_none());       // 1（未命中）
    let full = s.strip_suffix(String::from("hello"));
    println(full.is_some());       // 1（完全相等命中）
    println(full.unwrap().len());  // 0（剩余空串）
}
"#,
    );
    assert_eq!(out, "1\nhel\n1\n1\n0\n");
}

/// strip_suffix 边界：空后缀整体拷贝 / 长于自身未命中 / 尾部部分匹配未命中。
#[test]
fn string_strip_suffix_edge() {
    let out = run(
        r#"
fn main() {
    let s = String::from("filename.txt");
    let empty = s.strip_suffix(String::from(""));
    println(empty.is_some());      // 1（空后缀恒命中）
    println(empty.unwrap());       // filename.txt（整体拷贝）
    let ext = s.strip_suffix(String::from(".txt"));
    println(ext.is_some());        // 1
    println(ext.unwrap());         // filename（去掉 .txt）
    let long = s.strip_suffix(String::from(".txtx"));
    println(long.is_none());       // 1（后缀长于自身）
    let wrong = s.strip_suffix(String::from("x.txt"));
    println(wrong.is_none());      // 1（尾段相近但并非后缀）
}
"#,
    );
    assert_eq!(out, "1\nfilename.txt\n1\nfilename\n1\n1\n");
}

/// truncate 基本：截断到前 N 字节 / 0 与负数空串 / 超长整体拷贝 / 原串不受影响。
#[test]
fn string_truncate_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("abcdef");
    println(s.truncate(3));        // abc（前 3 字节）
    println(s.truncate(0).len());  // 0（0 截断空串）
    println(s.truncate(-1).len()); // 0（负数 clamp 空串）
    println(s.truncate(6));        // abcdef（恰好等长）
    println(s.truncate(100));      // abcdef（超长整体拷贝）
    println(s.len());              // 6（原串不受影响）
}
"#,
    );
    assert_eq!(out, "abc\n0\n0\nabcdef\nabcdef\n6\n");
}

/// 组合：strip + to_upper / strip_suffix 链式 / truncate + repeat / 前后剥离互补。
#[test]
fn string_strip_trunc_combine() {
    let out = run(
        r#"
fn main() {
    let s = String::from("  hello world  ");
    let stripped = s.trim();
    let p = stripped.strip_prefix(String::from("hello"));
    println(p.is_some());          // 1
    println(p.unwrap().to_upper()); // " WORLD"（剩余大写）
    let ext = String::from("report.md");
    let base = ext.strip_suffix(String::from(".md")).unwrap();
    println(base.to_upper());      // REPORT
    println(base.pad_end(10, 45)); // report----（小写原串 + 4 个 -，补到 10 字节）
    let t = String::from("hello world").truncate(5);
    println(t);                    // hello（截断）
    println(t.repeat(2));          // hellohello（截断后重复）
    let both = String::from("prefix-middle-suffix");
    let mid = both.strip_prefix(String::from("prefix-")).unwrap();
    let mid2 = mid.strip_suffix(String::from("-suffix")).unwrap();
    println(mid2);                 // middle（前后各剥一次）
}
"#,
    );
    assert_eq!(out, "1\n WORLD\nREPORT\nreport----\nhello\nhellohello\nmiddle\n");
}
