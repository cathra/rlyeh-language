# E1 平台内建 `__rlyeh_target_os`

> **所属阶段**：阶段 E
> **状态**：✅ 已完成
> **依赖**：E1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

平台内建消除平台相关假设。

## 背景

阶段 阶段 E 子任务，详见 阶段详情文档 [`stages/E.md`](../../stages/E.md)。

## 技术细节

Rlyeh 无 `#[cfg]` 属性机制，改由驱动注入平台内建：`target_os_code(target)`（triple→OS 码）+ `platform_builtin_ir` 注入 `define internal i32 @__rlyeh_target_os()`；codegen 对 `__rlyeh_` 前缀 extern 跳过 declare；`sockaddr_in4_with_layout(has_sin_len, ...)` 双布局（macOS sin_len / Linux 小端 family）。

## 验证

`platform_builtin_test.rs` 4 集成：target→OS 码、平台内建端到端、sockaddr 双布局、一致性。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
