# R1a `Interest`/`Event` 类型

> **所属阶段**：阶段 R
> **状态**：✅ 已完成
> **依赖**：O
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

NIO 兴趣与事件类型。

## 背景

阶段 阶段 R 子任务，详见 阶段详情文档 [`stages/R.md`](../../stages/R.md)。

## 技术细节

nio.rl：`Interest`（读/写/读写）+ `Event`（token + interest，`is_readable`/`is_writable`，revents 解析）；POLLIN=1/POLLOUT=4 掩码映射，POLLERR/POLLHUP 按可读报告。

## 验证

`crates/rlyeh-driver/tests/nio_test.rs`（R1/R2/R3 集成 7 用例全绿）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
