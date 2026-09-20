# V3-A3：std — `Iterator::Item` 落地 + 各 impl 具体化

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-A 细分子任务）
> **状态**：✅ 已完成（`Iterator` protocol 引入 `type Item` + `next -> Option<Self::Item>` + 各 impl 补 type Item，2026-08-27）
> **风险**：中（涉及多个 impl 与全量回归）
> **依赖**：V3-A1、V3-A2
> **权威来源**：`core.rl`（`Iterator` protocol 545 + 各 `impl Iterator`）、`std-lib.md` §2.3

## 目标

`Iterator::next` 签名改为 `Option<Self::Item>`（替代固定 i64），并让所有现有 `impl Iterator` 提供 `type Item = <具体类型>`。

## 背景

当前 `protocol Iterator { fn next(&mut self) -> Option<i64>; }` 元素固定 i64。迁移后需各 impl 显式声明关联类型。

## 改动范围

- **std `Iterator` protocol**：`type Item;` + `fn next(&mut self) -> Option<Self::Item>`。
- **各 `impl Iterator`**（Vec/数组/Range/Chars/Lines/自定义等）：补 `type Item = i64;` 或对应元素类型。
- **默认方法**（count/sum/any/all，V3-B 已有）改用 `Self::Item` 泛化（元素类型非 i64 时 sum/比较逻辑适配）。

## 验证

- [ ] `Range`/`Vec` 迭代器 `next()` 经关联类型仍正确返回元素。
- [ ] 现有 count/sum/any/all 用例（元素 i64）继续通过（回归重点）。
- [ ] 元素为 i64 以外的自定义迭代器可工作。
- [ ] 全量回归（现有适配器用例）不回归。

## 为什么是中风险

签名从具体 i64 改为泛化关联类型是**类型系统的结构性改动**，波及所有 impl 与默认方法；但 V3-A1/A2 已把载体与投影打通，本步为逐项落地 + 全量回归，风险集中在回归面而非未知机制。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-A（高风险）细化拆分而来 |
