# O1c `TcpStream`

> **所属阶段**：阶段 O
> **状态**：✅ 已完成
> **依赖**：O1b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`TcpStream` 连接对象。

## 背景

阶段 阶段 O 子任务，详见 阶段详情文档 [`stages/O.md`](../../stages/O.md)。

## 技术细节

`TcpStream::connect`/`peer_addr`/`shutdown` + 现有 `tcp_connect` 升级为返回 `TcpStream`（自由函数兼容壳）；平台自适应 sockaddr_in 布局（macOS sin_len / Linux 小端 family，经 `__rlyeh_target_os`）。

## 验证

`tcp_echo.{rlyeh,out}` + `net_socket_test.rs`。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
