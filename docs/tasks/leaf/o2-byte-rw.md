# O2 字节读写

> **所属阶段**：阶段 O
> **状态**：✅ 已完成
> **依赖**：O1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

TCP/File 字节读写。

## 背景

阶段 阶段 O 子任务，详见 阶段详情文档 [`stages/O.md`](../../stages/O.md)。

## 技术细节

`read(&mut [u8])`/`write(&[u8])`（数组切片实参，复用 G1 切片；TcpStream 与 File 共用实现）+ 逐行读取 helper。

## 验证

`tcp_echo.{rlyeh,out}`（read/write/read_line/shutdown）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
