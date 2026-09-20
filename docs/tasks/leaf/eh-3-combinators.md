# EH-3 `Option`/`Result` 组合子补全

> **级别**：P3 · **风险**：🟢 低 · **状态**：🟡 待办 · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.3 · **关联**：`t1a-vec-api.md`（API 风格参考）

## 目标
补齐常用组合子，消除样板、提升表达力。

## 现状
- `Option`：`from`/`map`/`and_then`/`unwrap`/`unwrap_or` ✅。
- `Result`：`map`/`map_err`/`and_then`/`propagate`/`unwrap`/`unwrap_or`/`expect` ✅。
- 缺：`or_else`/`unwrap_or_else`/`expect_err`/`ok()`/`err()`/`transpose`/`flatten` 等。

## 待补清单
- `Option`：`or_else`/`unwrap_or_else`/`ok_or`/`ok_or_else`/`flatten`/`transpose`。
- `Result`：`or_else`/`unwrap_or_else`/`expect_err`/`ok()`/`err()`/`transpose`/`flatten`/`copied`/`cloned`。

## 受影响组件
`rlyeh-std`（`core/option.rl`、`core/result.rl`）。

## 验证
- 单元：每个组合子 run-pass；与 Rust 同名语义对拍。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
