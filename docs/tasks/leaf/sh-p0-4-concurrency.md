# SH-P0-4 并发原语（Arc<Mutex>/atomic/线程 spawn）

> **级别**：P0（阻塞全栈自举） · **状态**：✅ 已完成（单元验证） · **归属**：0.2.0-H
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P0 / §5 长期跟踪项

## 目标
提供 `Arc<Mutex<T>>`/`Weak` 内部可变性、原子类型（`Atomic*`）与线程 `spawn` 能力，使 actor 运行时与 driver 的并发模型可用 Rlyeh 表达（与 SH-P0-2 跨边界闭包协同）。

## 技术细节
- 当前 Rlyeh 0.1.0 有 `Arc<T>`/`Weak<T>`（K3），但**无 `Mutex` 内部可变性原语、无原子类型、无线程 `spawn` 一等支持**。
- 受影响 Rust 代码（事实依据）：
  - `rlyeh-actor-runtime/src/runtime.rs:31` `Arc<Mutex<Option<Box<dyn ActorState>>>>`
  - `rlyeh-actor-runtime` 无锁并发依赖 `crossbeam`/`dashmap`（P2-1 外部 crate 等价项）
  - `rlyeh-driver` 增量编译缓存的并发访问
- 需设计：互斥锁 / 读写锁原语、原子类型与内存序、线程 `spawn`（接收跨边界闭包，依赖 SH-P0-2）。

## 受影响组件
`rlyeh-actor-runtime`（并发状态）、`rlyeh-driver`（增量编译并发）、未来 Rlyeh 版标准库并发原语。

## 验证
- 单元：Rlyeh 侧多线程 + `Arc<Mutex>` 计数器无数据竞争；原子自增正确。
- 对拍：等价于 actor-runtime 并发 mailbox 访问。

## 状态
✅ 已完成（单元验证）（0.2.0 必须项，阶段 H）。

## 实现纪要（2026-09-02 完成）

### 缺口复核（动手前的事实核对）
规划文档称「无 `Mutex` 内部可变性、无原子类型、无线程 `spawn`」，复核时发现
**仅原子类型为真缺口**：
- `Mutex<T>` / `RwLock<T>` + 守卫（`MutexGuard` / `RwLockReadGuard` /
  `RwLockWriteGuard`，desugar 作用域自动解锁）+ `Condvar` / `Barrier` /
  `Channel`（无界 / 有界 / 异步 `recv_async`）已在
  `crates/rlyeh-std/rlyeh/sync/module.rl` 落地（B4/P/P1/P5/Y4b-3/Y4c/W5 等）；
- `Thread::start(move || ..)` 跨线程执行已由 SH-P0-2 F-M4 落地。

故本叶子的实际交付为 **H-M2 原子类型 + 内存序**（H-M1/H-M3 已具备，
H-M4 为其验收）。

### H-M2：`AtomicI64` + `Ordering`
- **底层**：原子读改写无对应 C 链接符号（C11 `<stdatomic.h>` 的
  `atomic_fetch_add` 为泛型宏，不可链接），故沿用 `__rlyeh_*` 机制由 driver
  注入 LLVM IR —— `rlyeh-driver/src/platform_ir.rs::atomic_builtin_ir`：
  - RMW 族（`swap` / `fetch_add` / `fetch_sub` / `fetch_and` / `fetch_or` /
    `fetch_xor`）→ `atomicrmw <op> ... seq_cst, align 8`，均返回**旧值**；
  - `cas` → `cmpxchg ... seq_cst seq_cst`，返回旧值（调用方与 expected 比较判成功）；
  - `load` / `store` 各提供 `seq_cst` / `acquire`(仅 load) / `release`(仅 store) /
    `relaxed` 变体，供内存序分派。
  - 指针句柄为 `i64`（std 侧 `calloc(1, 8)` 分配，8 字节对齐），注入体内
    `inttoptr` 还原为 `i64*`。
- **语言层**：`crates/rlyeh-std/rlyeh/sync/module.rl` 新增
  `enum Ordering { Relaxed, Acquire, Release, AcqRel, SeqCst }` 与
  `struct AtomicI64 { p: i64 }`；`core.rl` 加 `__rlyeh_atomic_*` extern 声明
  与 `import sync::{AtomicI64, Ordering}`。
- **API**：`new` / `load` / `store` / `swap` / `fetch_{add,sub,and,or,xor}` /
  `compare_and_swap`（返回旧值）/ `compare_exchange`（返回 `bool`）/
  `load_with(order)` / `store_with(v, order)`。
- **内存序**：RMW 与 CAS 固定 **SeqCst**（最强序，跨线程 total order）；
  `load` 支持 Relaxed/Acquire/SeqCst，`store` 支持 Relaxed/Release/SeqCst
  （按 Ordering 分派到对应的注入变体）。
- 实现中修掉一个自造缺陷：`new(v)` 分配缓冲后未写入初始值（`calloc` 零初始化
  仅对 `new(0)` 正确），已改为构造后经 `__rlyeh_atomic_store_i64_seq_cst` 落值。

### 验收
- `tests/run-pass/atomic_i64.rl`（+`.out`）：读改写族旧值语义、位运算族、
  CAS 成功/失败、`load_with` / `store_with` 三种内存序。
- `tests/run-pass/concurrent_counter.rl`（+`.out`）：H-M4 并发计数器——
  两线程各自增 2000 次，`Arc<AtomicI64>`（无锁）与 `Arc<Mutex<i64>>`
  （`lock_guard` 守卫）两路径最终计数均恰为 4000（丢失更新会偏小），
  连跑 3 次结果稳定。
- `tests/compile-fail/atomic-no-such-method.rl`：锁定 API 边界——
  `AtomicI64` 不提供 `get` 等非原子访问器。
- 全量 `cargo test --workspace -- --test-threads=1` 740 用例全绿。

### 已知限制
- 仅 `AtomicI64`（i64 字长）；`AtomicBool` / `AtomicUsize` / `AtomicPtr` 未实现。
- 无 `fetch_update` / `fetch_max` / `fetch_min` 等派生 API；
  无 `compare_exchange_weak`（无伪失败变体，因 `cmpxchg` 强语义已满足 MVP）。
- `Ordering::AcqRel` 在 `load_with` / `store_with` 分派中归入 SeqCst
  （AcqRel 对纯 load / 纯 store 无独立语义）。
- 原子对象无析构（与 `Mutex` 一致：缓冲由 OS 在进程退出时回收）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从 §5 长期跟踪项提升为 P0-4 叶子（0.2.0 能力补齐） |
| 2026-09-02 | 复核确认 H-M1/H-M3 已具备，实现 H-M2 `AtomicI64` + `Ordering` 内存序；H-M4 并发计数器验收通过（740 全绿） |
