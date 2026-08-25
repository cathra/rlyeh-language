//! rlyeh-check 命令行入口。
//!
//! ```text
//! rlyeh-check <file>
//! ```
//!
//! 打印所有诊断（`line:col: level[rule]: message`）；
//! 存在 error 或 warning 时退出码为 1，否则为 0。

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: rlyeh-check <file>");
        return ExitCode::from(2);
    }
    let file = &args[1];

    let src = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rlyeh-check: cannot read {}: {}", file, e);
            return ExitCode::from(1);
        }
    };

    let diags = rlyeh_check::check_source(&src);
    if diags.is_empty() {
        println!("{}: ok", file);
        return ExitCode::SUCCESS;
    }
    let mut has_error = false;
    for d in &diags {
        eprintln!("{}: {}", file, d.render());
        if d.level == rlyeh_check::Level::Error {
            has_error = true;
        }
    }
    let summary = if has_error {
        "error"
    } else {
        "warning"
    };
    eprintln!("{}: {} diagnostics ({})", file, diags.len(), summary);
    ExitCode::from(1)
}
