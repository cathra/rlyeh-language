# U1 作用域栈重构 + 块级变量遮蔽

> **所属阶段**：阶段 U
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

typecheck 变量环境从单层 HashMap 重构为作用域栈。

## 背景

阶段 阶段 U 子任务，详见 阶段详情文档 [`stages/U.md`](../../stages/U.md)。

## 技术细节

typecheck 变量环境重构为作用域栈 `Vec<Scope>`（`Scope { vars, inits, dyn_concrete, is_fn }`），块级遮蔽 `let x` 语义落地。核心设计：① `push_scope(is_fn)`——true = 函数/闭包边界（查找不穿透）、false = 块/循环/match 臂（穿透）；② 存储槽 mangle——块内同名绑定返回 `name$N` 槽名；③ 全链路改用槽名。三处关键修复：fn 边界隔离顺序（`crossed_fn` 标志，修 15 个 run-pass SIGTRAP）、match 臂 `ref` 绑定类型注册、函数体 `check_block_inner`。

## 验证

`shadowing.{rlyeh,out}` 7 组输出 + `scoping_test.rs` + 全量 suite_test + cargo test 全绿 + clippy 0。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
