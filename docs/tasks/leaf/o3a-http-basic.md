# O3a HTTP 基础 `HttpClient`

> **所属阶段**：阶段 O
> **状态**：✅ 已完成
> **依赖**：O2、L2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`HttpClient` 的 `get`/`post`。

## 背景

阶段 阶段 O 子任务，详见 阶段详情文档 [`stages/O.md`](../../stages/O.md)。

## 技术细节

`get`/`post`：URL 解析 + 请求头构造 + 连接；每次请求新建连接 + `Connection: close`（无连接复用，MVP）；`HttpClient::get/post` 经 `tcp_connect` + 手工结构体字面量构造。

## 验证

`net_http_test.rs`（O3：本地 mock 服务器，`http_get_json`/`http_post_echo`/`http_not_found_status` 3/3 通过）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
