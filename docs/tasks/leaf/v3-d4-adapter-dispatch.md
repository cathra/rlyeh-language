# V3-D4：typecheck `try_check_adapter` 迁移为通用 trait 方法解析

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-D 细分子任务）
> **状态**：📋 规划
> **风险**：中（集中式分派重构，双轨过渡）
> **依赖**：V3-D1、V3-D2、V3-D3
> **权威来源**：`rlyeh-typecheck/check_expr.rs` 1744（`try_check_adapter`）、`rlyeh-typecheck`（`find_impl_for_method`）

## 目标

统一 `try_check_adapter` 的分派逻辑：对 `map`/`filter`/`fold`/`collect`/`take`/`skip` 全部走通用 trait 方法解析（`find_impl_for_method` + trait 默认方法回退），删除各方法的专用内建 desugar 分支。

## 背景

V3-D1~D3 已把各方法迁移为 trait 默认方法，但 `try_check_adapter` 仍可能有残留内建分支。本子任务做集中式收敛：**双轨过渡**（先保留内建 + 新增默认方法，逐一确认调用点后删内建），确保数组/Vec/自定义迭代器三个接收者源均走 trait 方法。

## 改动范围

- **typecheck**：`try_check_adapter` 重构为通用方法分派；核对三个接收者源（数组/Vec/自定义迭代器）均解析到 trait 默认方法。
- **兼容策略**：双轨过渡期间保留内建作为回退，逐一迁移调用点（J3 全部调用点）。
- **测试**：现有 `vec_api.rl` / 适配器链用例（map/filter/fold/collect/take/skip）经 trait 方法路径输出一致。

## 验证

- [ ] 六个适配器均经通用 trait 方法解析（非内建 desugar）。
- [ ] 数组 / Vec / 自定义迭代器三个接收者源全部支持。
- [ ] 全量回归（现有适配器链用例）输出一致。

## 为什么是中风险

集中式分派重构波及所有适配器调用路径，但 V3-D1~D3 已把各方法逐一落地为默认方法，本步为**分派收敛 + 双轨过渡**，每次改动可单独回归、失败可回退到内建分支。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-D（高风险）细化拆分而来 |
