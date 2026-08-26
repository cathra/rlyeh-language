# S2b `join_all` + 墙钟

> **所属阶段**：阶段 S
> **状态**：✅ 已完成
> **依赖**：S2a
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

线程版 `join_all` + 墙钟接入。

## 背景

阶段 阶段 S 子任务，详见 阶段详情文档 [`stages/S.md`](../../stages/S.md)。

## 技术细节

`thread::join_all(Vec<Thread>) -> Vec<i64>`（并发等待多线程，按传入顺序收集返回值）；墙钟 `__rlyeh_clock_monotonic`（clock_gettime CLOCK_MONOTONIC，注意 macOS 常量 =6 与 Linux =1 差异）接入，`Instant::now/elapsed` 睡眠期间推进（不支持平台退回 clock()）。

## 验证

`join_all.{rlyeh,out}`（3 线程各 sleep + join_all 收集求和 + 墙钟 elapsed）+ `thread_test.rs`。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
