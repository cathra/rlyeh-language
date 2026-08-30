# Y3 HTTP 连接复用 + sendfile 平台补全

> **所属阶段**：阶段 Y
> **状态**：✅ 完成（降级：sendfile Windows `TransmitFile` 分支因本机（macOS）不可编译验证保留，非 Unix 返回 Unsupported；`HttpClient` keep-alive 连接池已完整实现并 10/10 测试通过，2026-08-30）
> **依赖**：O3、R3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`HttpClient` 连接池（keep-alive）+ sendfile Windows 分支。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`net/http.rl` 重构：`HttpClient` 加池字段 `{conn_fd, conn_host, conn_port}`，`get/post`/`get_async`/`post_async` 改实例方法（`&mut self`），新增 `request_one`/`read_response`/`find_header_end`/`parse_content_length`：请求带 `Connection: keep-alive`、body 按 Content-Length 精确读（无则回退 EOF）、同 host:port 复用 + 失效自动丢弃重建重试一次。**sendfile Windows `TransmitFile` 分支保留**——本机无法编译验证，避免交付未验证分支，非 Unix 仍返回 Unsupported。

## 风险评估（2026-08-28）

sendfile Windows `TransmitFile` 分支为**不可验证项**（本机 macOS 无法编译验证 Windows 代码路径）。风险高——若交付未验证分支可能引入运行时错误。建议保持现状（非 Unix 返回 Unsupported），登记至专项文档，待 CI/Windows 环境统一验证。

## 验证

`http_keepalive_test.rs` 5 用例（复用/双实例/失效重试/POST 复用/无 CL 回退）+ net_http_test.rs 5 用例迁移（10/10 全绿）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-28 | 标注 sendfile Windows 分支不可验证（待专项） |
| 2026-08-30 | Y3 收口（降级）：`HttpClient` keep-alive 连接池（conn_fd/conn_host/conn_port 池 + `Connection: keep-alive` + request_one/read_response/find_header_end + 复用/失效重试一次）确认已实现且 10/10 测试通过；Windows `TransmitFile` 分支依风险评估有意保留（本机不可验证，非 Unix 返回 Unsupported），记为已知平台限制 |
