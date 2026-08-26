# H1 函数类型与函数指针

> **所属阶段**：阶段 H
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`fn(A) -> B` 函数类型 + 函数一等值全链路。

## 背景

阶段 阶段 H 子任务，详见 阶段详情文档 [`stages/H.md`](../../stages/H.md)。

## 技术细节

parser → typecheck `Type::Fn` 签名 + `infer_expr` 函数值推断 + `check_indirect_call` 兜底 → HIR `FnPtr`/`CallIndirect` → MIR/LIR `CallIndirect` → LLVM：函数指针统一存 `i8*` 槽，存储前按签名 `bitcast i64(...)* @fn to i8*`，调用时 bitcast 回后间接 call。关键修复：MIR DCE 漏算 CallIndirect 实参、MIR inline 缺 CallIndirect 分支、borrowck/regionck 补分支、`mir_lower_test` match。

## 验证

`compile-pass/fn_ptr.rl` + `run-pass/fn_ptr.{rlyeh,out}` 5 输出 + 全量 17 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
