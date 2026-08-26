# Y2 NIO 高性能后端

> **所属阶段**：阶段 Y
> **状态**：📋 规划
> **依赖**：R
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

poll(2) 之上增加 epoll/kqueue 平台后端。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`io/nio.rl` 的 poll(2) 后端之上增加 `Interest` 平台后端抽象——Linux `epoll`（`epoll_create1`/`epoll_ctl`/`epoll_wait`，O(1) 事件）、macOS `kqueue`（`kqueue`/`kevent`，`EVFILT_READ`/`EVFILT_WRITE`）；`Poller` 内部按 `__rlyeh_target_os()` 分派；为 W3 事件驱动 executor 提供平台后端。

## 验证

`nio_epoll_test.rs`（Linux）/`nio_kqueue_test.rs`（macOS 或 CI 矩阵）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
