# EH-6 错误聚合 / `try` 块

> **级别**：P3 · **风险**：🟠 中 · **状态**：🟡 待办 · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.6 · **关联**：`eh-4-dyn-error.md`

## 目标
- `try` 块：块内 `?` 不立即返回，而是聚合首个错误（或 `Vec` 累积全部错误）后统一处理。
- 迭代器错误累积：`Vec<Result<T,E>>` → `Result<Vec<T>, E>`（类 `collect`/`try_collect`）。

## 现状
- `?` 仅支持早返回（单错误）；无聚合/累积机制（RFC §2 表中 ❌）。

## 风险分解
- **M1（中）** `try` 块 desugar（`Result<_, E>` 收集到隐藏累加器）。
- **M2（中）** 迭代器 `try_collect` / `Vec` 错误累积。
- **M3（低）** 与 `DynError`（eh-4）组合：聚合为 `Vec<DynError>`。

## 受影响组件
`rlyeh-typecheck`（`?` desugar）、`rlyeh-codegen`、`rlyeh-std`（`core/result.rl`）。

## 验证
- 单元：`try { ... }` 内多 `?` 仅透出首个错误；迭代器累积全部错误。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
