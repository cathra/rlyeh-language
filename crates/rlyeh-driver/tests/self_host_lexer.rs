//! M-M1（SH-P2-7）自举对拍 harness：Rlyeh 版 lexer 与 Rust oracle 的 token 流逐行一致。
//!
//! 策略：同一份源码分别交给
//!   1. Rust oracle：[`rlyeh_driver::emit_tokens_str`]（driver 新增的 `--emit tokens`），
//!   2. Rlyeh 版 lexer：[`self-host/lexer.rl`] 经现有 Rust rlyeh-driver 编译运行，
//!      源码由 harness 以 `String::from("<corpus>")` 注入生成的 `main`。
//! 两者输出归一化（去除末尾换行）后逐行比对，必须完全一致。

use std::fs;
use std::path::{Path, PathBuf};

/// 读取 Rlyeh 版 lexer 源码（仅 `fn lex` 等函数，无 `main`）。
fn lexer_src() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../self-host/lexer.rl");
    fs::read_to_string(&p).expect("read self-host/lexer.rl")
}

/// 将 corpus 转义为 Rlyeh 字符串字面量（与 lexer 读取的 `\n`/`\"` 等转义对齐）。
fn escape_rl(src: &str) -> String {
    let mut out = String::with_capacity(src.len() * 2);
    for c in src.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// 用 Rlyeh 版 lexer 对 corpus 做词法分析，返回规范化 token 文本。
fn run_rlyeh_lexer(corpus: &str) -> String {
    let src = format!(
        "{}fn main() {{ let src = String::from(\"{}\"); let toks = lex(src); for t in toks {{ println(t); }} }}\n",
        lexer_src(),
        escape_rl(corpus),
    );
    let tmp: PathBuf = std::env::temp_dir().join("rlyeh_selfhost_lexer.rl");
    fs::write(&tmp, src).expect("write temp lexer");
    // 与 driver 顶层 `run_source` 一致：在 64MB 栈线程中编译+运行，
    // 避免大函数 typecheck 在小测试线程栈上溢出。
    let (out, _outcome) = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut driver =
                rlyeh_driver::IncrementalDriver::new(std::env::temp_dir()).with_force(true);
            driver.run_source_file(&tmp)
        })
        .unwrap()
        .join()
        .unwrap()
        .expect("rlyeh lexer run");
    out
}

/// 去除末尾换行，使 oracle 与 Rlyeh 输出可比（后者每行 `println` 自带换行）。
fn norm(s: &str) -> String {
    s.trim_end().to_string()
}

fn check(corpus: &str) {
    let oracle = rlyeh_driver::emit_tokens_str(corpus).expect("oracle emit tokens");
    let got = run_rlyeh_lexer(corpus);
    assert_eq!(norm(&got), norm(&oracle), "self-host lexer token mismatch");
}

#[test]
fn m_m1_lexer_matches_rust_oracle() {
    // corpus1：标识符/关键字/整数（十进制·hex·bin·`_`）/字符串/基础运算符/注释
    check(
        "fn add(a: i64, b: i64) -> i64 {\n    let sum = a + b;\n    return sum;\n}\n\
         let x = 42;\nlet y = 0xFF;\nlet z = 0b1010;\nlet w = 1_000;\nlet name = \"hello\";\n",
    );
    // corpus2：多字符运算符（== != <= >= && || -> => += << >> .. ..< ...）
    check(
        "fn update(a: i64, b: i64) -> i64 {\n\
         let and = a && b;\n    let or = a || b;\n    let eq = a == b;\n\
         let ne = a != b;\n    let le = a <= b;\n    let ge = a >= b;\n\
         let shl = a << 2;\n    let shr = a >> 3;\n    let add = a += 1;\n\
         if a == 0 { return -1; }\n    let r = a..b;\n    let s = a..<b;\n    let t = a...b;\n\
         match a { 0 => 1, _ => 2 }\n}\n",
    );
    // corpus3：M-M1b 切片（char 字面量 / 生命周期 / not in / 时间字面量 / 原始字符串 / 原始标识符）
    check(
        "let a = 'x';\nlet b = '0';\nlet c = 'A';\nlet r: &'r i64 = get();\n\
         fn handler<'a>(x: &'a i64) -> i64 { return 0; }\n\
         if not in (1, 2, 3) { let y = 1; }\n\
         let t1 = 9am;\nlet t2 = 6pm;\nlet t3 = 22:00;\nlet t4 = 9:30am;\n\
         let raw1 = r\"plain text\";\nlet raw2 = r#\"hash raw\"#;\nlet id = r#type;\n",
    );
    // corpus4：转义解码（\n \t \r \\ \" \' \xHH；仅 ASCII 范围，避开 UTF-8 多字节与浮点）
    check(
        "let s1 = \"line1\\nline2\";\nlet s2 = \"tab\\there\";\nlet s3 = \"quote\\\"inside\";\n\
         let s4 = \"backslash\\\\end\";\nlet s5 = \"hex\\x41byte\";\n\
         let c1 = '\\n';\nlet c2 = '\\t';\nlet c3 = '\\\\';\nlet c4 = '\\'';\n",
    );
    // corpus5：M-M1c① 浮点字面量（原始拼写对拍：FLOAT 1.0 / 1e10 / 2.5e-10 / 0.5）
    check(
        "let pi = 3.14;\nlet big = 1e10;\nlet small = 2.5e-10;\nlet half = 0.5f64;\nlet rate = 1.5e3;\n",
    );
}
