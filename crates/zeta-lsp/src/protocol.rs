//! LSP 协议最小类型集（MVP）。
//!
//! 仅覆盖当前服务器需要的结构：文档同步（full）+ 诊断推送。
//! 字段命名遵循 LSP 规范 camelCase，由 `serde` 统一序列化。

use serde::{Deserialize, Serialize};

/// 位置（0 起始行/列）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// 区间。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// 诊断级别（LSP 数值：1=Error 2=Warning 3=Information 4=Hint）。
pub mod severity {
    pub const ERROR: u8 = 1;
    pub const WARNING: u8 = 2;
}

/// 诊断条目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub range: Range,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
}

/// `textDocument/publishDiagnostics` 通知参数。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishDiagnosticsParams {
    pub uri: String,
    pub diagnostics: Vec<Diagnostic>,
}

/// `initialize` 请求结果（能力声明）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
}

/// 服务器能力（MVP：full 文本同步 + 诊断）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    /// 1 = full（每次变更推送全文）
    pub text_document_sync: u8,
}

/// 按行列构造位置（LSP 0 起始）。
pub fn pos(line: u32, character: u32) -> Position {
    Position { line, character }
}

/// 零宽区间（用于诊断锚点）。
pub fn zero_range(line: u32, character: u32) -> Range {
    Range {
        start: pos(line, character),
        end: pos(line, character),
    }
}
