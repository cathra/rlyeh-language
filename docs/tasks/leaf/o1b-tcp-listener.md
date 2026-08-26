# O1b `TcpListener`

> **所属阶段**：阶段 O
> **状态**：✅ 已完成
> **依赖**：O1a
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`TcpListener` 监听对象。

## 背景

阶段 阶段 O 子任务，详见 阶段详情文档 [`stages/O.md`](../../stages/O.md)。

## 技术细节

`TcpListener::bind(addr)`/`accept`/`local_addr`；Rust 绑定层 `rlyeh-std/src/net/` 扩展（现有自由函数保留兼容）。实际走 Rlyeh 侧直绑 libc extern（socket/bind/listen/accept/getsockname）。

## 验证

`tcp_echo.{rlyeh,out}`（O1b/O1c/O2：自连接 echo）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
