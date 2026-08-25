//! zeta-bench CLI：编译 Zeta 源码并基准计时。
//!
//! 用法:
//!   zeta-bench <file.zeta | 可执行文件> [选项]
//!
//! 传入 `.zeta` 源文件时，先调用 `zeta build`（复用当前二进制或 PATH 中的
//! `zeta`）编译到临时目录，再执行计时；传入可执行文件时直接计时。

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use zeta_bench::{BenchOptions, bench_executable, bench_source};

const USAGE: &str = "\
用法: zeta-bench <file.zeta | 可执行文件> [选项]

选项:
  --runs <N>      测量轮数（缺省 10）
  --warmup <N>    预热轮数，不计入统计（缺省 2）
  --out <路径>    产物输出路径（仅源码模式，缺省临时目录）
  --quiet         丢弃被测程序的标准输出/错误
  -h, --help      显示帮助";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut target: Option<PathBuf> = None;
    let mut runs = 10usize;
    let mut warmup = 2usize;
    let mut out: Option<PathBuf> = None;
    let mut quiet = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--runs" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(v) if v > 0 => runs = v,
                    _ => {
                        eprintln!("zeta-bench: --runs 需要正整数");
                        return ExitCode::from(2);
                    }
                }
            }
            "--warmup" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(v) => warmup = v,
                    _ => {
                        eprintln!("zeta-bench: --warmup 需要非负整数");
                        return ExitCode::from(2);
                    }
                }
            }
            "--out" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out = Some(PathBuf::from(v)),
                    None => {
                        eprintln!("zeta-bench: --out 缺少参数");
                        return ExitCode::from(2);
                    }
                }
            }
            "--quiet" => quiet = true,
            s if s.starts_with('-') => {
                eprintln!("zeta-bench: 未知选项 `{s}`");
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
            s => {
                if target.is_some() {
                    eprintln!("zeta-bench: 只能指定一个目标（`{s}` 多余）");
                    return ExitCode::from(2);
                }
                target = Some(PathBuf::from(s));
            }
        }
        i += 1;
    }

    let target = match target {
        Some(t) => t,
        None => {
            eprintln!("zeta-bench: 缺少目标文件");
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    let opts = BenchOptions {
        warmup,
        runs,
        quiet,
    };

    let report = if target.extension().map(|e| e == "zeta").unwrap_or(false) {
        let out_path = out.unwrap_or_else(|| {
            let mut p = std::env::temp_dir();
            p.push(format!("zeta-bench-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)));
            p
        });
        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        println!("编译: {} → {}", target.display(), out_path.display());
        bench_source(&target, &out_path, &opts, compile_with_zeta)
    } else {
        bench_executable(&target, &opts)
    };

    match report {
        Ok(r) => {
            println!("基准: {}", target.display());
            println!("{r}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("zeta-bench: {e}");
            ExitCode::FAILURE
        }
    }
}

/// 通过 `zeta build` 子命令编译源码。
///
/// 优先复用当前二进制（当它本身是 `zeta` 时），否则调用 PATH 中的 `zeta`。
fn compile_with_zeta(src: &Path, out: &Path) -> Result<(), String> {
    let zeta = find_zeta();
    let status = Command::new(&zeta)
        .args(["build", src.to_str().unwrap_or_default(), "-o", out.to_str().unwrap_or_default()])
        .status()
        .map_err(|e| format!("无法启动 `{zeta}`: {e}（`zeta build` 是否可用？）"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "`{zeta} build` 失败（退出码 {:?}）",
            status.code()
        ))
    }
}

/// 定位 `zeta` 可执行文件。
fn find_zeta() -> String {
    if let Ok(cur) = std::env::current_exe() {
        if let Some(name) = cur.file_name().and_then(|n| n.to_str()) {
            if name == "zeta" {
                return name.to_string();
            }
        }
    }
    "zeta".to_string()
}
