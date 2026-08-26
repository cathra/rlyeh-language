# W6 async 泛型/递归 + 闭包跨线程捕获

> **所属阶段**：阶段 W
> **状态**：✅ 已完成
> **依赖**：U3、H5
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

async fn 泛型参数 + 递归 async fn + 闭包跨线程捕获。

## 背景

阶段 阶段 W 子任务，详见 阶段详情文档 [`stages/W.md`](../../stages/W.md)。

## 技术细节

泛型 async fn（Part 1a：`async fn echo<T>` 泛型透传到 `__Fut_echo<T>`/`impl<T> Future`）；递归 async fn（Part 1b：struct 前向声明地基 + desugar 拓扑打破依赖环 + 递归子 future 槽 `Box<__Fut_>`）；闭包值跨线程捕获（`Thread::start(f, arg)` typecheck 特判生成线程入口 thunk + 线程输入对象，MVP 限单参数闭包）。**W 收尾验证**：修复递归函数多子 future 槽（非递归 + 递归槽并存）零值构造缺字段漏洞——`gen_fields` 分两遍。

## 验证

`w_edge_probe.rl` + `async_depth_probe.rl`（深度 1000）+ compile-fail/thread-closure-multiparam.rl + 全量 130 用例 + W 系列 driver 测试全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
