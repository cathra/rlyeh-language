//! # 测试运行器（`rlyeh test`）
//!
//! 扫描 `tests/` 目录下的三类用例并驱动编译/运行：
//!
//! | 目录 | 语义 |
//! |------|------|
//! | `tests/compile-pass/` | 每个 `.rl` 必须编译成功（编译到 LLVM IR，不链接） |
//! | `tests/compile-fail/` | 每个 `.rl` 必须编译失败；源内 `// expect: <片段>` 注释断言错误消息包含片段 |
//! | `tests/run-pass/` | 每个 `.rl` 必须编译运行成功；同名 `.out` 文件（若有）作为期望 stdout 精确对比 |
//!
//! 用例文件均可选：目录不存在或为空时该类别自动跳过。

use std::path::{Path, PathBuf};

use crate::{compile_file_to_llvm, run_source_file};

/// 用例类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestKind {
    /// 编译必须成功
    CompilePass,
    /// 编译必须失败
    CompileFail,
    /// 编译运行必须成功（可选 `.out` 输出对比）
    RunPass,
}

impl TestKind {
    /// 用例所在子目录名（相对测试根目录）。
    pub fn dir_name(self) -> &'static str {
        match self {
            TestKind::CompilePass => "compile-pass",
            TestKind::CompileFail => "compile-fail",
            TestKind::RunPass => "run-pass",
        }
    }

    /// 中文标签（CLI 输出用）。
    pub fn label(self) -> &'static str {
        match self {
            TestKind::CompilePass => "编译通过",
            TestKind::CompileFail => "编译失败",
            TestKind::RunPass => "运行通过",
        }
    }
}

/// 单个用例的检查结果。
#[derive(Debug, Clone)]
pub struct TestCaseResult {
    /// 用例类别。
    pub kind: TestKind,
    /// 用例相对路径（如 `compile-pass/hello.rl`）。
    pub name: String,
    /// 是否通过。
    pub passed: bool,
    /// 通过说明或失败详情。
    pub detail: String,
}

/// 测试套件运行汇总。
#[derive(Debug, Clone, Default)]
pub struct TestSummary {
    /// 用例总数。
    pub total: usize,
    /// 通过数。
    pub passed: usize,
    /// 失败数。
    pub failed: usize,
    /// 逐用例结果（保持扫描顺序）。
    pub results: Vec<TestCaseResult>,
}

/// 运行指定根目录下的测试套件（`compile-pass` / `compile-fail` / `run-pass` 三个子目录）。
///
/// 根目录或子目录不存在时静默跳过（空套件返回全零汇总）。
/// 用例文件按文件名排序，结果确定性可复现。
pub fn run_test_suite(root: &Path) -> TestSummary {
    let mut summary = TestSummary::default();
    for kind in [
        TestKind::CompilePass,
        TestKind::CompileFail,
        TestKind::RunPass,
    ] {
        let dir = root.join(kind.dir_name());
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue; // 子目录缺失：可选类别，跳过
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "rl"))
            .collect();
        files.sort();

        for file in files {
            let rel = file
                .strip_prefix(root)
                .unwrap_or(&file)
                .to_string_lossy()
                .to_string();
            let result = match kind {
                TestKind::CompilePass => run_compile_pass(&file, &rel),
                TestKind::CompileFail => run_compile_fail(&file, &rel),
                TestKind::RunPass => run_run_pass(&file, &rel),
            };
            summary.total += 1;
            if result.passed {
                summary.passed += 1;
            } else {
                summary.failed += 1;
            }
            summary.results.push(result);
        }
    }
    summary
}

/// compile-pass：编译到 LLVM IR 必须成功（不链接、不运行）。
fn run_compile_pass(file: &Path, name: &str) -> TestCaseResult {
    match compile_file_to_llvm(file) {
        Ok(_) => TestCaseResult {
            kind: TestKind::CompilePass,
            name: name.to_string(),
            passed: true,
            detail: "编译成功".to_string(),
        },
        Err(e) => TestCaseResult {
            kind: TestKind::CompilePass,
            name: name.to_string(),
            passed: false,
            detail: format!("预期编译成功，实际失败: {e}"),
        },
    }
}

/// compile-fail：编译必须失败；`// expect: <片段>` 注释断言错误消息包含片段。
fn run_compile_fail(file: &Path, name: &str) -> TestCaseResult {
    let expects = expected_error_fragments(file);
    match compile_file_to_llvm(file) {
        Ok(_) => TestCaseResult {
            kind: TestKind::CompileFail,
            name: name.to_string(),
            passed: false,
            detail: "预期编译失败，实际编译成功".to_string(),
        },
        Err(e) => {
            let msg = e.to_string();
            let missing: Vec<String> = expects
                .iter()
                .filter(|f| !msg.contains(f.as_str()))
                .cloned()
                .collect();
            let detail = if missing.is_empty() {
                format!("编译失败（符合预期）: {}", first_line(&msg))
            } else {
                format!(
                    "编译失败但错误消息缺少预期片段 {:?}。实际错误: {}",
                    missing,
                    first_line(&msg)
                )
            };
            TestCaseResult {
                kind: TestKind::CompileFail,
                name: name.to_string(),
                passed: missing.is_empty(),
                detail,
            }
        }
    }
}

/// run-pass：编译运行必须成功；同名 `.out` 文件（若有）作为期望 stdout 精确对比。
fn run_run_pass(file: &Path, name: &str) -> TestCaseResult {
    let expect_file = file.with_extension("out");
    let expected = std::fs::read_to_string(&expect_file).ok();
    match run_source_file(file) {
        Ok(actual) => match &expected {
            Some(exp) if *exp != actual => TestCaseResult {
                kind: TestKind::RunPass,
                name: name.to_string(),
                passed: false,
                detail: format!(
                    "输出不匹配\n--- 期望 ({}) ---\n{exp}--- 实际 ---\n{actual}---",
                    expect_file.display()
                ),
            },
            _ => TestCaseResult {
                kind: TestKind::RunPass,
                name: name.to_string(),
                passed: true,
                detail: if expected.is_some() {
                    "运行成功，输出匹配".to_string()
                } else {
                    "运行成功（无 .out 期望文件）".to_string()
                },
            },
        },
        Err(e) => TestCaseResult {
            kind: TestKind::RunPass,
            name: name.to_string(),
            passed: false,
            detail: format!("编译/运行失败: {e}"),
        },
    }
}

/// 提取源文件中所有 `// expect: <片段>` 注释（支持多行，逐行独立断言）。
///
/// 编译器错误消息（`DriverError` 的 Display）必须包含每个片段才算通过。
/// 注意：注入标准库后行号会偏移，片段应只写错误消息内容而非行号。
fn expected_error_fragments(file: &Path) -> Vec<String> {
    let Ok(source) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let rest = line.strip_prefix("// expect:")?;
            let frag = rest.trim();
            (!frag.is_empty()).then(|| frag.to_string())
        })
        .collect()
}

/// 取错误信息首行（错误带 `[typecheck] 行号: 列: error: ...` 前缀，首行即核心消息）。
fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}
