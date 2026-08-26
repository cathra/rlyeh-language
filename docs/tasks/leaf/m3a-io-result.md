# M3a io 自由函数 Result 化

> **所属阶段**：阶段 M
> **状态**：✅ 已完成
> **依赖**：M2b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

io 自由函数升级为 `Result<T, IoError>`。

## 背景

阶段 阶段 M 子任务，详见 阶段详情文档 [`stages/M.md`](../../stages/M.md)。

## 技术细节

`read_file`/`write_file`/`append_file`/`read_line` 从『空串/-1』升级为 `Result<T, IoError>`（io.rl + 绑定层返回码映射）。

## 验证

`io_result.{rlyeh,out}`（成功路径 Result 解包 + `?` 传播）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
