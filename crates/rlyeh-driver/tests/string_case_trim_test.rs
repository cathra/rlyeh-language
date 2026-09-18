//! `String` 大小写/裁剪集成测试（`to_upper` / `to_lower` / `trim`）。
//!
//! 覆盖：ASCII 大小写转换（纯字母/混合/非字母原样/空串）、首尾空白剥离
//! （多空白组合/仅首/仅尾/无空白原样/全空白空串）、大小写 + 裁剪 + 子串
//! 链式组合、`find`/`contains` 与大小写方法配合、拼接后处理。
//!
//! 实现：标准库纯 Rlyeh 方法（`to_upper`/`to_lower` 逐字节 push_byte 拷贝
//! + 比较链区间判断 ±32，`trim` 正反向双扫描后 `substring` 返回新缓冲）。
//!
//! 需要系统 clang（与 driver_test.rs / string_slice_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-string-case-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("String 大小写/裁剪测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 基本转换：`to_upper` 全小写转大写；`to_lower` 全大写转小写。
#[test]
fn to_upper_lower_basic() {
    let out = run(
        r#"
fn main() {
    let a = String::from("hello");
    let b = String::from("WORLD");
    println(a.to_upper());       // HELLO
    println(b.to_lower());       // world
    println(a.to_upper().to_lower());  // hello（先转大再转小 = 原样）
}
"#,
    );
    assert_eq!(out, "HELLO\nworld\nhello\n");
}

/// 混合大小写 + 非字母原样：数字/标点/空格不受影响。
#[test]
fn mixed_and_non_alpha() {
    let out = run(
        r#"
fn main() {
    let s = String::from("Hello, Rlyeh 123!");
    println(s.to_upper());       // HELLO, RLYEH 123!
    println(s.to_lower());       // hello, rlyeh 123!
    println(s.to_upper() == s.to_upper());
}
"#,
    );
    assert_eq!(out, "HELLO, RLYEH 123!\nhello, rlyeh 123!\ntrue\n");
}

/// 空串与单字符边界。
#[test]
fn empty_and_single() {
    let out = run(
        r#"
fn main() {
    let e = String::from("");
    println(e.to_upper().len());      // 0
    println(e.to_lower().len());      // 0
    println(e.trim().len());          // 0
    let c = String::from("a");
    println(c.to_upper());            // A
    let d = String::from("Z");
    println(d.to_lower());            // z
}
"#,
    );
    assert_eq!(out, "0\n0\n0\nA\nz\n");
}

/// 基本裁剪：首尾空格剥离、仅首/仅尾、内部空格保留。
#[test]
fn trim_basic() {
    let out = run(
        r#"
fn main() {
    let s = String::from("  hello  ");
    println(s.trim());            // hello
    let t = String::from("\t padded \n");
    println(t.trim());            // padded（制表/换行也剥离）
    let u = String::from("left");
    println(u.trim());            // 无空白原样
    println(u.trim() == u);       // true
    let v = String::from("a b c");
    println(v.trim());            // 内部空格保留
}
"#,
    );
    assert_eq!(out, "hello\npadded\nleft\ntrue\na b c\n");
}

/// 全空白串 → 空串；多空白组合（空格+制表混合）。
#[test]
fn trim_all_whitespace() {
    let out = run(
        r#"
fn main() {
    let w = String::from(" \t\r\n  ");
    println(w.trim().len());          // 0
    let x = String::from("  \t  abc  \r\n ");
    println(x.trim());                // abc
    println(x.trim().trim());         // 再裁剪仍为 abc
}
"#,
    );
    assert_eq!(out, "0\nabc\nabc\n");
}

/// 链式组合：trim + to_upper + substring + find/contains 配合。
#[test]
fn chain_combinations() {
    let out = run(
        r#"
fn main() {
    let s = String::from("  Rlyeh Lang  ");
    let t = s.trim().to_upper();          // "RLYEH LANG"
    println(t);
    println(t.substring(0, 4));           // RLYE（半开 [0,4) = 前 4 字节）
    println(t.find(String::from("LANG"))); // 6：匹配起始索引
    println(t.contains(String::from("ETA"))); // false：无 ETA 子串
    let q = String::from("  Hi, Rlyeh!  ");
    println(q.trim().to_lower().substring(0, 5)); // "hi, r"（半开，前 5 字节）
    println(q.trim().to_lower() == String::from("hi, rlyeh!"));
}
"#,
    );
    assert_eq!(out, "RLYEH LANG\nRLYE\n6\nfalse\nhi, r\ntrue\n");
}
