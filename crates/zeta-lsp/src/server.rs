//! Zeta 语言服务器核心。
//!
//! 状态机：维护 uri → 文档文本 的映射，收到文档同步通知后复用
//! `zeta-check` 静态分析并推送诊断（`textDocument/publishDiagnostics`）。
//!
//! `handle` 为纯函数式入口（入 JSON-RPC 消息，出待写回的响应/通知），
//! 便于单元测试与 stdio 事件循环复用。

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::protocol::{
    pos, Diagnostic, InitializeResult, PublishDiagnosticsParams, Range, ServerCapabilities,
    severity,
};

/// 服务器状态。
#[derive(Debug, Default)]
pub struct Server {
    /// uri → 文档全文（full 同步）。
    documents: HashMap<String, String>,
    /// `shutdown` 请求后置位；收到 `exit` 通知时退出。
    shutdown_requested: bool,
    /// `exit` 通知已收到。
    exit_requested: bool,
}

impl Server {
    /// 新建服务器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 处理一条 JSON-RPC 消息，返回待写回的输出消息（响应 / 通知）。
    ///
    /// 请求（带 `id`）会得到响应；通知（无 `id`）可能触发推送通知。
    pub fn handle(&mut self, msg: &Value) -> Vec<Value> {
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            // 非方法消息（如响应回传）：忽略
            return Vec::new();
        };
        let is_request = msg.get("id").is_some();
        match method {
            "initialize" => self.on_initialize(msg),
            "initialized" => Vec::new(),
            "textDocument/didOpen" => {
                self.on_did_open(msg);
                self.publish_current(msg)
            }
            "textDocument/didChange" => {
                self.on_did_change(msg);
                self.publish_current(msg)
            }
            "textDocument/didClose" => {
                self.on_did_close(msg);
                self.publish_current(msg)
            }
            "shutdown" => {
                self.shutdown_requested = true;
                self.respond(msg, Value::Null)
            }
            "exit" => {
                self.exit_requested = true;
                Vec::new()
            }
            _ => {
                if is_request {
                    // 方法未找到
                    self.respond_error(msg, -32601, "method not found")
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// 是否应退出事件循环（收到 `exit` 通知）。
    pub fn should_exit(&self) -> bool {
        self.exit_requested
    }

    // ==================== 协议处理 ====================

    fn on_initialize(&mut self, msg: &Value) -> Vec<Value> {
        let result = InitializeResult {
            capabilities: ServerCapabilities {
                // 1 = full 文本同步
                text_document_sync: 1,
            },
        };
        self.respond(msg, serde_json::to_value(result).unwrap_or(Value::Null))
    }

    fn on_did_open(&mut self, msg: &Value) {
        if let (Some(uri), Some(text)) = (document_uri(msg), document_text(msg)) {
            self.documents.insert(uri.to_string(), text.to_string());
        }
    }

    fn on_did_change(&mut self, msg: &Value) {
        // full sync：变更内容在 params.contentChanges[0].text
        if let Some(uri) = document_uri(msg) {
            if let Some(text) = msg
                .pointer("/params/contentChanges/0/text")
                .and_then(Value::as_str)
            {
                self.documents.insert(uri.to_string(), text.to_string());
            }
        }
    }

    fn on_did_close(&mut self, msg: &Value) {
        if let Some(uri) = document_uri(msg) {
            self.documents.remove(uri);
        }
    }

    // ==================== 诊断 ====================

    /// 对 `msg` 关联的文档运行静态分析并构造推送通知。
    ///
    /// - 文档已关闭（didClose 后）：推送空诊断（清除）。
    fn publish_current(&self, msg: &Value) -> Vec<Value> {
        let Some(uri) = document_uri(msg) else {
            return Vec::new();
        };
        let params = match self.documents.get(uri) {
            Some(text) => {
                let diags = zeta_check::check_source(text);
                PublishDiagnosticsParams {
                    uri: uri.to_string(),
                    diagnostics: diags.iter().map(map_diagnostic).collect(),
                }
            }
            None => PublishDiagnosticsParams {
                uri: uri.to_string(),
                diagnostics: Vec::new(),
            },
        };
        let ok = serde_json::to_value(params).unwrap_or(Value::Null);
        vec![json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": ok,
        })]
    }

    /// 构造 JSON-RPC 响应（`id` 原样回传）。
    fn respond(&self, msg: &Value, result: Value) -> Vec<Value> {
        match msg.get("id").cloned() {
            Some(id) => vec![json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result,
            })],
            None => Vec::new(),
        }
    }

    /// 构造 JSON-RPC 错误响应。
    fn respond_error(&self, msg: &Value, code: i64, message: &str) -> Vec<Value> {
        match msg.get("id").cloned() {
            Some(id) => vec![json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": message },
            })],
            None => Vec::new(),
        }
    }
}

/// 提取消息的文档 uri。
fn document_uri(msg: &Value) -> Option<&str> {
    msg.pointer("/params/textDocument/uri").and_then(Value::as_str)
}

/// 提取消息的文档全文（didOpen 的 text 字段）。
fn document_text(msg: &Value) -> Option<&str> {
    msg.pointer("/params/textDocument/text").and_then(Value::as_str)
}

/// 将 zeta-check 诊断映射为 LSP 诊断。
///
/// zeta-check 行列从 1 起；LSP 从 0 起。锚点取诊断字符（1 字符宽）。
fn map_diagnostic(d: &zeta_check::Diagnostic) -> Diagnostic {
    let (line, col) = (d.line as u32, d.col as u32);
    let range = Range {
        start: pos(line - 1, col - 1),
        end: pos(line - 1, col), // 覆盖该字符
    };
    Diagnostic {
        range,
        severity: Some(match d.level {
            zeta_check::Level::Error => severity::ERROR,
            zeta_check::Level::Warning => severity::WARNING,
        }),
        source: Some("zeta".to_string()),
        code: Some(d.rule.to_string()),
        message: d.message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(json: &str) -> Value {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn initialize_returns_capabilities() {
        let mut server = Server::new();
        let out = server.handle(&msg(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#,
        ));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["id"], 1);
        assert_eq!(out[0]["result"]["capabilities"]["textDocumentSync"], 1);
    }

    #[test]
    fn did_open_pushes_empty_diagnostics_for_clean_code() {
        let mut server = Server::new();
        let out = server.handle(&msg(
            r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///a.zeta","languageId":"zeta","version":1,"text":"fn main() {\n    println(1);\n}\n"}}}"#,
        ));
        assert_eq!(out.len(), 1);
        let notif = &out[0];
        assert_eq!(notif["method"], "textDocument/publishDiagnostics");
        assert_eq!(notif["params"]["diagnostics"].as_array().unwrap().len(), 0);
        assert_eq!(notif["params"]["uri"], "file:///a.zeta");
    }

    #[test]
    fn did_open_reports_unused_variable() {
        let mut server = Server::new();
        let out = server.handle(&msg(
            r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///b.zeta","languageId":"zeta","version":1,"text":"fn main() {\n    let x = 1;\n}\n"}}}"#,
        ));
        let diags = out[0]["params"]["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d["severity"], 2); // warning
        assert_eq!(d["code"], "unused-variable");
        assert_eq!(d["source"], "zeta");
        assert_eq!(d["range"]["start"]["line"], 1); // 0 起始（第二行）
        // zeta-check 的 let 绑定无自身 span，锚点取 init 表达式起点：
        // "    let x = 1;" 中 `1` 的 1-based col 13 → 0-based character 12
        assert_eq!(d["range"]["start"]["character"], 12);
        assert!(d["message"].as_str().unwrap().contains("x"));
    }

    #[test]
    fn did_open_reports_parse_error() {
        let mut server = Server::new();
        let out = server.handle(&msg(
            r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///c.zeta","languageId":"zeta","version":1,"text":"fn main( {"}}}"#,
        ));
        let diags = out[0]["params"]["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0]["severity"], 1); // error
        assert_eq!(diags[0]["code"], "parse-error");
    }

    #[test]
    fn did_change_rechecks_text() {
        let mut server = Server::new();
        server.handle(&msg(
            r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///d.zeta","languageId":"zeta","version":1,"text":"fn main() {\n    println(1);\n}\n"}}}"#,
        ));
        // 变更后引入未使用变量
        let out = server.handle(&msg(
            r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///d.zeta","version":2},"contentChanges":[{"text":"fn main() {\n    let q = 2;\n}\n"}]}}"#,
        ));
        let diags = out[0]["params"]["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0]["code"], "unused-variable");
    }

    #[test]
    fn did_close_clears_diagnostics() {
        let mut server = Server::new();
        server.handle(&msg(
            r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///e.zeta","languageId":"zeta","version":1,"text":"fn main() {\n    let q = 2;\n}\n"}}}"#,
        ));
        let out = server.handle(&msg(
            r#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///e.zeta"}}}"#,
        ));
        let diags = out[0]["params"]["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 0); // 已清除
    }

    #[test]
    fn shutdown_then_unknown_request() {
        let mut server = Server::new();
        let out = server.handle(&msg(r#"{"jsonrpc":"2.0","id":7,"method":"shutdown"}"#));
        assert_eq!(out[0]["result"], Value::Null);
        assert!(!server.should_exit());

        let out = server.handle(&msg(r#"{"jsonrpc":"2.0","id":8,"method":"textDocument/hover","params":{}}"#));
        assert_eq!(out[0]["error"]["code"], -32601);

        server.handle(&msg(r#"{"jsonrpc":"2.0","method":"exit"}"#));
        assert!(server.should_exit());
    }
}
