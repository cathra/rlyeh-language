# EH-1 `?` + `From` 已落地复核与文档修正

> **级别**：P3 · **风险**：🟢 低 · **状态**：🟡 待办（复核/收尾） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.1 · **关联**：`sh-p1-6-question-from.md`、`u4-self-return.md`、`m2b-error-convert.md`

## 目标
复核 `?` 运算符 + `From` 自动转换的落地状态（调用侧 + 定义侧 `-> Self` 返回），修正 `std-lib.md` §12 中 U4「未支持」的过时标注，并补充对拍验证。

## 现状
- 调用侧：`expr?` desugar 为 `match { Ok(v) => v, Err(e) => return Err(From::from(e)) }`（`rlyeh-typecheck/src/check_expr/index_enum.rs` `check_question`）。
- 定义侧：`-> Self` 返回（U4）与 `impl Y: From<X>` 均已支持（run-pass：`self_return.rl`/`mem_take.rl`/`error_conversion.rl`；std 内置 `impl IoError: From<IoErrorKind>`，`io/error.rl`）。
- `std-lib.md` 概览 §12 原写「`-> Self` 返回未支持（归属 U4）」及 §12 代码块用 `description`/`cause`——**已过时**，由本叶子修正。

## 验证
- 单元：内层 `IoError` → 外层 `Result<T, CompileError>` 经 `?` 自动 `From` 传播；对拍现有 run-pass 套件（`error_conversion.rl`）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子；修正 std-lib.md §12 过时标注 |
