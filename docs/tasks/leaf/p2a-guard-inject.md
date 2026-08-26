# P2a 注入机制验证

> **所属阶段**：阶段 P
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

guard 局部变量作用域结束自动解锁注入。

## 背景

阶段 阶段 P 子任务，详见 阶段详情文档 [`stages/P.md`](../../stages/P.md)。

## 技术细节

编译器对 guard 局部变量作用域结束自动 `unlock` 注入（`rlyeh-desugar/src/guard.rs` 对方法名 `lock_guard` 特判，块尾注入；if/match 分支内提前 return/break 不注入）。

## 验证

`mutex_guard.{rlyeh,out}`（P2）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
