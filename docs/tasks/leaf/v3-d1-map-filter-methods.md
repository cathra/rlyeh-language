# V3-D1：`map`/`filter` trait 默认方法 + 绑定包装迭代器

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-D 细分子任务）
> **状态**：📋 规划
> **风险**：中（单一适配器对，独立可测）
> **依赖**：V3-B、V3-C
> **权威来源**：`core.rl`（`Map`/`Filter` 结构体）、`rlyeh-typecheck/check_expr.rs` 1744（`try_check_adapter`）

## 目标

`map`/`filter` 从 typecheck 内建 desugar 迁移为 `Iterator` trait 默认方法，返回 V3-C 的 `Map`/`Filter` 包装迭代器。

## 背景

迁移后 `v.map(|x| x*10)` 走 trait 默认方法 + `Map` 包装迭代器（V3-C 已定义），不再收集到 Vec。

## 改动范围

- **std**：`Iterator` trait 增 `fn map<B>(self, f: fn(Self::Item) -> B) -> Map<Self, B>` / `fn filter(self, p: fn(Self::Item) -> bool) -> Filter<Self, P>` 默认方法（绑定 V3-C 结构体 + `next()`）。
- **typecheck**：`try_check_adapter` 对 `map`/`filter` 优先走通用 trait 方法解析（`find_impl_for_method` / trait 默认方法回退）。
- **测试**：`v.map(...).collect()` / `v.iter().filter(pred).take(3)` 链式保持输出一致。

## 验证

- [ ] `v.map(|x| x*10)` 返回 `Map` 包装迭代器，`next()` 变换正确。
- [ ] `v.iter().filter(pred)` 返回 `Filter`，跳过不满足项。
- [ ] 现有 `vec_api.rl` 中 map/filter 用例输出一致（回归）。

## 为什么是中风险

每次仅迁移**一对适配器**（map/filter），绑定 V3-C 结构体，风险点集中在本对包装迭代器 + 方法解析回退，独立可测、失败可回退。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-D（高风险）细化拆分而来 |
