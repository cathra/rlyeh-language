//! `String` 字典序比较（`<` / `>` / `<=` / `>=`）集成测试。
//!
//! 覆盖：等长前缀不同字节定大小、前缀相同长度兜底（短者小）、完全相等、
//! 四个方向运算符交叉验证、复杂表达式操作数、if 分支 + 排序模拟。
//!
//! 实现：`bytes_cmp` 内建（`memcmp` 有符号扩展 i64）+ desugar 层
//! 前缀比较 + 长度兜底。
//!
//! 需要系统 clang（与 driver_test.rs / string_eq_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-string-cmp-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("String 排序测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 等长、首字节不同：memcmp 在首个不同字节即定大小（'a' < 'b'，'r' < 'z'）。
#[test]
fn prefix_diff_same_len() {
    let out = run(
        r#"
fn main() {
    println(String::from("apple") < String::from("banana"));
    println(String::from("banana") < String::from("apple"));
    println(String::from("zeta") > String::from("rust"));
    println(String::from("rust") > String::from("zeta"));
}
"#,
    );
    assert_eq!(out, "true\nfalse\ntrue\nfalse\n");
}

/// 前缀相同、长度不同：前缀字节全等时短者更小（长度兜底）。
#[test]
fn prefix_same_len_diff() {
    let out = run(
        r#"
fn main() {
    println(String::from("abc") < String::from("abcd"));
    println(String::from("abcd") < String::from("abc"));
    println(String::from("abcd") > String::from("abc"));
    println(String::from("abc") > String::from("abcd"));
}
"#,
    );
    assert_eq!(out, "true\nfalse\ntrue\nfalse\n");
}

/// 完全相等：`<` / `>` 为 false，`<=` / `>=` 为 true。
#[test]
fn equal_strings() {
    let out = run(
        r#"
fn main() {
    let a = String::from("zeta");
    let b = String::from("zeta");
    println(a < b);
    println(a > b);
    println(a <= b);
    println(a >= b);
}
"#,
    );
    assert_eq!(out, "false\nfalse\ntrue\ntrue\n");
}

/// 四个方向运算符交叉验证（不等场景）+ `==`/`!=` 回归（内容相等不受影响）。
#[test]
fn four_ops_cross_check() {
    let out = run(
        r#"
fn main() {
    let a = String::from("abc");
    let b = String::from("abd");
    println(a < b);
    println(a > b);
    println(a <= b);
    println(a >= b);
    println(a == b);
    println(a != b);
    println(a == String::from("abc"));
}
"#,
    );
    assert_eq!(out, "true\nfalse\ntrue\nfalse\nfalse\ntrue\ntrue\n");
}

/// 复杂表达式操作数：拼接结果参与比较（desugar 层绑定唯一临时变量防重复求值）。
#[test]
fn complex_expr_operands() {
    let out = run(
        r#"
fn main() {
    let a = String::from("a");
    let b = String::from("b");
    let c = String::from("abc");
    // "ab" < "b"（'a' < 'b'）；"ab" < "abc"（前缀相同，长度兜底）
    println((a + b) < b);
    println((a + b) < c);
    println((a + b) <= c);
    println((a + b) > b);
}
"#,
    );
    assert_eq!(out, "true\ntrue\ntrue\nfalse\n");
}

/// if 分支使用 + 排序模拟：三串中取最大（字典序）。
#[test]
fn if_branch_and_max() {
    let out = run(
        r#"
fn main() {
    let x = String::from("pear");
    let y = String::from("apple");
    let z = String::from("grape");
    let mut m = x;
    if y > m { m = y; }
    if z > m { m = z; }
    println(m);
    if x < y {
        println(String::from("x<y"));
    } else {
        println(String::from("x>=y"));
    }
}
"#,
    );
    assert_eq!(out, "pear\nx>=y\n");
}
