//! `HashMap<K, V>` 的 `len` / `is_empty` 集成测试（文件入口 API，
//! 自动注入 `rlyeh-std/rlyeh/`）。
//!
//! 覆盖：空表 is_empty、insert 后 len 增长、同键覆盖不重复计数、remove
//! 墓碑后 len 递减、clear 后归零、扩容 rehash 后 len 保持、String 键实例化。
//!
//! 需要系统 clang（与 std_test.rs / agg_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-hmlen-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("HashMap len/is_empty 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 空表与插入后的 len / is_empty。
#[test]
fn hashmap_len_is_empty_basic() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    println(m.is_empty());      // true（空）
    println(m.len());           // 0
    m.insert(1, 10);
    m.insert(2, 20);
    m.insert(3, 30);
    println(m.is_empty());      // false
    println(m.len());           // 3
}
"#,
    );
    assert_eq!(out, "true\n0\nfalse\n3\n");
}

/// 同键覆盖不重复计数；remove 墓碑后递减。
#[test]
fn hashmap_len_overwrite_remove() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(1, 10);
    m.insert(1, 99);            // 覆盖：len 不变
    println(m.len());           // 1
    m.insert(2, 20);
    m.insert(3, 30);
    println(m.len());           // 3
    m.remove(1);                // 墓碑：len 递减
    println(m.len());           // 2
    m.remove(1);                // 重复删除：无效果
    println(m.len());           // 2
    m.remove(2);
    m.remove(3);
    println(m.is_empty());      // true（全部删除后）
    println(m.len());           // 0
}
"#,
    );
    assert_eq!(out, "1\n3\n2\n2\ntrue\n0\n");
}

/// 扩容 rehash 后 len 保持；clear 后归零。
#[test]
fn hashmap_len_grow_clear() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    let mut i = 0;
    while i < 12 {              // 触发扩容（初始 cap 8，第 5 键起翻倍）
        m.insert(i, i * 10);
        i = i + 1;
    }
    println(m.len());           // 12（rehash 后保持）
    m.clear();
    println(m.is_empty());      // true
    println(m.len());           // 0
    m.insert(1, 1);
    println(m.len());           // 1（清空后可复用）
}
"#,
    );
    assert_eq!(out, "12\ntrue\n0\n1\n");
}

/// String 键实例化。
#[test]
fn hashmap_len_string_key() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<String, i64> = HashMap::new();
    println(m.is_empty());      // true
    m.insert(String::from("a"), 1);
    m.insert(String::from("b"), 2);
    m.insert(String::from("a"), 100);  // 同内容键覆盖
    println(m.len());           // 2
    m.remove(String::from("b"));
    println(m.len());           // 1
}
"#,
    );
    assert_eq!(out, "true\n2\n1\n");
}
