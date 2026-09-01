# SH-P0-4 并发原语（Arc<Mutex>/atomic/线程 spawn）

> **级别**：P0（阻塞全栈自举） · **状态**：⏳ 规划中 · **归属**：0.2.0-H
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
⏳ 规划中（0.2.0 必须项，阶段 H）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从 §5 长期跟踪项提升为 P0-4 叶子（0.2.0 能力补齐） |
