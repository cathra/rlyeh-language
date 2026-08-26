# B4 sync 模块

> **所属阶段**：阶段 B
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

pthread 锁绑定：`Mutex`/`RwLock`。

## 背景

阶段 阶段 B 子任务，详见 阶段详情文档 [`stages/B.md`](../../stages/B.md)。

## 技术细节

`Mutex { p: i64 }`/`RwLock { p: i64 }` 基于 extern FFI 绑定 pthread；原语承载于 `calloc` 缓冲（统一 256 字节保守分配，选 calloc 避 malloc redefinition）；extern `i32` 返回支持（`extern_ret32` 标记 + codegen `sext i32`）根治 int 返回高位未定义。顺带修复内联 pass 局部变量重命名 bug（`map_local` 非参数一律重命名）。

## 验证

`sync_test.rs` 6 用例：Mutex trylock EBUSY/临界区/循环、RwLock 读写共享/写锁排他/交替。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
