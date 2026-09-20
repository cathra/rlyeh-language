# V3-A4：`F::Item` 关联类型投影完善 + 自定义迭代器用例

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-A 细分子任务）
> **状态**：✅ 已完成（命名类型关联投影 `Range::Item` + 自定义迭代器用例，2026-08-27）
> **风险**：低（收尾 + 测试补充）
> **依赖**：V3-A3
> **权威来源**：`rlyeh-typecheck`（`resolve_ast_type` 投影路径）、`tests/run-pass`

## 目标

完善 `F::Item` 关联类型投影的边界情况，补充自定义迭代器用例与回归测试。

## 背景

V3-A2 打通 `Self::Item`，V3-A3 落地 Iterator 具体化。本子任务补 `F::Item`（命名类型上的关联投影）边界 + 测试。

## 改动范围

- **typecheck**：`F::Item` 形式（已命名类型 `F` 的关联投影）解析一致性（与 U4 `F::Output` 对齐）。
- **测试**：新增自定义迭代器 `impl Iterator for MyRange { type Item = i64; }` 用例；迭代器作为函数参数/返回值时 `Item` 投影正确。

## 实施情况（已完成，2026-08-27）

- **typecheck `resolve_ast_type`**：`F::Item` 命名类型关联投影——在 `name.rsplit_once("::")` 后，base 非泛型参数时经 `eval_assoc_projection`（查该类型 protocol impl 的 `assoc_types`）立即求值，返回具体类型（与 U4 泛型参数投影 `F::Output` 并存）。此前命名类型 `Range::Item` 报 `undefined type`。
- **测试**：新增 `v3a4_item_projection.rl`（自定义 `Range { type Item = i64 }` + 命名类型投影注解 + 迭代器经函数传递 for 接入 + `next()` 返回）。

## 验证

- [x] `F::Item` 在已命名类型上投影正确（`let x: Range::Item = 7;` 通过）。
- [x] 自定义迭代器经函数传递/返回后 `next()` 正确（`sum_range` for 接入输出 6；`next()` 返回 10）。
- [x] 全量回归通过（141 用例全过）。

## 为什么是低风险

纯边界完善 + 测试补充，不引入新机制；主流程已由 V3-A2/A3 覆盖。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-A（高风险）细化拆分而来 |
| 2026-08-27 | 完成命名类型 `F::Item` 关联投影 + 自定义迭代器用例 |
