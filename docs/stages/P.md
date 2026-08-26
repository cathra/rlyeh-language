# 阶段 P — 并发通道与同步

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-*.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：`Mutex`/`RwLock` 裸 `lock/unlock/try_*` 已实现（§6.1 ✅）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| P1A | **无界队列 Channel**：`sync::Channel`（Mutex + Condvar 队列，`Rc<Channel>` 共享；`send`/`recv`/`try_send`/`try_recv`/`close`；无界队列 send 恒不阻塞，recv 空挂起） | ✅ 已完成 | [`p1a-channel-queue.md`](../tasks/leaf/p1a-channel-queue.md) |
| P1B | **`Sender`/`Receiver` 对象化**：`channel()` → `ChannelPair { tx, rx }` + `send`/`recv` + 多 Sender/Receiver 共享（`Rc` clone，K3 ✅；目标 API 为 `Arc` 泛型版，规划） | ✅ 已完成 | [`p1b-sender-receiver.md`](../tasks/leaf/p1b-sender-receiver.md) |
| P1C | **`try_*`/`close`/`iter`**：`try_send`（恒 true）/`try_recv`（空 → None）+ `close`（recv 耗尽返回 None）+ `iter`（`next()` 接入 for 循环，J2 形态） | ✅ 已完成 | [`p1c-try-close-iter.md`](../tasks/leaf/p1c-try-close-iter.md) |
| P2A | **注入机制验证**：编译器对 guard 局部变量作用域结束自动 `unlock` 注入（`rlyeh-desugar/src/guard.rs` 对方法名 `lock_guard` 特判，块尾注入；if/match 分支内提前 return/break 不注入） | ✅ 已完成 | [`p2a-guard-inject.md`](../tasks/leaf/p2a-guard-inject.md) |
| P2B | **`MutexGuard` 接线**：`Mutex::lock_guard()` 返回 `MutexGuard`（`p` 裸指针承载 pthread_mutex_t*）+ 作用域结束自动解锁注入；`RwLockWriteGuard` 目标 API 规划 | ✅ 已完成 | [`p2b-mutex-guard.md`](../tasks/leaf/p2b-mutex-guard.md) |
| P3 | **`Condvar`/`Barrier`**：`Condvar::wait/notify_one/notify_all`（与 Mutex 配对）+ `Barrier::new(count)/wait`（`PTHREAD_BARRIER_SERIAL_THREAD` 领头线程语义简化） | ✅ 已完成 | [`p3-condvar-barrier.md`](../tasks/leaf/p3-condvar-barrier.md) |

**验收**：见任务树 [`stage-m-t.md`](../tasks/stage-m-t.md) 各子任务叶子的「验证」字段；全量回归通过。
