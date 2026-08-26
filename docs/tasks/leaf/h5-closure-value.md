# H5 闭包值对象

> **所属阶段**：阶段 H
> **状态**：✅ 已完成
> **依赖**：H3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`let f = |x: i64| body; f(..)` 绑定后反复调用。

## 背景

阶段 阶段 H 子任务，详见 阶段详情文档 [`stages/H.md`](../../stages/H.md)。

## 技术细节

desugar：① 闭包匿名函数 `__closure_N(cap..., x)`（`emit_closure_fn`）；② 闭包值 = 捕获聚合对象（每捕获一槽）——绑定展开为 `HirExpr::Block`（Let + FieldSet 逐槽拷贝 + 尾 Variable），f 绑定为对象指针，类型 `Type::Closure { captures, params, ret, fn_name }`；③ 调用点 `f(args)` desugar 为 `__closure_N(FieldGet(f, 0, cap0)..., 实参...)`。重构：捕获收集提取 `check_closure_body_with_captures` + 匿名函数生成 `emit_closure_fn` 公共函数。MVP：参数须全注解、仅按值捕获、不跨函数边界。

## 验证

`closure_value.{rlyeh,out}` 14 输出 + 2 compile-fail + 全量 122 套件 681 测试。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
