# N1c 读写与元数据方法

> **所属阶段**：阶段 N
> **状态**：✅ 已完成
> **依赖**：N1b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`File` 读写与元数据方法。

## 背景

阶段 阶段 N 子任务，详见 阶段详情文档 [`stages/N.md`](../../stages/N.md)。

## 技术细节

`read_to_string`/`read(&mut [u8])`/`write(&[u8])`/`write_all`/`flush`/`metadata`/`size`。切片实参降级：`read(cap)`/`write(String)`，`&[u8]` 留待切片借用成熟；`metadata` MVP 返回文件大小。

## 验证

`file_io.{rlyeh,out}` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
