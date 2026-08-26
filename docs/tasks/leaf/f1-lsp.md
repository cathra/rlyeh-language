# F1 LSP 服务器（MVP）

> **所属阶段**：阶段 F
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

新 crate `rlyeh-lsp` + `rlyeh lsp` CLI。

## 背景

阶段 阶段 F 子任务，详见 阶段详情文档 [`stages/F.md`](../../stages/F.md)。

## 技术细节

JSON-RPC 2.0 over stdio（Content-Length 帧，自研 `jsonrpc.rs` 无外部 LSP 依赖）+ full 文本同步（didOpen/didChange/didClose）+ 诊断推送（复用 rlyeh-check，映射为 LSP Diagnostic）+ initialize 声明 capabilities + shutdown/exit 生命周期；`server.rs::handle` 纯函数入口。

## 验证

`server.rs` 11 单测 + `lsp_e2e_test.rs` 2 进程测试全协议往返。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
