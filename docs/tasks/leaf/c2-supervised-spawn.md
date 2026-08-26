# C2 语言级受监督 spawn

> **所属阶段**：阶段 C
> **状态**：✅ 已完成
> **依赖**：C1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`Actor::new_supervised(strategy)` 受监督启动。

## 背景

阶段 阶段 C 子任务，详见 阶段详情文档 [`stages/C.md`](../../stages/C.md)。

## 技术细节

`new_supervised(strategy)`（0=OneForOne 1=AllForOne 2=RestartForOne）→ `rlyeh_actor_spawn_supervised("<__handle>", "<__state_new>", strategy)`（factory 传符号名，runtime 内部 dlsym）。修复 `String::from` NUL 终止 bug（原展开 alloc_bytes(len)+copy_bytes(len) 末尾无 NUL → alloc_bytes(len+1)+copy_bytes(len+1)）。

## 验证

C2 受监督 spawn 用例；`actor_test.rs` 全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
