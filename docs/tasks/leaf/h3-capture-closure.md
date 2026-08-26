# H3 捕获闭包（IIFE MVP）

> **所属阶段**：阶段 H
> **状态**：✅ 已完成
> **依赖**：H2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`(|x, y| body)(args)` 立即调用，外层变量按值捕获。

## 背景

阶段 阶段 H 子任务，详见 阶段详情文档 [`stages/H.md`](../../stages/H.md)。

## 技术细节

desugar 为匿名函数 `__closure_N(cap1, cap2, x, y)`（捕获变量作前置参数）+ 调用点普通函数调用，零新增 IR 节点。捕获收集迭代重查法（环境 = 已发现捕获 + 闭包参数，逐轮把 UndefinedVariable 中存在于外层环境的名称收为捕获并重查，循环收敛）；字符串字面量实参经 `check_string_from` 升级 String 语义。MVP 约束：仅 IIFE、`move` 忽略、无嵌套捕获。

## 验证

`capture_closure.{rlyeh,out}` 8 输出 + 全量 39 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
