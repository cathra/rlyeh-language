# EH-5 `#[derive(Error)]`（thiserror 式）

> **级别**：P2 · **风险**：🟠 中 · **状态**：🟡 待办 · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.5 · **关联**：`sh-p1-2-derive.md`、`q1b-derive-macro.md`

## 目标
为错误枚举/结构体提供 derive，自动生成 `Error`/`Display`/`From` 实现，消除手写样板：
- 枚举变体 `#[error("msg {0}")]` → `message()`；
- `#[from]` 字段 → `impl E: From<T>`；
- `#[source]` 字段 → `source()`。

## 现状
- 错误 derive 完全缺失（RFC §2 表中 ❌）。
- 已有 JSON derive 先例（`q1b`/`q1c`），可复用 derive 框架与 codegen 注入逻辑。

## 风险分解
- **M1（中）** derive 解析属性（`#[error]`/`#[from]`/`#[source]`）。
- **M2（中）** 生成 `message()`/`source()`/`impl From`（复用 `eh-2` 机制）。
- **M3（低）** 与既有 `#[derive(...)]` 框架整合（向后兼容）。

## 受影响组件
`rlyeh-parser`（属性解析）、`rlyeh-typecheck`/`rlyeh-codegen`（derive codegen）、`derive` 框架。

## 验证
- 单元：带 derive 的错误枚举 run-pass；`?` 经 `#[from]` 自动转换。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
