//! # zeta-lsp
//!
//! Zeta 语言服务器（Language Server Protocol，MVP）。
//!
//! 在 stdin/stdout 上以 JSON-RPC 2.0（Content-Length 帧）与客户端通信：
//!
//! - `initialize` / `shutdown` / `exit`：生命周期
//! - `textDocument/didOpen|didChange|didClose`：full 文本同步
//! - `textDocument/publishDiagnostics`：复用 `zeta-check` 静态分析推送诊断
//!   （语法错误 + 未使用变量 / 恒常条件 / 冗余比较 / 不可达代码）
//!
//! 命令行启动：`zeta-lsp`（或 `zeta lsp`）。

pub mod jsonrpc;
pub mod protocol;
pub mod server;

use std::io;

/// 在 stdin/stdout 上运行 LSP 服务器事件循环，直至收到 `exit` 或流关闭。
pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    let mut server = server::Server::new();

    while let Some(body) = jsonrpc::read_frame(&mut reader)? {
        let Ok(msg) = serde_json::from_slice::<serde_json::Value>(&body) else {
            // 非法 JSON：跳过（LSP 要求忽略坏帧而非终止）
            continue;
        };
        for out in server.handle(&msg) {
            let bytes = serde_json::to_vec(&out)?;
            jsonrpc::write_frame(&mut writer, &bytes)?;
        }
        if server.should_exit() {
            break;
        }
    }
    Ok(())
}
