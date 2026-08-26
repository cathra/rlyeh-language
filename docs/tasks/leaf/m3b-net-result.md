# M3b net 自由函数 Result 化

> **所属阶段**：阶段 M
> **状态**：✅ 已完成
> **依赖**：M2b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

net 自由函数同步升级为 `Result`。

## 背景

阶段 阶段 M 子任务，详见 阶段详情文档 [`stages/M.md`](../../stages/M.md)。

## 技术细节

`tcp_connect`/`send_all`/`recv_some`/`hostname` 同步升级（net.rl）。

## 验证

`io_result.{rlyeh,out}` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
