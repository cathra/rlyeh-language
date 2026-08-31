# 阶段 S — 异步运行时（线程前置）

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-m-t.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：普通函数 `async fn`/`.await` 为 MVP 同步语义（L1 ✅，`async fn` ≡ 同步函数、`.await` ≡ 直接求值）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| S0A | **线程绑定**：driver 注入 `__rlyeh_thread_spawn`/`__rlyeh_thread_join`/`__rlyeh_thread_self`（pthread_create/join/self C ABI 封装，线程入口 `i64 (i8*)*`；替代原计划 `rlyeh-std/src/thread.rs` Rust 绑定层） | ✅ 已完成 | [`s0a-thread-bind.md`](../tasks/leaf/s0a-thread-bind.md) |
| S0B | **`Thread::start(f: fn() -> i64)`**：语言侧 `thread` 模块（`spawn` 为保留关键字，方法名取 `start`）；函数指针值按地址整数经 extern i64 形参传递（typecheck 放宽 Fn→I64 + codegen ptrtoint） | ✅ 已完成 | [`s0b-thread-start.md`](../tasks/leaf/s0b-thread-start.md) |
| S0C | **`join` + 返回值传递**：`Thread::join` 返回值槽读取（pthread_join i64*）；启动失败映射 `IoError`（M1b） | ✅ 已完成 | [`s0c-thread-join.md`](../tasks/leaf/s0c-thread-join.md) |
| S0D | **WASI/Windows 短路**：注入层 os 码 4/5 返回 -1 的 stub，`Thread::start` 返回 `Err`（Unsupported） | ✅ 已完成 | [`s0d-wasi-short.md`](../tasks/leaf/s0d-wasi-short.md) |
| S0E | **加固**：栈分配（默认栈大小确认）、线程局部状态确认、内存模型文档化（MVP 无 TLS 需求） | ✅ 已完成 | [`s0e-hardening.md`](../tasks/leaf/s0e-hardening.md) |
| S1A | **`Future`/`Poll` trait 定义**：`enum Poll<T> { Ready(T), Pending }` + `trait Future { fn poll(&mut self) -> Poll<i64>; }`——关联类型 `type Output` / `Pin<&mut Self>` / `Context` 验证不可行（parser 无 trait `type` 成员、dyn 不可作函数参数），按计划退化指示 Output 固定 i64；`rlyeh-std/rlyeh/future.rl` + core.rl 重导出 | ✅ 已完成 | [`s1a-future-poll.md`](../tasks/leaf/s1a-future-poll.md) |
| S1B | **`block_on` 手动轮询 MVP**：`block_on<T>(f: &mut T) -> i64` 泛型形态（loop+match 轮询，实例化时按具体类型解析 poll，无约束宽松语义） | ✅ 已完成 | [`s1b-block-on.md`](../tasks/leaf/s1b-block-on.md) |
| S1C | **`async fn` 状态机**：L1 同步语义改造为真实状态机（`await` 挂起/恢复）——desugar（`rlyeh-desugar`：analyze 段切分 + generate 结构体/impl/构造器生成），状态编号 `2k`/`2k+1`（首轮询/恢复），跨 await 变量提升，`block_on` 轮询驱动 | ✅ 已完成 | [`s1c-async-state-machine.md`](../tasks/leaf/s1c-async-state-machine.md) |
| S2A | **`sleep`**：`thread::sleep(Duration)`——注入 `__rlyeh_thread_sleep`（usleep 绑定，micros 截断 u32 上限约 71 分钟）；补 `Duration::seconds/milliseconds` 构造器；`Instant` 基准 clock()（CPU 时钟）睡眠期间不推进（墙钟随 S2b 接入） | ✅ 已完成 | [`s2a-sleep.md`](../tasks/leaf/s2a-sleep.md) |
| S2B | **`join_all` + 墙钟**：线程版 `thread::join_all(Vec<Thread>) -> Vec<i64>`（并发等待多线程，按传入顺序收集返回值）；墙钟 `__rlyeh_clock_monotonic`（clock_gettime CLOCK_MONOTONIC，注意 macOS 常量 =6 与 Linux =1 差异）接入，`Instant::now/elapsed` 睡眠期间推进（不支持平台退回 clock()）。Future 泛型版规划（依赖 S1c + 泛型方法） | ✅ 已完成 | [`s2b-join-all.md`](../tasks/leaf/s2b-join-all.md) |
| S2C | **`timeout`**：Future 超时包装——`timeout<T>(duration, &mut fut) -> Result<i64, i64>`（MVP 退化：`TimeoutError` 规划中，超时 `Err(-1)`；参数顺序对齐规划 API；超时判定经墙钟 `__rlyeh_clock_monotonic`，返回 -1 退回 clock()——MVP 静态方法调用不支持模块路径前缀故直用 extern；忙等轮询，事件驱动规划随 R1 Poller）。Future 版 `Result<F::Output, TimeoutError>` 规划（依赖 S1c） | ✅ 已完成 | [`s2c-timeout.md`](../tasks/leaf/s2c-timeout.md) |
| S3A | **Channel `recv_async`**：异步接收（依赖 P1 + S1/S2 事件循环） | ✅ 已完成 | [`s3a-recv-async.md`](../tasks/leaf/s3a-recv-async.md) |
| S3B | **HTTP async 方法**：`HttpClient` 异步接口（依赖 O3 + S2） | ✅ 已完成 | [`s3b-http-async.md`](../tasks/leaf/s3b-http-async.md) |

**验收**：见任务树 [`stage-m-t.md`](../tasks/stage-m-t.md) 各子任务叶子的「验证」字段；全量回归通过。
