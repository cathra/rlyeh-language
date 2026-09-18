//! Rlyeh 编译器 CLI（MVP）。
//!
//! ```text
//! rlyeh run <file.rl>                          # 编译并运行（增量缓存）
//! rlyeh build <file.rl> [-o <out>]             # 编译为可执行文件（增量缓存）
//! rlyeh test [<tests-dir>]                       # 运行 tests/ 目录用例（默认 ./tests）
//! rlyeh run|build <file> --force                 # 忽略缓存，强制全量编译
//! rlyeh run|build <file> --cache-dir <dir>       # 指定缓存根目录（默认源文件所在目录）
//! rlyeh run|build <file> --no-std                # 不注入标准库预置（标准库预置）
//! rlyeh run|build <file> --verbose               # 打印缓存命中/未命中与统计
//! rlyeh build <file> --target <triple>           # 交叉编译（如 arm64-apple-macosx / x86_64-apple-macosx）
//! rlyeh run|build <file> --emit <ir|ast|hir|ast-user|hir-user> [-o <out>]  # 仅导出中间表示文本（LLVM IR / AST / HIR），不编译运行
//! ```
//!
//! `rlyeh test` 扫描 `<tests-dir>/compile-pass|compile-fail|run-pass` 三个子目录：
//! compile-pass 要求编译成功；compile-fail 要求编译失败（源内 `// expect:` 注释断言
//! 错误消息片段）；run-pass 要求编译运行成功（同名 `.out` 文件作为期望输出对比）。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rlyeh_driver::error::DriverError;
use rlyeh_driver::IncrementalDriver;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("run") => {
            let Some(file) = args.get(2) else {
                eprintln!("用法: rlyeh run <file.rl> [--cache-dir <dir>] [--force] [--verbose]");
                return ExitCode::from(2);
            };
            let opts = match CliOpts::parse(&args[3..], file) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(2);
                }
            };
            if let Some(code) = handle_emit(file, &opts) {
                return code;
            }
            match run_file(file, &opts) {
                Ok(output) => {
                    print!("{output}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("错误: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("build") => {
            let Some(file) = args.get(2) else {
                eprintln!("用法: rlyeh build <file.rl> [-o <out>] [--cache-dir <dir>] [--force] [--verbose]");
                return ExitCode::from(2);
            };
            let mut opts = match CliOpts::parse(&args[3..], file) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(2);
                }
            };
            if let Some(code) = handle_emit(file, &opts) {
                return code;
            }
            let out = opts.take_out().unwrap_or_else(|| PathBuf::from("rlyeh-out"));
            let profile = opts.profile.take();
            match build_file(file, &out, &opts) {
                Ok(()) => {
                    match &opts.target {
                        Some(t) => println!("编译完成: {}（目标 {t}）", out.display()),
                        None => println!("编译完成: {}", out.display()),
                    }
                    // F2：PGO 数据回灌——构建期注入 `.rl_profile` 预测区域大小
                    if let Some(p) = profile {
                        match rlyeh_driver::region_profile_report(&p) {
                            Ok(report) => {
                                println!("\n区域大小预测（{}）:", p.display());
                                print!("{report}");
                            }
                            Err(e) => eprintln!(
                                "rlyeh: 警告: 忽略 --profile（{}）: {e}",
                                p.display()
                            ),
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("错误: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("test") => {
            let dir = args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("tests"));
            let summary = rlyeh_driver::test_runner::run_test_suite(&dir);
            for r in &summary.results {
                let mark = if r.passed { "通过" } else { "失败" };
                println!("[{mark}] {:<26} {}", r.name, r.detail.lines().next().unwrap_or(""));
                if !r.passed {
                    for line in r.detail.lines().skip(1) {
                        println!("       {line}");
                    }
                }
            }
            println!(
                "测试汇总: 共 {} 用例, 通过 {}, 失败 {}, 跳过 {}",
                summary.total,
                summary.passed,
                summary.failed,
                summary.skipped
            );
            if summary.failed > 0 {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Some("fmt") => run_fmt(&args[2..]),
        Some("doc") => run_doc(&args[2..]),
        Some("bench") => run_bench(&args[2..]),
        Some("profile") => run_profile(&args[2..]),
        Some("new") => run_new(&args[2..]),
        Some("publish") => run_publish(&args[2..]),
        Some("lsp") => match rlyeh_lsp::run_stdio() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("rlyeh lsp: {e}");
                ExitCode::FAILURE
            }
        },
        Some("check") => {
            let Some(file) = args.get(2) else {
                eprintln!("用法: rlyeh check <file.rl>");
                return ExitCode::from(2);
            };
            match rlyeh_driver::check_source_file(std::path::Path::new(file)) {
                Ok(diags) if diags.is_empty() => {
                    println!("{file}: ok");
                    ExitCode::SUCCESS
                }
                Ok(diags) => {
                    for d in &diags {
                        eprintln!("{file}: {}", d.render());
                    }
                    eprintln!("{file}: {} 条诊断", diags.len());
                    ExitCode::FAILURE
                }
                Err(e) => {
                    eprintln!("rlyeh check: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("--version") | Some("-V") => {
            println!("rlyeh {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "Rlyeh 编译器（MVP）\n\
                 用法:\n  \
                 rlyeh run <file.rl> [--cache-dir <dir>] [--force] [--no-std] [--verbose] 编译并运行\n  \
                 rlyeh build <file.rl> [-o <out>] [--cache-dir <dir>] [--force] [--no-std] [--verbose] [--target <triple>] 编译为可执行文件（--target 交叉编译 / wasm32-wasi 生成 .wasm）\n  \
                 rlyeh run|build <file> --emit <ir|ast|hir|ast-user|hir-user> [-o <out>] 仅导出中间表示文本（LLVM IR / AST / HIR），不编译运行\n  \
                 rlyeh test [<tests-dir>] 运行 tests/ 目录用例（compile-pass/compile-fail/run-pass）\n  \
                 rlyeh fmt <file.rl> [--check] [-w|--write] [--indent N] 格式化代码（默认输出到 stdout）\n  \
                 rlyeh check <file.rl> 静态分析（未使用变量/恒常条件/冗余比较/不可达代码）\n  \
                 rlyeh doc <file.rl> [--out <file.md>] [--title <标题>] 提取 /// 注释生成 Markdown 文档\n  \
                 rlyeh bench <file.rl> [-o <out>] [--runs N] [--warmup N] 编译并基准计时\n  \
                 rlyeh new <name> [--lib] 创建新项目脚手架（Rlyeh.toml + src/main.rl 或 lib.rl）\n  \
                 rlyeh publish [--registry <URL>] [--verbose] 打包发布到 dagon 注册表\n  \
                 rlyeh lsp 启动语言服务器（LSP over stdio，诊断推送）\n  \
                 rlyeh profile <file.rl_profile> [--out <report.md>] PGO 画像 → 区域大小预测报告\n  \
                 rlyeh --version 版本信息"
            );
            ExitCode::from(2)
        }
    }
}

/// CLI 选项（缓存目录 / 强制全量 / 禁用标准库 / 详细输出 / 自定义产物路径 / 交叉编译目标）。
/// `--emit` 目标：导出中间表示文本而非编译运行。
enum EmitTarget {
    Ir,
    Ast,
    Hir,
    AstUser,
    HirUser,
}

impl EmitTarget {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "ir" => Ok(EmitTarget::Ir),
            "ast" => Ok(EmitTarget::Ast),
            "hir" => Ok(EmitTarget::Hir),
            "ast-user" => Ok(EmitTarget::AstUser),
            "hir-user" => Ok(EmitTarget::HirUser),
            other => Err(format!("未知 --emit 目标: {other}（支持 ir/ast/hir/ast-user/hir-user）")),
        }
    }
    fn name(&self) -> &'static str {
        match self {
            EmitTarget::Ir => "LLVM IR",
            EmitTarget::Ast => "AST",
            EmitTarget::AstUser => "AST (user-only)",
            EmitTarget::Hir => "HIR",
            EmitTarget::HirUser => "HIR (user-only)",
        }
    }
}

struct CliOpts {
    cache_dir: PathBuf,
    force: bool,
    no_std: bool,
    verbose: bool,
    out: Option<PathBuf>,
    target: Option<String>,
    profile: Option<PathBuf>,
    emit: Option<EmitTarget>,
}

impl CliOpts {
    /// 解析选项。`file` 用于推导默认缓存目录（源文件所在目录）。
    fn parse(args: &[String], file: &str) -> Result<Self, String> {
        let mut cache_dir = Path::new(file)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut force = false;
        let mut no_std = false;
        let mut verbose = false;
        let mut out = None;
        let mut target = None;
        let mut profile = None;
        let mut emit = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--emit" => {
                    i += 1;
                    let e = args.get(i).ok_or("--emit 需要目标（ir/ast/hir）")?;
                    emit = Some(EmitTarget::parse(e)?);
                }
                "--profile" => {
                    i += 1;
                    let p = args.get(i).ok_or("--profile 需要 .rl_profile 路径")?;
                    profile = Some(PathBuf::from(p));
                }
                "--cache-dir" => {
                    i += 1;
                    let dir = args.get(i).ok_or("--cache-dir 需要目录参数")?;
                    cache_dir = PathBuf::from(dir);
                }
                "--force" => force = true,
                "--no-std" => no_std = true,
                "--verbose" => verbose = true,
                "--target" => {
                    i += 1;
                    let t = args.get(i).ok_or("--target 需要 LLVM 目标 triple（如 arm64-apple-macosx / wasm32-wasi）")?;
                    target = Some(t.clone());
                }
                "-o" => {
                    i += 1;
                    let o = args.get(i).ok_or("-o 需要路径参数")?;
                    out = Some(PathBuf::from(o));
                }
                other => return Err(format!("未知选项: {other}")),
            }
            i += 1;
        }
        Ok(Self {
            cache_dir,
            force,
            no_std,
            verbose,
            out,
            target,
            profile,
            emit,
        })
    }

    /// 取出自定义产物路径（build 专用）。
    fn take_out(&mut self) -> Option<PathBuf> {
        self.out.take()
    }
}

/// 按 `--emit` 目标导出中间表示文本（Ir/Ast/Hir 及其 user-only 变体）。
fn emit_file(path: &str, target: &EmitTarget) -> Result<String, DriverError> {
    match target {
        EmitTarget::Ir => rlyeh_driver::compile_file_to_llvm(Path::new(path)),
        EmitTarget::Ast => rlyeh_driver::emit_ast(Path::new(path)),
        EmitTarget::Hir => rlyeh_driver::emit_hir(Path::new(path)),
        EmitTarget::AstUser => rlyeh_driver::emit_ast_user(Path::new(path)),
        EmitTarget::HirUser => rlyeh_driver::emit_hir_user(Path::new(path)),
    }
}

/// 若指定 `--emit`，导出中间表示文本并直接返回退出码；否则返回 `None`（继续编译运行）。
fn handle_emit(file: &str, opts: &CliOpts) -> Option<ExitCode> {
    let target = opts.emit.as_ref()?;
    match emit_file(file, target) {
        Ok(out) => {
            if let Some(o) = &opts.out {
                if let Err(e) = std::fs::write(o, &out) {
                    eprintln!("错误: {e}");
                    return Some(ExitCode::FAILURE);
                }
                println!("已导出 {} 文本: {}", target.name(), o.display());
            } else {
                print!("{out}");
            }
            Some(ExitCode::SUCCESS)
        }
        Err(e) => {
            eprintln!("错误: {e}");
            Some(ExitCode::FAILURE)
        }
    }
}

/// 增量编译运行入口文件（自动加载 `mod foo;` 外部模块）。
fn run_file(path: &str, opts: &CliOpts) -> Result<String, DriverError> {
    if opts.target.is_some() {
        return Err(DriverError::Usage(
            "`rlyeh run` 不支持 --target（交叉编译产物无法在本机运行，请用 `rlyeh build --target ...`）"
                .to_string(),
        ));
    }
    let mut driver = new_driver(opts);
    let (stdout, outcome) = driver.run_source_file(Path::new(path))?;
    if opts.verbose {
        report(&outcome);
    }
    Ok(stdout)
}

/// 增量编译入口文件（含外部模块）为可执行文件。
fn build_file(path: &str, out: &Path, opts: &CliOpts) -> Result<(), DriverError> {
    let mut driver = new_driver(opts);
    let outcome = driver.build_executable_file(Path::new(path), out)?;
    if opts.verbose {
        report(&outcome);
    }
    Ok(())
}

/// 构建增量驱动。
fn new_driver(opts: &CliOpts) -> IncrementalDriver {
    // L3 PGO 回灌：`--profile` 提供区域推荐容量，注入 adaptive 区域初始大小
    let hints = opts
        .profile
        .as_ref()
        .and_then(|p| rlyeh_driver::region_hints_from_profile(p).ok())
        .unwrap_or_default();
    IncrementalDriver::new(opts.cache_dir.clone())
        .with_force(opts.force)
        .with_no_std(opts.no_std)
        .with_target(opts.target.clone())
        .with_region_hints(hints)
}

/// 打印缓存命中/未命中与统计（`--verbose`）。
fn report(outcome: &rlyeh_driver::BuildOutcome) {
    let stats = &outcome.stats;
    if outcome.cache_hit {
        println!("[缓存] 命中（跳过完整流水线）");
    } else {
        println!("[缓存] 未命中（执行完整流水线）");
    }
    println!(
        "[缓存] 统计: 命中 {} / 总 {}（命中率 {:.1}%）{}",
        stats.hits,
        stats.total(),
        stats.hit_rate() * 100.0,
        if stats.recovered > 0 {
            format!("（损坏恢复 {} 次）", stats.recovered)
        } else {
            String::new()
        }
    );
}

/// `rlyeh fmt`：格式化源码（默认 stdout；`--check` 检查是否已格式化；`-w` 写回）。
fn run_fmt(args: &[String]) -> ExitCode {
    let mut file: Option<String> = None;
    let mut check = false;
    let mut write = false;
    let mut indent = 4;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--check" => check = true,
            "-w" | "--write" => write = true,
            // PC-5 / PC-8：输出旧写法（`trait` / `impl Trait for Type`）；
            // 默认输出新语法（`protocol` / `impl Type: Trait`）。
            "--indent" => {
                i += 1;
                let Some(v) = args.get(i) else {
                    eprintln!("rlyeh fmt: --indent 需要数值参数");
                    return ExitCode::from(2);
                };
                match v.parse::<usize>() {
                    Ok(n) if n > 0 && n <= 16 => indent = n,
                    _ => {
                        eprintln!("rlyeh fmt: 非法缩进宽度 '{v}'");
                        return ExitCode::from(2);
                    }
                }
            }
            s if s.starts_with('-') => {
                eprintln!("rlyeh fmt: 未知选项 '{s}'");
                return ExitCode::from(2);
            }
            s => {
                if file.is_some() {
                    eprintln!("rlyeh fmt: 仅支持单个输入文件");
                    return ExitCode::from(2);
                }
                file = Some(s.to_string());
            }
        }
        i += 1;
    }
    let Some(file) = file else {
        eprintln!("用法: rlyeh fmt <file.rl> [--check] [-w|--write] [--indent N]");
        return ExitCode::from(2);
    };

    let src = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rlyeh fmt: 无法读取 {file}: {e}");
            return ExitCode::from(1);
        }
    };
    let opts = rlyeh_fmt::FmtOptions {
        indent_width: indent,
    };
    let formatted = match rlyeh_fmt::format_source_with_options(&src, &opts) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rlyeh fmt: {file}: {e}");
            return ExitCode::from(1);
        }
    };

    if check {
        if formatted == src {
            println!("{file}: 已格式化");
            ExitCode::SUCCESS
        } else {
            eprintln!("{file}: 需要格式化");
            ExitCode::from(1)
        }
    } else if write {
        match std::fs::write(&file, &formatted) {
            Ok(()) => {
                println!("{file}: 已格式化");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("rlyeh fmt: 无法写入 {file}: {e}");
                ExitCode::from(1)
            }
        }
    } else {
        print!("{formatted}");
        ExitCode::SUCCESS
    }
}

/// `rlyeh doc`：提取 `///` 文档注释生成 Markdown（默认 stdout，`--out` 写文件）。
fn run_doc(args: &[String]) -> ExitCode {
    let mut file: Option<String> = None;
    let mut out: Option<PathBuf> = None;
    let mut title: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out = Some(PathBuf::from(v)),
                    None => {
                        eprintln!("rlyeh doc: --out 缺少参数");
                        return ExitCode::from(2);
                    }
                }
            }
            "--title" => {
                i += 1;
                match args.get(i) {
                    Some(v) => title = Some(v.clone()),
                    None => {
                        eprintln!("rlyeh doc: --title 缺少参数");
                        return ExitCode::from(2);
                    }
                }
            }
            s if s.starts_with('-') => {
                eprintln!("rlyeh doc: 未知选项 '{s}'");
                return ExitCode::from(2);
            }
            s => {
                if file.is_some() {
                    eprintln!("rlyeh doc: 仅支持单个输入文件");
                    return ExitCode::from(2);
                }
                file = Some(s.to_string());
            }
        }
        i += 1;
    }
    let Some(file) = file else {
        eprintln!("用法: rlyeh doc <file.rl> [--out <file.md>] [--title <标题>]");
        return ExitCode::from(2);
    };

    let options = rlyeh_doc::DocOptions {
        title: title.or_else(|| {
            Path::new(&file)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| format!("{s} — Rlyeh 文档"))
        }),
    };
    let doc = match rlyeh_driver::doc_source_file(Path::new(&file), &options) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("rlyeh doc: {e}");
            return ExitCode::from(1);
        }
    };

    match &out {
        Some(p) => {
            if let Err(e) = std::fs::write(p, &doc) {
                eprintln!("rlyeh doc: 无法写入 {}: {e}", p.display());
                return ExitCode::from(1);
            }
            println!("已生成: {}", p.display());
            ExitCode::SUCCESS
        }
        None => {
            print!("{doc}");
            ExitCode::SUCCESS
        }
    }
}

/// `rlyeh profile`：读取 PGO 画像（`.rl_profile`）生成区域大小预测报告（阶段 F2 数据回灌）。
///
/// 用法: `rlyeh profile <file.rl_profile> [--out <report.md>]`
/// 默认输出到 stdout；`--out` 写文件。
fn run_profile(args: &[String]) -> ExitCode {
    let mut file: Option<String> = None;
    let mut out: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out = Some(PathBuf::from(v)),
                    None => {
                        eprintln!("rlyeh profile: --out 缺少参数");
                        return ExitCode::from(2);
                    }
                }
            }
            s if s.starts_with('-') => {
                eprintln!("rlyeh profile: 未知选项 '{s}'");
                return ExitCode::from(2);
            }
            s => {
                if file.is_some() {
                    eprintln!("rlyeh profile: 仅支持单个画像文件");
                    return ExitCode::from(2);
                }
                file = Some(s.to_string());
            }
        }
        i += 1;
    }
    let Some(file) = file else {
        eprintln!("用法: rlyeh profile <file.rl_profile> [--out <report.md>]");
        return ExitCode::from(2);
    };

    let report = match rlyeh_driver::region_profile_report(Path::new(&file)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("rlyeh profile: {e}");
            return ExitCode::from(1);
        }
    };

    match &out {
        Some(p) => {
            if let Err(e) = std::fs::write(p, &report) {
                eprintln!("rlyeh profile: 无法写入 {}: {e}", p.display());
                return ExitCode::from(1);
            }
            println!("已生成: {}", p.display());
            ExitCode::SUCCESS
        }
        None => {
            print!("{report}");
            ExitCode::SUCCESS
        }
    }
}

/// `rlyeh bench`：编译源码并基准计时（`--runs`/`--warmup` 控制轮数）。
fn run_bench(args: &[String]) -> ExitCode {
    let mut file: Option<String> = None;
    let mut out: Option<PathBuf> = None;
    let mut runs = 10usize;
    let mut warmup = 2usize;
    let mut force = false;
    let mut cache_dir: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out = Some(PathBuf::from(v)),
                    None => {
                        eprintln!("rlyeh bench: -o 缺少参数");
                        return ExitCode::from(2);
                    }
                }
            }
            "--runs" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(v) if v > 0 => runs = v,
                    _ => {
                        eprintln!("rlyeh bench: --runs 需要正整数");
                        return ExitCode::from(2);
                    }
                }
            }
            "--warmup" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(v) => warmup = v,
                    _ => {
                        eprintln!("rlyeh bench: --warmup 需要非负整数");
                        return ExitCode::from(2);
                    }
                }
            }
            "--cache-dir" => {
                i += 1;
                match args.get(i) {
                    Some(v) => cache_dir = Some(PathBuf::from(v)),
                    None => {
                        eprintln!("rlyeh bench: --cache-dir 缺少参数");
                        return ExitCode::from(2);
                    }
                }
            }
            "--force" => force = true,
            s if s.starts_with('-') => {
                eprintln!("rlyeh bench: 未知选项 '{s}'");
                return ExitCode::from(2);
            }
            s => {
                if file.is_some() {
                    eprintln!("rlyeh bench: 仅支持单个输入文件");
                    return ExitCode::from(2);
                }
                file = Some(s.to_string());
            }
        }
        i += 1;
    }
    let Some(file) = file else {
        eprintln!("用法: rlyeh bench <file.rl> [-o <out>] [--runs N] [--warmup N]");
        return ExitCode::from(2);
    };

    let out = out.unwrap_or_else(|| PathBuf::from("rlyeh-out"));
    let cache_dir = cache_dir.unwrap_or_else(|| {
        Path::new(&file)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    });
    let opts = CliOpts {
        cache_dir,
        force,
        no_std: false,
        verbose: false,
        out: None,
        target: None,
        profile: None,
        emit: None,
    };

    if let Err(e) = build_file(&file, &out, &opts) {
        eprintln!("rlyeh bench: 编译失败: {e}");
        return ExitCode::from(1);
    }

    let bench_opts = rlyeh_bench::BenchOptions {
        warmup,
        runs,
        quiet: false,
    };
    match rlyeh_bench::bench_executable(&out, &bench_opts) {
        Ok(report) => {
            println!("基准: {file}");
            println!("产物: {}", out.display());
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("rlyeh bench: {e}");
            ExitCode::from(1)
        }
    }
}

/// `rlyeh new`：创建新项目脚手架（委托 dagon 的 `cmd_new`）。
///
/// 用法: `rlyeh new <name> [--lib]`
/// 生成 `Rlyeh.toml` 清单 + `src/main.rl`（可执行项目）或 `src/lib.rl`（库项目，
/// `--lib`）。目录已存在或包名非法时报错。
fn run_new(args: &[String]) -> ExitCode {
    let mut name: Option<String> = None;
    let mut lib = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--lib" => lib = true,
            s if s.starts_with('-') => {
                eprintln!("rlyeh new: 未知选项 '{s}'");
                return ExitCode::from(2);
            }
            s => {
                if name.is_some() {
                    eprintln!("rlyeh new: 仅支持单个项目名");
                    return ExitCode::from(2);
                }
                name = Some(s.to_string());
            }
        }
        i += 1;
    }
    let Some(name) = name else {
        eprintln!("用法: rlyeh new <name> [--lib]");
        return ExitCode::from(2);
    };
    let ctx = dagon::commands::Ctx::new(false);
    match dagon::commands::cmd_new(&ctx, &name, lib) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("错误: {e}");
            ExitCode::from(1)
        }
    }
}

/// `rlyeh publish`：将当前目录项目打包发布到 dagon 注册表。
///
/// 用法: `rlyeh publish [--registry <URL>] [--verbose]`
/// 发布前置检查（Rlyeh.toml 版本号、重复版本拦截）由 dagon 完成。
fn run_publish(args: &[String]) -> ExitCode {
    let mut verbose = false;
    let mut registry: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--registry" => {
                i += 1;
                match args.get(i) {
                    Some(v) => registry = Some(v.clone()),
                    None => {
                        eprintln!("rlyeh publish: --registry 缺少参数");
                        return ExitCode::from(2);
                    }
                }
            }
            "--verbose" => verbose = true,
            other => {
                eprintln!("rlyeh publish: 未知参数: {other}");
                eprintln!("用法: rlyeh publish [--registry <URL>] [--verbose]");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }
    let mut ctx = dagon::commands::Ctx::new(verbose);
    ctx.registry = registry;
    match dagon::commands::cmd_publish(&ctx, None) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("错误: {e}");
            ExitCode::from(1)
        }
    }
}
