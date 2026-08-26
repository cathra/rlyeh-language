# W1 Future 泛型化

> **所属阶段**：阶段 W
> **状态**：✅ 已完成
> **依赖**：U2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`trait Future { type Output; fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output>; }`。

## 背景

阶段 阶段 W 子任务，详见 阶段详情文档 [`stages/W.md`](../../stages/W.md)。

## 技术细节

`future.rl` trait 签名升级 `type Output` + `cx: &mut Context`；新增 `struct Context { _unit: i64 }` 占位；`block_on`/`timeout` 内 `let mut cx = Context { _unit: 0 };` + `f.poll(&mut cx)`；desugar `gen_impl` poll 签名/impl types/`Poll<Self::Output>` + `poll_call` `&mut *cx` 透传；10 处手写 impl（tests+examples）同步新签名。

## 验证

`async_full_test.rs`（W1/W2）+ suite_test 全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
