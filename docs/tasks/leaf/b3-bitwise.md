# 位运算全链路（B3 前置）

> **所属阶段**：阶段 B
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`&`/`|`/`^`/`<<`/`>>` 全链路打通。

## 背景

阶段 阶段 B 子任务，详见 阶段详情文档 [`stages/B.md`](../../stages/B.md)。

## 技术细节

HIR `HirBinaryOp` 5 新变体、typecheck 映射、MIR const_fold 折叠（`wrapping_shl/shr`）、LIR `binary_result_type` 统一 I64、codegen `and/or/xor/shl/ashr`（右移算术 ashr）。

## 验证

`bitwise_test.rs` 6 用例：基础/移位负数/优先级/字节打包/掩码/循环累积。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
