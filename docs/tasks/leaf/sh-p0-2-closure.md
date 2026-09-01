# SH-P0-2 跨函数边界闭包 + `move` + `'static`

> **级别**：P0（阻塞全栈自举） · **状态**：⏳ 规划中 · **归属**：0.2.0-F（2026-09-01 修正：P0 语言特性上移为 0.2.0 必须项）
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P0-2

## 目标
支持闭包作为一等值**跨函数边界传递**（作 fn 实参 / 返回值），`move` 语义生效，并支持 `'static` 生命周期约束，使 actor 调度器与 driver 线程模型可用 Rlyeh 表达。

## 技术细节
- 当前 Rlyeh 0.1.0：「捕获闭包值不跨函数边界（作 fn 实参/返回值报错）、`move` 忽略、无生命周期」（见 `docs/guide/13-references-limits.md` §闭包）。
- 受影响 Rust 代码（事实依据）：
  - `rlyeh-actor-runtime/src/runtime.rs:114` `F: Fn() -> Box<dyn ActorState> + Send + Sync + 'static`
  - `rlyeh-actor-runtime/src/scheduler.rs:67` `.spawn(move || Self::worker_loop(...))`
  - `rlyeh-actor-runtime/src/ffi.rs:293` `rt.spawn_supervised(move || cf.make(), strat)`
  - `rlyeh-actor-runtime/src/builtin.rs:75` `Box<dyn Fn() + Send + Sync>`
  - `rlyeh-driver/src/lib.rs:73-85` `std::thread::Builder::new().stack_size(64MB).spawn(move || {...})`
- 需设计：闭包值对象的跨边界捕获（环境堆分配 + 生命周期标注）、`move` 所有权转移语义、`'static` 约束检查。

## 受影响组件
`rlyeh-actor-runtime`（调度器 / supervisor / 线程 worker）、`rlyeh-driver`（增量编译线程模型）。

## 验证
- 单元：Rlyeh 侧 `spawn(move || ...)` 跨线程执行并通过超时保护的并发测试。
- 对拍：等价于 actor-runtime worker_loop 的 spawn 行为。

## 状态
⏳ 规划中（**0.2.0 必须项（语言特性）**，对应阶段 F；运行时*重写*属 0.3.0）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P0-2 拆出为叶子 |
