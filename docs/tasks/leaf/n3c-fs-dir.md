# N3c `fs` 目录操作

> **所属阶段**：阶段 N
> **状态**：✅ 已完成
> **依赖**：N3b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`fs` 目录操作与 `r#rename` 遮蔽修复。

## 背景

阶段 阶段 N 子任务，详见 阶段详情文档 [`stages/N.md`](../../stages/N.md)。

## 技术细节

`fs::remove_file`/`remove_dir_all`/`rename`/`create_dir`/`create_dir_all`/`read_dir`（目录条目迭代）。`rename` 经 `r#rename` 根命名空间显式引用修复模块内遮蔽——lexer 保留 `r#` 前缀 + parser 声明名归一化 + typecheck `resolve_callable` 跳过模块内优先。

## 验证

`fs_dir.rl` 验收：目录创建/遍历/删除 + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
