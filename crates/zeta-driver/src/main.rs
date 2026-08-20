//! Zeta 编译器 CLI（MVP）。
//!
//! ```text
//! zeta run <file.zeta>                          # 编译并运行（增量缓存）
//! zeta build <file.zeta> [-o <out>]             # 编译为可执行文件（增量缓存）
//! zeta run|build <file> --force                 # 忽略缓存，强制全量编译
//! zeta run|build <file> --cache-dir <dir>       # 指定缓存根目录（默认源文件所在目录）
//! zeta run|build <file> --no-std                # 不注入标准库预置（core.zeta）
//! zeta run|build <file> --verbose               # 打印缓存命中/未命中与统计
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use zeta_driver::error::DriverError;
use zeta_driver::IncrementalDriver;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("run") => {
            let Some(file) = args.get(2) else {
                eprintln!("用法: zeta run <file.zeta> [--cache-dir <dir>] [--force] [--verbose]");
                return ExitCode::from(2);
            };
            let opts = match CliOpts::parse(&args[3..], file) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(2);
                }
            };
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
                eprintln!("用法: zeta build <file.zeta> [-o <out>] [--cache-dir <dir>] [--force] [--verbose]");
                return ExitCode::from(2);
            };
            let mut opts = match CliOpts::parse(&args[3..], file) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(2);
                }
            };
            let out = opts.take_out().unwrap_or_else(|| PathBuf::from("zeta-out"));
            match build_file(file, &out, &opts) {
                Ok(()) => {
                    println!("编译完成: {}", out.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("错误: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("--version") | Some("-V") => {
            println!("zeta {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "Zeta 编译器（MVP）\n\
                 用法:\n  \
                 zeta run <file.zeta> [--cache-dir <dir>] [--force] [--no-std] [--verbose] 编译并运行\n  \
                 zeta build <file.zeta> [-o <out>] [--cache-dir <dir>] [--force] [--no-std] [--verbose] 编译为可执行文件\n  \
                 zeta --version 版本信息"
            );
            ExitCode::from(2)
        }
    }
}

/// CLI 选项（缓存目录 / 强制全量 / 禁用标准库 / 详细输出 / 自定义产物路径）。
struct CliOpts {
    cache_dir: PathBuf,
    force: bool,
    no_std: bool,
    verbose: bool,
    out: Option<PathBuf>,
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

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--cache-dir" => {
                    i += 1;
                    let dir = args.get(i).ok_or("--cache-dir 需要目录参数")?;
                    cache_dir = PathBuf::from(dir);
                }
                "--force" => force = true,
                "--no-std" => no_std = true,
                "--verbose" => verbose = true,
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
        })
    }

    /// 取出自定义产物路径（build 专用）。
    fn take_out(&mut self) -> Option<PathBuf> {
        self.out.take()
    }
}

/// 增量编译运行入口文件（自动加载 `mod foo;` 外部模块）。
fn run_file(path: &str, opts: &CliOpts) -> Result<String, DriverError> {
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
    IncrementalDriver::new(opts.cache_dir.clone())
        .with_force(opts.force)
        .with_no_std(opts.no_std)
}

/// 打印缓存命中/未命中与统计（`--verbose`）。
fn report(outcome: &zeta_driver::BuildOutcome) {
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
