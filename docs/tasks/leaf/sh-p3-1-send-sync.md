# SH-P3-1 `Send` / `Sync` 自动 trait（放宽/标记）

> **级别**：P3（并发安全地基，可放宽） · **风险**：🟠 中 · **状态**：⏳ 规划中 · **归属**：0.2.0-Y
> **索引**：[`../self-hosting.md`](../self-hosting.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.25

## 目标
支持 `Send` / `Sync` 标记 auto-trait 声明与基本自动推导，在并发原语（H）下对 `spawn` / `Arc<Mutex<T>>` 等施加 `T: Send + Sync` 约束，提供线程安全基线（MVP 可放宽/告警式，不阻断编译）。

## 现状
- Rlyeh 无 `Send`/`Sync` 概念；actor-runtime 的 `F: Fn() + Send + Sync + 'static`（P0-2 事实）在语言层无对应约束；当前并发靠运行时保证，编译期无检查。

## 风险分解（→ 中/低危）
- **M1（中）** `trait Send {}` / `trait Sync {}` 标记 trait 声明（auto-trait 语义最简：仅标记）。
- **M2（中）** 自动推导：标量、`Box<T>`/`Arc<T>`/`Rc<T>`（T: Sync 时）等基本类型为 `Send+Sync`（放宽规则，先覆盖常见情形）。
- **M3（中）** `spawn` / `Arc<Mutex<T>>` / 通道处 `T: Send + Sync` 约束检查（MVP 默认满足、未满足时**告警**而非硬错，待类型系统成熟收紧）。
- **L1（低）** 并发用例：跨线程传 `Arc<Mutex<i64>>` 通过约束检查。

## 受影响组件
`rlyeh-typecheck`（auto-trait 推导/约束）、`rlyeh-actor-runtime` / `rlyeh-driver`（并发边界）。

## 验证
- 单元：跨线程传 `Arc<Mutex<i64>>` 计数器无数据竞争且通过 `Send+Sync` 检查。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从并发安全地基依赖中拆出（可放宽，非硬阻塞） |
