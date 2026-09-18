//! `Vec<T>` 常用方法集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/`）。
//!
//! 覆盖：`contains`（i64/String 元素相等判断）、`remove`（删除前移 + 返回值）、
//! `insert`（中间/头部/尾部插入 + 扩容）、`clear`（清空 + 复用）、混合组合。
//!
//! 需要系统 clang（与 driver_test.rs / std_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-vec-common-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("Vec 常用方法测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// contains 基本：i64 命中/未命中/空 Vec/多出现。
#[test]
fn vec_contains_basic() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(10);
    v.push(20);
    v.push(30);
    println(v.contains(20));   // true（中间）
    println(v.contains(99));   // false（未命中）
    println(v.contains(10));   // true（头部）
    println(v.contains(30));   // true（尾部）
    let mut e: Vec<i64> = Vec::new();
    println(e.contains(1));    // false（空 Vec）
    v.push(20);
    println(v.contains(20));   // true（多出现）
}
"#,
    );
    assert_eq!(out, "true\nfalse\ntrue\ntrue\nfalse\ntrue\n");
}

/// contains String 元素：内容相等命中 / 大小写敏感 / 子串不等。
#[test]
fn vec_contains_string() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<String> = Vec::new();
    v.push(String::from("alpha"));
    v.push(String::from("beta"));
    println(v.contains(String::from("alpha")));  // true（内容相等）
    println(v.contains(String::from("gamma")));  // false（未命中）
    println(v.contains(String::from("ALPHA")));  // false（大小写敏感）
    println(v.contains(String::from("alp")));    // false（子串 != 元素）
    println(v.len());                            // 2（contains 不修改）
}
"#,
    );
    assert_eq!(out, "true\nfalse\nfalse\nfalse\n2\n");
}

/// remove 基本：删中间/头部/尾部 + 返回值 + len + 剩余顺序。
#[test]
fn vec_remove_basic() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::with_capacity(4);
    v.push(10);
    v.push(20);
    v.push(30);
    v.push(40);
    let x = v.remove(1);      // 删 20 -> [10,30,40]
    println(x);               // 20
    println(v.len());         // 3
    println(v.get(0));        // 10
    println(v.get(1));        // 30
    println(v.get(2));        // 40
    let h = v.remove(0);      // 删 10 -> [30,40]
    println(h);               // 10
    let t = v.remove(1);      // 删 40 -> [30]
    println(t);               // 40
    println(v.len());         // 1
    println(v.get(0));        // 30
}
"#,
    );
    assert_eq!(out, "20\n3\n10\n30\n40\n10\n40\n1\n30\n");
}

/// remove 序列：连续删除到空 + 复用 push。
#[test]
fn vec_remove_sequence() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    let a = v.remove(0);
    let b = v.remove(0);
    let c = v.remove(0);
    println(a + b + c);       // 6
    println(v.len());         // 0
    println(v.is_empty());    // 1
    v.push(7);
    v.push(8);
    println(v.len());         // 2
    println(v.get(0) + v.get(1)); // 15
}
"#,
    );
    assert_eq!(out, "6\n0\n1\n2\n15\n");
}

/// insert 基本：中间/头部/尾部（i == len）插入 + 连续插入扩容。
#[test]
fn vec_insert_basic() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(10);
    v.push(30);
    v.insert(1, 20);          // 中间 -> [10,20,30]
    println(v.len());         // 3
    println(v.get(0));        // 10
    println(v.get(1));        // 20
    println(v.get(2));        // 30
    v.insert(0, 0);           // 头部 -> [0,10,20,30]
    println(v.get(0));        // 0
    println(v.len());         // 4
    v.insert(4, 99);          // 尾部（i == len）-> [0,10,20,30,99]
    println(v.get(4));        // 99
    println(v.len());         // 5
    // 连续头部插入触发扩容（new cap 4 -> 8 -> 16）
    let mut k = 0;
    while k < 8 {
        v.insert(0, k);
        k = k + 1;
    }
    println(v.len());         // 13
    println(v.get(0));        // 7（最后一次插入在最前）
    println(v.get(12));       // 99（原尾部仍最后）
}
"#,
    );
    assert_eq!(out, "3\n10\n20\n30\n0\n4\n99\n5\n13\n7\n99\n");
}

/// 混合组合：push + insert + remove + contains + clear 全链路。
#[test]
fn vec_common_combo() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    v.push(4);
    v.insert(2, 99);          // [1,2,99,3,4]
    let x = v.remove(0);      // 删 1 -> [2,99,3,4]
    println(x);               // 1
    println(v.contains(99));  // true
    println(v.contains(1));   // false
    v.clear();                // []
    println(v.len());         // 0
    println(v.is_empty());    // 1
    v.push(5);
    v.push(6);
    println(v.get(0) + v.get(1)); // 11
    println(v.contains(6));   // true
}
"#,
    );
    assert_eq!(out, "1\ntrue\nfalse\n0\n1\n11\ntrue\n");
}
