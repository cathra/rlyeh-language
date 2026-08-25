//! `HashMap<K, V>` 标准库集成测试（文件入口 API，自动注入 `zeta-std/zeta/core.zeta`）。
//!
//! 覆盖：`HashMap::new` / `with_capacity` 构造、insert / get / contains_key / len、
//! 同键覆盖、翻倍扩容 rehash（20 键压力）、remove 墓碑语义、负数键、
//! 多泛型（`HashMap<i64, i64>` 与 `HashMap<i64, f64>`）。
//!
//! 需要系统 clang（与 driver_test.rs / vec_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-hashmap-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("HashMap 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 基础：`new()` 构造 + insert / get / contains_key / len / is_empty。
#[test]
fn hashmap_insert_get() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    println(m.is_empty());
    m.insert(1, 100);
    m.insert(2, 200);
    println(m.len());
    println(m.get(1).unwrap());
    println(m.get(2).unwrap());
    println(m.contains_key(3));
    println(m.get(99).is_some());
    println(m.is_empty());
}
"#,
    );
    assert_eq!(out, "true\n2\n100\n200\nfalse\n0\nfalse\n");
}

/// 同键覆盖：len 不变，值更新。
#[test]
fn hashmap_overwrite() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(7, 1);
    m.insert(7, 2);
    println(m.len());
    println(m.get(7).unwrap());
}
"#,
    );
    assert_eq!(out, "1\n2\n");
}

/// 翻倍扩容 rehash：插入 20 个键（cap 8 → 16 → 32），全部可查且值正确。
#[test]
fn hashmap_grow_rehash() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    let mut i = 0;
    while i < 20 {
        m.insert(i * 10, i * 100);
        i = i + 1;
    }
    println(m.len());
    println(m.get(0).unwrap());
    println(m.get(190).unwrap());
    let mut s = 0;
    let mut j = 0;
    while j < 20 {
        s = s + m.get(j * 10).unwrap();
        j = j + 1;
    }
    println(s);
}
"#,
    );
    // 0+100+...+1900 = 100 * (0+1+...+19) = 100 * 190 = 19000
    assert_eq!(out, "20\n0\n1900\n19000\n");
}

/// remove 墓碑语义：删除后 len 减、未删键仍可查、删键不可查、可重新插入。
#[test]
fn hashmap_remove_tombstone() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(1, 10);
    m.insert(2, 20);
    m.insert(3, 30);
    let v = m.remove(2);
    println(v.unwrap());
    println(m.len());
    println(m.contains_key(2));
    println(m.get(1).unwrap());
    println(m.get(3).unwrap());
    m.insert(2, 99);
    println(m.get(2).unwrap());
    println(m.len());
}
"#,
    );
    assert_eq!(out, "20\n2\nfalse\n10\n30\n99\n3\n");
}

/// 负数键：哈希取绝对值逻辑，正负键共存。
#[test]
fn hashmap_negative_keys() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(-5, 1);
    m.insert(5, 2);
    m.insert(-100, 3);
    println(m.len());
    println(m.get(-5).unwrap());
    println(m.get(5).unwrap());
    println(m.get(-100).unwrap());
}
"#,
    );
    assert_eq!(out, "3\n1\n2\n3\n");
}

/// `with_capacity(n)` 预留容量 + f64 值类型（多泛型实例化）。
#[test]
fn hashmap_with_capacity_f64() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, f64> = HashMap::with_capacity(32);
    println(m.cap());
    m.insert(1, 1.5);
    m.insert(2, 2.5);
    println(m.get(1).unwrap());
    println(m.get(2).unwrap());
    println(m.len());
}
"#,
    );
    assert_eq!(out, "32\n1.500000\n2.500000\n2\n");
}
