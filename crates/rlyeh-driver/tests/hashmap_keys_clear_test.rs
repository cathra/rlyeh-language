//! `HashMap<K, V>` 键集/值集/清空集成测试（文件入口 API，自动注入 `zeta-std/zeta/core.zeta`）。
//!
//! 覆盖：`keys()` / `values()`（稀疏遍历存活槽位返回 `Vec<K>` / `Vec<V>`）、
//! `clear()`（完全重置容量不变 + 复用）、键集排序断言（开放寻址遍历顺序与
//! 插入无关，断言一律先 `sort`）、组合链路。
//!
//! 前置特性：`Vec<T>::sort`（i64 数值 / String 字典序）+ `String` 内容相等。
//!
//! 注意：`HashMap::new()` 初始 cap=8（第 5 键触发扩容至 16）。
//!
//! 需要系统 clang（与 hashmap_test.rs / vec_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-hmkeys-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("HashMap 键集/清空测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// clear 基本：插入后清空 → len/is_empty/contains_key 全空 → 重新插入可查。
#[test]
fn hashmap_clear_basic() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(1, 100);
    m.insert(2, 200);
    m.insert(3, 300);
    println(m.len());               // 3
    m.clear();
    println(m.len());               // 0
    println(m.is_empty());          // true
    println(m.contains_key(1));     // false（键已清空）
    println(m.contains_key(3));     // false
    m.insert(9, 900);
    println(m.len());               // 1（清空后可复用）
    println(m.get(9).unwrap());     // 900
}
"#,
    );
    assert_eq!(out, "3\n0\ntrue\nfalse\nfalse\n1\n900\n");
}

/// clear 保持容量 + 复用：cap 8 清空后仍 8，重新插入不触发扩容（< 5 键）。
#[test]
fn hashmap_clear_keeps_capacity() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(1, 1);
    m.insert(2, 2);
    m.insert(3, 3);
    m.insert(4, 4);
    println(m.cap());               // 8（未扩容）
    m.clear();
    println(m.cap());               // 8（容量不变）
    m.insert(11, 11);
    m.insert(22, 22);
    m.insert(33, 33);
    m.insert(44, 44);
    println(m.len());               // 4
    println(m.cap());               // 8（复用旧容量，无墓碑残留）
    println(m.get(33).unwrap());    // 33
    m.clear();
    println(m.is_empty());          // true
}
"#,
    );
    assert_eq!(out, "8\n8\n4\n8\n33\ntrue\n");
}

/// keys 基本（i64 键）：扩容后键集长度正确 + sort 后逐位断言。
#[test]
fn hashmap_keys_basic() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(50, 1);
    m.insert(10, 1);
    m.insert(40, 1);
    m.insert(20, 1);
    m.insert(30, 1);               // 第 5 键触发扩容
    let ks = m.keys();
    println(ks.len());             // 5
    ks.sort();
    println(ks.get(0));            // 10
    println(ks.get(1));            // 20
    println(ks.get(2));            // 30
    println(ks.get(3));            // 40
    println(ks.get(4));            // 50
}
"#,
    );
    assert_eq!(out, "5\n10\n20\n30\n40\n50\n");
}

/// keys String 键：字典序排序后断言 + 与 values 等长（同槽遍历）。
#[test]
fn hashmap_keys_string() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<String, i64> = HashMap::new();
    m.insert(String::from("pear"), 1);
    m.insert(String::from("apple"), 2);
    m.insert(String::from("mango"), 3);
    let ks = m.keys();
    println(ks.len());             // 3
    ks.sort();
    println(ks.get(0));            // apple
    println(ks.get(1));            // mango
    println(ks.get(2));            // pear
    let vs = m.values();
    println(vs.len());             // 3（与 keys 等长）
    vs.sort();
    println(vs.get(0));            // 1
    println(vs.get(1));            // 2
    println(vs.get(2));            // 3
}
"#,
    );
    assert_eq!(out, "3\napple\nmango\npear\n3\n1\n2\n3\n");
}

/// values 基本（i64 值）：含重复值 + sort 断言 + len 与键数一致。
#[test]
fn hashmap_values_basic() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(1, 42);
    m.insert(2, 7);
    m.insert(3, 42);               // 重复值
    m.insert(4, 100);
    let vs = m.values();
    println(vs.len());             // 4
    vs.sort();
    println(vs.get(0));            // 7
    println(vs.get(1));            // 42
    println(vs.get(2));            // 42
    println(vs.get(3));            // 100
}
"#,
    );
    assert_eq!(out, "4\n7\n42\n42\n100\n");
}

/// 组合：insert + remove 后 keys/values 只含存活键值 + clear 终态。
#[test]
fn hashmap_keys_values_combine() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(1, 10);
    m.insert(2, 20);
    m.insert(3, 30);
    m.insert(4, 40);
    m.insert(5, 50);
    m.remove(3);                   // 墓碑（不破坏探测链）
    println(m.len());              // 4
    let ks = m.keys();
    ks.sort();
    println(ks.len());             // 4（不含被删键 3）
    println(ks.get(0));            // 1
    println(ks.get(1));            // 2
    println(ks.get(2));            // 4
    println(ks.get(3));            // 5
    let vs = m.values();
    vs.sort();
    println(vs.len());             // 4
    println(vs.get(3));            // 50
    m.clear();
    println(m.is_empty());         // true
    println(m.keys().len());       // 0（清空后键集为空）
}
"#,
    );
    assert_eq!(out, "4\n4\n1\n2\n4\n5\n4\n50\ntrue\n0\n");
}
