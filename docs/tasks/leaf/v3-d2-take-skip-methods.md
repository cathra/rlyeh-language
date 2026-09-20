# V3-D2：`take`/`skip` protocol 默认方法 + 绑定包装迭代器

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-D 细分子任务）
> **状态**：📋 规划
> **风险**：中（单一适配器对，独立可测）
> **依赖**：V3-B、V3-C
> **权威来源**：`core.rl`（`Take`/`Skip` 结构体）、`rlyeh-typecheck/check_expr.rs` 1744（`try_check_adapter`）

## 目标

`take`/`skip` 从 typecheck 内建 desugar 迁移为 `Iterator` protocol 默认方法，返回 V3-C 的 `Take`/`Skip` 包装迭代器。

## 背景

迁移后 `v.iter().take(3)` 走 protocol 默认方法 + `Take` 包装迭代器（V3-C 已定义），计数归零返回 None。

## 改动范围

- **std**：`Iterator` protocol 增 `fn take(self, n: i64) -> Take<Self>` / `fn skip(self, n: i64) -> Skip<Self>` 默认方法（绑定 V3-C 结构体 + `next()`）。
- **typecheck**：`try_check_adapter` 对 `take`/`skip` 优先走通用 protocol 方法解析。
- **测试**：`v.iter().take(3)` / `skip(2)` 链式输出一致。

## 验证

- [ ] `v.iter().take(3)` 返回 `Take`，计数归零后 `next()` 返回 None。
- [ ] `v.iter().skip(2)` 先跳够再产出。
- [ ] 现有 take/skip 用例输出一致（回归）。

## 为什么是中风险

与 V3-D1 同模式（一对适配器 + 绑定 V3-C 结构体），复用 V3-D1 打通的方法解析路径，风险进一步收敛。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-D（高风险）细化拆分而来 |
