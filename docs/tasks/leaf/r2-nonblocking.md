# R2 非阻塞

> **所属阶段**：阶段 R
> **状态**：✅ 已完成
> **依赖**：R1b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

fd 非阻塞设置。

## 背景

阶段 阶段 R 子任务，详见 阶段详情文档 [`stages/R.md`](../../stages/R.md)。

## 技术细节

`set_nonblocking(fd, bool)`/`is_nonblocking(fd)`（fcntl F_GETFL/F_SETFL + O_NONBLOCK=0x4；WASI 下短路 Err 禁用文档化）。

## 验证

`nio_test.rs`（非阻塞往返）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
