# Y3 HTTP 连接复用 + sendfile 平台补全

> **所属阶段**：阶段 Y
> **状态**：🔧 部分完成
> **依赖**：O3、R3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`HttpClient` 连接池（keep-alive）+ sendfile Windows 分支。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`net/http.rl` 重构：`HttpClient` 加池字段 `{conn_fd, conn_host, conn_port}`，`get/post`/`get_async`/`post_async` 改实例方法（`&mut self`），新增 `request_one`/`read_response`/`find_header_end`/`parse_content_length`：请求带 `Connection: keep-alive`、body 按 Content-Length 精确读（无则回退 EOF）、同 host:port 复用 + 失效自动丢弃重建重试一次。**sendfile Windows `TransmitFile` 分支保留**——本机无法编译验证，避免交付未验证分支，非 Unix 仍返回 Unsupported。

## 验证

`http_keepalive_test.rs` 5 用例（复用/双实例/失效重试/POST 复用/无 CL 回退）+ net_http_test.rs 5 用例迁移（10/10 全绿）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
