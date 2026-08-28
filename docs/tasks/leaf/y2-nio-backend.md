# Y2 NIO 高性能后端

> **所属阶段**：阶段 Y
> **状态**：🔧 部分完成（Y2a kqueue FFI 绑定 + Y2b kqueue 等待真实事件验证 ✅，2026-08-28；完整 Poller 分派待专项）
> **依赖**：R
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

poll(2) 之上增加 epoll/kqueue 平台后端。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`io/nio.rl` 的 poll(2) 后端之上增加 `Interest` 平台后端抽象——Linux `epoll`（`epoll_create1`/`epoll_ctl`/`epoll_wait`，O(1) 事件）、macOS `kqueue`（`kqueue`/`kevent`，`EVFILT_READ`/`EVFILT_WRITE`）；`Poller` 内部按 `__rlyeh_target_os()` 分派；为 W3 事件驱动 executor 提供平台后端。

## 风险评估

中等风险：涉及底层 FFI（kevent 结构体布局、epoll_event/EVFILT 常量），需新增 extern 绑定。当前系统为 macOS，kqueue 可直接验证；Linux epoll 需 CI 矩阵或按平台分派后本机仅能验证分派逻辑。

## 拆分方案（中 → 中低风险粒度）

### Y2a（✅ 已完成，2026-08-28）
- core.rl 新增 `kqueue() -> i32`/`kevent(kq, changelist: String, nchanges, eventlist: String, nevents, timeout: String) -> i32` extern；io/nio.rl 新增 `kevent_make(fd, filter, flags)`（32 字节 kevent 结构体小端构造：ident uintptr 8B + filter int16 2B + flags uint16 2B + fflags uint32 4B + data intptr 8B + udata ptr 8B）、`kqueue_new()`、`kevent_ctl(kq, changes, nchanges)`。
- 测试 `y2a_kqueue.{rl,out}`：kqueue() 创建 + kevent 字节布局（ident/filter=-1=EVFILT_READ/flags=1=EV_ADD）+ 真实 socketpair fd 提交 EV_ADD（返回 0 成功），输出 5。

### Y2b（✅ 核心能力验证，2026-08-28；完整 Poller 分派待专项）
- io/nio.rl 新增 `kevent_wait(kq, nevents, timeout_ms)`（kevent 等待 + timespec 构造）；`Event` 解析就绪 ident。
- **验证** `y2b_kqueue_wait.{rl,out}`：kqueue + socketpair → fd1 写数据 → fd0 EVFILT_READ 就绪 → kevent 等待返回 ident=fd0（4 项断言全过）。
- **待专项（破坏性）**：`Poller` 结构加 `kq` 字段 + `new`/`register`/`deregister`/`poll` 按 `__rlyeh_target_os()` 分派 kqueue（macOS）vs poll（兜底）vs epoll（Linux）。波及 core.rl/future.rl/examples/tests 等 31 处，登记 lang-defects。

## 验证

`y2a_kqueue.{rl,out}` + `y2b_kqueue_wait.{rl,out}`（165 用例全绿）；`nio_epoll_test.rs`（Linux）/`nio_kqueue_test.rs`（macOS 或 CI 矩阵，完整分派）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-28 | 风险拆分为 Y2a（FFI 绑定）/ Y2b（分派） |
| 2026-08-28 | Y2a kqueue FFI 绑定完成（160 用例全绿） |
| 2026-08-28 | Y2b kqueue 等待真实事件验证完成（165 用例全绿；完整 Poller 分派待专项） |
