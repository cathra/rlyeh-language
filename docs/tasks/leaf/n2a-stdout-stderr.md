# N2a stdout/stderr 模块

> **所属阶段**：阶段 N
> **状态**：✅ 已完成
> **依赖**：M
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

stdout/stderr 对象化输出。

## 背景

阶段 阶段 N 子任务，详见 阶段详情文档 [`stages/N.md`](../../stages/N.md)。

## 技术细节

`stdout`/`stderr`（`write`/`writeln`/`flush`）+ 新增 extern `write(fd,..)`；`flush` MVP no-op。

## 验证

`stdout_stderr.{rlyeh,out}`（N2）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
