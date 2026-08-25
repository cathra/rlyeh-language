//! `String` 分割/重复/填充集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/core.rl`）。
//!
//! 覆盖：`split`（返回 `Vec<String>`：多段/不含分隔符/空 sep/连续分隔符/
//! 尾部分隔符/空串）、`repeat`（多次/0 次/1 次）、`pad_start`/`pad_end`
//! （左/右填充/不足不填充）、混合组合。
//!
//! 需要系统 clang（与 driver_test.rs / std_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-string-split-pad-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("String 分割/重复/填充测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// split 基本：逗号多段 / 不含分隔符单段 / len + get 访问。
#[test]
fn string_split_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("apple,banana,cherry");
    let parts = s.split(String::from(","));
    println(parts.len());        // 3
    println(parts.get(0));       // apple
    println(parts.get(1));       // banana
    println(parts.get(2));       // cherry
    let one = String::from("solo");
    let p2 = one.split(String::from(","));
    println(p2.len());           // 1（不含分隔符）
    println(p2.get(0));          // solo（整体一段）
}
"#,
    );
    assert_eq!(out, "3\napple\nbanana\ncherry\n1\nsolo\n");
}

/// split 边界：空 sep 整体拷贝 / 连续分隔符空串段 / 尾部分隔符尾空段 / 空串。
#[test]
fn string_split_edge() {
    let out = run(
        r#"
fn main() {
    let a = String::from("hello");
    let p1 = a.split(String::from(""));
    println(p1.len());           // 1（空 sep 整体拷贝）
    println(p1.get(0));          // hello
    let b = String::from("a,,b");
    let p2 = b.split(String::from(","));
    println(p2.len());           // 3（连续分隔符产生空串段）
    println(p2.get(0));          // a
    println(p2.get(1));          // （空段）
    println(p2.get(2));          // b
    let c = String::from("x,y,");
    let p3 = c.split(String::from(","));
    println(p3.len());           // 3（尾部分隔符尾空段）
    println(p3.get(2));          // （空段）
    let d = String::from("");
    let p4 = d.split(String::from(","));
    println(p4.len());           // 1
    println(p4.get(0));          // （空串 -> 空行）
}
"#,
    );
    assert_eq!(out, "1\nhello\n3\na\n\nb\n3\n\n1\n\n");
}

/// split + String 操作组合：分割结果参与 to_upper / len / 拼接。
#[test]
fn string_split_combo() {
    let out = run(
        r#"
fn main() {
    let s = String::from("apple,banana,cherry");
    let parts = s.split(String::from(","));
    let first = parts.get(0);
    println(first.to_upper());   // APPLE
    println(first.len());        // 5
    let joined = first + String::from("-") + parts.get(2);
    println(joined);             // apple-cherry
    let last = parts.get(2);
    println(last.starts_with(String::from("cher")));  // true
}
"#,
    );
    assert_eq!(out, "APPLE\n5\napple-cherry\ntrue\n");
}

/// repeat 基本：3 次 / 0 次空 / 1 次自身 / 结果 len。
#[test]
fn string_repeat_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("ab");
    let r3 = s.repeat(3);
    println(r3);                 // ababab
    println(r3.len());           // 6
    let r0 = s.repeat(0);
    println(r0.len());           // 0（空串）
    let r1 = s.repeat(1);
    println(r1);                 // ab（自身）
    println(s.repeat(2) == String::from("abab"));  // true（内容相等）
}
"#,
    );
    assert_eq!(out, "ababab\n6\n0\nab\ntrue\n");
}

/// pad 基本：pad_start 左填充 / pad_end 右填充 / 不足不填充。
#[test]
fn string_pad_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("42");
    let l = s.pad_start(5, 48);   // 48 = '0'
    println(l);                   // 00042
    println(l.len());             // 5
    let r = s.pad_end(5, 48);
    println(r);                   // 42000
    let dash = s.pad_end(4, 45);  // 45 = '-'
    println(dash);                // 42--
    let over = s.pad_start(2, 48);
    println(over);                // 42（total <= len 原样）
    let mut e = String::new();
    let pe = e.pad_end(3, 48);
    println(pe);                  // 000（空串填充）
    println(pe.len());            // 3
}
"#,
    );
    assert_eq!(out, "00042\n5\n42000\n42--\n42\n000\n3\n");
}

/// 组合链式：trim + split + to_upper + repeat + pad 全链路。
#[test]
fn string_split_pad_combo() {
    let out = run(
        r#"
fn main() {
    let csv = String::from("  alpha,beta,gamma  ");
    let t = csv.trim();
    let parts = t.split(String::from(","));
    println(parts.len());              // 3
    println(parts.get(0));             // alpha
    let upp = parts.get(1).to_upper();
    println(upp);                      // BETA
    let r = parts.get(2).repeat(2);
    println(r);                        // gammagamma
    let p = parts.get(0).pad_start(6, 48);   // alpha len 5 -> 1 个 '0'
    println(p);                        // 0alpha
    let q = parts.get(0).pad_end(8, 45);     // 45 = '-'
    println(q);                        // alpha---
    println(q.len());                  // 8
}
"#,
    );
    assert_eq!(out, "3\nalpha\nBETA\ngammagamma\n0alpha\nalpha---\n8\n");
}
