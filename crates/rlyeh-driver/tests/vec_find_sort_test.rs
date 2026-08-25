//! `Vec<T>` 查找/排序集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/core.rl`）。
//!
//! 覆盖：`find`（首个命中下标 / 未命中 -1 / 空 Vec / String 内容相等）、
//! `sort`（升序 / 已排序不动 / 空与单元素 / 重复元素 / String 字典序）、
//! 混合组合（sort + find + remove + contains）。
//!
//! 需要系统 clang（与 driver_test.rs / std_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-vec-find-sort-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("Vec 查找/排序测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// find 基本：i64 首个命中 / 未命中 -1 / 多出现取首个 / 空 Vec。
#[test]
fn vec_find_basic() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(10);
    v.push(20);
    v.push(30);
    v.push(20);
    println(v.find(20));   // 1（首个命中，非 3）
    println(v.find(10));   // 0（头部）
    println(v.find(30));   // 2（尾部）
    println(v.find(99));   // -1（未命中）
    let mut e: Vec<i64> = Vec::new();
    println(e.find(1));    // -1（空 Vec）
}
"#,
    );
    assert_eq!(out, "1\n0\n2\n-1\n-1\n");
}

/// find String 元素：内容相等命中下标 / 未命中 / 大小写敏感。
#[test]
fn vec_find_string() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<String> = Vec::new();
    v.push(String::from("alpha"));
    v.push(String::from("beta"));
    v.push(String::from("gamma"));
    println(v.find(String::from("beta")));   // 1
    println(v.find(String::from("alpha")));  // 0
    println(v.find(String::from("omega")));  // -1（未命中）
    println(v.find(String::from("BETA")));   // -1（大小写敏感）
}
"#,
    );
    assert_eq!(out, "1\n0\n-1\n-1\n");
}

/// sort 基本：乱序升序 + len 不变 + 首尾正确。
#[test]
fn vec_sort_basic() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(50);
    v.push(10);
    v.push(40);
    v.push(20);
    v.push(30);
    v.sort();                     // [10,20,30,40,50]
    println(v.len());             // 5
    println(v.get(0));            // 10
    println(v.get(1));            // 20
    println(v.get(2));            // 30
    println(v.get(3));            // 40
    println(v.get(4));            // 50
}
"#,
    );
    assert_eq!(out, "5\n10\n20\n30\n40\n50\n");
}

/// sort 边界：已排序不动 / 单元素 / 空 Vec / 重复元素升序。
#[test]
fn vec_sort_edge() {
    let out = run(
        r#"
fn main() {
    let mut s: Vec<i64> = Vec::new();
    s.push(1);
    s.push(2);
    s.push(3);
    s.sort();                     // 已排序
    println(s.get(0));            // 1
    println(s.get(2));            // 3
    let mut one: Vec<i64> = Vec::new();
    one.push(7);
    one.sort();
    println(one.get(0));          // 7（单元素不动）
    let mut e: Vec<i64> = Vec::new();
    e.sort();
    println(e.len());             // 0（空 Vec 安全）
    let mut d: Vec<i64> = Vec::new();
    d.push(3);
    d.push(1);
    d.push(3);
    d.push(2);
    d.push(1);
    d.sort();                     // [1,1,2,3,3]（重复元素）
    println(d.get(0) + d.get(1)); // 2
    println(d.get(2));            // 2
    println(d.get(3) + d.get(4)); // 6
}
"#,
    );
    assert_eq!(out, "1\n3\n7\n0\n2\n2\n6\n");
}

/// sort String：字典序排序。
#[test]
fn vec_sort_string() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<String> = Vec::new();
    v.push(String::from("pear"));
    v.push(String::from("apple"));
    v.push(String::from("cherry"));
    v.push(String::from("banana"));
    v.sort();
    println(v.get(0));            // apple
    println(v.get(1));            // banana
    println(v.get(2));            // cherry
    println(v.get(3));            // pear
}
"#,
    );
    assert_eq!(out, "apple\nbanana\ncherry\npear\n");
}

/// 组合：sort + find + remove + contains 全链路。
#[test]
fn vec_find_sort_combo() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(50);
    v.push(10);
    v.push(30);
    v.push(20);
    v.sort();                     // [10,20,30,50]
    println(v.get(0));            // 10
    println(v.find(30));          // 2（排序后下标）
    println(v.find(99));          // -1
    let x = v.remove(2);          // 删 30
    println(x);                   // 30
    println(v.contains(30));      // false
    println(v.contains(50));      // true
    println(v.len());             // 3
}
"#,
    );
    assert_eq!(out, "10\n2\n-1\n30\nfalse\ntrue\n3\n");
}
