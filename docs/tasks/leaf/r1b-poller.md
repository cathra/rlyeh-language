# R1b `Poller` 封装

> **所属阶段**：阶段 R
> **状态**：✅ 已完成
> **依赖**：R1a
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`Poller` poll(2) 封装。

## 背景

阶段 阶段 R 子任务，详见 阶段详情文档 [`stages/R.md`](../../stages/R.md)。

## 技术细节

`Poller::new`/`register`/`reregister`/`deregister`/`poll(timeout_ms)`——poll(2) 注册表状态容器（fds/events/tokens 三数组），重复 register→AlreadyExists、未注册 deregister→NotFound。

## 验证

`nio_test.rs`（Poller 就绪/reregister/deregister/错误路径）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
