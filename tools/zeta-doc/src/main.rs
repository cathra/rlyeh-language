//! zeta-doc CLI：从 Zeta 源码生成 Markdown 文档。
//!
//! 用法:
//!   zeta-doc <file.zeta> [--out <file.md>] [--title <标题>]
//!
//! 缺省输出到标准输出；`--out` 指定输出文件。

use std::path::PathBuf;
use std::process::ExitCode;

use zeta_doc::{DocOptions, doc_source};

const USAGE: &str = "\
用法: zeta-doc <file.zeta> [选项]

选项:
  --out <file.md>    输出到文件（缺省为标准输出）
  --title <标题>     文档标题（缺省为「Zeta 文档」）
  -h, --help         显示帮助";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut file: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut title: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--out" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out = Some(PathBuf::from(v)),
                    None => {
                        eprintln!("zeta-doc: --out 缺少参数");
                        eprintln!("{USAGE}");
                        return ExitCode::from(2);
                    }
                }
            }
            "--title" => {
                i += 1;
                match args.get(i) {
                    Some(v) => title = Some(v.clone()),
                    None => {
                        eprintln!("zeta-doc: --title 缺少参数");
                        eprintln!("{USAGE}");
                        return ExitCode::from(2);
                    }
                }
            }
            s if s.starts_with('-') => {
                eprintln!("zeta-doc: 未知选项 `{s}`");
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
            s => {
                if file.is_some() {
                    eprintln!("zeta-doc: 只能指定一个源文件（`{s}` 多余）");
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
                file = Some(PathBuf::from(s));
            }
        }
        i += 1;
    }

    let file = match file {
        Some(f) => f,
        None => {
            eprintln!("zeta-doc: 缺少源文件");
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    let source = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("zeta-doc: 无法读取 {}: {e}", file.display());
            return ExitCode::FAILURE;
        }
    };

    let options = DocOptions {
        title: title.or_else(|| {
            file.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| format!("{s} — Zeta 文档"))
        }),
    };

    let doc = match doc_source(&source, &options) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("zeta-doc: 文档生成失败: {e}");
            return ExitCode::FAILURE;
        }
    };

    match &out {
        Some(p) => {
            if let Err(e) = std::fs::write(p, &doc) {
                eprintln!("zeta-doc: 无法写入 {}: {e}", p.display());
                return ExitCode::FAILURE;
            }
            println!("已生成: {}", p.display());
        }
        None => print!("{doc}"),
    }
    ExitCode::SUCCESS
}
