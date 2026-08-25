//! `String` 内容相等比较（`==` / `!=`）集成测试。
//!
//! 覆盖：同内容不同对象相等、异内容同长度不等、长度不同不等（短路）、
//! 前缀相同长度不同不等、`!=` 取反、if 分支选择、复杂表达式直接比较。
//!
//! 需要系统 clang（与 driver_test.rs / string_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-string-eq-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("String 相等测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 相同内容（不同对象）相等；不同内容同长度不等。
#[test]
fn string_eq_same_content() {
    let out = run(
        r#"
fn main() {
    let a = String::from("rlyeh");
    let b = String::from("rlyeh");
    let c = String::from("rust");
    println(a == b);
    println(a == c);
    println(a != c);
}
"#,
    );
    assert_eq!(out, "true\nfalse\ntrue\n");
}

/// 长度不同 → 不等（len 短路，不比较内容）。
#[test]
fn string_eq_diff_len() {
    let out = run(
        r#"
fn main() {
    let a = String::from("hello");
    let b = String::from("hello!");
    let c = String::from("hell");
    println(a == b);
    println(a == c);
    println(a != b);
}
"#,
    );
    assert_eq!(out, "false\nfalse\ntrue\n");
}

/// 前缀相同但长度不同（"abc" vs "abcd"）→ 不等。
#[test]
fn string_eq_prefix_not_equal() {
    let out = run(
        r#"
fn main() {
    let a = String::from("abc");
    let b = String::from("abcd");
    println(a == b);
    println(b != a);
}
"#,
    );
    assert_eq!(out, "false\ntrue\n");
}

/// 相同长度、仅中间一个字节不同 → 不等（memcmp 检出）。
#[test]
fn string_eq_middle_diff() {
    let out = run(
        r#"
fn main() {
    let a = String::from("rlyeh");
    let b = String::from("zexa");
    println(a == b);
    println(a != b);
}
"#,
    );
    assert_eq!(out, "false\ntrue\n");
}

/// 比较结果驱动 if 分支（字典风格判断）。
#[test]
fn string_eq_if_branch() {
    let out = run(
        r#"
fn main() {
    let user = String::from("admin");
    let ok = String::from("admin");
    if user == ok {
        println("welcome");
    } else {
        println("denied");
    }
    if user != ok {
        println("bad");
    } else {
        println("good");
    }
}
"#,
    );
    assert_eq!(out, "welcome\ngood\n");
}

/// 复杂表达式直接比较（构造器调用结果不落中间变量，验证临时绑定）。
#[test]
fn string_eq_expr_operands() {
    let out = run(
        r#"
fn main() {
    let a = String::from("alpha");
    let b = String::from("beta");
    println(String::from("alpha") == a);
    println(String::from("beta") != b);
    println(a == String::from("beta"));
}
"#,
    );
    assert_eq!(out, "true\nfalse\nfalse\n");
}
