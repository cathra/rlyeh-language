# J1 数组迭代

> **所属阶段**：阶段 J
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`for x in arr` desugar 为索引遍历循环。

## 背景

阶段 阶段 J 子任务，详见 阶段详情文档 [`stages/J.md`](../../stages/J.md)。

## 技术细节

数组以指针存储、长度编译期已知，元素读取复用 `HirExpr::Index` 步长 8 / u8 按字节；`check_for` 分派加 `Type::Array` 分支（`check_for_array`）。

## 验证

`array_for.{rlyeh,out}` 5 输出。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
