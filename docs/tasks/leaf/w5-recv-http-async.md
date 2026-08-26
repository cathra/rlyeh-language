# W5 `recv_async`/HTTP async 真异步

> **所属阶段**：阶段 W
> **状态**：✅ 已完成
> **依赖**：W3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`recv_async` 挂起直到数据/close + HTTP async 经 NIO 非阻塞。

## 背景

阶段 阶段 W 子任务，详见 阶段详情文档 [`stages/W.md`](../../stages/W.md)。

## 技术细节

`recv_async` 真异步——`Channel` 携带 socketpair 唤醒 fd + `RecvAsync` future（try_recv + `cx.fd` 挂起）；`get_async`/`post_async` 真异步——`GetAsync` future（connect/写同步 + 非阻塞读响应 wait_fd 挂起，`extract_body` 解析；POST 带 body + Content-Length）；`block_on` 驱动返回 `Response`（前置 `block_on` 泛型化返回 `F::Output`）。

## 验证

`net_http_test.rs` `http_get_async`/`http_post_async` 5/5 + `channel.rl`/`recv_async.{rl,out}` 全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
