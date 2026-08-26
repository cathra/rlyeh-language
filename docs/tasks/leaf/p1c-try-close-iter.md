# P1c `try_*`/`close`/`iter`

> **所属阶段**：阶段 P
> **状态**：✅ 已完成
> **依赖**：P1b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

通道 try/close/iter 方法。

## 背景

阶段 阶段 P 子任务，详见 阶段详情文档 [`stages/P.md`](../../stages/P.md)。

## 技术细节

`try_send`（恒 true）/`try_recv`（空 → None）+ `close`（recv 耗尽返回 None）+ `iter`（`next()` 接入 for 循环，J2 形态）。

## 验证

`channel.{rlyeh,out}`（try_* + close 耗尽 None + iter 接入 for）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
