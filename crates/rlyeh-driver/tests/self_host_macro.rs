//! M-M3c 差分对拍：Rlyeh 版最小 macro 展开器（`self-host/macro.rl`）与 Rust oracle
//! （`rlyeh_driver::emit_macro_expand` / `emit_macro_expand_rules`，底层 `crates/rlyeh-macro`）
//! 的展开 token 流逐行一致。
//!
//! Harness 与 `self_host_lexer.rs` 同构：把 `self-host/lexer.rl` + `self-host/macro.rl`
//! 拼接后交给 driver 编译（`macro.rl` 复用 lexer.rl 的 `lex`，Rlyeh 无 include），
//! 再由生成的 `main` 注入三段源码（matcher / transcriber / invocation 的**组内容**）。
//!
//! 覆盖子集：单规则 / 多规则、字面 token、`$x:ident` / `$x:ty` / `$x:tt` / `$x:expr`、
//! 简单重复 `$($x),*` / `$($x),+`、嵌套定界组、重复展开。
//!
//! **明确不支持**（与 Rust oracle 的 MVP 限制同源，不在本切片对拍范围）：
//! - hygiene / 前向引用 / `$crate`；
//! - 嵌套重复（`$($($x)*)*`）；
//! - 其它元变量种类（`pat` / `stmt` / `path` / `item` 等，Rust oracle 亦返回错误）；
//! - `$x:ty` 只捕获**单个 token**（`Vec<i64>` 只捕获 `Vec`；Rust `rlyeh-macro` 亦然）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// 串行化锁：复用 driver 内部 `temp_dir()/rlyeh-run` 可执行文件，并行会互相覆盖。
static MACRO_RUN_LOCK: Mutex<()> = Mutex::new(());
/// 每次调用的唯一序号，避免并行残留临时文件干扰。
static MACRO_RUN_SEQ: AtomicU64 = AtomicU64::new(0);

/// 读取并拼接 Rlyeh 版 lexer + macro 源码（`macro.rl` 依赖 `lex`，须后置）。
fn macro_src() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let read = |rel: &str| -> String {
        let p = Path::new(manifest).join(rel);
        let p = if p.is_file() { p } else { PathBuf::from(rel) };
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };
    let mut src = read("../../self-host/lexer.rl");
    src.push('\n');
    src.push_str(&read("../../self-host/macro.rl"));
    src
}

/// 把任意字符串转义为 Rlyeh 字符串字面量内容（与 lexer / parser harness 同款）。
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

/// 用 Rlyeh 版展开器跑单规则 corpus，返回规范化 token 文本（失败返回 Err）。
fn run_rlyeh_macro_result(
    matcher: &str,
    transcriber: &str,
    invocation: &str,
) -> Result<String, rlyeh_driver::error::DriverError> {
    let _guard = MACRO_RUN_LOCK.lock().unwrap();
    let src = format!(
        "{}\nfn main() {{\n    let m = String::from(\"{}\");\n    let t = String::from(\"{}\");\n    let i = String::from(\"{}\");\n    let r = expand_macro(m, t, i);\n    println(r);\n}}\n",
        macro_src(),
        escape_rl(matcher),
        escape_rl(transcriber),
        escape_rl(invocation),
    );
    let seq = MACRO_RUN_SEQ.fetch_add(1, Ordering::SeqCst);
    let tmp: PathBuf = std::env::temp_dir().join(format!("rlyeh_selfhost_macro_{seq}.rl"));
    fs::write(&tmp, src).expect("write temp macro");
    // 与 driver 顶层 run_source 一致：64MB 栈线程编译+运行，避免 typecheck 栈溢出。
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

/// 单规则：Rlyeh 版展开器输出（合法输入）。
fn run_rlyeh_macro(matcher: &str, transcriber: &str, invocation: &str) -> String {
    run_rlyeh_macro_result(matcher, transcriber, invocation).expect("rlyeh macro run")
}

/// 用 Rlyeh 版展开器跑多规则 corpus（`expand_macro_rules`），返回规范化 token 文本。
fn run_rlyeh_macro_rules_result(
    matchers: &[&str],
    transcribers: &[&str],
    invocation: &str,
) -> Result<String, rlyeh_driver::error::DriverError> {
    let _guard = MACRO_RUN_LOCK.lock().unwrap();
    let mut body = String::new();
    body.push_str("    let mut ms: Vec<String> = Vec::new();\n");
    for m in matchers {
        body.push_str(&format!(
            "    ms.push(String::from(\"{}\"));\n",
            escape_rl(m)
        ));
    }
    body.push_str("    let mut ts: Vec<String> = Vec::new();\n");
    for t in transcribers {
        body.push_str(&format!(
            "    ts.push(String::from(\"{}\"));\n",
            escape_rl(t)
        ));
    }
    body.push_str(&format!(
        "    let i = String::from(\"{}\");\n    let r = expand_macro_rules(ms, ts, i);\n    println(r);\n",
        escape_rl(invocation)
    ));
    let src = format!("{}\nfn main() {{\n{}}}\n", macro_src(), body);
    let seq = MACRO_RUN_SEQ.fetch_add(1, Ordering::SeqCst);
    let tmp: PathBuf = std::env::temp_dir().join(format!("rlyeh_selfhost_macro_rules_{seq}.rl"));
    fs::write(&tmp, src).expect("write temp macro rules");
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

fn run_rlyeh_macro_rules(matchers: &[&str], transcribers: &[&str], invocation: &str) -> String {
    run_rlyeh_macro_rules_result(matchers, transcribers, invocation).expect("rlyeh macro rules run")
}

/// 去除 `println` 自带尾随换行，使两侧可比。
fn norm(s: &str) -> String {
    s.trim_end().to_string()
}

/// 单规则对拍：Rlyeh 输出与 Rust oracle 应逐行一致。
fn check(matcher: &str, transcriber: &str, invocation: &str) {
    let expected = norm(
        &rlyeh_driver::emit_macro_expand(matcher, transcriber, invocation)
            .unwrap_or_else(|e| panic!("oracle macro expand failed: {e}")),
    );
    let got = norm(&run_rlyeh_macro(matcher, transcriber, invocation));
    assert_eq!(
        got, expected,
        "\n[macro mismatch]\nmatcher: {matcher}\ntranscriber: {transcriber}\ninvocation: {invocation}\n"
    );
}

/// 多规则对拍。
fn check_rules(matchers: &[&str], transcribers: &[&str], invocation: &str) {
    let expected = norm(
        &rlyeh_driver::emit_macro_expand_rules(matchers, transcribers, invocation)
            .unwrap_or_else(|e| panic!("oracle macro rules expand failed: {e}")),
    );
    let got = norm(&run_rlyeh_macro_rules(matchers, transcribers, invocation));
    assert_eq!(
        got, expected,
        "\n[macro rules mismatch]\nmatchers: {matchers:?}\ntranscribers: {transcribers:?}\ninvocation: {invocation}\n"
    );
}

#[test]
fn m_m3c_single_rule_matches_rust_oracle() {
    // 恒等宏 + $x:ident
    check("$x:ident", "$x", "foo");
    check("$x:ident", "prefix $x", "foo");
    // 字面 token 交换（含标点）
    check("+ -", "- +", "+ -");
    check("a b c", "c b a", "a b c");
    // 两参宏（ident 交换）
    check("$a:ident , $b:ident", "$b , $a", "x , y");
    check("$a:ident , $b:ident", "[ $a , $b ]", "x , y");
    // 字面 + 元变量混合
    check(
        "let $n:ident = $e:expr ;",
        "$n = $e ;",
        "let count = 1 + 2 * 3 ;",
    );
    // $x:tt：单 token / 整组（定界组作为整体捕获与转写）
    check("$x:tt", "$x", "42");
    check("$x:tt", "[ $x ]", "( 1 + 2 )");
    check("$x:tt", "{ $x }", "abc");
    // 嵌套定界组 matcher
    check("[ $x:tt ]", "( $x )", "[ abc ]");
    check("( $a:ident ) $b:ident", "[ $b $a ]", "( x ) y");
    // $x:ty（MVP：单 token）
    check("$t:ty", "Vec < $t >", "i64");
    check("$t:ty", "$t", "String");
    // $x:expr：token 级优先级爬升捕获
    check("$e:expr", "( $e )", "1 + 2 * 3");
    check("$e:expr", "f $e", "( 1 , 2 )");
    check("$e:expr", "$e", "a . b");
    check("$e:expr", "$e", "- 1");
    check("$e:expr", "$e", "x as i64");
    check("$e:expr", "$e", "f ( x ) + 1");
    check("$e:expr", "$e", "0 ..< 10");
    check("$a:expr , $b:expr", "[ $a , $b ]", "1 + 2 , 3");
    // 简单重复：$($x),* / $($x),+
    check("$( $x:expr ),*", "[ $( $x ),* ]", "1 , 2 , 3");
    check("$( $x:expr ),*", "[ $( $x ),* ]", "");
    check("$( $x:expr ),+", "[ $( $x ),+ ]", "7");
    check("$( $x:expr ),+", "[ $( $x ),+ ]", "1 , 2");
    // 可选重复：$($x),? / $($x)?（0 次 / 1 次）
    check("$( $x:expr ),?", "[ $( $x ),? ]", "");
    check("$( $x:expr ),?", "[ $( $x ),? ]", "1");
    check("$( $x:ident )?", "$( $x )?", "foo");
    check("$( $x:ident )?", "$( $x )?", "");
    // 无元变量的重复（0 次 / + 一次）
    check("", "[ $( + ),* ]", "");
    check("", "[ $( + ),+ ]", "");
    // 重复片段内多 token
    check("$( $x:expr ),*", "[ $( ( $x ) ),* ]", "1 , 2");
    // 重复 + 定界组（tt 捕获整组）
    check("$( $x:tt ),*", "{ $( $x ),* }", "a , ( b ) , c");
    // 空 matcher / 空 transcriber / 空调用（恒等空展开）
    check("", "", "");
    // 比较表达式捕获 / 原始标识符
    check("$e:expr", "$e", "a > b");
    check("$x:ident", "$x", "r#type");
}

#[test]
fn m_m3c_multi_rule_matches_rust_oracle() {
    // 首条字面规则命中 / 回退到通配规则
    check_rules(&["0", "$x:expr"], &["a", "b"], "0");
    check_rules(&["0", "$x:expr"], &["a", "b"], "9");
    // 规则顺序：ident 规则优先于 tt 规则
    check_rules(&["$x:ident", "$x:tt"], &["i", "t"], "foo");
    // 首条不匹配（字面 +）→ 回退 tt
    check_rules(&["+", "$x:tt"], &["plus", "other"], "-");
    // 第三条规则命中（前两条均不匹配）
    check_rules(&["a", "b", "$x:tt"], &["ra", "rb", "rt"], "c");
    // 多规则 + 不同 arity
    check_rules(
        &["$a:ident , $b:ident", "$x:tt"],
        &["[ $a $b ]", "$x"],
        "x , y",
    );
}
