//! M-M2 差分对拍：Rlyeh 版 parser（`self-host/parser.rl`）与 Rust oracle
//! (`rlyeh_driver::emit_ast_canonical_expr`) 对表达式源码生成的规范 S-表达式 AST
//! 文本逐字节一致。
//!
//! 切片1（M-M2a）范围：整数/标识符/一元负号/括号/二元算术与比较/逻辑/位运算。
//! M-M2b（语句/let/类型/模式）、M-M2c（控制流）、M-M3a（字段访问）、M-M2e（索引/调用/方法）依次扩展。

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

/// 读取仓库 `examples/` 下的示例源码（用于 dogfood 对拍）。
fn example_src(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = format!("{manifest}/../../examples/{name}");
    fs::read_to_string(&path)
        .unwrap_or_else(|_| fs::read_to_string(format!("examples/{name}")).expect("read example"))
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
        "{}fn main() {{ let s = String::from(\"{}\"); let toks = tokenize(s); let r = parse_expr(toks, 0); let ast = r.s; println(ast); }}\n",
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
    // 类型标注（M-M2b2：渲染为 (type ...) 节点）
    check_program("let x: i64 = 1 + 2;");
    check_program("let x: u32 = 10;");
    check_program("let x: String = 0;");
    check_program("let x: &i64 = 0;");
    check_program("let x: &mut i64 = 0;");
    check_program("let x: Vec<i64> = 0;");
    check_program("let x: Result<i64, String> = 0;");
    check_program("let x: Vec<Result<i64, String>> = 0;");   // 嵌套泛型
    check_program("let x: _ = 0;");                          // 推断
    check_program("let mut y: Vec<i64> = 0;");
    // 元组类型标注（M-M2b3）：(A, B) -> (tuple-type ...)
    check_program("let x: (i64, i64) = 0;");
    check_program("let x: (i64, String, bool) = 0;");
    check_program("let x: (Vec<i64>, u32) = 0;");
    check_program("let x: Vec<(i64, String)> = 0;");         // 元组在泛型内
    check_program("let mut y: (i64, i64) = 0;");
    // 数组类型标注（M-M2b3）：[T; N] -> (array T N)
    check_program("let x: [i64; 4] = 0;");
    check_program("let x: [(i64, i64); 4] = 0;");            // 元素为元组
    check_program("let x: [Vec<Result<i64, String>>; 2] = 0;");  // 嵌套泛型 + shr 拆分
    // 模式：扁平元组解构与 _ 通配（M-M2b2）
    check_program("let (a, b) = 1 + 2;");
    check_program("let (a, _, c) = 1;");
    check_program("let mut (a, b) = 1;");
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
    // 混合：模式 + 类型标注 + 嵌套块
    check_program("let (a, b) = 1 + 2; { let (c, _) = a; c }");
}

#[test]
fn m_m2c_control_flow_ast_matches_rust_oracle() {
    // if / else（M-M2c：语句与块尾表达式双形态）
    check_program("if x { }");
    check_program("if x { y }");
    check_program("if x { } else { }");
    check_program("if x { a } else { b }");
    check_program("if x { a } else if y { b } else { c }");   // else if 收束为嵌套 if
    check_program("if x < 5 { a } else { b }");               // 条件含比较链
    check_program("if (x) { y }");                            // 条件加括号
    // while / loop
    check_program("while x { }");
    check_program("while x { y }");
    check_program("while i < 10 { i }");
    check_program("loop { }");
    check_program("loop { x }");
    // 嵌套控制流
    check_program("if x { if y { z } }");
    check_program("while x { if y { z } }");
    check_program("if x { while y { z } }");
    // 控制流作块尾表达式（裸，无 semi）
    check_program("{ if x { } }");
    check_program("{ if x { } else { } }");
    check_program("{ while x { } }");
    check_program("{ loop { } }");
    // 控制流后接语句 → semi 包裹
    check_program("if x { } x");
    check_program("while x { } y");
    // 控制流作 let 初始化表达式（M-M2c 表达式双形态之一）
    check_program("let r = if x { 1 } else { 2 };");
    check_program("let r: i64 = if x { 1 } else { 2 };");
    check_program("let r = loop { 1 };");
    check_program("let r = while x { 1 };");
    check_program("let r: Vec<i64> = if x { 1 } else { 2 };");
}

#[test]
fn m_m3a_field_access_ast_matches_rust_oracle() {
    // 字段访问（后缀链，可多层）— M-M3a-part1（无递归实现）
    check("a.b");
    check("a.b.c");
    check("a.x + b.y");
    check("(a + b).c");
    check("a.b.c.d");
    check("x.y == z.w");
    check("p.x * 2");
    // 字段访问作 let 初始化 / 控制流条件（沿用既有机制）
    check_program("let r = a.b;");
    check_program("let r = a.b.c;");
    check_program("if a.b { c.d } else { e.f }");
}

#[test]
fn m_m2e_index_call_ast_matches_rust_oracle() {
    // 索引访问（可嵌套 / 含表达式）— 后缀标记化，无递归
    check("a[0]");
    check("a[i + 1]");
    check("arr[i][j]");
    check("m.get(k).value");
    check("v[0].len()");
    check("a.x[1].b[2]");
    // 函数调用 / 方法调用（统一规范为 (call ...)）
    check("foo()");
    check("foo(1, 2)");
    check("a.b(1)");
    check("a.b(x, y)");
    check("add(1, 2).scale(3)");
    check("f(g(x))");
    check("foo().bar()");
    // 混合：字段 + 索引 + 调用 组合
    check("v[0].len()");
    check("map.get(k).push(1)");
    check("(a + b)[i].call(x)");
    // 索引/调用作 let 初始化（控制流双形态沿用既有机制）
    check_program("let r = a[0];");
    check_program("let r = foo(1, 2);");
    check_program("if a[i].ok { b.call() } else { c[0] }");
}

#[test]
fn m_m3_tuple_array_literal_ast_matches_rust_oracle() {
    // 元组字面量
    check("(1, 2)");
    check("(1, 2, 3)");
    check("(1 + 2, 3)");
    check("((1, 2))");
    check("(a, b)");
    check("(x.y, z.w)");
    check("(1, 2) + (3, 4)");
    // 数组字面量
    check("[1, 2, 3]");
    check("[1 + 2, 3 * 4]");
    check("[(1, 2), 3]");
    check("[a.b, c[0]]");
    check("[[1, 2], [3, 4]]");
    // 元组/数组作调用实参与 let 初始化
    check("foo((1, 2))");
    check("bar([1, 2, 3])");
    check_program("let r = (1, 2);");
    check_program("let r = [1, 2, 3];");
    check_program("let (a, b) = (1, 2);");
    check_program("let x = [(1, 2), (3, 4)];");
}

#[test]
fn m_m4_for_range_ast_matches_rust_oracle() {
    // for 循环 + 范围迭代（M-M4）：for <pat> in <iter> <block>
    // 三种范围形态：..< 左闭右开 / ... 闭区间 / <.. 左开右闭
    check_program("for i in 0..<10 {}");
    check_program("for i in 0...10 {}");
    check_program("for i in 0<..10 {}");
    check_program("for i in 0..<10 { i }");
    check_program("for i in 0..<10 { let x = i; x }");
    // 元组模式（复用 parse_pattern_tokens）
    check_program("for (a, b) in 0<..10 { a }");
    check_program("for (x, y) in 0...5 { x }");
    // 迭代器为数组字面量（非范围表达式）
    check_program("for x in [1, 2, 3] {}");
    check_program("for x in [(1, 2), (3, 4)] {}");
    // for 后接其他语句（控制帧收束不泄漏）
    check_program("for i in 0..<10 {} let y = i;");
    // 多语句 for 体（let 绑定，非裸赋值——裸赋值语句超出 M-M4 范围）
    check_program("let n = 0; for i in 0..<10 { let s = n + i; s } n");
}

#[test]
fn m_m5_match_ast_matches_rust_oracle() {
    // match 表达式（M-M5）：match <subj> { <pat> => <block>, ... }
    // 单臂：字面量 / 通配 / 标识符
    check_program("match x { 1 => { 1 } }");
    check_program("match x { _ => { 0 } }");
    check_program("match x { a => { a } }");
    // 多臂 + 尾逗号
    check_program("match x { 1 => { 1 }, 2 => { 2 } }");
    check_program("match x { 1 => { 1 }, 2 => { 2 }, }");
    // 布尔字面量模式
    check_program("match x { true => { 1 }, false => { 0 } }");
    // 元组模式（扁平单 token 元素）
    check_program("match p { (a, b) => { a } }");
    check_program("match p { (1, 2) => { 1 } }");
    // 或模式（两路）
    check_program("match x { 1 | 2 => { 1 } }");
    // 范围模式（三种：..< 左闭右开 / ... 闭区间 / <.. 左开右闭）
    check_program("match x { 0..<10 => { 1 } }");
    check_program("match x { 0...10 => { 1 } }");
    check_program("match x { 0<..10 => { 1 } }");
    // 守卫（pat if cond => body）
    check_program("match x { a if a > 0 => { a } }");
    check_program("match x { a if a > 0 => { a }, _ => { 0 } }");
    // 嵌套 match（臂体内）
    check_program("match x { 1 => { match y { 2 => { 3 } } } }");
    // match 后接其他语句（帧收束不泄漏）
    check_program("match x { 1 => { 1 } } let z = 0;");
}

#[test]
fn m_m6_fn_item_ast_matches_rust_oracle() {
    // 函数项（M-M6）：[pub] fn <name>(<params>) [-> <ret>] <block>
    // 无参数 / 空体
    check_program("fn main() {}");
    // 基本参数 + 返回类型
    check_program("fn add(a: i64, b: i64) -> i64 { a }");
    check_program("fn id(x: i64) -> i64 { x }");
    // 无返回类型 + 有体
    check_program("fn foo() { 1 }");
    // pub 函数
    check_program("pub fn pub_fn() {}");
    // mut 参数
    check_program("fn f(mut x: i64) -> i64 { x }");
    // 泛型 / 引用参数类型
    check_program("fn vec_sum(v: Vec<i64>) -> i64 { 0 }");
    check_program("fn ref_fn(x: &i64) -> i64 { 0 }");
    // 函数体含语句与块尾表达式
    check_program("fn main() { let x = 1; x }");
    // 函数体含控制流（if/else 作块尾表达式）
    check_program("fn nested(a: i64) -> i64 { if a > 0 { 1 } else { 0 } }");
    // 多函数项
    check_program("fn a() {} fn b() {}");
    // 函数项后接顶层语句
    check_program("fn a() {} let z = 0;");
}

#[test]
fn m_m7_literals_ast_matches_rust_oracle() {
    // M-M7 字面量补全：字符串 + 布尔 + 注释跳过
    // 布尔字面量
    check_program("true");
    check_program("false");
    check_program("let b = true;");
    check_program("if true { 1 }");
    check_program("foo(true, false);");
    check_program("println(true);");
    // 字符串字面量
    check_program("\"hi\"");
    check_program("let s = \"hello world\";");
    check_program("println(\"Hello, Rlyeh!\");");
    check_program("\"\"");
    check_program("foo(\"a\", \"b\");");
    check_program("let s = \"6 * 7 = \";");
    // 注释跳过（行注释 / 块注释）
    check_program("1; // 行注释\n2;");
    check_program("// 前缀注释\nlet x = 1;");
    check_program("let x = 1; /* 块注释 */ let y = 2;");
    // 字符串/布尔作 match 模式（M-M5 回归）
    check_program("match b { true => { 1 }, false => { 0 } }");
}

#[test]
fn m_m7_dogfood_examples_ast_matches_rust_oracle() {
    // dogfood：用仓库真实示例文件与 oracle 对拍（含注释 + 字符串 + 布尔）
    check_program(&example_src("hello-world.rl"));
    check_program(&example_src("arith-print.rl"));
}

#[test]
fn m_m8_struct_enum_decl_ast_matches_rust_oracle() {
    // M-M8 结构体 / 枚举声明（项）
    // 结构体
    check_program("struct Point { x: i64, y: i64 }");
    check_program("struct Empty { }");
    check_program("pub struct P { a: i64 }");
    check_program("struct S { a: Vec<i64> }");
    check_program("struct R { r: &i64 }");
    // 枚举
    check_program("enum Color { Red, Green }");
    check_program("enum E { A, B(i64), C(i64, String) }");
    check_program("enum S { V { x: i64 } }");
    check_program("enum Mix { A, B(i64), C { f: i64 } }");
    check_program("pub enum PE { A }");
    // 与函数项混排
    check_program("struct P { x: i64 } fn main() {}");
    check_program("enum E { A } fn f() -> i64 { 0 } let z = 0;");
}
