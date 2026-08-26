# 阶段 Y — IO/网络/并发/智能指针收尾

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-*.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：N/O/P/R 阶段已实现 File/Path/fs、TCP/HTTP、Mutex/RwLock/Condvar/Barrier/Channel、NIO（poll 版）、sendfile（macOS）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| Y1 | **File 目标 API**：`open_with(path, OpenMode)`（`open(path)` 保留兼容壳）；`read(&mut [u8])`/`write(&[u8])` 切片实参（§4.1 目标签名，替代 `read(cap)`/`write(String)` 降级——依赖切片借用成熟 + 数组切片参数化）；`metadata` 返回完整 `Metadata`（size/mtime/is_file/is_dir，替代仅返回大小的 MVP） | ✅ 部分完成 | [`y1-file-api.md`](../tasks/leaf/y1-file-api.md) |
| Y2 | **NIO 高性能后端**：`io/nio.rl` 的 poll(2) 后端之上增加 `Interest` 平台后端抽象——Linux `epoll`（`epoll_create1`/`epoll_ctl`/`epoll_wait`，O(1) 事件）、macOS `kqueue`（`kqueue`/`kevent`，`EVFILT_READ`/`EVFILT_WRITE`）；`Poller` 内部按 `__rlyeh_target_os()` 分派；为 W3 事件驱动 executor 提供平台后端 | 📋 规划 | [`y2-nio-backend.md`](../tasks/leaf/y2-nio-backend.md) |
| Y3 | **HTTP 连接复用 + sendfile 平台补全**：`HttpClient` 连接池（keep-alive：`Connection: keep-alive` + 复用空闲连接，替代每请求新建 + close）；sendfile Windows `TransmitFile` 分支（§4.5「非 Unix 返回 Unsupported」消除） | 🔧 部分完成 | [`y3-http-keepalive.md`](../tasks/leaf/y3-http-keepalive.md) |
| Y4 | **锁 guard 完整 + Channel 泛型化**：`Mutex<T>`/`RwLock<T>` 泛型化（目标签名 `lock(&self) -> MutexGuard<T>`，替代裸 `lock/unlock` + `lock_guard()` 命名特判——方法名对齐目标 API）；`RwLockWriteGuard`/`RwLockReadGuard`（P2b 规划）；`Deref`/`DerefMut` 语义（`*guard` 解引用访问数据，替代 MVP 独立访问器）；`Channel<T>` 泛型化（元素不再限 i64）、`bounded_channel(capacity)` 有界队列（send 满阻塞）、`SendError<T>`/`RecvError`/`TryRecvError` 错误类型（替代 `Option` 退化）、`Arc<LockFreeQueue>`（跨线程 Sender/Receiver，依赖 U3） | 📋 规划 | [`y4-lock-guard-channel.md`](../tasks/leaf/y4-lock-guard-channel.md) |
| Y5 | **`Box::leak` 目标签名**：`fn leak(self) -> &'static mut T`（依赖 U5 AddrOf 任意目标，替代 `*mut T` 裸指针退化）；`'static` 宽松丢弃（G4 现状） | 📋 规划 | [`y5-box-leak-signature.md`](../tasks/leaf/y5-box-leak-signature.md) |
| Y6 | **错误体系完整化**：`trait Error { fn message(&self) -> String; fn source(&self) -> Option<&dyn Error>; }`（补 `source` 链，M2a 现状仅 message）；`Into::into()` 自动转换 + `?` 运算符的 From 自动转换（目标：`?` 在 `E: Into<F>` 时自动转——依赖 U3/U4，`io::Error` → 自定义错误；MVP 无 where 约束时退化显式 `into()` 调用） | 📋 规划 | [`y6-error-source.md`](../tasks/leaf/y6-error-source.md) |
| Y7 | **UDP**：`net/udp.rl`（§1 目标架构目录）——`UdpSocket::bind`/`send_to`/`recv_from`/`local_addr`（libc `socket(AF_INET, SOCK_DGRAM)` + `sendto`/`recvfrom`，平台 sockaddr_in 双布局复用 O1a，WASI 短路） | ✅ 已完成 | [`y7-udp.md`](../tasks/leaf/y7-udp.md) |
| Y8 | **`thread::Builder::stack_size`**：`Builder::new()/stack_size(bytes)/spawn(f)`——`pthread_attr_setstacksize`（S0e 规划项，替代 attr=NULL 默认栈：Linux 8MB/macOS 512KB 的定制手段） | ✅ 已完成 | [`y8-thread-stack.md`](../tasks/leaf/y8-thread-stack.md) |

**验收**：见任务树 [`stage-u-z.md`](../tasks/stage-u-z.md) 各子任务叶子的「验证」字段；全量回归通过。

> **总依赖关系**：U（全部）→ V（V1/V3/V4 依赖 U1–U3；V2/V5 依赖 V1）+ X3/X4（依赖 U3/U4）→ W（W1 依赖 U2；W2 依赖 U1；W3 依赖 R + Y2 可选；W4/W6 依赖 U3）→ Y（Y1 依赖 U1 切片成熟；Y4/Y6 依赖 U3/U4；Y2 独立；Y3/Y7/Y8 独立）。
> **跟踪与验收约定**：沿用 §5——每阶段 `cargo test --workspace` 全绿 + clippy 0 警告；运行时/并发类测试带超时保护；完成后同步更新 std-lib.md 状态总览（勾销对应规划章节）+ 本文档状态标识 + CODEBUDDY.md §5.5。

---

### 6.4. 执行记录

> 阶段 G–T / U–Z 的任务具体执行情况已归档至任务树：
> - 阶段 G–L：[`tasks/stage-g-l.md`](../tasks/stage-g-l.md)（§执行记录）
> - 阶段 M–T：[`tasks/stage-m-t.md`](../tasks/stage-m-t.md)（§执行记录）
> - 阶段 U–Z：[`tasks/stage-u-z.md`](../tasks/stage-u-z.md)（§执行记录）
>
> 本文件保留计划主体（§2/§3/§3b/§3c 阶段详情）与状态标识；详细实现流水见任务树 / git 历史。

**状态摘要**：阶段 G–L 全部完成、阶段 M–T 全部完成、U 全部完成、V 进行中、W 全部完成、X 进行中（X1 ✅）、Y 规划（见 §2/§3c.1）。

---

### 6.5. 跟踪与验收约定

1. 每个阶段/任务完成须满足：`cargo test --workspace` 全绿 + `cargo clippy --workspace --all-targets` 0 警告。
2. 涉及运行时/并发类测试（Actor、通道、锁、join、GC 周期）须按 `design/00_项目总览.md` 硬性规则带超时保护，挂起即视为失败。
3. 任务完成后同步更新：本文档状态标识 + 任务树（`tasks/` 阶段索引进度与叶子文档）+ `guide.md` §13（勾销对应限制条目，并更新 `grammar.md` 顶部实现状态标注）。
4. 每个阶段产出对应的集成测试（仿 development-plan.md 各阶段 `*_test.rs` 用例并全量回归）。
5. 优先交付顺序建议：G1/G2（引用地基 + str）→ I1/I2（宏 + 格式化）→ H1/H2（函数指针 + 无捕获闭包）→ K1（`?`）→ J 全阶段 → 其余。

---

- [x] **T 阶段：集合与迭代器收尾**（2026-08-24）：T1a Vec / T1b String / T1c HashMap API 补齐 + T2 `Iterator` trait（元素固定 i64）+ T3a `Box::leak`（裸指针退化）/ T3b Rc/Arc/Weak 核对。各任务执行情况与技术细节见任务树 [`stage-m-t.md`](../tasks/stage-m-t.md) T 阶段叶子文档（`t1a`–`t3b`）。

> **维护者**：Rlyeh Language Team
> **最后更新**：2026-08-25

> **维护者**：Rlyeh Language Team
> **最后更新**：2026-08-26
