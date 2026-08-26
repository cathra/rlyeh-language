# Y5 `Box::leak` 目标签名

> **所属阶段**：阶段 Y
> **状态**：📋 规划
> **依赖**：U5
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`fn leak(self) -> &'static mut T`（替代裸指针退化）。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`fn leak(self) -> &'static mut T`（依赖 U5 AddrOf 任意目标，替代 `*mut T` 裸指针退化）；`'static` 宽松丢弃（G4 现状）。

## 验证

`box_leak_ref.{rlyeh,out}`。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
