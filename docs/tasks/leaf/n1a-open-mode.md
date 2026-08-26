# N1a `OpenMode` + 绑定层

> **所属阶段**：阶段 N
> **状态**：✅ 已完成
> **依赖**：M
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

定义 `OpenMode` 枚举 + Rust 绑定层验证。

## 背景

阶段 阶段 N 子任务，详见 阶段详情文档 [`stages/N.md`](../../stages/N.md)。

## 技术细节

`enum OpenMode { Read, Write, Append, ReadWrite, Create }` + 新增 `rlyeh-std/src/file.rs`。绑定层修订：验证 rlyeh-driver 不链接 rlyeh-std crate，Rust 绑定层无法接线，改为 io.rl 直接 libc extern（fopen 族）+ `open_mode_str` 模式字符串映射，跨平台无 `O_*` flags 差异。

## 验证

`file_io.{rlyeh,out}`（N1）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
