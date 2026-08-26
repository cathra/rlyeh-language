# K2 `Box<T>` 堆分配装箱

> **所属阶段**：阶段 K
> **状态**：✅ 已完成
> **依赖**：G1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`Box::new(v)` 编译器内建 + `*` 解引用 + 自动剥层。

## 背景

阶段 阶段 K 子任务，详见 阶段详情文档 [`stages/K.md`](../../stages/K.md)。

## 技术细节

布局：`Box<T>` 栈 1 槽 Ptr 存堆指针，堆分配 `slot_count(T)` 个 8 字节槽连续对象区。核心：`check_box_new` + `type_slot_count`（标量/struct/enum/数组/元组/引用/Box）+ `peel_box`/`peel_refs_and_boxes`/`box_ptr_hir`。接线五处：Deref 分支、字段访问剥层、方法 receiver 改写、索引 base 改写、as_str 特判。嵌套 `Box<Box<i64>>` 可用。

## 验证

`box_new.{rlyeh,out}` 15 输出 + compile-pass/box_ty.rl + compile-fail/box_bad.rl + 30 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
