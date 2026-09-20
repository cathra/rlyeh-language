# Comptime M1 `comptime` 块 / 函数求值

> **级别**：P1 · **风险**：🔴 高 · **状态**：🟡 待办 · **归属**：0.2.0-Z
> **索引**：[`../rfc/comptime.md`](../rfc/comptime.md) §5 M1 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.26

## 目标
接入类型检查后的 HIR：`comptime { }` 块与 `comptime fn` 在全部实参编译期已知时解释执行，结果回填 AST（值→字面量/常量，类型→类型注解/单态化输入）交回既有 codegen；引入最小内建集（`@typeOf`/`@sizeOf`/`@as`）；建立差分对拍 harness。

## 现状
无 comptime 求值入口。

## 验证
corpus-comptime-1：`comptime fn` 算术/控制流/递归（`fib`）、`comptime { }` 块求值；gold oracle 逐值对拍（`tests/self-host-comptime/`）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | RFC 评审通过，并入 0.2.0-Z，建叶子 |
