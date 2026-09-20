# V3-D3：`collect` protocol 默认方法 + `fold` 归约迁移

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-D 细分子任务）
> **状态**：📋 规划
> **风险**：中（消费型终结器，逻辑独立）
> **依赖**：V3-B、V3-C
> **权威来源**：`core.rl`（`collect`/`fold` 语义）、`rlyeh-typecheck/check_expr.rs` 1744（`try_check_adapter`）

## 目标

`collect` 从内建 desugar 迁移为 `Iterator` protocol 默认方法（收集到 `Vec<Self::Item>`），`fold` 归约在 V3-B 已有默认方法基础上对齐迁移。

## 背景

`collect()` 是消费型终结器，把迭代器元素收集进 Vec。`fold(init, f)` 已在 V3-B 有默认方法，本子任务统一两者为 protocol 方法路径。

## 改动范围

- **std**：`Iterator` protocol 增 `fn collect(self) -> Vec<Self::Item>` 默认方法（MVP 固定收集到 Vec）。
- **typecheck**：`try_check_adapter` 对 `collect`/`fold` 优先走通用 protocol 方法解析。
- **测试**：适配器链 `.map(...).collect()` / `.fold(0, ...)` 输出一致。

## 验证

- [ ] `v.map(|x| x*10).collect()` 返回 Vec、元素变换正确。
- [ ] `v.iter().fold(0, |acc, x| acc + x)` 归约正确。
- [ ] 现有 collect/fold 用例输出一致（回归）。

## 为什么是中风险

`collect` 为终结器（不返回包装迭代器），逻辑独立；`fold` 已在 V3-B 打通，本步为统一到 protocol 方法解析路径。风险点集中、可回退。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-D（高风险）细化拆分而来 |
