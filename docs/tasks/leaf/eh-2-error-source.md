# EH-2 `Error::source` 真实根因链 + 全错误类型补齐

> **级别**：P2 · **风险**：🟠 中 · **状态**：🟡 待办 · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.2 · **关联**：`m2a-error-protocol.md`、`m2b-error-convert.md`、`y6-*`

## 目标
将 `Error::source()` 从 `None` 占位升级为真实根因链；为所有 std 错误类型（`IoError`/`TimeoutError` 等）补齐 `source()`；可选补充 `kind()` 分类方法。

## 现状
- `Error` protocol 已落地 `message()`（M2a ✅）与 `source()`（Y6）：`io/error.rl` 中 `impl IoError: Error` 的 `source()` 当前返回 `Option::None` 占位，未接真实根因。
- `IoError` 已含 `kind: IoErrorKind`（M1b），可作为 `kind()` 基础。

## 风险分解
- **M1（中）** `IoError::source()` 返回内部 `Option<&dyn Error>`（如包装底层 OS 错误时填充）。
- **M2（低）** `TimeoutError` 及后续错误类型补齐 `source()` / `message()`。
- **M3（低）** 可选 `kind()` 分类方法（`IoErrorKind` 已具备）。

## 受影响组件
`rlyeh-std`（`io/error.rl`、`future/timeout` 等错误类型）、`Error` protocol 定义。

## 验证
- 单元：`IoError { kind, message, source: Some(&e) }` 经 `source()` 取回根因；链式 `source()?.source()` 遍历。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
