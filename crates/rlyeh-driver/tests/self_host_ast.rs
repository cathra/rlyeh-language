//! M-M3a 节点级差分对拍：Rlyeh 版类型化 AST（`self-host/ast.rl`）从规范 S-表达式
//! 文本解析出 arena，再输出「前序 head 序列」；与 Rust oracle
//! (`rlyeh_driver::emit_ast_kinds` / `emit_ast_kinds_expr`，直接遍历 Rust AST)
//! 逐行比对，而非文本拼接比对。
//!
//! 每个 corpus 同时做两项断言：
//! 1. 节点级：Rlyeh `ast_kinds` 前序 head 序列 == Rust oracle head 序列；
//! 2. roundtrip：Rlyeh `render_ast(parse_ast(canonical))` == canonical 原文。
//!
//! harness 参照 `self_host_parser.rs`：读取 ast.rl 全文 + 注入 canonical 文本生成
//! `fn main`，在 64MB 栈线程中经 `IncrementalDriver` 编译运行。用例串行（共享
//! driver 临时产物），每次使用唯一临时源文件名。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// 串行化锁：driver 内部临时可执行文件/目录共享，并行会互相覆盖。
static AST_RUN_LOCK: Mutex<()> = Mutex::new(());
static AST_RUN_SEQ: AtomicU64 = AtomicU64::new(0);

/// Rlyeh 侧输出中 kinds 与 roundtrip 的分隔标记。
const MARKER: &str = "@@RENDER@@";

/// 读取 Rlyeh 版类型化 AST 模型源码（`self-host/ast.rl`）。
fn ast_src() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = format!("{manifest}/../../self-host/ast.rl");
    fs::read_to_string(&path)
        .unwrap_or_else(|_| fs::read_to_string("self-host/ast.rl").expect("read ast.rl"))
}

/// 读取仓库 `examples/` 下的示例源码（dogfood 对拍）。
fn example_src(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = format!("{manifest}/../../examples/{name}");
    fs::read_to_string(&path)
        .unwrap_or_else(|_| fs::read_to_string(format!("examples/{name}")).expect("read example"))
}

/// 把任意字符串转义为 Rlyeh 字符串字面量内容（与 parser/lexer harness 同款）。
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

/// 一次 Rlyeh 运行的产物：前序 head 序列 + roundtrip 回渲染文本。
struct RunOut {
    kinds: Vec<String>,
    rendered: String,
}

/// 用 Rlyeh `self-host/ast.rl` 对 canonical 文本跑 `parse_ast` -> `ast_kinds` +
/// `render_ast`（单次编译运行同时取两项产物，减少 harness 编译次数）。
fn run_rlyeh_ast(text: &str) -> RunOut {
    let _guard = AST_RUN_LOCK.lock().unwrap();
    let escaped = escape_rl(text);
    let src = format!(
        "{ast}\nfn main() {{\n    let s = String::from(\"{escaped}\");\n    let ast = parse_ast(s);\n    let ks = ast_kinds(ast);\n    for k in ks {{ println(k); }}\n    println(\"{marker}\");\n    let s2 = String::from(\"{escaped}\");\n    let ast2 = parse_ast(s2);\n    println(render_ast(ast2));\n}}\n",
        ast = ast_src(),
        marker = MARKER,
    );
    let seq = AST_RUN_SEQ.fetch_add(1, Ordering::SeqCst);
    let tmp: PathBuf = std::env::temp_dir().join(format!("rlyeh_selfhost_ast_{seq}.rl"));
    fs::write(&tmp, src).expect("write temp ast source");
    let res = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let mut driver =
                rlyeh_driver::IncrementalDriver::new(std::env::temp_dir()).with_force(true);
            driver.run_source_file(&tmp)
        })
        .unwrap()
        .join()
        .unwrap()
        .expect("rlyeh ast run");
    let stdout = res.0;
    let mut kinds: Vec<String> = Vec::new();
    let mut rendered = String::new();
    let mut in_render = false;
    for line in stdout.lines() {
        if in_render {
            rendered.push_str(line);
        } else if line == MARKER {
            in_render = true;
        } else {
            kinds.push(line.to_string());
        }
    }
    RunOut { kinds, rendered }
}

fn oracle_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        Vec::new()
    } else {
        text.lines().map(|l| l.to_string()).collect()
    }
}

/// 表达式级对拍：oracle `emit_ast_kinds_expr` vs Rlyeh `ast_kinds(parse_ast(text))`
/// + roundtrip 断言。
fn check_expr(corpus: &str) {
    let canonical = rlyeh_driver::emit_ast_canonical_expr(corpus).expect("oracle expr canonical");
    let expected =
        oracle_lines(&rlyeh_driver::emit_ast_kinds_expr(corpus).expect("oracle expr kinds"));
    let got = run_rlyeh_ast(&canonical);
    assert_eq!(
        got.kinds, expected,
        "\n[expr node kinds mismatch]\ncorpus: {corpus}\ncanonical: {canonical}\n"
    );
    assert_eq!(
        got.rendered, canonical,
        "\n[expr roundtrip mismatch]\ncorpus: {corpus}\n"
    );
}

/// 程序级对拍：oracle `emit_ast_kinds` vs Rlyeh `ast_kinds(parse_ast(text))`
/// + roundtrip 断言。
fn check_program(corpus: &str) {
    let canonical = rlyeh_driver::emit_ast_canonical(corpus).expect("oracle program canonical");
    let expected =
        oracle_lines(&rlyeh_driver::emit_ast_kinds(corpus).expect("oracle program kinds"));
    let got = run_rlyeh_ast(&canonical);
    assert_eq!(
        got.kinds, expected,
        "\n[program node kinds mismatch]\ncorpus: {corpus}\ncanonical: {canonical}\n"
    );
    assert_eq!(
        got.rendered, canonical,
        "\n[program roundtrip mismatch]\ncorpus: {corpus}\n"
    );
}

#[test]
fn m_m3a_ast_expression_nodes_match_rust_oracle() {
    // 字面量 / 标识符 / 一元负号
    check_expr("1");
    check_expr("42");
    check_expr("x");
    check_expr("-5");
    check_expr("-(1 + 2)");
    // 二元运算优先级与结合性
    check_expr("1 + 2");
    check_expr("1 + 2 * 3");
    check_expr("(1 + 2) * 3");
    check_expr("10 - 2 - 3");
    check_expr("a + b * c");
    // 比较 / 逻辑 / 位运算
    check_expr("1 < 2 && 3 > 4");
    check_expr("a == b || c != d");
    check_expr("x & y | z");
    check_expr("1 + 2 << 3");
    // 字段访问 / 索引 / 调用 / 方法调用
    check_expr("a.b.c");
    check_expr("a[0]");
    check_expr("a[i + 1]");
    check_expr("foo()");
    check_expr("foo(1, 2)");
    check_expr("a.b(1)");
    check_expr("v[0].len()");
    check_expr("(a + b)[i].call(x)");
    // 元组 / 数组字面量 / unit
    check_expr("(1, 2)");
    check_expr("(1 + 2, 3)");
    check_expr("[1, 2, 3]");
    check_expr("[(1, 2), 3]");
    check_expr("()");
    // 字符串（含空格，验证原子以括号定界）/ 布尔 / 路径
    check_expr("\"hello world\"");
    check_expr("true");
    check_expr("Option::Some(7)");
    check_expr("Option::None");
    check_expr("Enum::Variant(1)");
}

#[test]
fn m_m3a_ast_statement_item_nodes_match_rust_oracle() {
    // let / let mut / 类型标注
    check_program("let x = 1 + 2;");
    check_program("let mut y = x * 3;");
    check_program("let x: i64 = 1 + 2;");
    check_program("let x: Result<i64, String> = 0;");
    check_program("let x: Vec<Result<i64, String>> = 0;");
    check_program("let x: _ = 0;");
    check_program("let x: (i64, String) = 0;");
    check_program("let x: [Vec<i64>; 2] = 0;");
    check_program("let (a, b) = (1, 2);");
    // 块 / 块尾表达式 / semi
    check_program("{ let x = 1; x }");
    check_program("let n = 10; { let m = n * 2; m } n + 1");
    check_program("{ return 1; }");
    check_program("{ return; }");
    // 控制流
    check_program("if x { a } else { b }");
    check_program("if x { a } else if y { b } else { c }");
    check_program("while i < 10 { i }");
    check_program("loop { }");
    check_program("let r = if x { 1 } else { 2 };");
    // for + range
    check_program("for i in 0..<10 { i }");
    check_program("for i in 0...10 {}");
    check_program("for (a, b) in 0<..10 { a }");
    check_program("for x in [1, 2, 3] {}");
    // match + arm（字面量/元组/或/范围/守卫）
    check_program("match x { 1 => { 1 }, 2 => { 2 }, }");
    check_program("match p { (a, b) => { a } }");
    check_program("match x { 1 | 2 => { 1 } }");
    check_program("match x { 0..<10 => { 1 } }");
    check_program("match x { a if a > 0 => { a }, _ => { 0 } }");
    // fn / params / param / 返回类型
    check_program("fn main() {}");
    check_program("fn add(a: i64, b: i64) -> i64 { a }");
    check_program("pub fn f(mut x: i64, y: &i64) -> Vec<i64> { x }");
    check_program("fn id<T>(x: T) -> T { x }");
    // struct / enum / variant / generics
    check_program("struct Point { x: i64, y: i64 }");
    check_program("pub struct P<T> { x: T }");
    check_program("enum Color { Red, Green }");
    check_program("enum E { A, B(i64), C(i64, String) }");
    check_program("enum S<T> { V { x: T } }");
    check_program("struct P { x: i64 } fn main() {}");
    // 字符串 / 布尔 / 路径表达式（程序级）
    check_program("let s = \"hello world\";");
    check_program("let b = true;");
    check_program("let a: Option<i64> = Option::Some(7);");
}

#[test]
fn m_m3a_ast_dogfood_examples_match_rust_oracle() {
    // dogfood：仓库真实示例文件（含字符串/布尔/调用/注释）
    check_program(&example_src("hello-world.rl"));
    check_program(&example_src("arith-print.rl"));
}

/// 覆盖度自检（不编译 Rlyeh，纯 oracle 侧）：断言上列 corpus 确实覆盖 M-M3a 要求的
/// 全部节点 head，防止后续 corpus 编辑悄悄丢掉某类节点。
#[test]
fn m_m3a_ast_required_node_heads_covered() {
    let expr_corpora = [
        "1",
        "x",
        "-1",
        "1 + 2",
        "a.b",
        "a[0]",
        "f(1)",
        "(1, 2)",
        "[1, 2]",
        "\"hi\"",
        "true",
        "Option::None",
        "()",
    ];
    let program_corpora = [
        "let x: Vec<i64> = 1;",
        "let x: _ = 0;",
        "let y: &i64 = 0;",
        "let z: (i64, String) = 0;",
        "let w: [i64; 2] = 0;",
        "let (a, b) = 1;",
        "{ 1 }",
        "1;",
        "{ return 1; }",
        "if x { }",
        "while x { }",
        "loop { }",
        "for i in 0..<10 { }",
        "match x { 1 => { 1 } }",
        "fn f(a: i64) -> i64 { a }",
        "fn g<T>(x: T) { }",
        "struct P { x: i64 }",
        "enum E { A(i64) }",
    ];
    let mut heads: std::collections::HashSet<String> = std::collections::HashSet::new();
    for c in expr_corpora {
        let text = rlyeh_driver::emit_ast_kinds_expr(c).expect("oracle expr kinds");
        for line in text.lines() {
            heads.insert(line.to_string());
        }
    }
    for c in program_corpora {
        let text = rlyeh_driver::emit_ast_kinds(c).expect("oracle program kinds");
        for line in text.lines() {
            heads.insert(line.to_string());
        }
    }
    let required = [
        "program",
        "int",
        "var",
        "neg",
        "bin",
        "field",
        "index",
        "call",
        "tuple",
        "array-lit",
        "str",
        "bool",
        "path",
        "unit",
        "let",
        "block",
        "semi",
        "if",
        "while",
        "loop",
        "for",
        "range",
        "match",
        "arm",
        "fn",
        "params",
        "param",
        "struct",
        "enum",
        "variant",
        "generics",
        "return",
        "ref",
        "infer",
        "tuple-pat",
        "tuple-type",
        "array",
        "type",
    ];
    for r in required {
        assert!(heads.contains(r), "覆盖度自检缺失节点 head: {r}");
    }
}
