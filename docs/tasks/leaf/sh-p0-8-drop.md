# SH-P0-8 `Drop` trait / 析构 / RAII

> **级别**：P0（阻塞运行时重写 + IR 资源清理） · **风险**：🔴 高 · **状态**：⏳ 规划中 · **归属**：0.2.0-Q
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.17

## 目标
支持 `trait Drop { fn drop(&mut self); }` + `impl Drop for T`，作用域结束（拥有所有权变量）自动调用 `drop`，实现 RAII：MutexGuard 自动解锁、arena 自动释放、`Rc`/`Arc` 计数递减、`File` 自动关闭。Rlyeh 0.1.0 完全无析构机制（MVP 靠注入解锁/OS 回收，std-lib.md 多处标注「无 Drop」）。

## 现状
- 语言无 `Drop` trait；codegen 不发射作用域尾清理。
- 受影响 Rust 事实：`sync/module.rl` 目标 `MutexGuard` 作用域尾自动解锁、`rlyeh-std` 多处「进程退出时由 OS 回收（无 Drop）」、`Vec`/`HashMap` 析构。

## 风险分解（→ 中/低危）
- **M1（中）** `trait Drop { fn drop(&mut self); }` 声明 + `impl Drop for T` 检测（与现有 trait/impl 机制复用，需 A 泛型 trait 落地后接）。
- **M2（中）** 作用域尾自动插入 `x.drop()`：仅对**拥有所有权**的栈变量，按声明逆序；临时值与借用变量跳过（借用检查 G1 协同）。
- **M3（中）** 字段级 drop：struct 含拥有字段（如 `Vec` 内部堆指针）在变量 drop 时递归 drop 其拥有字段。
- **M4（中）** 智能指针接入 `Drop`：`Box` 释放堆、`Rc`/`Arc` `strong_count-1`（归零释放）、`Gc` 块外逃逸登记。
- **L1（低）** 差分对拍 Rust 参考，确认 drop 调用时序与作用域一致。

## 受影响组件
`rlyeh-typecheck`（drop 插入点分析）、`rlyeh-codegen`（作用域清理代码发射）、`rlyeh-std`（MutexGuard/Vec/HashMap/File 接 Drop）。

## 验证
- Rlyeh 侧 `MutexGuard` 越界自动解锁、`arena` 越界自动释放（计数器/日志断言）。
- 单元：含拥有 `Vec` 字段的 struct 离开作用域触发递归释放，无泄漏。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从运行时重写 + IR 资源清理依赖中拆出（语言完全缺失） |
