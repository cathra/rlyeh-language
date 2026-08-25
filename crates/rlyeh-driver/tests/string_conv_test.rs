//! `String` 数值转换集成测试（`int_to_string` / `string_to_int`）。
//!
//! 覆盖：整数转字符串（零/正整数/多位数/负数/负号前缀）、转换结果与 String
//! 操作组合（len/starts_with/ends_with/拼接/substring）、字符串转整数（纯数字/
//! 前导零/负数/空串/遇非数字停止）、转换结果参与算术、数字 ↔ 字符串往返。
//!
//! 实现：core.zeta 顶层自由函数（`int_to_string` 逐位取模存 Vec 后反向输出 +
//! 负数 '-' 前缀 + 0 特判；`string_to_int` 累加解析 + 遇非数字停止 + 负号支持）。
//!
//! 需要系统 clang（与 string_common_ops_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-string-conv-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("String 数值转换测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 整数转字符串基本：零/正整数/多位数/单数字/拼接结果。
#[test]
fn int_to_string_basic() {
    let out = run(
        r#"
fn main() {
    println(int_to_string(0));          // 0（零特判）
    println(int_to_string(42));         // 42
    println(int_to_string(1234567));    // 1234567
    println(int_to_string(9));          // 9
    println(int_to_string(1000));       // 1000
    println(int_to_string(0) + int_to_string(7)); // 07（结果可拼接）
}
"#,
    );
    assert_eq!(out, "0\n42\n1234567\n9\n1000\n07\n");
}

/// 整数转字符串负数：负号前缀 + 数值部分正确。
#[test]
fn int_to_string_negative() {
    let out = run(
        r#"
fn main() {
    println(int_to_string(-42));        // -42
    println(int_to_string(-1));         // -1
    println(int_to_string(-7));         // -7
    println(int_to_string(-100));       // -100
    println(int_to_string(-1000000));   // -1000000
    println(int_to_string(-42) == String::from("-42")); // true
}
"#,
    );
    assert_eq!(out, "-42\n-1\n-7\n-100\n-1000000\ntrue\n");
}

/// 转换结果与 String 操作组合：len/starts_with/ends_with/拼接/substring。
#[test]
fn int_to_string_combo() {
    let out = run(
        r#"
fn main() {
    let n = 2024;
    let year = int_to_string(n);
    println(year);                                  // 2024
    println(year.len);                              // 4
    println(year.starts_with(String::from("20")));  // true
    println(year.ends_with(String::from("24")));    // true
    let msg = String::from("year: ") + year;        // 拼接
    println(msg);                                   // year: 2024
    println(int_to_string(-2024).substring(1, 3));  // 20（跳过负号）
}
"#,
    );
    assert_eq!(out, "2024\n4\ntrue\ntrue\nyear: 2024\n20\n");
}

/// 字符串转整数基本：纯数字/前导零/单数字/可参与算术。
#[test]
fn string_to_int_basic() {
    let out = run(
        r#"
fn main() {
    println(string_to_int(String::from("0")));       // 0
    println(string_to_int(String::from("42")));      // 42
    println(string_to_int(String::from("1234567"))); // 1234567
    println(string_to_int(String::from("007")));     // 7（前导零）
    println(string_to_int(String::from("9")));       // 9
    println(string_to_int(String::from("999")) + 1); // 1000（可参与算术）
}
"#,
    );
    assert_eq!(out, "0\n42\n1234567\n7\n9\n1000\n");
}

/// 字符串转整数边界：负数/空串/遇非数字停止/开头非数字。
#[test]
fn string_to_int_edge() {
    let out = run(
        r#"
fn main() {
    println(string_to_int(String::from("-42")));     // -42
    println(string_to_int(String::from("-1")));      // -1
    println(string_to_int(String::from("")));        // 0（空串）
    println(string_to_int(String::from("12abc34"))); // 12（遇非数字停止）
    println(string_to_int(String::from("abc")));     // 0（开头非数字）
    println(string_to_int(String::from("  42")));    // 0（前导空白停止）
}
"#,
    );
    assert_eq!(out, "-42\n-1\n0\n12\n0\n0\n");
}

/// 往返 + 算术组合：数字→字符串→数字一致、负数往返、计算后转字符串。
#[test]
fn roundtrip_and_arith() {
    let out = run(
        r#"
fn main() {
    // 数字 → 字符串 → 数字往返
    let a = 12345;
    let s = int_to_string(a);
    let b = string_to_int(s);
    println(b);                                     // 12345
    println(a == b);                                // true
    // 负数往返
    let c = string_to_int(int_to_string(-6789));
    println(c);                                     // -6789
    // 计算后转字符串
    println(int_to_string(7 * 8));                  // 56
    println(int_to_string(1000 / 4));               // 250
    println(int_to_string(17 % 5));                 // 2
    // 解析结果参与比较
    let score = string_to_int(String::from("98")) > 90;
    println(score);                                 // true
}
"#,
    );
    assert_eq!(out, "12345\ntrue\n-6789\n56\n250\n2\ntrue\n");
}
