# B3 net 模块

> **所属阶段**：阶段 B
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

net 绑定：socket/主机名/字节序。

## 背景

阶段 阶段 B 子任务，详见 阶段详情文档 [`stages/B.md`](../../stages/B.md)。

## 技术细节

`gethostname` extern + `hostname()`；`socketpair`/`socket`/`connect`/`close`/`r#send`/`r#recv`（send/recv 为 actor 保留字用 r#）+ `htons`/`socketpair_stream`/`fd_at`/`send_all`/`recv_some`/`sockaddr_in4`/`tcp_connect`。

## 验证

`net_socket_test.rs` 6 用例：htons/socketpair/fd 数组/多包/大块/sockaddr/拒绝。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
