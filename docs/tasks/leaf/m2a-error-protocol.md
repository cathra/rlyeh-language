# M2a `Error` protocol

> **所属阶段**：阶段 M
> **状态**：✅ 已完成
> **依赖**：M1b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

定义 `Error` protocol 并验证 dyn 分派。

## 背景

阶段 阶段 M 子任务，详见 阶段详情文档 [`stages/M.md`](../../stages/M.md)。

## 技术细节

`protocol Error { fn message(&self) -> String; }` + `impl IoError: Error`（`message()` 读 `self.message` 字段）+ `describe(e: &dyn Error)` 验证（H4 dyn Protocol ✅）。H4 限制——dyn Protocol 仅可作局部变量绑定，`describe` 类调用于函数体内构造 `let d: dyn Error = ...`。

## 验证

`tests/run-pass/error_protocol.rl` + `error_protocol.{rlyeh,out}`。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
