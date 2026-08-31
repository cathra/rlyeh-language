# 阶段 O — 网络对象化

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-m-t.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：已从仅自由函数（`tcp_connect`/`socketpair_stream`/`send_all`/`recv_some`/`hostname`）升级为完整 TCP 对象 + SocketAddr + HTTP 同步 MVP。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| O1A | **`SocketAddr`**：`SocketAddr::new(ip, port)` + `ip()`/`port()` 访问器（字符串 IP 解析 + 端口打包） | ✅ 已完成 | [`o1a-socketaddr.md`](../tasks/leaf/o1a-socketaddr.md) |
| O1B | **`TcpListener`**：`TcpListener::bind(addr)`/`accept`/`local_addr`；Rust 绑定层 `rlyeh-std/src/net/` 扩展（现有自由函数保留兼容） | ✅ 已完成 | [`o1b-tcp-listener.md`](../tasks/leaf/o1b-tcp-listener.md) |
| O1C | **`TcpStream`**：`TcpStream::connect`/`peer_addr`/`shutdown` + 现有 `tcp_connect` 升级为返回 `TcpStream`（自由函数兼容壳） | ✅ 已完成 | [`o1c-tcp-stream.md`](../tasks/leaf/o1c-tcp-stream.md) |
| O2 | **字节读写**：`read(&mut [u8])`/`write(&[u8])`（数组切片实参，复用 G1 切片；TcpStream 与 File 共用实现）+ 逐行读取 helper | ✅ 已完成 | [`o2-byte-rw.md`](../tasks/leaf/o2-byte-rw.md) |
| O3A | **HTTP 基础**：`HttpClient`（`get`/`post`：URL 解析 + 请求头构造 + 连接复用） | ✅ 已完成 | [`o3a-http-basic.md`](../tasks/leaf/o3a-http-basic.md) |
| O3B | **HTTP `Response`**：`text()`/`status` + `json::<T>()` 反序列化（依赖 L2 json） | ✅ 已完成 | [`o3b-http-response.md`](../tasks/leaf/o3b-http-response.md) |

**验收**：见任务树 [`stage-m-t.md`](../tasks/stage-m-t.md) 各子任务叶子的「验证」字段；全量回归通过。

**执行记录（2026-08）**：见任务树 [`stage-m-t.md`](../tasks/stage-m-t.md) O 阶段叶子文档（`o1a`–`o3b`，含网络绑定层、sockaddr 双布局、`Result` 化、HTTP 连接模型等实现细节）。
