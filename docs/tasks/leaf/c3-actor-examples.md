# C3 示例与测试固化

> **所属阶段**：阶段 C
> **状态**：✅ 已完成
> **依赖**：C2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

actor 示例与集成测试固化。

## 背景

阶段 阶段 C 子任务，详见 阶段详情文档 [`stages/C.md`](../../stages/C.md)。

## 技术细节

`examples/actor-ping-pong.rl`（ask 往返 + send 异步 + FIFO，输出 11/12/2/4）、`examples/actor-supervisor.rl`（`new_supervised(0)` 崩溃恢复，输出 5/0/3）；集成测试补 `crash_without_supervisor_stops_actor`、`send_fifo_order`。

## 验证

C3 示例运行 + 集成测试全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
