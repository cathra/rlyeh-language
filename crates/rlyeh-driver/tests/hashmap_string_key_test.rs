//! `HashMap<String, V>` 字符串键集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/core.rl`）。
//!
//! 覆盖：字符串键 insert / get / contains_key / len、同内容不同对象键哈希一致性、
//! 同键覆盖、remove 墓碑语义、翻倍扩容 rehash（12 键压力）、未命中键、
//! `for (k, v) in m` 元组模式迭代。
//!
//! 前置特性：`String::from` 字面量构造 + `s1 == s2` 内容相等比较（bytes_eq 内建）
//! + `hash_value(String)` → djb2 内容哈希 desugar。
//!
//! 需要系统 clang（与 driver_test.rs / vec_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-hmstr-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("HashMap 字符串键测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 基础：`String::from` 键 insert / get / contains_key / len / is_empty。
#[test]
fn string_key_insert_get() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<String, i64> = HashMap::new();
    m.insert(String::from("alpha"), 1);
    m.insert(String::from("beta"), 2);
    println(m.len());
    println(m.get(String::from("alpha")).unwrap());
    println(m.get(String::from("beta")).unwrap());
    println(m.contains_key(String::from("alpha")));
    println(m.contains_key(String::from("gamma")));
    println(m.is_empty());
}
"#,
    );
    assert_eq!(out, "2\n1\n2\ntrue\nfalse\nfalse\n");
}

/// 同内容不同对象键：哈希一致（djb2）+ 内容相等（bytes_eq），命中同一槽位。
#[test]
fn string_key_content_hash_consistent() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<String, i64> = HashMap::new();
    let a = String::from("hello");
    m.insert(a, 42);
    // 与 a 内容相同但独立构造的新对象（不同 data 指针、相同内容）
    println(m.get(String::from("hello")).unwrap());
    // 前缀相同的不同字符串不得误命中（内容相等比较兜底）
    m.insert(String::from("hell"), 7);
    println(m.get(String::from("hell")).unwrap());
    println(m.get(String::from("hello")).unwrap());
    println(m.len());
}
"#,
    );
    assert_eq!(out, "42\n7\n42\n2\n");
}

/// 同键覆盖：len 不变，值更新。
#[test]
fn string_key_overwrite() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<String, i64> = HashMap::new();
    m.insert(String::from("k"), 1);
    m.insert(String::from("k"), 2);
    println(m.len());
    println(m.get(String::from("k")).unwrap());
}
"#,
    );
    assert_eq!(out, "1\n2\n");
}

/// remove 墓碑语义：删除后 len 减、未删键仍可查、删键不可查、可重新插入。
#[test]
fn string_key_remove_tombstone() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<String, i64> = HashMap::new();
    m.insert(String::from("a"), 10);
    m.insert(String::from("b"), 20);
    m.insert(String::from("c"), 30);
    let v = m.remove(String::from("b"));
    println(v.unwrap());
    println(m.len());
    println(m.contains_key(String::from("b")));
    println(m.get(String::from("a")).unwrap());
    println(m.get(String::from("c")).unwrap());
    m.insert(String::from("b"), 99);
    println(m.get(String::from("b")).unwrap());
    println(m.len());
}
"#,
    );
    assert_eq!(out, "20\n2\nfalse\n10\n30\n99\n3\n");
}

/// 翻倍扩容 rehash：12 个不同字符串键（cap 8 → 16），rehash 重新哈希全部可查且值正确。
#[test]
fn string_key_grow_rehash() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<String, i64> = HashMap::new();
    m.insert(String::from("k00"), 0);
    m.insert(String::from("k01"), 100);
    m.insert(String::from("k02"), 200);
    m.insert(String::from("k03"), 300);
    m.insert(String::from("k04"), 400);
    m.insert(String::from("k05"), 500);
    m.insert(String::from("k06"), 600);
    m.insert(String::from("k07"), 700);
    m.insert(String::from("k08"), 800);
    m.insert(String::from("k09"), 900);
    m.insert(String::from("k10"), 1000);
    m.insert(String::from("k11"), 1100);
    println(m.len());
    println(m.get(String::from("k00")).unwrap());
    println(m.get(String::from("k05")).unwrap());
    println(m.get(String::from("k11")).unwrap());
    println(m.get(String::from("k09")).unwrap());
    println(m.contains_key(String::from("k03")));
    println(m.contains_key(String::from("zzz")));
}
"#,
    );
    // 首/中/末位抽查：rehash 后探测链正确则全部命中
    assert_eq!(out, "12\n0\n500\n1100\n900\ntrue\nfalse\n");
}

/// 未命中键：`get` 返回 None（is_some 为 0）、`contains_key` 为 false。
#[test]
fn string_key_missing() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<String, i64> = HashMap::new();
    m.insert(String::from("present"), 5);
    println(m.get(String::from("absent")).is_some());
    println(m.contains_key(String::from("absent")));
    println(m.get(String::from("present")).unwrap());
}
"#,
    );
    assert_eq!(out, "0\nfalse\n5\n");
}

/// `for (k, v) in m` 元组模式迭代：字符串键计数 + 值求和。
#[test]
fn string_key_for_iterate() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<String, i64> = HashMap::new();
    m.insert(String::from("a"), 1);
    m.insert(String::from("b"), 2);
    m.insert(String::from("c"), 3);
    let mut n = 0;
    let mut s = 0;
    for (k, v) in m {
        n = n + 1;
        s = s + v;
        if k == String::from("a") {
            s = s + 100;
        }
    }
    println(n);
    println(s);
}
"#,
    );
    // 3 键各值 1+2+3=6，k=="a" 时追加 100 → 106
    assert_eq!(out, "3\n106\n");
}
