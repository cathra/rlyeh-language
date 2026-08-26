# W2 await 状态机扩展

> **所属阶段**：阶段 W
> **状态**：✅ 已完成
> **依赖**：U1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

await 位置扩展到控制流块内 + 表达式嵌套 + 跨 await 引用类型。

## 背景

阶段 阶段 W 子任务，详见 阶段详情文档 [`stages/W.md`](../../stages/W.md)。

## 技术细节

`analyze.rs` 状态机从「直线切段」升级为「控制流图展开」——if/match/loop/for/while 子块按 await 点递归展开为扁平段列表（段带显式后继 `next`、`CONT_PENDING` 汇合占位回填）；表达式嵌套 await 经 `extract_expr_awaits` 临时变量提取；跨 await 变量类型扩展到 f64/bool/char/String。验收修复：pattern_has_bindings 误判 `_`、尾表达式 flush_tail 合并、String 字段零值用 `String::from("")`。

## 验证

`w2_probe.rl`（match 内 await + 跨 await String/bool）+ `async_if_await`/`async_loop_await`/`async_while_min` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
