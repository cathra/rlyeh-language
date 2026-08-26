# S1a `Future`/`Poll` trait 定义

> **所属阶段**：阶段 S
> **状态**：✅ 已完成
> **依赖**：S0
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

定义 `Future`/`Poll` trait。

## 背景

阶段 阶段 S 子任务，详见 阶段详情文档 [`stages/S.md`](../../stages/S.md)。

## 技术细节

`enum Poll<T> { Ready(T), Pending }` + `trait Future { fn poll(&mut self) -> Poll<i64>; }`——关联类型 `type Output` / `Pin<&mut Self>` / `Context` 验证不可行（parser 无 trait `type` 成员、dyn 不可作函数参数），按计划退化指示 Output 固定 i64；`rlyeh-std/rlyeh/future.rl` + core.rl 重导出。

## 验证

`block_on.{rlyeh,out}`（S1a/S1b：Poll 构造/解构 + Future impl 三轮轮询 + block_on 手动轮询，输出 42/3）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
