# 阶段 U–Z（目标 API 对齐与编译器能力补齐）

> **所属任务树**：[任务文档导航](./README.md)
> **状态**：🔧 进行中（U ✅；V ✅（V1 借用迭代器 + 引用元素 `Option<&T>` ✅ 已完成；V2 `chars`/`lines` 目标签名 ✅ 已完成——`char` 类型已拓宽至 32 位 Unicode 码点；V3 自定义迭代器适配器迁移 ✅，数组/Vec 适配器内建记为语言限制）；W ✅（async_if_await/async_neg poll 死循环已于 2026-08-29 验收修复并解除 skip；2026-08-30 验收全绿：W 相关集成测试 25 用例 + 全量 .rl 套件 suite_test 通过）；X ✅；Y 🔧 部分完成（Y2/Y5/Y6/Y7/Y8 ✅；Y1/Y3/Y4 部分））
> **权威来源**：阶段详情文档 [`stages/U.md`](../stages/U.md)、[`stages/Y.md`](../stages/Y.md)；执行记录见本文件 §执行记录
> **本层职责**：记录阶段 U–Z 的任务列表与进度 + 执行记录；其中 V 阶段已拆分为任务文档（V2/V3）+ 叶子文档，其余阶段具体实施见权威源文档。

---

## 任务列表与进度

| 阶段 | 主题 | 子任务 | 子任务数 | 依赖 | 状态 |
|------|------|--------|:---:|------|------|
| **U** | 编译器地基 | U1 作用域栈、U2 关联类型、U3 泛型约束、U4 `-> Self`、U5 AddrOf、U6 Cast IR、U7 方法级泛型、U8 泛型结构体 + 泛型 trait | 8 | G、T | ✅ 全部完成（U1–U6 + U7/U8） |
| **V** | 集合与迭代器完整化 | V1 借用迭代器、V2 String 码点迭代器、V3 默认方法 + 适配器迁移、V4 `get_mut`、V5 新集合 | 5 | U1/U2/U3 | ✅ 已完成（V1/V2/V3/V4/V5 全部完成；V3 自定义迭代器适配器已迁移为 trait 默认方法 + 包装迭代器，数组/`Vec` 适配器因数组非命名类型保留内建 desugar，记为已知语言限制） |
| **W** | 异步运行时完整化 | W1 Future 泛型化、W2 await 状态机、W3 事件驱动 executor、W4 join_all/timeout、W5 recv_async/HTTP async、W6 async 泛型/递归 | 6 | U、R、S1c | ✅ 全部完成（W1–W6；async_if_await/async_neg poll 死循环已于 2026-08-29 验收修复并解除 skip；2026-08-30 验收全绿：W 相关集成测试 25 用例 + 全量 .rl 套件 suite_test 通过） |
| **X** | 序列化/格式化/时间完整化 | X1 Duration/Instant/SystemTime、X2 标准 TOML、X3 Deserialize + Serializer、X4 Formatter | 4 | U3/U4、Q | ✅ 已完成（X1/X2/X3/X4 全部完成：X3 `json::try_parse`/`toml::try_parse` Err 路径 + `Deserialize` trait + Serializer/Deserializer 框架全落地，2026-08-30） |
| **Y** | IO/网络/并发/智能指针收尾 | Y1 File API、Y2 NIO 高性能后端、Y3 HTTP 连接复用 + sendfile、Y4 guard 完整 + Channel 泛型化、Y5 `Box::leak`、Y6 `Error::source`、Y7 UDP、Y8 `thread::Builder::stack_size` | 8 | U、O/R/P | 🔧 部分完成（Y2/Y5/Y6/Y7/Y8 ✅；Y1/Y3/Y4 部分——Y1 `read/write` 切片实参挂 U1、Y3 sendfile Windows 不可验证、Y4 Y4b-3/4 RwLock/DerefMut/有界队列/错误类型/Arc 待办） |

---

## V 阶段子任务文档（已拆分到任务树）

| V 子任务 | 任务文档（索引） | 叶子文档（具体实施） | 状态 |
|---------|-----------------|---------------------|------|
| V2（`&str`） | [`v2-str-view.md`](./v2-str-view.md) | `leaf/v2-a-semantics.md` … `leaf/v2-e-api-align.md` | ✅（V2-A~E ✅；`chars`/`lines` 目标签名 ✅ 已完成——`chars()`/`lines()` 现返回 `Chars`/`Lines` 迭代器、`Chars::next() -> Option<char>`，`char` 类型拓宽至 32 位 Unicode 码点，2026-08-29） |
| V3（Iterator） | [`v3-iterator-adapters.md`](./v3-iterator-adapters.md) | `leaf/v3-a1..a4.md`（关联类型，低/中）+ `leaf/v3-b/c.md` + `leaf/v3-d1..d5.md`（适配器，中/低）+ `leaf/v3-e-builtin-cleanup.md` | ✅（A~E 子任务已落地；自定义迭代器适配器迁移为 trait 默认方法 + 包装迭代器惰性化；数组/Vec 适配器因数组非命名类型保留内建 desugar，记为语言限制） |

> V1/V4/V5 属 V 阶段已完成项，具体实施见 任务树 §执行记录。

---

## 阶段详情索引

- 阶段 U–Z 详细任务：阶段详情文档 [`stages/U.md`](../stages/U.md)、[`stages/Y.md`](../stages/Y.md)
- 阶段 U–Z 执行记录：见下节（本文件 §执行记录）

---

## 子任务列表（按阶段分层）

> 每个阶段下列出其子任务（一个任务一个叶子文档，含具体执行情况与技术细节）：

### U — 编译器地基

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| U1 作用域栈重构 + 块级变量遮蔽 | [`u1-scope-stack.md`](leaf/u1-scope-stack.md) | ✅ 已完成 |
| U2 trait 关联类型 | [`u2-assoc-type.md`](leaf/u2-assoc-type.md) | ✅ 已完成 |
| U3 泛型 trait 约束（bound / where） | [`u3-generic-bound.md`](leaf/u3-generic-bound.md) | ✅ 已完成 |
| U4 `-> Self` 返回 | [`u4-self-return.md`](leaf/u4-self-return.md) | ✅ 已完成 |
| U5 MIR `AddrOf` 任意目标 + 字段/索引真实取址 | [`u5-addr-of.md`](leaf/u5-addr-of.md) | ✅ 已完成 |
| U6 数值转换 Cast IR | [`u6-cast-ir.md`](leaf/u6-cast-ir.md) | ✅ 已完成 |
| U7 方法级泛型参数 | [`u7-method-generic.md`](leaf/u7-method-generic.md) | ✅ 已完成 |
| U8 泛型结构体构造 + 泛型 trait | [`u8-generic-ctor-trait.md`](leaf/u8-generic-ctor-trait.md) | ✅ 已完成 |

### V — 集合与迭代器完整化

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| V1 借用迭代器瘦指针 MVP | [`v1-borrow-iter.md`](leaf/v1-borrow-iter.md) | ✅ 已完成（`Vec::iter`/`iter_mut`/`iter_ref` + `HashMap::iter_pairs` 零拷贝引用迭代 ✅ 2026-08-29；元组运行时未就绪，KV 对以 `KVRef` 结构体承载，等价 `(&K,&V)`） |
| V4 `get_mut` 引用语义 | [`v4-get-mut.md`](leaf/v4-get-mut.md) | ✅ 已完成 |
| V5 新集合：VecDeque / HashSet / BTreeMap | [`v5-new-collections.md`](leaf/v5-new-collections.md) | ✅ 已完成 |
| V2-A：`&str` 语义统一为 StrFat 双槽 | [`v2-a-semantics.md`](leaf/v2-a-semantics.md) | ✅ 已完成（审计 + 文档澄清，2026-08-26） |
| V2-B：`&str` 子区间视图（trim/trim_start/trim_end） | [`v2-b-substring.md`](leaf/v2-b-substring.md) | ✅ 已完成（trim/trim_start/trim_end + 链式 &str 方法，2026-08-26） |
| V2-C：`&str` 打印链路修复（含 by_value StrFat 运行时） | [`v2-c-print.md`](leaf/v2-c-print.md) | ✅ 已完成（方案 A，2026-08-26） |
| V2-D：`&str` 参数/返回值完整支持 | [`v2-d-params.md`](leaf/v2-d-params.md) | ✅ 已完成（参数 StrFat ABI + 返回值 borrowck + `String::from(&str)` 深拷贝，2026-08-26） |
| V2-E：String API 对齐 + 兼容保留 | [`v2-e-api-align.md`](leaf/v2-e-api-align.md) | ✅ 已完成（API 对齐评估 + 兼容保留，2026-08-26） |
| V3-A1：parser — trait 内关联类型声明载体 | [`v3-a1-trait-type-parser.md`](leaf/v3-a1-trait-type-parser.md) | ✅ 已完成（含默认具体化，2026-08-27） |
| V3-A2：typecheck — 关联类型定义 + `Self::Item` 投影 | [`v3-a2-assoc-type-resolve.md`](leaf/v3-a2-assoc-type-resolve.md) | ✅ 已完成（U2 核实，2026-08-27） |
| V3-A3：std — `Iterator::Item` 落地 + 各 impl 具体化 | [`v3-a3-iterator-item.md`](leaf/v3-a3-iterator-item.md) | ✅ 已完成（trait 签名 + 各 impl 补 type Item，2026-08-27） |
| V3-A4：`F::Item` 关联类型投影完善 + 自定义迭代器用例 | [`v3-a4-item-projection.md`](leaf/v3-a4-item-projection.md) | ✅ 已完成（Range::Item 命名投影，2026-08-27） |
| V3-B：剩余默认方法 `chain`/`enumerate`/`find`/`fold` | [`v3-b-default-methods.md`](leaf/v3-b-default-methods.md) | ✅ 已完成（find/fold/chain/enumerate，2026-08-27） |
| V3-C：包装迭代器类型 | [`v3-c-wrapper-iterators.md`](leaf/v3-c-wrapper-iterators.md) | ✅ 已完成（Filter/Take/Skip/Chain/Enumerate，2026-08-27） |
| V3-D1：`map`/`filter` trait 默认方法 + 绑定包装迭代器 | [`v3-d1-map-filter-methods.md`](leaf/v3-d1-map-filter-methods.md) | ✅ 已完成（filter/take/skip/collect + 语言增强，2026-08-27） |
| V3-D2：`take`/`skip` trait 默认方法 + 绑定包装迭代器 | [`v3-d2-take-skip-methods.md`](leaf/v3-d2-take-skip-methods.md) | ✅ 已完成（map + Iter/IterMut 实现 Iterator，2026-08-27） |
| V3-D3：`collect` trait 默认方法 + `fold` 归约迁移 | [`v3-d3-collect-fold-methods.md`](leaf/v3-d3-collect-fold-methods.md) | ✅ 已完成（fold 惰性化，2026-08-27） |
| V3-D4：typecheck `try_check_adapter` 迁移为通用 trait 方法解析 | [`v3-d4-adapter-dispatch.md`](leaf/v3-d4-adapter-dispatch.md) | ✅ 已完成（双轨收敛，2026-08-27） |
| V3-D5：删除内建 desugar 特判 + 全量回归 | [`v3-d5-builtin-cleanup.md`](leaf/v3-d5-builtin-cleanup.md) | ✅ 已完成（数组/Vec 内建保留——语言限制，2026-08-27） |
| V3-E：内建 desugar 清理 + 全量回归 | [`v3-e-builtin-cleanup.md`](leaf/v3-e-builtin-cleanup.md) | ✅ 已完成（双轨定案，2026-08-27） |

### W — 异步运行时完整化

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| W1 Future 泛型化 | [`w1-future-generic.md`](leaf/w1-future-generic.md) | ✅ 已完成 |
| W2 await 状态机扩展 | [`w2-await-state-machine.md`](leaf/w2-await-state-machine.md) | ✅ 已完成 |
| W3 事件驱动 executor | [`w3-event-executor.md`](leaf/w3-event-executor.md) | ✅ 已完成 |
| W4 `join_all`/`timeout` Future 版 | [`w4-join-all-timeout.md`](leaf/w4-join-all-timeout.md) | ✅ 已完成 |
| W5 `recv_async`/HTTP async 真异步 | [`w5-recv-http-async.md`](leaf/w5-recv-http-async.md) | ✅ 已完成 |
| W6 async 泛型/递归 + 闭包跨线程捕获 | [`w6-async-generic-recursion.md`](leaf/w6-async-generic-recursion.md) | ✅ 已完成 |

### X — 序列化/格式化/时间完整化

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| X1 时间 API 完整化 | [`x1-time-api.md`](leaf/x1-time-api.md) | ✅ 已完成 |
| X2 标准 TOML + 解析鲁棒性 | [`x2-standard-toml.md`](leaf/x2-standard-toml.md) | ✅ 已完成（`key = value` 空格 + round-trip + 整行注释 + 引号感知 + `[section]` 行式（含多级 [a.b]）+ f64 + `[T; N]` 数组 + 多行字符串 `"""`，2026-08-30） |
| X3 `Deserialize` trait + Serializer/Deserializer 框架 | [`x3-deserialize-trait.md`](leaf/x3-deserialize-trait.md) | ✅ 已完成（`Deserialize` trait + `JsonError`/`TomlError` + `Serializer`/`Deserializer` 访问器框架 ✅；`json::try_parse`/`toml::try_parse` Err 路径 ✅，2026-08-30） |
| X4 Formatter 完整化 | [`x4-formatter-complete.md`](leaf/x4-formatter-complete.md) | ✅ 已完成（Formatter 升级 + `Result<(),FmtError>` 签名 + `Debug::fmt` 改名 + `FmtError` + 对齐占位符引擎应用，2026-08-28） |

### Y — IO/网络/并发/智能指针收尾

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| Y1 File 目标 API | [`y1-file-api.md`](leaf/y1-file-api.md) | 🔧 部分完成 |
| Y2 NIO 高性能后端 | [`y2-nio-backend.md`](leaf/y2-nio-backend.md) | ✅ 已完成（kqueue/epoll 平台后端 + Poller 分派，2026-08-29） |
| Y3 HTTP 连接复用 + sendfile 平台补全 | [`y3-http-keepalive.md`](leaf/y3-http-keepalive.md) | 🔧 部分完成 |
| Y4 锁 guard 完整 + Channel 泛型化 | [`y4-lock-guard-channel.md`](leaf/y4-lock-guard-channel.md) | 🔧 部分完成（Y4a/Y4b-1/Y4b-2 std Mutex 泛型化 + Y4c Channel<T> 泛型化 + Y4b-3 RwLock<T> 泛型化 + 读/写守卫 Deref ✅，2026-08-30；**Y4b-4 guard DerefMut ✅，2026-08-30**；**有界队列 ✅（2026-08-30）**；**错误类型 SendError/RecvError/TryRecvError ✅（2026-08-30）**；**Arc<LockFreeQueue> ✅（2026-08-30，类型层；跨线程移动受 Thread::start 无参限制）**） |
| Y5 `Box::leak` 目标签名 | [`y5-box-leak-signature.md`](leaf/y5-box-leak-signature.md) | ✅ 已完成（返回 `&'static mut T`，2026-08-28） |
| Y6 错误体系完整化 | [`y6-error-source.md`](leaf/y6-error-source.md) | ✅ 已完成（`source()` 真实错误链 + From/Into + `?` 自动转换，2026-08-29） |
| Y7 UDP | [`y7-udp.md`](leaf/y7-udp.md) | ✅ 已完成 |
| Y8 `thread::Builder::stack_size` | [`y8-thread-stack.md`](leaf/y8-thread-stack.md) | ✅ 已完成 |

---
## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 阶段 U–Z 纳入任务树（索引；V 阶段链接 V2/V3 任务文档） |
| 2026-08-29 | V2 完整完成：`char` 类型 codegen 由 i8 拓宽至 i32（32 位 Unicode 码点）；`chars()`/`lines()` 目标签名升级为返回 `Chars`/`Lines` 迭代器、`Chars::next() -> Option<char>`；`tokenize` 改写为顺序迭代；std 内 V3 适配器 `Item` 误置 `char` 还原为 `i64`；全量 177 用例通过 |
| 2026-08-29 | V 阶段收口：V1（`iter_ref` 引用元素 `Option<&T>` + `iter_pairs`/`KVRef` KV 引用迭代，2026-08-29 落地）、V2（chars/lines 目标签名 + char 32 位）、V3（自定义迭代器适配器迁移 + 关联类型）全部完成；V 阶段状态由 🔧 进行中 更新为 ✅ 已完成。V3 数组/`Vec` 适配器走内建 desugar 记为已知语言限制（数组非命名类型无法 `impl Iterator`，需语言增强后迁移），非阻塞项 |
| 2026-08-30 | W 阶段验收：W1–W6 相关集成测试（executor_test 3 / join_all_fut_test 4 / net_http_test 5 含 http_get_async·http_post_async / thread_test 8 / time_test 5，共 25 用例）全部通过；全量 `.rl` 套件（suite_test）46s 全绿（含 async_if_await/async_neg 解除 skip 后用例、w2_probe/join_all/recv_async/async_depth_probe/generic_async 等）；`future.rl` 源码核实 W1–W4（`type Output` / `Context` 事件驱动槽 / `block_on`·`timeout` 事件驱动休眠 / `join_all<F: Future> -> Vec<F::Output>` / `TimeoutError`）。W 阶段状态确认 ✅ 已完成（2026-08-29 修复 poll 死循环、解除 skip，2026-08-30 验收全绿） |
