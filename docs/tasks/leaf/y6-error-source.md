# Y6 错误体系完整化

> **所属阶段**：阶段 Y
> **状态**：📋 规划
> **依赖**：U3/U4
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`trait Error` 补 `source` 链 + `Into::into()` 自动转换。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`trait Error { fn message(&self) -> String; fn source(&self) -> Option<&dyn Error>; }`（补 `source` 链，M2a 现状仅 message）；`Into::into()` 自动转换 + `?` 运算符的 From 自动转换（依赖 U3/U4；MVP 无 where 约束时退化显式 `into()` 调用）。

## 验证

`error_source.{rlyeh,out}`。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
