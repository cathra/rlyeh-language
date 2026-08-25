//! `rlyeh-lsp` 可执行入口：在 stdin/stdout 上运行 LSP 服务器。

use std::process::ExitCode;

fn main() -> ExitCode {
    match rlyeh_lsp::run_stdio() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rlyeh-lsp: {e}");
            ExitCode::FAILURE
        }
    }
}
