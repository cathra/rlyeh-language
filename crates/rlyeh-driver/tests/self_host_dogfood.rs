//! M-M4（SH-P2-7）前端自举 dogfood：lexer + ast + parser 'driver 串联' 对真实语料对拍。
//!
//! 与 self_host_lexer.rs / self_host_parser.rs / self_host_ast.rs 的单层切片不同，
//! 本测试把三层**拼成一个自举前端**交给 Rust rlyeh-driver 编译，一次运行同时产出：
//!   1. lex(src) 的规范化 token 文本；
//!   2. parse_program(src) 的规范 S-表达式 AST（M-M3b 真·arena 渲染）；
//!   3. ast_kinds(parse_ast(canonical)) 的前序节点 head 序列；
//!   4. render_ast(parse_ast(canonical)) 的 roundtrip 文本。
//!
//! 四项分别与 Rust 前端 oracle 对拍：
//!   - emit_tokens_str；
//!   - emit_ast_canonical；
//!   - emit_ast_kinds；
//!   - canonical 原文（roundtrip）。
//!
//! 语料来自仓库**真实文件**（examples/ 与 tests/run-pass/），覆盖自举 parser 已支持的
//! 语言子集；未覆盖语法（module/import、impl/trait、闭包、async/actor、region、结构体/枚举
//! 字面量与模式、切片类型、复合赋值等）在叶子 sh-p2-7-m4-integration.md 明确登记。
//!
//! Rlyeh 无 include：harness 按 lexer.rl → ast.rl → parser/*.rl（文件名序）拼接。
//! 三层原本各自独立编译（lexer 与 parser 的 is_ident_cont 同名），parser 侧辅助函数
//! 加 p_ 前缀以允许同一编译单元串联。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// 串行化锁：复用 driver 内部临时产物，测试并行会互相覆盖。
static DOGFOOD_RUN_LOCK: Mutex<()> = Mutex::new(());
static DOGFOOD_RUN_SEQ: AtomicU64 = AtomicU64::new(0);

const TOKENS_MARKER: &str = "@@TOKENS@@";
const AST_MARKER: &str = "@@AST@@";
const KINDS_MARKER: &str = "@@KINDS@@";
const RENDER_MARKER: &str = "@@RENDER@@";

/// 读取仓库相对路径文件；兼容 cargo 从 workspace 根或 crate 根运行。
fn read_repo(rel: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    // cargo test 的工作目录是 crate 根（crates/rlyeh-driver），语料在 workspace 根；
    // 依次尝试 crate 根、workspace 根（../../）与当前目录，兼容不同 harness 启动方式。
    let candidates = [
        manifest.join(rel),
        manifest.join("../../").join(rel),
        PathBuf::from(rel),
    ];
    let path = candidates
        .into_iter()
        .find(|p| p.is_file())
        .unwrap_or_else(|| manifest.join("../../").join(rel));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// 自举前端完整源码：lexer.rl + ast.rl + parser/*.rl（文件名序）。
fn frontend_src() -> String {
    let mut src = read_repo("../../self-host/lexer.rl");
    src.push('\n');
    src.push_str(&read_repo("../../self-host/ast.rl"));
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../self-host/parser");
    let dir = if dir.is_dir() {
        dir
    } else {
        PathBuf::from("self-host/parser")
    };
    let mut shards: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read dir {}: {e}", dir.display()))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "rl"))
        .collect();
    assert!(!shards.is_empty(), "no parser shards in {}", dir.display());
    shards.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    for shard in &shards {
        src.push_str(
            &fs::read_to_string(shard).unwrap_or_else(|e| panic!("read {}: {e}", shard.display())),
        );
    }
    src
}

/// 把任意源码转义为 Rlyeh 字符串字面量内容。
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

/// 一次自举前端运行的完整产物。
struct FrontendOut {
    tokens: Vec<String>,
    ast: String,
    kinds: Vec<String>,
    rendered: String,
}

/// 编译并运行「lexer + ast + parser」自举前端，对单文件源码产出四层结果。
fn run_self_host_frontend(corpus: &str) -> FrontendOut {
    let _guard = DOGFOOD_RUN_LOCK.lock().unwrap();
    let src = format!(
        "{frontend}\nfn main() {{\n    let s = String::from(\"{escaped}\");\n    let toks = lex(s);\n    println(\"{tok_marker}\");\n    let mut ti = 0;\n    while ti < toks.len {{ println(toks.get(ti)); ti = ti + 1; }}\n    let s2 = String::from(\"{escaped}\");\n    let canonical = parse_program(s2);\n    println(\"{ast_marker}\");\n    println(canonical);\n    let ast = parse_ast(canonical);\n    println(\"{kinds_marker}\");\n    let ks = ast_kinds(ast);\n    let mut ki = 0;\n    while ki < ks.len {{ println(ks.get(ki)); ki = ki + 1; }}\n    println(\"{render_marker}\");\n    println(render_ast(parse_ast(canonical)));\n}}\n",
        frontend = frontend_src(),
        escaped = escape_rl(corpus),
        tok_marker = TOKENS_MARKER,
        ast_marker = AST_MARKER,
        kinds_marker = KINDS_MARKER,
        render_marker = RENDER_MARKER,
    );
    let seq = DOGFOOD_RUN_SEQ.fetch_add(1, Ordering::SeqCst);
    let tmp: PathBuf = std::env::temp_dir().join(format!("rlyeh_selfhost_dogfood_{seq}.rl"));
    fs::write(&tmp, src).expect("write temp dogfood source");
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
        .expect("self-host frontend run");
    let stdout = res.0;
    let mut out = FrontendOut {
        tokens: Vec::new(),
        ast: String::new(),
        kinds: Vec::new(),
        rendered: String::new(),
    };
    #[derive(Clone, Copy, PartialEq)]
    enum Stage {
        Tokens,
        Ast,
        Kinds,
        Render,
    }
    let mut stage = Stage::Tokens;
    for line in stdout.lines() {
        match line {
            TOKENS_MARKER => stage = Stage::Tokens,
            AST_MARKER => stage = Stage::Ast,
            KINDS_MARKER => stage = Stage::Kinds,
            RENDER_MARKER => stage = Stage::Render,
            _ => match stage {
                Stage::Tokens => out.tokens.push(line.to_string()),
                Stage::Ast => out.ast = line.to_string(),
                Stage::Kinds => out.kinds.push(line.to_string()),
                Stage::Render => out.rendered = line.to_string(),
            },
        }
    }
    out
}

fn lines_of(text: &str) -> Vec<String> {
    if text.is_empty() {
        Vec::new()
    } else {
        text.lines().map(|l| l.to_string()).collect()
    }
}

/// 单文件四层对拍：token / AST 文本 / 节点 kinds / AST roundtrip。
fn check_file(rel: &str) {
    let corpus = read_repo(rel);
    let expected_tokens = lines_of(
        &rlyeh_driver::emit_tokens_str(&corpus)
            .unwrap_or_else(|e| panic!("oracle tokens for {rel}: {e}")),
    );
    let expected_ast = rlyeh_driver::emit_ast_canonical(&corpus)
        .unwrap_or_else(|e| panic!("oracle canonical for {rel}: {e}"))
        .trim()
        .to_string();
    let expected_kinds = lines_of(
        &rlyeh_driver::emit_ast_kinds(&corpus)
            .unwrap_or_else(|e| panic!("oracle kinds for {rel}: {e}")),
    );
    let got = run_self_host_frontend(&corpus);
    assert_eq!(
        got.tokens, expected_tokens,
        "\n[M-M4 token mismatch] {rel}\n"
    );
    assert_eq!(got.ast, expected_ast, "\n[M-M4 AST mismatch] {rel}\n");
    assert_eq!(
        got.kinds, expected_kinds,
        "\n[M-M4 node-kind mismatch] {rel}\n"
    );
    assert_eq!(
        got.rendered, expected_ast,
        "\n[M-M4 AST roundtrip mismatch] {rel}\n"
    );
}

/// 已覆盖的真实语料（自举 parser 当前支持的语言子集）。
///
/// 选择标准：仅使用 fn/let/if/while/for/loop/match/fn 调用/路径表达式/方法调用/
/// 元组与元组字段访问/字符串与布尔字面量/注释/struct·enum 声明等已实现语法。
const COVERED: &[&str] = &[
    // examples/（指南示例 + 基准/冒烟项目）
    "examples/hello-world.rl",
    "examples/arith-print.rl",
    "examples/by-chapter/01-what-is-rlyeh.rl",
    "examples/by-chapter/02-quick-start.rl",
    "examples/by-chapter/11-targets-toolchain.rl",
    "examples/projects/benchmarks/fib/fib.rl",
    "examples/projects/smoke/util/mod.rl",
    "examples/std-demos/02-collections/str_value.rl",
    // tests/run-pass（真实回归语料）
    "tests/run-pass/hello.rl",
    "tests/run-pass/arith.rl",
    "tests/run-pass/tuple_value.rl",
    "tests/run-pass/v2a_str_semantics.rl",
    "tests/run-pass/v2c_print.rl",
    "tests/run-pass/path_ops.rl",
    "tests/run-pass/stdout_stderr.rl",
    "tests/run-pass/sleep_join.rl",
];

#[test]
fn m_m4_dogfood_real_corpus_matches_rust_frontend() {
    for rel in COVERED {
        check_file(rel);
    }
}

/// 自举 parser 明确未覆盖的语法族（与叶子 sh-p2-7-m4-integration.md 的清单一致）。
const UNSUPPORTED: &[(&str, &str)] = &[
    (
        "module/import/use",
        "模块系统与导入（M-M8 之后的项级语法尚未接入）",
    ),
    ("impl/trait/protocol", "trait/impl 块与协议声明"),
    ("closure", "闭包字面量 |x| .. 与迭代器适配器"),
    ("async/await/actor", "异步、await、actor/spawn 并发语法"),
    ("region/gc_region", "区域内存语法"),
    (
        "struct-literal",
        "结构体字面量 S { x: 1 } 与结构体/枚举解构模式",
    ),
    ("enum-pattern", "枚举变体模式与 if let/while let 复杂模式"),
    ("slice/ref-expr", "切片类型 &[T] 与表达式位置的一元 &/*"),
    ("compound-assign", "复合赋值 +=/-= 与裸赋值语句"),
    (
        "macro/attribute",
        "宏调用（vec!/assert!）、属性 #[..]、derive",
    ),
    ("const/static/type", "const/static/类型别名与 where 约束"),
    (
        "float-escape",
        "浮点字面量与含转义的字符串/字符字面量（自举 tokenizer 尚未覆盖）",
    ),
];

/// 覆盖清单自检：确保 dogfood 语料同时来自 examples 与 tests/run-pass，
/// 并锁定「已覆盖 + 未覆盖」两份清单的存在性（防止后续误删）。
#[test]
fn m_m4_dogfood_manifest_is_complete() {
    assert!(
        COVERED.len() >= 16,
        "dogfood 覆盖语料过少: {}",
        COVERED.len()
    );
    assert!(
        COVERED
            .iter()
            .filter(|p| p.starts_with("examples/"))
            .count()
            >= 8,
        "dogfood 语料 examples/ 少于 8 个"
    );
    assert!(
        COVERED
            .iter()
            .filter(|p| p.starts_with("tests/run-pass/"))
            .count()
            >= 8,
        "dogfood 语料 tests/run-pass/ 少于 8 个"
    );
    assert!(
        UNSUPPORTED.len() >= 8,
        "未覆盖语法清单过少: {}",
        UNSUPPORTED.len()
    );
    for (name, reason) in UNSUPPORTED {
        assert!(!name.is_empty() && !reason.is_empty());
    }
}
