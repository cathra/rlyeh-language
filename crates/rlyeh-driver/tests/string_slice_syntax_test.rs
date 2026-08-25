//! `String` 范围切片语法集成测试（`s[lo..<hi]` / `s[lo...hi]` / `s[lo<..hi]`）。
//!
//! 覆盖：三种 range 运算符的边界语义、越界 clamp、切片与 `==`/`len` 组合、
//! 切片链式（切片结果再切片）、拼接结果切片、变量/表达式下界上界。
//!
//! 实现：typecheck 层 `check_slice` 将切片 desugar 为 `String::substring`
//! 方法调用（`..<` 半开直通、`...` 闭区间 end+1、`<..` 左开 start+1），
//! 复用量化方法实例化路径注册函数体。
//!
//! 需要系统 clang（与 string_slice_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "zeta-string-slice-syntax-{}-{seq}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("String 范围切片语法测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 三种 range 运算符的边界语义（"hello world"）：
/// `..<` 左闭右开、`...` 双闭、`<..` 左开右闭。
#[test]
fn range_operator_semantics() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello world");
    println(s[0..<5]);        // [0, 5)     → hello
    println(s[6...10]);       // [6, 10]    → world
    println(s[1<..5]);        // (1, 5]     → llo space
    println(s[0...0]);        // [0, 0]     → h
    println(s[0<..10]);       // (0, 10]    → ello world
    println(s[0..<0].len());  // 空区间
}
"#,
    );
    assert_eq!(out, "hello\nworld\nllo \nh\nello world\n0\n");
}

/// 越界 clamp：负下界/超长上界收敛到边界，反向/空区间返回空串。
#[test]
fn slice_clamp() {
    let out = run(
        r#"
fn main() {
    let s = String::from("abcdef");
    println(s[-3..<2]);        // ab
    println(s[4..<99]);        // ef
    println(s[3...1].len());   // 反向区间 → 空
    println(s[6..<10].len());  // 越界后收敛为空区间
    println(s[0...2]);         // abc
    println(s[2<..5]);         // (2, 5] → def
}
"#,
    );
    assert_eq!(out, "ab\nef\n0\n0\nabc\ndef\n");
}

/// 切片结果与 `==` / `len` / 原串比较的组合。
#[test]
fn slice_compare_and_len() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello world");
    println(s[0..<5] == String::from("hello"));
    println(s[6...10] == String::from("world"));
    println(s[1<..5].len());            // ello → 4
    println(s[0..<11] == s);            // 全区间 == 原串
    println(s[0..<0].len());            // 空串
    println(s[3...7] == String::from("lo wo"));
}
"#,
    );
    assert_eq!(out, "true\ntrue\n4\ntrue\n0\ntrue\n");
}

/// 切片链式：切片结果（String）可再次切片。
#[test]
fn slice_chain() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello world");
    println(s[0..<5][1..<3]);      // hello → el
    println(s[6...10][0..<3]);     // world → wor
    println(s[0..<11][4...8]);     // o wor
    println(s[1<..5][0..<2]);      // "llo " → ll
    println(s[0..<5][0..<5] == s[0..<5]);
}
"#,
    );
    assert_eq!(out, "el\nwor\no wor\nll\ntrue\n");
}

/// 拼接结果切片 + 切片结果拼接 + 链式组合。
#[test]
fn concat_slice_combo() {
    let out = run(
        r#"
fn main() {
    let a = String::from("foo");
    let b = String::from("bar");
    let c = a + b;                       // foobar
    println(c[2..<5]);                   // oba
    let d = c[0..<3] + c[3...5];         // foo + bar
    println(d == c);                     // true
    println(c[1..<5][0..<2]);            // ooba → oo
    println(c[0..<6][1<..5]);            // foobar → obar
}
"#,
    );
    assert_eq!(out, "oba\ntrue\noo\nobar\n");
}

/// 变量/表达式下界上界：`find` 下标喂切片、算术表达式边界。
#[test]
fn slice_variable_bounds() {
    let out = run(
        r#"
fn main() {
    let s = String::from("the quick brown fox");
    let p = s.find(String::from("quick"));   // 4
    println(s[p..<p + 5]);                   // quick
    let lo = 10;
    let hi = 15;
    println(s[lo...hi - 1]);                 // brown（10..=14）
    println(s[p..<p + 4] + s[p + 4..<p + 5]); // quic + k
    println(s[p...p + 4] == String::from("quick"));
    println(s[p + 5..<p + 10]);              // " brow"
}
"#,
    );
    assert_eq!(out, "quick\nbrown\nquick\ntrue\n brow\n");
}
