//! M-M2 差分对拍：Rlyeh 版 parser（`self-host/parser.rl`）与 Rust oracle
//! (`rlyeh_driver::emit_ast_canonical_expr`) 对表达式源码生成的规范 S-表达式 AST
//! 文本逐字节一致。
//!
//! 切片1（M-M2a）范围：整数/标识符/一元负号/括号/二元算术与比较/逻辑/位运算。

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// 串行化锁：与 `self_host_lexer.rs` 共用 driver 内部 `temp_dir()/rlyeh-run` 可执行文件，
/// 并行会互相覆盖；加锁串行执行，并每次使用唯一临时源文件。
static PARSER_RUN_LOCK: Mutex<()> = Mutex::new(());
static PARSER_RUN_SEQ: AtomicU64 = AtomicU64::new(0);

/// 读取 Rlyeh 版 parser 源码（`self-host/parser.rl`）。
fn parser_src() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = format!("{manifest}/../../self-host/parser.rl");
    fs::read_to_string(&path)
        .unwrap_or_else(|_| fs::read_to_string("self-host/parser.rl").expect("read parser.rl"))
}

/// 把任意字符串转义为 Rlyeh 字符串字面量内容（与 lexer harness 同款）。
fn escape_rl(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// 用 Rlyeh 版 parser 解析 corpus，返回规范 AST 文本（合法输入）。
fn run_rlyeh_parser(corpus: &str) -> String {
    run_rlyeh_parser_result(corpus).expect("rlyeh parser run")
}

/// 同 [`run_rlyeh_parser`]，但返回 `Result`（供负向/边界用例断言）。
fn run_rlyeh_parser_result(corpus: &str) -> Result<String, rlyeh_driver::error::DriverError> {
    let _guard = PARSER_RUN_LOCK.lock().unwrap();
    let src = format!(
        "{}fn main() {{ let s = String::from(\"{}\"); let ast = parse(s); println(ast); }}\n",
        parser_src(),
        escape_rl(corpus),
    );
    let seq = PARSER_RUN_SEQ.fetch_add(1, Ordering::SeqCst);
    let tmp: PathBuf = std::env::temp_dir().join(format!("rlyeh_selfhost_parser_{seq}.rl"));
    fs::write(&tmp, src).expect("write temp parser");
    let res = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut driver =
                rlyeh_driver::IncrementalDriver::new(std::env::temp_dir()).with_force(true);
            driver.run_source_file(&tmp)
        })
        .unwrap()
        .join()
        .unwrap()?;
    Ok(res.0)
}

/// 规范化：去除行尾空白与首尾空行，便于断言（此处 AST 为单行，主要用于稳健性）。
fn norm(s: &str) -> String {
    s.trim().to_string()
}

/// 对拍：Rlyeh 版 parser 与 Rust oracle 对 corpus 的表达式 AST 应一致。
fn check(corpus: &str) {
    let expected = norm(&rlyeh_driver::emit_ast_canonical_expr(corpus).expect("oracle parse"));
    let got = norm(&run_rlyeh_parser(corpus));
    assert_eq!(got, expected, "\ncorpus: {corpus}\n");
}

/// 用 Rlyeh 版 parser 的 `parse_program` 解析整段程序 corpus，返回规范 AST 文本（合法输入）。
fn run_rlyeh_parser_program(corpus: &str) -> String {
    run_rlyeh_parser_program_result(corpus).expect("rlyeh parser run")
}

/// 同 [`run_rlyeh_parser_program`]，但返回 `Result`。
fn run_rlyeh_parser_program_result(corpus: &str) -> Result<String, rlyeh_driver::error::DriverError> {
    let _guard = PARSER_RUN_LOCK.lock().unwrap();
    let src = format!(
        "{}fn main() {{ let s = String::from(\"{}\"); let ast = parse_program(s); println(ast); }}\n",
        parser_src(),
        escape_rl(corpus),
    );
    let seq = PARSER_RUN_SEQ.fetch_add(1, Ordering::SeqCst);
    let tmp: PathBuf = std::env::temp_dir().join(format!("rlyeh_selfhost_parser_{seq}.rl"));
    fs::write(&tmp, src).expect("write temp parser");
    let res = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut driver =
                rlyeh_driver::IncrementalDriver::new(std::env::temp_dir()).with_force(true);
            driver.run_source_file(&tmp)
        })
        .unwrap()
        .join()
        .unwrap()?;
    Ok(res.0)
}

/// 对拍：Rlyeh 版 parser 与 Rust oracle 对 corpus 的程序级 AST 应一致。
fn check_program(corpus: &str) {
    let expected = norm(&rlyeh_driver::emit_ast_canonical(corpus).expect("oracle parse"));
    let got = norm(&run_rlyeh_parser_program(corpus));
    assert_eq!(got, expected, "\ncorpus: {corpus}\n");
}

#[test]
fn m_m2a_expression_ast_matches_rust_oracle() {
    // 基础运算 + 优先级
    check("1 + 2");
    check("1 + 2 * 3");
    check("(1 + 2) * 3");
    check("10 - 2 - 3");          // 左结合
    check("2 * 3 + 4 * 5");       // 乘除优先
    check("100 / 4 / 5");         // 左结合
    // 一元负号
    check("-5");
    check("-5 + 3");
    check("-(1 + 2)");
    check("3 * -4");
    // 标识符与嵌套括号
    check("a + b * c");
    check("((a))");
    check("(a + b) * (c - d)");
    // 比较 / 逻辑 / 位运算混合，验证优先级
    check("1 < 2 && 3 > 4");
    check("a == b || c != d");
    check("1 + 2 << 3");
    check("x & y | z");
    check("a ^ b & c");
    // 较复杂的混合表达式
    check("(a + b) * c - d / e == f && g >= h || i < j");
}

#[test]
fn m_m2b_statement_ast_matches_rust_oracle() {
    // 顶层 let / let mut + 表达式语句
    check_program("let x = 1 + 2;");
    check_program("let mut y = x * 3;");
    check_program("let a = 1; let b = 2; a + b");
    check_program("let n = 10; n - 1");
    // 带类型标注（M-M2b1 忽略 anno，规范输出与无标注一致）
    check_program("let x: i64 = 1 + 2;");
    // 嵌套块 + 块尾表达式
    check_program("{ let x = 1; x }");
    check_program("{ let x = 1; let y = 2; x + y }");
    check_program("{ { let z = 5; } }");
    check_program("{ let a = 1; { let b = 2; b } a }");
    // 顶层裸表达式
    check_program("1 + 2 * 3");
    // return（块内）
    check_program("{ return 1; }");
    check_program("{ return; }");
    check_program("{ let x = 7; return x + 1; }");
    // 混合：let + 块表达式 + 顶层裸表达式
    check_program("let n = 10; { let m = n * 2; m } n + 1");
}
