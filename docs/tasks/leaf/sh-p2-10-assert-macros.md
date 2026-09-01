# SH-P2-10 `panic!` / `assert!` / `unreachable!` / `todo!` 宏

> **级别**：P2 · **风险**：🟡 低 · **状态**：⏳ 规划中 · **归属**：0.2.0-W
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.23

## 目标
提供开发期断言/终止宏 `panic!(msg)` / `assert!(cond)` / `assert_eq!(a, b)` / `unreachable!()` / `todo!()`，desugar 为格式化打印 + 进程终止（或崩溃信号），对齐 Rust 编译器内部断言习惯（actor-model.md 明记「`panic!` 宏未实现，属规划」）。

## 现状
- `panic!` 宏未实现（actor 方法靠返回 `-1` 触发崩溃协议替代）；`assert!`/`unreachable!`/`todo!` 仅在设计文档的 Rust 示例出现，非 Rlyeh 内建。

## 风险分解（→ 低危）
- **L1（低）** `panic!(msg)` / `unreachable!()` / `todo!()`：desugar 为 `eprintln!`(stderr) + 终止（复用 N4 stderr 宏 + 进程退出内建）；`assert!(cond[, msg])` / `assert_eq!(a, b)`：条件假则 `panic!`。
- **L2（低）** 差分对拍：断言触发时机与 Rust 一致（仅需语义对齐，无新 IR 节点）。

## 受影响组件
`rlyeh-macro`（宏声明）、`rlyeh-typecheck`（desugar）、`rlyeh-std`（打印/退出内建）。

## 验证
- 单元：`assert!(1 + 1 == 2)` 通过；`assert_eq!(x, y)` 不等触发 panic 终止。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从编译器内部断言依赖中拆出 |
