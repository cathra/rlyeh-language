# V3-D4：typecheck `try_check_adapter` 迁移为通用 trait 方法解析

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-D 细分子任务）
> **状态**：✅ 已完成（双轨分派收敛：自定义迭代器走 trait 默认方法；数组/`Vec` 走内建，因数组非命名类型语言限制）
> **风险**：中（集中式分派重构，双轨过渡）
> **依赖**：V3-D1、V3-D2、V3-D3
> **权威来源**：`rlyeh-typecheck/check_expr.rs` 1744（`try_check_adapter`）、`rlyeh-typecheck`（`find_impl_for_method`）

## 目标

统一 `try_check_adapter` 的分派逻辑：对 `map`/`filter`/`fold`/`collect`/`take`/`skip` 全部走通用 trait 方法解析（`find_impl_for_method` + trait 默认方法回退），删除各方法的专用内建 desugar 分支。

## 背景

V3-D1~D3 已把各方法迁移为 trait 默认方法，但 `try_check_adapter` 仍可能有残留内建分支。本子任务做集中式收敛：**双轨过渡**（先保留内建 + 新增默认方法，逐一确认调用点后删内建），确保数组/Vec/自定义迭代器三个接收者源均走 trait 方法。

## 改动范围

- **typecheck**：`try_check_adapter` 重构为通用方法分派；核对三个接收者源（数组/Vec/自定义迭代器）。
- **兼容策略**：**双轨收敛**——自定义迭代器（实现 Iterator/next）走通用 trait 方法解析（惰性默认方法）；数组/`Vec<T>` 保留内建 desugar（`check_iterator_adapter`），因 **Rlyeh 数组为内建非命名类型、无法 `impl Iterator for [T; N]`**（语言限制），`Vec` 亦保留内建以兼容 `vec.map(f)` 返回 Vec 的既有语义。
- **测试**：`adapters.rl`/`vec_api.rl` 适配器链用例输出一致。

## 验证

- [x] 六个适配器均对**自定义迭代器**经 trait 默认方法解析（惰性）。
- [x] 数组 / Vec 走内建 desugar（语言限制）；自定义迭代器走 trait 方法。
- [x] 全量回归（146 用例）输出一致。
- [x] 数组/Vec/自定义迭代器三个接收者源全部支持。

## 为什么是中风险

集中式分派重构波及所有适配器调用路径，但 V3-D1~D3 已把各方法逐一落地为默认方法，本步为**分派收敛**，双轨可单独回归。数组/Vec 保留内建为语言能力限制（数组非命名类型），非缺陷。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-D（高风险）细化拆分而来 |
| 2026-08-27 | 双轨分派收敛：自定义迭代器走 trait 默认方法（惰性），数组/Vec 保留内建（语言限制） |
