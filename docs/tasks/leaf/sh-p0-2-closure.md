# SH-P0-2 跨函数边界闭包 + `move` + `'static`

> **级别**：P0（阻塞全栈自举） · **状态**：🟢 完成（F-M1 无捕获闭包跨 fn + F-M2 `move` 生效 + F-M3 `'static` 校验 + F-M4 `spawn(move)` 跨线程执行，均落地并验证） · **归属**：0.2.0-F（2026-09-01 修正：P0 语言特性上移为 0.2.0 必须项）
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
🟢 完成（**0.2.0 必须项（语言特性）**，对应阶段 F；运行时*重写*属 0.3.0）。

- ✅ **F-M1 无捕获闭包跨 fn（复用 H2/H5）**：无捕获闭包值经 `try_closure_value_as_fn` 降级为 fn 指针，可作 fn 实参 / 返回值（H5 补全路径已实现）。
- ✅ **F-M2 `move` 所有权转移生效**：`move` 闭包（`CaptureMode::Move`）捕获环境按值（堆分配聚合对象）转移；`Type::Closure` 新增 `is_move` 字段记录捕获模式。跨线程 `Thread::start(move || ..)` 要求闭包为 `move`（有捕获的非 move 闭包报「spawn 需要 `move` 闭包」）。
- ✅ **F-M3 `'static` 约束检查**：跨线程闭包的捕获类型经 `type_contains_ref` 校验，禁止捕获 `&T`/`&mut T` 等借用引用（指向外层栈帧 → 悬垂指针），否则报「含借用引用（非 'static）」。
- ✅ **F-M4 `spawn(move || ...)` 跨线程执行**：`Thread::start(move || ..)`（零参 move 闭包）经 W6 机制（输入对象堆分配捕获环境 + `__thread_entry_N` thunk 新线程调用闭包）跨线程执行，等价于 actor-runtime `spawn(move || worker_loop)`。注：`spawn` 为 actor 派生保留关键字，线程启动统一用 `Thread::start`。
  - 验证：`tests/run-pass/thread-spawn-move.rl`（两 move 闭包跨线程捕获标量 + 拥有堆数据 String，`join` 校验返回值，输出 `spawn-ok`）+ `tests/compile-fail/thread-spawn-nomove.rl`（非 move 捕获闭包报 `move` 错误）+ `tests/compile-fail/thread-spawn-ref.rl`（捕获引用报 `'static` 错误）；完整套件 218/218 通过。

> 说明：一般「闭包作 fn 实参/返回值」所需的**闭包参数类型语法**（`fn f(cb: closure-type)`）仍规划中；当前跨边界闭包以 `Thread::start(move || ..)` 专用通道落地，满足自举 checkpoint（driver/actor 调度器的 `spawn(move)` 调用）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P0-2 拆出为叶子 |
| 2026-09-01 | 实现 F-M2/F-M3/F-M4：`Thread::start(move || ..)` 跨线程执行（捕获环境堆分配 + thunk + `'static` 校验拒绝捕获引用；`Type::Closure` 加 `is_move` 字段）；新增 `thread-spawn-move.rl`（run-pass）+ `thread-spawn-nomove.rl`/`thread-spawn-ref.rl`（compile-fail）；完整套件 218/218 通过；状态由 ⏳ 规划中 改为 🟢 完成 |
