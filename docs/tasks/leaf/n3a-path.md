# N3a `Path` 对象

> **所属阶段**：阶段 N
> **状态**：✅ 已完成
> **依赖**：M、G1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`Path` 路径对象。

## 背景

阶段 阶段 N 子任务，详见 阶段详情文档 [`stages/N.md`](../../stages/N.md)。

## 技术细节

`crates/rlyeh-std/rlyeh/fs.rl`：`Path::new`/`join`/`parent`/`file_name`/`path_extension`/`exists`/`is_file`/`is_dir`（路径拼接/父目录/文件名/扩展名 + 存在性/文件/目录判断）。

## 验证

`path_ops.rl` 验收 + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
