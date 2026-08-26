# 阶段 R — 高性能 IO（NIO + sendfile）

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-*.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：**已完成**（2026-08，R 阶段收官）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| R1A | **`Interest`/`Event` 类型**：`Interest`（读/写/读写）+ `Event`（token + interest，`is_readable`/`is_writable`，revents 解析） | ✅ 已完成 | [`r1a-interest-event.md`](../tasks/leaf/r1a-interest-event.md) |
| R1B | **`Poller` 封装**：`Poller::new`/`register`/`reregister`/`deregister`/`poll(timeout_ms)`——poll(2) 注册表状态容器（fds/events/tokens 三数组），重复 register→AlreadyExists、未注册 deregister→NotFound | ✅ 已完成（nio.rl | [`r1b-poller.md`](../tasks/leaf/r1b-poller.md) |
| R2 | **非阻塞**：`set_nonblocking(fd, bool)`/`is_nonblocking(fd)`（fcntl F_GETFL/F_SETFL + O_NONBLOCK=0x4；WASI 下短路 Err 禁用文档化） | ✅ 已完成（nio.rl | [`r2-nonblocking.md`](../tasks/leaf/r2-nonblocking.md) |
| R3 | **`sendfile` 模块 + `File::sendfile_to(sock_fd, offset)`**：零拷贝传输（driver 注入 `__rlyeh_sendfile` 平台内建；count=0 发到 EOF；**in_fd 须为只读 fd**，O_WRONLY 句柄报 EBADF，需 reopen Read 后调用） | ✅ 已完成 | [`r3-sendfile.md`](../tasks/leaf/r3-sendfile.md) |

**验收**：见任务树 [`stage-m-t.md`](../tasks/stage-m-t.md) 各子任务叶子的「验证」字段；全量回归通过。
