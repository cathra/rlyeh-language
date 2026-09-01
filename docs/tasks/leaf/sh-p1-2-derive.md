# SH-P1-2 trait derive 宏

> **级别**：P1（阻塞前端自举） · **状态**：⏳ 规划中 · **归属**：0.2.0-C
> **索引**：[`../self-hosting-p1.md`](../self-hosting-p1.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P1-5 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.3

## 目标
提供 `#[derive(Debug/Clone/PartialEq)]` 等**编译期 trait 自动实现**框架，消除 AST/HIR/MIR/LIR 的 `derive(Debug/Clone/PartialEq)`×70+ 样板，使 Rlyeh 侧重写这些 IR 时可表达等价派生。

## 技术细节
- 当前 Rlyeh 0.1.0：「无 trait derive 宏」（见 `docs/guide/13-references-limits.md`）。已有 `macro_rules!` 声明式宏（I1），本任务扩展为**属性宏 + 编译期 trait 自动实现**。
- 受影响 Rust 代码（事实依据，来自 `crates/` 核查）：`rlyeh-ast/src/lib.rs` `derive`×33、`rlyeh-hir`×15、`rlyeh-typecheck/types.rs`×10、工作区 `serde`/`clap` 的 `features=["derive"]`。
- 子任务（对应 0.2.0-C）：C1 `#[derive(Debug)]` / C2 `#[derive(Clone)]` / C3 `#[derive(PartialEq)]` / C4 derive 宏框架（用户可扩展）。

## 受影响组件
所有 IR crate（ast/hir/mir/lir/desugar/typecheck）、编译器内 `Debug`/`Clone`/`PartialEq` 派生点。

## 验证
- 单元：Rlyeh 侧 `#[derive(Debug)]` 后可用 `dbg!`；`#[derive(Clone)]`/`PartialEq` 等价手写实现。
- 验收：编译器改动后 `cargo test --workspace` 全绿。

## 状态
⏳ 规划中（0.2.0 必须项，阶段 C）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P1-5 拆出为叶子 |
