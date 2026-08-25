//! D2 集成测试：rlyeh-fmt / rlyeh-check 工具链在 driver 层的可用性。
//!
//! 覆盖：
//! - fmt：对 `tests/` 与 `examples/` 中的真实源码格式化后仍可再解析（round-trip）；
//!   - 幂等性（format(format(x)) == format(x)）。
//! - check：干净代码零诊断；违规代码报告对应规则；解析错误报告 parse-error。

use std::path::Path;

use rlyeh_driver::check_source_file;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap()
}

/// 收集工作区下所有 `.rl` 源文件（tests/ 与 examples/）。
fn collect_rlyeh_sources() -> Vec<std::path::PathBuf> {
    let root = workspace_root();
    let mut files = Vec::new();
    for dir in ["tests", "examples"] {
        let dir = root.join(dir);
        let entries = std::fs::read_dir(&dir).expect("dir exists");
        for e in entries.flatten() {
            let p = e.path();
            // 项目源码扩展名为 `.rl`（旧名 `.rlyeh` 兼容保留）
            if p.extension().is_some_and(|x| x == "rl" || x == "rlyeh") {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

/// 真实源码格式化后仍可再次解析，且格式幂等。
#[test]
fn fmt_real_sources_roundtrip_idempotent() {
    let files = collect_rlyeh_sources();
    assert!(!files.is_empty(), "expected at least one .rl source");
    for path in files {
        let src = std::fs::read_to_string(&path).expect("read source");
        let formatted = rlyeh_fmt::format_source(&src)
            .unwrap_or_else(|e| panic!("fmt {} failed: {}", path.display(), e));
        // 格式化结果必须可再次解析
        rlyeh_parser::parse(&formatted)
            .unwrap_or_else(|e| panic!("reparse {} failed: {}\n{}", path.display(), e, formatted));
        // 幂等
        let twice = rlyeh_fmt::format_source(&formatted).expect("second fmt succeeds");
        assert_eq!(formatted, twice, "fmt not idempotent for {}", path.display());
    }
}

/// 格式化应保留可运行语义：对 run-pass 样例格式化后再编译运行，输出不变。
#[test]
fn fmt_preserves_semantics() {
    let root = workspace_root();
    let hello = root.join("tests/run-pass/hello.rl");
    let src = std::fs::read_to_string(&hello).expect("read hello.rl");
    let formatted = rlyeh_fmt::format_source(&src).expect("fmt hello.rl");

    let dir = std::env::temp_dir().join(format!("rlyeh-fmt-sem-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    let tmp = dir.join("hello-formatted.rl");
    std::fs::write(&tmp, &formatted).expect("write formatted source");

    let orig_out = rlyeh_driver::run_source_file(&hello).expect("run original");
    let fmt_out = rlyeh_driver::run_source_file(&tmp).expect("run formatted");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(orig_out, fmt_out, "fmt changed program behavior");
}

/// 干净代码不应产生任何诊断。
#[test]
fn check_clean_source_no_diagnostics() {
    let root = workspace_root();
    let hello = root.join("tests/run-pass/hello.rl");
    let diags = check_source_file(&hello).expect("check hello.rl");
    assert!(diags.is_empty(), "clean file produced: {:?}", diags);
}

/// 违规代码按规则报告，且位置与行号正确。
#[test]
fn check_reports_rules() {
    let dir = std::env::temp_dir().join(format!("rlyeh-check-t-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    let tmp = dir.join("dirty.rl");
    std::fs::write(
        &tmp,
        "fn main() {\n    let unused = 1;\n    if true { println(1); }\n    let b = 1 == 1;\n    println(b);\n}\n",
    )
    .expect("write dirty source");

    let diags = check_source_file(&tmp).expect("check dirty source");
    let _ = std::fs::remove_dir_all(&dir);

    let rendered: Vec<String> = diags.iter().map(|d| d.render()).collect();
    assert!(
        rendered.iter().any(|d| d.contains("unused-variable")),
        "missing unused-variable: {:?}",
        rendered
    );
    assert!(
        rendered.iter().any(|d| d.contains("constant-condition")),
        "missing constant-condition: {:?}",
        rendered
    );
    assert!(
        rendered.iter().any(|d| d.contains("redundant-compare")),
        "missing redundant-compare: {:?}",
        rendered
    );
    // 行号断言：unused-variable 在第 2 行
    assert!(
        rendered.iter().any(|d| d.starts_with("2:") && d.contains("unused-variable")),
        "wrong location for unused-variable: {:?}",
        rendered
    );
}

/// 无法解析的源码应产生 parse-error 错误诊断。
#[test]
fn check_reports_parse_error() {
    let diags = rlyeh_check::check_source("fn broken(");
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule, "parse-error");
    assert_eq!(diags[0].level, rlyeh_check::Level::Error);
}
