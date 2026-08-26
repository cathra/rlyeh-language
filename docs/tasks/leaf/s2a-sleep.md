# S2a `sleep`

> **所属阶段**：阶段 S
> **状态**：✅ 已完成
> **依赖**：S0
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

线程睡眠。

## 背景

阶段 阶段 S 子任务，详见 阶段详情文档 [`stages/S.md`](../../stages/S.md)。

## 技术细节

`thread::sleep(Duration)`——注入 `__rlyeh_thread_sleep`（usleep 绑定，micros 截断 u32 上限约 71 分钟）；补 `Duration::seconds/milliseconds` 构造器；`Instant` 基准 clock()（CPU 时钟）睡眠期间不推进（墙钟随 S2b 接入）。

## 验证

`sleep_join.{rlyeh,out}`（sleep 返回值 0）+ `time_test.rs`。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
