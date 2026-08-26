# W4 `join_all`/`timeout` Future 版

> **所属阶段**：阶段 W
> **状态**：✅ 已完成
> **依赖**：W1、U3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`join_all<F: Future> -> Vec<F::Output>` + `timeout -> Result<F::Output, TimeoutError>`。

## 背景

阶段 阶段 W 子任务，详见 阶段详情文档 [`stages/W.md`](../../stages/W.md)。

## 技术细节

`TimeoutError` 类型 + `timeout` 返回 `Result<F::Output, TimeoutError>` + `future::join_all<F: Future>(Vec<F>) -> Vec<F::Output>`（并发轮询直至全部 Ready，结果用 `HashMap<i64, F::Output>` 按序收集）。**`F::Output` 关联类型投影完整实现**——`Type::AssocProjection` + `resolve_ast_type` 识别 + 实例化求值；前置修复 `check_generic_bounds` 泛型 bound 裸名解析（`F: Future`）。

## 验证

`join_all_fut.{rl,out}` + `join_all_any.{rl,out}`（Vec<String>）+ `join_all_fut_test.rs` 4 用例 + `time_test` 5/5 全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
