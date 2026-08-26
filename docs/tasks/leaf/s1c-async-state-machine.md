# S1c `async fn` 状态机

> **所属阶段**：阶段 S
> **状态**：✅ 已完成
> **依赖**：S1b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

L1 同步语义改造为真实状态机。

## 背景

阶段 阶段 S 子任务，详见 阶段详情文档 [`stages/S.md`](../../stages/S.md)。

## 技术细节

desugar（`rlyeh-desugar`：`analyze.rs` 段切分 await 位置 + 跨 await 变量提升 + 数据依赖拓扑排序 + `generate.rs` 生成 `struct __Fut_N` + `impl Future` poll 状态机 + 构造器）；状态编号 `2k`/`2k+1`（首轮询/恢复），Ready `2k+2`、Pending `2k+1`；block_on 轮询驱动。

## 验证

`async-fns.{rlyeh,out}`（状态机语义）+ `async_await.{rlyeh,out}`（手写状态机与 desugar 同构对照，输出 43）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
