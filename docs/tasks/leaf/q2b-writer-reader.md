# Q2b 流式 writer/reader

> **所属阶段**：阶段 Q
> **状态**：✅ 已完成
> **依赖**：Q2a、N、O
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`to_writer`/`from_reader` 流式接口。

## 背景

阶段 阶段 Q 子任务，详见 阶段详情文档 [`stages/Q.md`](../../stages/Q.md)。

## 技术细节

`json::to_writer(w, v)` → `w.write_all(json::stringify(v))`，返回 `Result<i64, io::error::IoError>`；`json::from_reader::<T>(r)` → `json::parse::<T>(r.read_to_string().unwrap())`（读失败经 `unwrap` 死循环 MVP 语义）；首参须 `File`/`&File`/`&mut File`，TcpStream 留待流式 read_all 方法化。

## 验证

`json_api.{rlyeh,out}`（to_writer 写文件 + from_reader 读文件 round-trip）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
