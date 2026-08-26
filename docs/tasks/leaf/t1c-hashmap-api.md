# T1c HashMap 目标 API 补齐

> **所属阶段**：阶段 T
> **状态**：✅ 已完成
> **依赖**：J
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

HashMap 目标 API：`iter`/`keys`/`values`。

## 背景

阶段 阶段 T 子任务，详见 阶段详情文档 [`stages/T.md`](../../stages/T.md)。

## 技术细节

`keys`/`values`/`clear` 等已有；新增 `iter`（MVP 退化——返回键缓冲，与 `keys` 同构，目标 `Iter<'_, K, V>` 键值对迭代器规划）、`get_mut`（值拷贝，目标 `&mut V` 引用规划）。

## 验证

`hashmap_api.{rlyeh,out}`（iter 键求和、iter/keys 同构、get_mut）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
