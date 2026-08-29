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
//!
//! 用例默认**并行执行**（worker 数 = `min(可用核, 8)`，见 `parallel_workers`）：每个用例
//! 使用独立临时目录与独立 `clang` 子进程/运行进程，路径相互隔离，故线程安全；汇总结果
//! 按扫描顺序（文件名排序）回填，输出确定性可复现。worker 封顶 8 是为规避无界并行 spawn
//! `clang` 触发文件描述符耗尽（`os error 35` / EMFILE，见 CHANGELOG 集成测试注记）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

use crate::{compile_file_to_llvm, run_source_file};

/// 并行执行用例时启用的最大 worker 线程数。
///
/// 每个 run-pass 用例会调用 `clang` 子进程（汇编 + 链接），无界并行可能触发
/// 文件描述符耗尽（`os error 35`，EMFILE，见 CHANGELOG 集成测试注记）。此处
/// 按可用核数封顶到一个保守上限，平衡提速与稳定性。
fn parallel_workers() -> usize {
    // 支持 `RLYEH_WORKERS` 环境变量显式覆盖并行度（如 `RLYEH_WORKERS=1` 规避
    // 内存暴涨）。默认上限降为 2——每个 worker 线程独立 spawn 一次完整编译
    // （lexer→codegen→LLVM IR→clang），在测试文件多时 N 路并行编译的内存峰值
    // 会线性叠加；18 核机器上旧默认 6 路并行在 130+ 用例下触发系统内存耗尽
    // （"Your system has run out of application memory"，2026-08-26 实测）。
    if let Ok(s) = std::env::var("RLYEH_WORKERS") {
        if let Ok(n) = s.trim().parse::<usize>() {
            return n.max(1);
        }
    }
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    cores.min(2).max(1)
}

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
    /// 跳过数（文件头 `// skip: <原因>` 注释标记，避免触发已知问题如内存暴涨）。
    pub skipped: usize,
    /// 逐用例结果（保持扫描顺序）。
    pub results: Vec<TestCaseResult>,
}

/// 单个待运行用例。
struct PendingTask {
    kind: TestKind,
    file: PathBuf,
    rel: String,
}

/// 运行指定根目录下的测试套件（`compile-pass` / `compile-fail` / `run-pass` 三个子目录）。
///
/// 根目录或子目录不存在时静默跳过（空套件返回全零汇总）。
/// 用例文件按文件名排序，结果确定性可复现；用例并行执行（独立临时目录/进程），
/// 但汇总结果仍按扫描顺序输出。
pub fn run_test_suite(root: &Path) -> TestSummary {
    let mut tasks: Vec<PendingTask> = Vec::new();
    let mut skipped_marker = 0usize;
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
            // 文件头 `// skip: <原因>` 注释：跳过该用例（计入 summary.skipped）。
            // 用于标记已知问题（如 async 运行时 poll 死循环导致内存暴涨，2026-08-26），
            // 避免全量测试在触发处内存耗尽；修复后移除注释即可恢复。
            if file_has_skip_marker(&file) {
                skipped_marker += 1;
                continue;
            }
            tasks.push(PendingTask { kind, file, rel });
        }
    }
    let results = run_tasks_parallel(tasks);

    let mut summary = TestSummary::default();
    summary.total = results.len();
    summary.skipped = skipped_marker;
    for r in results {
        if r.passed {
            summary.passed += 1;
        } else {
            summary.failed += 1;
        }
        summary.results.push(r);
    }
    summary
}

/// 并行执行用例：按索引分配任务到 worker 线程，结果按索引排序回填。
fn run_tasks_parallel(tasks: Vec<PendingTask>) -> Vec<TestCaseResult> {
    let n = tasks.len();
    if n == 0 {
        return Vec::new();
    }
    let workers = parallel_workers().min(n);
    if workers <= 1 {
        return tasks.iter().map(run_one).collect();
    }

    let next = AtomicUsize::new(0);
    let tasks = std::sync::Arc::new(tasks);
    let (tx, rx) = mpsc::channel::<(usize, TestCaseResult)>();

    std::thread::scope(|scope| {
        for _ in 0..workers {
            let next = &next;
            let tasks = tasks.clone();
            let tx = tx.clone();
            // 工作线程显式放大栈（64MB）：run-pass 并行编译 LLVM 时默认 2MB
            // 线程栈会溢出 abort（P7d-1 验证期发现），故用 Builder 指定栈大小。
            std::thread::Builder::new()
                .stack_size(64 * 1024 * 1024)
                .spawn_scoped(scope, move || {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(task) = tasks.get(i) else {
                            break; // 任务取尽
                        };
                        let result = run_one(task);
                        // 发送失败（接收端已 drop）即退出
                        if tx.send((i, result)).is_err() {
                            break;
                        }
                    }
                })
                .unwrap();
        }
        drop(tx); // 关闭发送端，使主线程循环终止
        let mut results: Vec<Option<TestCaseResult>> = (0..n).map(|_| None).collect();
        for (i, r) in rx {
            results[i] = Some(r);
        }
        // 理论上全部回填；防御性取未回填项为失败占位
        results
            .into_iter()
            .map(|r| {
                r.unwrap_or_else(|| TestCaseResult {
                    kind: TestKind::CompilePass,
                    name: "<unknown>".to_string(),
                    passed: false,
                    detail: "用例执行未返回结果".to_string(),
                })
            })
            .collect()
    })
}

/// 执行单个用例。
/// 检查文件头是否含 `// skip:` 注释（已知问题标记，跳过该用例）。
fn file_has_skip_marker(file: &Path) -> bool {
    std::fs::read_to_string(file)
        .map(|s| {
            s.lines()
                .take(20) // 只扫描文件头
                .any(|l| l.trim_start().starts_with("// skip:"))
        })
        .unwrap_or(false)
}

fn run_one(task: &PendingTask) -> TestCaseResult {
    match task.kind {
        TestKind::CompilePass => run_compile_pass(&task.file, &task.rel),
        TestKind::CompileFail => run_compile_fail(&task.file, &task.rel),
        TestKind::RunPass => run_run_pass(&task.file, &task.rel),
    }
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
