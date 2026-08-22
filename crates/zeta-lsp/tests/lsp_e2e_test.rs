//! `zeta-lsp` 端到端测试：spawn 真实二进制，走 stdio Content-Length 帧协议。

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};

use serde_json::Value;

/// 已启动的 LSP 服务器子进程。
struct LspProcess {
    child: Child,
}

impl LspProcess {
    fn spawn() -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_zeta-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn zeta-lsp");
        Self { child }
    }

    fn send(&mut self, body: &Value) {
        let bytes = serde_json::to_vec(body).unwrap();
        let stdin = self.child.stdin.as_mut().unwrap();
        write!(stdin, "Content-Length: {}\r\n\r\n", bytes.len()).unwrap();
        stdin.write_all(&bytes).unwrap();
        stdin.flush().unwrap();
    }

    fn recv(&mut self) -> Value {
        let stdout = self.child.stdout.as_mut().unwrap();
        let body = read_frame(stdout).expect("read frame");
        serde_json::from_slice(&body).expect("parse json")
    }

    fn wait(mut self) -> std::process::ExitStatus {
        // 关闭 stdin 使服务器看到流关闭
        drop(self.child.stdin.take());
        self.child.wait().expect("wait")
    }
}

/// 从 reader 读一帧（Content-Length 头 + body）。
fn read_frame(reader: &mut impl Read) -> Option<Vec<u8>> {
    let mut headers = Vec::new();
    let mut buf = [0u8; 1];
    loop {
        let n = reader.read(&mut buf).unwrap();
        if n == 0 {
            return None;
        }
        headers.push(buf[0]);
        if headers.len() >= 4 && &headers[headers.len() - 4..] == b"\r\n\r\n" {
            break;
        }
    }
    let head = String::from_utf8_lossy(&headers);
    let len = head
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim().eq_ignore_ascii_case("Content-Length")).then(|| v.trim().parse::<usize>().ok())?
        })
        .unwrap();
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).unwrap();
    Some(body)
}

#[test]
fn full_lifecycle_with_diagnostics() {
    let mut server = LspProcess::spawn();

    // 1. initialize
    server.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));
    let resp = server.recv();
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["capabilities"]["textDocumentSync"], 1);
    assert!(resp.get("error").is_none());

    // 2. initialized 通知
    server.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    }));

    // 3. didOpen：含未使用变量 → 应推送 warning 诊断
    server.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///tmp/e2e.zeta",
                "languageId": "zeta",
                "version": 1,
                "text": "fn main() {\n    let unused = 42;\n    println(1);\n}\n"
            }
        }
    }));
    let notif = server.recv();
    assert_eq!(notif["method"], "textDocument/publishDiagnostics");
    let diags = notif["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0]["severity"], 2);
    assert_eq!(diags[0]["code"], "unused-variable");
    assert_eq!(diags[0]["range"]["start"]["line"], 1);

    // 4. didChange：语法错误 → error 诊断
    server.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": "file:///tmp/e2e.zeta", "version": 2 },
            "contentChanges": [{ "text": "fn main( {" }]
        }
    }));
    let notif = server.recv();
    let diags = notif["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0]["severity"], 1);
    assert_eq!(diags[0]["code"], "parse-error");

    // 5. didClose：诊断清空
    server.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": { "textDocument": { "uri": "file:///tmp/e2e.zeta" } }
    }));
    let notif = server.recv();
    assert_eq!(
        notif["params"]["diagnostics"].as_array().unwrap().len(),
        0
    );

    // 6. shutdown + exit：进程正常退出
    server.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "shutdown"
    }));
    let resp = server.recv();
    assert_eq!(resp["id"], 2);
    assert!(resp.get("error").is_none());

    server.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "exit"
    }));
    let status = server.wait();
    assert!(status.success());
}

#[test]
fn unknown_request_gets_method_not_found() {
    let mut server = LspProcess::spawn();
    server.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "textDocument/hover",
        "params": {}
    }));
    let resp = server.recv();
    assert_eq!(resp["id"], 9);
    assert_eq!(resp["error"]["code"], -32601);

    server.send(&serde_json::json!({ "jsonrpc": "2.0", "method": "exit" }));
    let status = server.wait();
    assert!(status.success());
}
