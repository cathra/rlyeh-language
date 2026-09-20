# 阶段 M–T（标准库深度完善）

> **所属任务树**：[任务文档导航](./README.md)
> **状态**：✅ 全部完成（59 个子任务）
> **权威来源**：阶段详情文档 [`stages/M.md`](../stages/M.md)、[`stages/T.md`](../stages/T.md)；执行记录见本文件 §执行记录
> **本层职责**：记录阶段 M–T 的任务列表与进度 + 执行记录；具体实施见权威源文档（development-plan.md）。

---

## 任务列表与进度

| 阶段 | 主题 | 子任务（关键交付，按序） | 子任务数 | 依赖 | 状态 |
|------|------|---------|:---:|------|------|
| **M** | 错误处理基底 | `IoErrorKind`/`IoError`（M1a/M1b）、`Error`/`From`/`Into` protocol（M2a/M2b）、std 错误约定 Result 化（M3a/M3b/M3c） | 7 | K1、H4、G1 | ✅ 已完成 |
| **N** | 文件系统与 IO 对象化 | `File`/`OpenMode`（N1a/N1b/N1c）、stdin/stdout/stderr（N2a/N2b）、`Path`/`fs`（N3a/N3b/N3c）、`eprintln!`/`eprint!`（N4） | 9 | M、G1、I2 | ✅ 已完成 |
| **O** | 网络对象化 | `SocketAddr`/`TcpListener`/`TcpStream`（O1a/O1b/O1c）、字节读写（O2）、HTTP 同步 MVP（O3a/O3b） | 6 | M、L2、G1 | ✅ 已完成 |
| **P** | 并发通道与同步 | Channel 绑定/对象化/收尾（P1a/P1b/P1c）、锁 guard 语义（P2a/P2b）、`Condvar`/`Barrier`（P3） | 6 | K3、Mutex ✅；P3 依赖 S0 线程 | ✅ 已完成（P1a–P1c、P2a/P2b、P3） |
| **Q** | 序列化与格式化 protocol | serde protocol/derive（Q1a/Q1b/Q1c）、json 泛型 API（Q2a/Q2b）、`Display`/`Debug` + Formatter（Q3a/Q3b）、TOML（Q4） | 8 | I、L2、H4 | ✅ 已完成（Q1–Q4） |
| **R** | 高性能 IO | `Interest`/`Event`/`Poller`（R1a/R1b）、非阻塞（R2）、sendfile（R3） | 4 | O、nio ✅ | ✅ 已完成 |
| **S** | 异步运行时 | S0 线程支持（S0a–S0e）、`Future`/`Poll`/`block_on`（S1a/S1b/S1c）、`join_all`/`timeout`/`sleep`（S2a/S2b/S2c）、async channel/http（S3a/S3b） | 13 | P、R、S0 | ✅ 已完成（S0a–S0e、S1a–S1c、S2a–S2c、S3a/S3b） |
| **T** | 集合与迭代器收尾 | Vec/String/HashMap API 补齐（T1a/T1b/T1c）、`Iterator` protocol（T2）、智能指针收尾（T3a/T3b） | 6 | J、K、G | ✅ 已完成（T1a/T1b/T1c、T2、T3a/T3b） |

> 共 **59** 个子任务，字母后缀（a/b/c）须按序完成。阶段详情见权威源文档。

---

## 阶段详情索引

- 阶段 M–T 详细任务：阶段详情文档 [`stages/M.md`](../stages/M.md)、[`stages/T.md`](../stages/T.md)
- 阶段 M–T 执行记录：见本文件 §执行记录

---

## 子任务列表（按阶段分层）

> 每个阶段下列出其子任务（一个任务一个叶子文档，含具体执行情况与技术细节）：

### M — 错误处理基底

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| M1a `IoErrorKind` 枚举 | [`m1a-ioerrorkind.md`](leaf/m1a-ioerrorkind.md) | ✅ 已完成 |
| M1b `IoError` 结构 | [`m1b-ioerror.md`](leaf/m1b-ioerror.md) | ✅ 已完成 |
| M2a `Error` protocol | [`m2a-error-protocol.md`](leaf/m2a-error-protocol.md) | ✅ 已完成 |
| M2b 错误转换约定（`From`/`Into` 验证） | [`m2b-error-convert.md`](leaf/m2b-error-convert.md) | ✅ 已完成 |
| M3a io 自由函数 Result 化 | [`m3a-io-result.md`](leaf/m3a-io-result.md) | ✅ 已完成 |
| M3b net 自由函数 Result 化 | [`m3b-net-result.md`](leaf/m3b-net-result.md) | ✅ 已完成 |
| M3c 测试与示例迁移 | [`m3c-test-migrate.md`](leaf/m3c-test-migrate.md) | ✅ 已完成 |

### N — 文件系统与 IO 对象化

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| N1a `OpenMode` + 绑定层 | [`n1a-open-mode.md`](leaf/n1a-open-mode.md) | ✅ 已完成 |
| N1b `File` 对象化 | [`n1b-file-object.md`](leaf/n1b-file-object.md) | ✅ 已完成 |
| N1c 读写与元数据方法 | [`n1c-rw-metadata.md`](leaf/n1c-rw-metadata.md) | ✅ 已完成 |
| N2a stdout/stderr 模块 | [`n2a-stdout-stderr.md`](leaf/n2a-stdout-stderr.md) | ✅ 已完成 |
| N2b stdin 增强 | [`n2b-stdin.md`](leaf/n2b-stdin.md) | ✅ 已完成 |
| N3a `Path` 对象 | [`n3a-path.md`](leaf/n3a-path.md) | ✅ 已完成 |
| N3b `fs` 核心读写 | [`n3b-fs-rw.md`](leaf/n3b-fs-rw.md) | ✅ 已完成 |
| N3c `fs` 目录操作 | [`n3c-fs-dir.md`](leaf/n3c-fs-dir.md) | ✅ 已完成 |
| N4 `eprintln!`/`eprint!` 宏 | [`n4-eprintln.md`](leaf/n4-eprintln.md) | ✅ 已完成 |

### O — 网络对象化

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| O1a `SocketAddr` | [`o1a-socketaddr.md`](leaf/o1a-socketaddr.md) | ✅ 已完成 |
| O1b `TcpListener` | [`o1b-tcp-listener.md`](leaf/o1b-tcp-listener.md) | ✅ 已完成 |
| O1c `TcpStream` | [`o1c-tcp-stream.md`](leaf/o1c-tcp-stream.md) | ✅ 已完成 |
| O2 字节读写 | [`o2-byte-rw.md`](leaf/o2-byte-rw.md) | ✅ 已完成 |
| O3a HTTP 基础 `HttpClient` | [`o3a-http-basic.md`](leaf/o3a-http-basic.md) | ✅ 已完成 |
| O3b HTTP `Response` | [`o3b-http-response.md`](leaf/o3b-http-response.md) | ✅ 已完成 |

### P — 并发通道与同步

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| P1a 无界队列 Channel | [`p1a-channel-queue.md`](leaf/p1a-channel-queue.md) | ✅ 已完成 |
| P1b `Sender`/`Receiver` 对象化 | [`p1b-sender-receiver.md`](leaf/p1b-sender-receiver.md) | ✅ 已完成 |
| P1c `try_*`/`close`/`iter` | [`p1c-try-close-iter.md`](leaf/p1c-try-close-iter.md) | ✅ 已完成 |
| P2a 注入机制验证 | [`p2a-guard-inject.md`](leaf/p2a-guard-inject.md) | ✅ 已完成 |
| P2b `MutexGuard` 接线 | [`p2b-mutex-guard.md`](leaf/p2b-mutex-guard.md) | ✅ 已完成 |
| P3 `Condvar`/`Barrier` | [`p3-condvar-barrier.md`](leaf/p3-condvar-barrier.md) | ✅ 已完成 |

### Q — 序列化与格式化 protocol

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| Q1a `Serialize`/`Deserialize` protocol 定义 | [`q1a-serde-protocol.md`](leaf/q1a-serde-protocol.md) | ✅ 已完成 |
| Q1b derive 宏语法 | [`q1b-derive-macro.md`](leaf/q1b-derive-macro.md) | ✅ 已完成 |
| Q1c derive 生成接线 | [`q1c-derive-gen.md`](leaf/q1c-derive-gen.md) | ✅ 已完成 |
| Q2a 泛型 API 入口 | [`q2a-generic-api.md`](leaf/q2a-generic-api.md) | ✅ 已完成 |
| Q2b 流式 writer/reader | [`q2b-writer-reader.md`](leaf/q2b-writer-reader.md) | ✅ 已完成 |
| Q3a `Display`/`Debug` protocol + `Formatter` | [`q3a-display-debug.md`](leaf/q3a-display-debug.md) | ✅ 已完成 |
| Q3b 格式化引擎接入 | [`q3b-format-engine.md`](leaf/q3b-format-engine.md) | ✅ 已完成 |
| Q4 TOML 模块 | [`q4-toml.md`](leaf/q4-toml.md) | ✅ 已完成 |

### R — 高性能 IO

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| R1a `Interest`/`Event` 类型 | [`r1a-interest-event.md`](leaf/r1a-interest-event.md) | ✅ 已完成 |
| R1b `Poller` 封装 | [`r1b-poller.md`](leaf/r1b-poller.md) | ✅ 已完成 |
| R2 非阻塞 | [`r2-nonblocking.md`](leaf/r2-nonblocking.md) | ✅ 已完成 |
| R3 `sendfile` 模块 | [`r3-sendfile.md`](leaf/r3-sendfile.md) | ✅ 已完成 |

### S — 异步运行时

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| S0a 线程绑定 | [`s0a-thread-bind.md`](leaf/s0a-thread-bind.md) | ✅ 已完成 |
| S0b `Thread::start` | [`s0b-thread-start.md`](leaf/s0b-thread-start.md) | ✅ 已完成 |
| S0c `join` + 返回值传递 | [`s0c-thread-join.md`](leaf/s0c-thread-join.md) | ✅ 已完成 |
| S0d WASI/Windows 短路 | [`s0d-wasi-short.md`](leaf/s0d-wasi-short.md) | ✅ 已完成 |
| S0e 加固与文档化 | [`s0e-hardening.md`](leaf/s0e-hardening.md) | ✅ 已完成 |
| S1a `Future`/`Poll` protocol 定义 | [`s1a-future-poll.md`](leaf/s1a-future-poll.md) | ✅ 已完成 |
| S1b `block_on` 手动轮询 MVP | [`s1b-block-on.md`](leaf/s1b-block-on.md) | ✅ 已完成 |
| S1c `async fn` 状态机 | [`s1c-async-state-machine.md`](leaf/s1c-async-state-machine.md) | ✅ 已完成 |
| S2a `sleep` | [`s2a-sleep.md`](leaf/s2a-sleep.md) | ✅ 已完成 |
| S2b `join_all` + 墙钟 | [`s2b-join-all.md`](leaf/s2b-join-all.md) | ✅ 已完成 |
| S2c `timeout` | [`s2c-timeout.md`](leaf/s2c-timeout.md) | ✅ 已完成 |
| S3a Channel `recv_async` | [`s3a-recv-async.md`](leaf/s3a-recv-async.md) | ✅ 已完成 |
| S3b HTTP async 方法 | [`s3b-http-async.md`](leaf/s3b-http-async.md) | ✅ 已完成 |

### T — 集合与迭代器收尾

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| T1a Vec 目标 API 补齐 | [`t1a-vec-api.md`](leaf/t1a-vec-api.md) | ✅ 已完成 |
| T1b String 目标 API 补齐 | [`t1b-string-api.md`](leaf/t1b-string-api.md) | ✅ 已完成 |
| T1c HashMap 目标 API 补齐 | [`t1c-hashmap-api.md`](leaf/t1c-hashmap-api.md) | ✅ 已完成 |
| T2 `Iterator` protocol 定义 | [`t2-iterator-protocol.md`](leaf/t2-iterator-protocol.md) | ✅ 已完成 |
| T3a `Box::leak` | [`t3a-box-leak.md`](leaf/t3a-box-leak.md) | ✅ 已完成 |
| T3b Rc/Arc 目标 API + `Weak::upgrade` | [`t3b-rc-arc-api.md`](leaf/t3b-rc-arc-api.md) | ✅ 已完成 |

---
## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 阶段 M–T 纳入任务树（索引，权威源 development-plan.md） |
