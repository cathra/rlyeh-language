//! zeta-fmt 命令行入口。
//!
//! ```text
//! zeta-fmt [--check] [-w|--write] [--indent N] <file>
//! ```
//!
//! - 无选项：格式化结果打印到 stdout。
//! - `--check`：仅检查文件是否已格式化（已格式化 exit 0，否则 exit 1）。
//! - `-w`/`--write`：将格式化结果写回文件。
//! - `--indent N`：缩进宽度（默认 4）。

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut file: Option<String> = None;
    let mut check = false;
    let mut write = false;
    let mut indent = 4;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--check" => check = true,
            "-w" | "--write" => write = true,
            "--indent" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("zeta-fmt: --indent requires a value");
                    return ExitCode::from(2);
                }
                match args[i].parse::<usize>() {
                    Ok(n) if n > 0 && n <= 16 => indent = n,
                    _ => {
                        eprintln!("zeta-fmt: invalid indent width '{}'", args[i]);
                        return ExitCode::from(2);
                    }
                }
            }
            s if s.starts_with('-') => {
                eprintln!("zeta-fmt: unknown flag '{}'", s);
                return ExitCode::from(2);
            }
            s => {
                if file.is_some() {
                    eprintln!("zeta-fmt: multiple input files not supported");
                    return ExitCode::from(2);
                }
                file = Some(s.to_string());
            }
        }
        i += 1;
    }

    let Some(file) = file else {
        eprintln!("usage: zeta-fmt [--check] [-w|--write] [--indent N] <file>");
        return ExitCode::from(2);
    };

    let src = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("zeta-fmt: cannot read {}: {}", file, e);
            return ExitCode::from(1);
        }
    };

    let opts = zeta_fmt::FmtOptions { indent_width: indent };
    let formatted = match zeta_fmt::format_source_with_options(&src, &opts) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("zeta-fmt: {}: {}", file, e);
            return ExitCode::from(1);
        }
    };

    if check {
        if formatted == src {
            println!("{}: formatted", file);
            ExitCode::SUCCESS
        } else {
            eprintln!("{}: needs formatting", file);
            ExitCode::from(1)
        }
    } else if write {
        match std::fs::write(&file, &formatted) {
            Ok(()) => {
                println!("{}: formatted", file);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("zeta-fmt: cannot write {}: {}", file, e);
                ExitCode::from(1)
            }
        }
    } else {
        print!("{}", formatted);
        ExitCode::SUCCESS
    }
}
