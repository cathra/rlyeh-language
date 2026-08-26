# 阶段 T — 集合与迭代器收尾

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-*.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：Vec/HashMap/String 核心 API 已实现（§3 ✅），适配器内建 desugar 可用（J3 ✅）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| T1A | **Vec 目标 API 补齐**：`sort`/`binary_search`/`iter`（按 std-lib.md 目标 API 清单核对） | ✅ 已完成 | [`t1a-vec-api.md`](../tasks/leaf/t1a-vec-api.md) |
| T1B | **String 目标 API 补齐**：`chars`/`lines`/`split`（部分已有）/`replace`/`to_uppercase`/`to_lowercase`/`trim` | ✅ 已完成 | [`t1b-string-api.md`](../tasks/leaf/t1b-string-api.md) |
| T1C | **HashMap 目标 API 补齐**：`iter`/`keys`/`values` 等（按 std-lib.md 目标 API 清单核对） | ✅ 已完成 | [`t1c-hashmap-api.md`](../tasks/leaf/t1c-hashmap-api.md) |
| T2 | **`Iterator` trait 定义**：`trait Iterator { type Item; fn next(&mut self) -> Option<Item>; }`——**风险**：先做关联类型 `type Item` 最小验证（H4 仅验证过非泛型 dyn）；适配器**保持内建 desugar 不迁移**（避免重构风险），trait 定义先行供自定义迭代器接入（J2 已支持方法式 `next() -> Option<T>`，trait 化为可选演进） | ✅ 已完成 | [`t2-iterator-trait.md`](../tasks/leaf/t2-iterator-trait.md) |
| T3A | **`Box::leak`**：泄漏堆对象返回裸指针 | ✅ 已完成 | [`t3a-box-leak.md`](../tasks/leaf/t3a-box-leak.md) |
| T3B | **Rc/Arc 目标 API + `Weak::upgrade` 完善**（std-lib.md §11 目标 API 清单逐项核对） | ✅ 已完成 | [`t3b-rc-arc-api.md`](../tasks/leaf/t3b-rc-arc-api.md) |

**验收**：见任务树 [`stage-m-t.md`](../tasks/stage-m-t.md) 各子任务叶子的「验证」字段；全量回归通过。

> **T 阶段已知限制**（实测发现）：typecheck 变量环境 `variables: HashMap<String, Type>` 按名全局索引、**无作用域隔离**——同名遮蔽（match 臂绑定与后续 let 绑定同名）时类型互相覆盖。✅ 已解决（U1 作用域栈重构完成：按作用域分层 + 块级遮蔽存储槽 mangle，执行情况见 [`u1-scope-stack.md`](../tasks/leaf/u1-scope-stack.md)）。

---

### 6.3c. 标准库后续完善计划（阶段 U–Z：目标 API 对齐与编译器能力补齐）

> **需求来源**：[`std-lib.md`](./std-lib.md) 各章节「目标 API（规划）」「MVP 退化」「规划中」标注，以及阶段 M–T 完成后遗留的**已知退化项**（M2b/Q1a/S1a/T1a/T1b/T1c/T2/T3a 的 MVP 降级实现）。
> **前置**：阶段 G–T 已全部完成（2026-08-24），MVP 标准库完整可用（错误体系 / IO 对象化 / 网络 / 并发 / 序列化 / NIO / 线程 / 异步状态机 / 集合收尾）。
> **定位**：本计划不做新语言特性，聚焦 ① 消除已实测验证的编译器能力缺口（阶段 U，全部为 std 完整化的硬前置），② 把 MVP 退化项升级为 std-lib.md 目标 API（阶段 V–Y）。
>
> **编译器能力缺口盘点（阶段 U 集中解决）**：
>
> | # | 缺口 | 实测依据 | 影响面 |
> |---|------|---------|--------|
> | 1 | typecheck 变量环境**按名全局索引、无作用域栈**，同名遮蔽互相覆盖（后续按类型分支输出异常） | T 阶段已知限制（§3b.9 / std-lib.md 顶部） | `get_mut` 引用语义、借用迭代器、引用返回值 |
> | 2 | trait **关联类型** `type Item` / `type Output` 无载体 | S1a/T2 已验证（parser/typecheck 无 trait `type` 成员） | `Iterator` 泛型元素、`Future` 泛型 Output |
> | 3 | **泛型 trait 约束** `T: Bound`（where 子句 / bound）不可用 | M2b 已验证（blanket impl 不可行） | `json::to_string<T: Serialize>`、`timeout<F: Future>` |
> | 4 | trait 方法返回 **`-> Self`** 未支持（typecheck `undefined type Self`） | M2b/Q1a 已验证 | `Deserialize::from_json`、`From::from`、`Into::into` |
> | 5 | **MIR `AddrOf` 仅支持变量取址** | T3a 已验证（`&*b` 堆地址取引用不可行） | `Box::leak` 目标签名 `&'static mut T` |

#### 6.3c.1 计划总览（阶段 U–Z）

| 阶段 | 主题 | 子任务（关键交付，按序） | 子任务数 | 依赖 | 状态 |
|------|------|---------|:---:|------|------|
| **U** | 编译器地基（std 完整化前置） | U1 作用域栈重构、U2 trait 关联类型、U3 泛型约束 where、U4 `-> Self` 返回、U5 MIR AddrOf 任意目标、U6 数值转换 Cast IR、**U7 方法级泛型参数**、**U8 泛型结构体构造 + 泛型 trait** | 8 | G、T 已知限制 | ✅ 已完成（U1–U8 全部落地；执行情况见 [`stage-u-z.md`](../tasks/stage-u-z.md) 叶子文档） |
| **V** | 集合与迭代器完整化 | V1 借用迭代器（`Iter`/`IterMut`/`Iter<'_, K, V>`）、V2 String 码点迭代器（`Chars`/`Lines`）、V3 Iterator 默认方法 + 适配器迁移、V4 `get_mut` 引用语义、V5 新集合（HashSet/BTreeMap/VecDeque） | 5 | U1/U2/U3 | 🔧 进行中（V1/V4/V5 ✅、V3 默认方法基础 ✅、V2 打印/参数 ✅；V2-B/V2-E、V3-A~D 待办，见 [`stage-u-z.md`](../tasks/stage-u-z.md) 与 [`v2-str-view.md`](../tasks/v2-str-view.md)/[`v3-iterator-adapters.md`](../tasks/v3-iterator-adapters.md)） |
| **W** | 异步运行时完整化 | W1 Future 泛型化（Output/Pin/Context）、W2 await 状态机扩展（控制流/表达式嵌套/引用跨 await）、W3 事件驱动 executor（epoll/kqueue/io_uring + Future 挂起）、W4 `join_all`/`timeout` Future 版 + `TimeoutError`、W5 `recv_async`/HTTP async 真异步、W6 async 泛型/递归 + 闭包跨线程捕获 | 6 | U、R（Poller）、S1c | ✅ **全部完成（W1–W6）**（执行情况见 [`stage-u-z.md`](../tasks/stage-u-z.md) W 叶子文档） |
| **X** | 序列化/格式化/时间完整化 | X1 Duration/Instant/SystemTime 完整 API、X2 标准 TOML（空格形式/`[section]`/注释/多行字符串）+ 解析鲁棒性、X3 `Deserialize` trait + Serializer/Deserializer 框架 + `JsonError`/`TomlError`、X4 Formatter 完整化（`Result<(), FmtError>`） | 4 | U3/U4、Q | 🔧 进行中（X1 ✅；X2/X3/X4 规划，见 [`stage-u-z.md`](../tasks/stage-u-z.md) X 叶子文档） |
| **Y** | IO/网络/并发/智能指针收尾 | Y1 File `open_with`/`read(&mut [u8])`/`write(&[u8])`/Metadata、Y2 NIO 高性能后端（epoll/kqueue）、Y3 HTTP 连接复用 + sendfile Windows `TransmitFile`、Y4 Mutex/RwLock guard 完整 + Channel 泛型化/bounded、Y5 `Box::leak` 目标签名、Y6 `Error::source` + `Into::into` 自动转换、Y7 UDP（`net/udp.rl`）、Y8 `thread::Builder::stack_size` | 8 | U、O/R/P | 📋 规划 |

> **推荐执行路线**：
> - **快赢线**（独立性强、不依赖 U 全量，可先行交付）：X1（Duration 构造器/读取器补齐，纯 std）→ Y8（stack_size，driver 注入扩展）→ Y1（`File::open_with`，libc extern 扩展）→ Y3（sendfile Windows 平台分支）。
> - **主线（依赖驱动）**：U1（作用域栈，解锁引用语义）→ U2（关联类型）→ U3（泛型约束）→ U4（`Self` 返回）→ V（V1→V4→V3→V2→V5，随 U 逐项解锁）→ X3/X4（依赖 U3/U4）→ W（W1→W2→W4→W3→W5→W6，W3 事件驱动为最大单点）→ Y 收尾。
> - **优先级建议**：U1（消除已知限制、风险最低收益最广）> V4（`get_mut` 引用语义，集合 API 高频）> X1（时间 API 补全，零依赖）> U2（解锁 Iterator/Future 泛型化）> W2（await 控制流块，日常编写收益大）> W3（事件驱动 executor，收益最大但风险最高）> 其余。
