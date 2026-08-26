# H2 无捕获闭包

> **所属阶段**：阶段 H
> **状态**：✅ 已完成
> **依赖**：H1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`|x, y| expr` desugar 匿名函数 + 函数指针，零运行时开销。

## 背景

阶段 阶段 H 子任务，详见 阶段详情文档 [`stages/H.md`](../../stages/H.md)。

## 技术细节

`check_closure_expected`：按预期 fn 签名取参数类型、`std::mem::take` 清空变量环境实现无捕获隔离、body 尾部表达式兼容返回类型、注册 `fn_signatures` + 注入 `HirItem::Fn`。接线三处：check_call/check_indirect_call 实参循环 + check_stmt Let 分支。捕获检测：body 引用外部变量改写 Unsupported「闭包捕获外部变量（H3 规划）」。

## 验证

`closure.{rlyeh,out}` 7 输出 + compile-pass/closure.rl + compile-fail/closure-capture.rl + 20 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
