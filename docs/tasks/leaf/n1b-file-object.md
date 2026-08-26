# N1b `File` 对象化

> **所属阶段**：阶段 N
> **状态**：✅ 已完成
> **依赖**：N1a
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`File` 对象封装句柄。

## 背景

阶段 阶段 N 子任务，详见 阶段详情文档 [`stages/N.md`](../../stages/N.md)。

## 技术细节

`File::open_with(path, mode)`/`File::create` + 句柄封装 + `close`（Rlyeh 无 Drop，MVP 显式 `close()` 语义；Y1 起 `File::open(path)` 为默认只读兼容壳）。

## 验证

`file_io.{rlyeh,out}` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
