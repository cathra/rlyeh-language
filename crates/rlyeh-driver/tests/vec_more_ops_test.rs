//! `Vec<T>` 补充方法集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/`）。
//!
//! 覆盖：`first`/`last`（含空容器返回 None）、`reverse`、`swap`、
//! `binary_search`（升序命中下标 / 未命中 -1，i64 + String 双实例化）。
//!
//! 需要系统 clang（与 std_test.rs / agg_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-vecmore-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("Vec 补充方法测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// first / last：含空容器（None）与多元素。
#[test]
fn vec_first_last() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    println(v.first().is_none());   // 1（空）
    println(v.last().is_none());    // 1（空）
    v.push(10);
    v.push(20);
    v.push(30);
    println(v.first().unwrap());    // 10
    println(v.last().unwrap());     // 30
    // 单元素：first == last
    let mut one: Vec<i64> = Vec::new();
    one.push(7);
    println(one.first().unwrap());  // 7
    println(one.last().unwrap());   // 7
}
"#,
    );
    assert_eq!(out, "1\n1\n10\n30\n7\n7\n");
}

/// reverse / swap：原地修改 + 下标读取验证。
#[test]
fn vec_reverse_swap() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(1); v.push(2); v.push(3); v.push(4);
    v.reverse();
    println(v.get(0));   // 4
    println(v.get(3));   // 1
    v.swap(0, 3);
    println(v.get(0));   // 1
    println(v.get(3));   // 4
    // reverse 后再 reverse 复原
    v.reverse();
    println(v.get(0));   // 4
    println(v.get(3));   // 1
}
"#,
    );
    assert_eq!(out, "4\n1\n1\n4\n4\n1\n");
}

/// binary_search：升序数组命中下标 / 未命中 -1 / 边界元素。
#[test]
fn vec_binary_search_i64() {
    let out = run(
        r#"
fn main() {
    let mut vs: Vec<i64> = Vec::new();
    vs.push(1); vs.push(3); vs.push(5); vs.push(7); vs.push(9);
    println(vs.binary_search(5));   // 2
    println(vs.binary_search(1));   // 0（左边界）
    println(vs.binary_search(9));   // 4（右边界）
    println(vs.binary_search(4));   // -1（未命中）
    println(vs.binary_search(0));   // -1（小于全部）
    println(vs.binary_search(10));  // -1（大于全部）
}
"#,
    );
    assert_eq!(out, "2\n0\n4\n-1\n-1\n-1\n");
}

/// binary_search：String 实例化（字典序比较）。
#[test]
fn vec_binary_search_string() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<String> = Vec::new();
    v.push(String::from("apple"));
    v.push(String::from("banana"));
    v.push(String::from("cherry"));
    println(v.binary_search(String::from("banana")));  // 1
    println(v.binary_search(String::from("apple")));   // 0
    println(v.binary_search(String::from("cherry")));  // 2
    println(v.binary_search(String::from("grape")));   // -1
    // sort + binary_search 组合
    let mut u: Vec<String> = Vec::new();
    u.push(String::from("pear"));
    u.push(String::from("fig"));
    u.push(String::from("date"));
    u.sort();
    println(u.binary_search(String::from("date")));    // 0
    println(u.binary_search(String::from("fig")));     // 1
    println(u.binary_search(String::from("pear")));    // 2
}
"#,
    );
    assert_eq!(out, "1\n0\n2\n-1\n0\n1\n2\n");
}

/// 组合：first/last 与 reverse/swap/binary_search 混合使用。
#[test]
fn vec_more_combined() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(5); v.push(1); v.push(3); v.push(2); v.push(4);
    v.sort();
    println(v.first().unwrap());           // 1
    println(v.last().unwrap());            // 5
    println(v.binary_search(3));           // 2
    v.reverse();
    println(v.first().unwrap());           // 5
    v.swap(0, 4);
    println(v.get(0));                     // 1
    println(v.get(4));                     // 5
    println(v.binary_search(5));           // 4
}
"#,
    );
    assert_eq!(out, "1\n5\n2\n5\n1\n5\n4\n");
}
