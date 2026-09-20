# SH-P3-1 `Send` / `Sync` 自动 trait（放宽/标记）

> **级别**：P3（并发安全地基，可放宽） · **风险**：🟠 中 · **状态**：✅ M1/M2/M3/L1 已落地（告警式并发安全基线） · **归属**：0.2.0-Y
> **索引**：[`../self-hosting.md`](../self-hosting.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.25

## 目标
支持 `Send` / `Sync` 标记 auto-trait 声明与基本自动推导，在并发原语（H）下对 `spawn` / `Arc<Mutex<T>>` 等施加 `T: Send + Sync` 约束，提供线程安全基线（MVP 可放宽/告警式，不阻断编译）。

## 现状
- Rlyeh 无 `Send`/`Sync` 概念；actor-runtime 的 `F: Fn() + Send + Sync + 'static`（P0-2 事实）在语言层无对应约束；当前并发靠运行时保证，编译期无检查。

## 风险分解（→ 中/低危）
- **M1（中）** ✅ `protocol Send {}` / `protocol Sync {}` 标记 auto-trait 声明（Rlyeh 用 `protocol` 关键字，`trait` 已从语法移除；空 body trait 经 `collect_trait` 正常收集）。声明置于 std 根 `module.rl`，全局可见，供用户书写 `T: Send` / `T: Sync` 约束占位。
- **M2（中）** ✅ 自动推导：新增 `is_send_sync(ty, ctx)` 内建谓词（types.rs），覆盖标量 / 字符串 / 单元 / never / 函数指针、常见并发包装器（`Arc`/`Mutex`/`RwLock`/`AtomicI64`/`Vec`/`String`/`Condvar`/`Barrier`/通道/`Future` 等递归内部类型）、结构体（全部字段递归）/ 枚举（全部变体字段递归）、元组 / 数组 / 闭包捕获；`Rc`/`Weak` 与裸指针 / 引用 / `dyn` 判定为非 Send+Sync。带递归深度护栏（`depth > 32` 保守通过，防 `Box<Node>` 自引用无限展开）。未知具名类型乐观通过（不误报）。
- **M3（中）** ✅ `Thread::start(move || ..)` 并发边界检查（`check_move_closure_spawn`，thread.rs）：在既有 `'static` 硬错检查之后，对每个捕获类型调用 `is_send_sync`，不满足时经 `ctx.emit_warning(WarningKind::NotSendSync)` 发出 **W002 告警**（位置对准 `Thread::start` 调用处，附 `= help:` 建议），**不阻断编译**。
- **L1（低）** ✅ 并发用例：`tests/run-pass/send_sync_thread.rl`（跨线程共享 `Arc<Mutex<i64>>`，确定性输出 `1`）+ `concurrent_counter.rl`（双线程各 2000 次自增无丢失）；`crates/rlyeh-typecheck` 单测 `send_sync_predicate` 直接校验 `is_send_sync` 各分支（标量/元组/数组/包装器→`true`；裸指针/引用/`Rc`→`false`）。

> **告警式说明（MVP 放宽）**：当前 `Send`/`Sync` 为编译器内建特判的 auto-trait（与 Rust 一致，不由用户 `impl` 触发），其推导为**启发式**且未满足时仅**告警**不硬错——待类型系统成熟（如约束可收紧为硬阻塞、支持 trait 对象 `dyn T + Send` 精确判定、覆盖更多 `std` 类型）后再升级。通道 `Sender<T>`/`Receiver<T>` 处暂未接入（MVP 聚焦 `Thread::start` 跨线程闭包边界）。

## 受影响组件
`rlyeh-typecheck`（auto-trait 推导/约束）、`rlyeh-actor-runtime` / `rlyeh-driver`（并发边界）。

## 验证
- 单元：跨线程传 `Arc<Mutex<i64>>` 计数器无数据竞争且通过 `Send+Sync` 检查。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从并发安全地基依赖中拆出（可放宽，非硬阻塞） |
| 2026-09-20 | M1/M2/M3/L1 落地：std 根声明 `protocol Send {}`/`protocol Sync {}`；`is_send_sync` 内建谓词（types.rs，含递归护栏）；`Thread::start(move || ..)` 边界经 W002 告警式检查（thread.rs + warning.rs）；run-pass `send_sync_thread.rl` + 单测 `send_sync_predicate` 覆盖 |
