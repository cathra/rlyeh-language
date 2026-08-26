# C1 actor desugar 全链路

> **所属阶段**：阶段 C
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`actor` 语法展开为普通结构 + 生成消息处理方法。

## 背景

阶段 阶段 C 子任务，详见 阶段详情文档 [`stages/C.md`](../../stages/C.md)。

## 技术细节

`expand_actor` 展开为普通结构 + 生成 `<Actor>::__state_new`（N 槽 calloc 缓冲 + 字段初值）/`__m<i>`（第 i 个方法：self 槽数组 + 消息槽参数）/`__handle`（按 kind 分发，槽 4/5 为 kind 与返回槽）/`__handle_message`；runtime `CallbackActor` 承载；方法返回 -1 = 崩溃信号（`u64::MAX` → Panic）；自动生成 `rlyeh_actor_spawn`/`spawn_supervised`/`ask`/`send` extern（用户显式声明则跳过）。排障：返回 -1 误触崩溃 → Panic 前先 `reply(0u64)`；supervisor 重启竞态 → 崩溃时保持 `running=true` 不放回旧 state。

## 验证

`actor_test.rs` 8 用例全绿；C1 阶段验收。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
