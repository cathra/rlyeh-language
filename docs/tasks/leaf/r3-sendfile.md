# R3 `sendfile` 模块

> **所属阶段**：阶段 R
> **状态**：✅ 已完成
> **依赖**：O
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

零拷贝 sendfile 传输。

## 背景

阶段 阶段 R 子任务，详见 阶段详情文档 [`stages/R.md`](../../stages/R.md)。

## 技术细节

`sendfile` 模块 + `File::sendfile_to(sock_fd, offset)`：driver 注入 `__rlyeh_sendfile` 平台内建（macOS sendfile(2) 6 参签名，Linux/Windows/WASI 差异注入层屏蔽）；count=0 发到 EOF；in_fd 须为只读 fd，O_WRONLY 句柄报 EBADF，需 reopen Read 后调用。

## 验证

`nio_test.rs`（sendfile 自由函数与 `File::sendfile_to` 零拷贝传输）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
