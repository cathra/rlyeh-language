# 阶段 W — 异步运行时完整化

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-u-z.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：S 阶段已实现线程、`Future`/`Poll`/`block_on`、async fn 状态机（S1c）、线程版 `join_all`、`timeout`（MVP 退化）、`recv_async`/HTTP async（阻塞语义退化）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| W1 | **Future 泛型化**：`protocol Future { type Output; fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output>; }`（依赖 U2 关联类型；`Context`/`Pin<&mut Self>` 为占位类型——`Pin` 语义 MVP 可退化 `&mut Self`）；async fn desugar 输出 `Output` 泛型化（当前固定 i64） | ✅ 已完成 | [`w1-future-generic.md`](../tasks/leaf/w1-future-generic.md) |
| W2 | **await 状态机扩展**：await 位置扩展到控制流块内（if/match/loop 体，状态机段切分按控制流图而非直线）、表达式中间嵌套 await（`a.await + b.await`）；跨 await 变量从 `i64` 扩展到引用类型（依赖 U1）；消除 S1c 显式报错限制 | ✅ 已完成 | [`w2-await-state-machine.md`](../tasks/leaf/w2-await-state-machine.md) |
| W3 | **事件驱动 executor**：`rlyeh-async-runtime`（或语言侧 `async/executor.rl` + `task.rl`）——基于 R1 Poller（poll(2)，后续接 epoll/kqueue，见 Y2）的事件循环 + 任务队列 + Waker/唤醒注册：`block_on` 从忙等轮询改为「poll Pending → 注册 fd/定时器 → 事件就绪唤醒」；为 W5 提供真异步底座。**风险**：单线程事件循环 + Future 状态机 + fd 注册表三者联动是 MVP 后最大子系统，建议分两步——先「定时器唤醒」（sleep/超时事件驱动），再「fd 事件唤醒」 | ✅ 两步完成（定时器唤醒 + fd 事件唤醒；执行情况见 [`w3-event-executor.md`](../tasks/leaf/w3-event-executor.md) |
| W4 | **`join_all`/`timeout` Future 版**：`join_all<F: Future>(futures: Vec<F>) -> Vec<F::Output>`（依赖 W1 + 泛型集合；并发轮询，非线程版）；`timeout<F: Future>(d, fut) -> Result<F::Output, TimeoutError>` + `TimeoutError` 类型（§12 错误体系扩展），替代 `Err(-1)` 退化 | ✅ 完成 | [`w4-join-all-timeout.md`](../tasks/leaf/w4-join-all-timeout.md) |
| W5 | **`recv_async`/HTTP async 真异步**：`Receiver::recv_async` 挂起直到数据/close（事件驱动，替代阻塞等价语义）；`HttpClient::get_async`/`post_async` 经 NIO 非阻塞连接 + 事件驱动读写（依赖 W3 + Y2） | ✅ 完成 | [`w5-recv-http-async.md`](../tasks/leaf/w5-recv-http-async.md) |
| W6 | **async 泛型/递归 + 闭包跨线程捕获**：async fn 泛型参数（依赖 U3）、递归 async fn（状态机字段自引用，防无限结构体——指针槽）；闭包值跨线程捕获（`Thread::start` 接收有捕获闭包，§10.1 约束消除，依赖 H5 扩展） | ✅ 已完成 | [`w6-async-generic-recursion.md`](../tasks/leaf/w6-async-generic-recursion.md) |

**验收**：见任务树 [`stage-u-z.md`](../tasks/stage-u-z.md) 各子任务叶子的「验证」字段；全量回归通过。
