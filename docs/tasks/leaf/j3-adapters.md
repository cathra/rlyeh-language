# J3 适配器

> **所属阶段**：阶段 J
> **状态**：✅ 已完成
> **依赖**：J2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`map`/`filter`/`fold`/`collect`/`take`/`skip` 内建 desugar。

## 背景

阶段 阶段 J 子任务，详见 阶段详情文档 [`stages/J.md`](../../stages/J.md)。

## 技术细节

`check_method_call` 特判（接收者 = 数组/Vec/自定义迭代器）：`closure_return_ty`（闭包返回类型预推断 → 收集容器类型注解，避免 Vec<Infer>）+ `ty_to_ast` + `try_check_adapter`/`check_iterator_adapter`；闭包经 `check_closure_expected` 注入匿名函数；收集容器 `let mut __out: Vec<U> = Vec::new()`；循环复用现有分派；结果 Vec 可链式。

## 验证

`adapters.{rlyeh,out}` 13 输出（含链式）+ compile-fail/adapter-badargs.rl + 27 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
