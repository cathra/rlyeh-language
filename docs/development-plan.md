# 开发计划 — Rlyeh 0.1.0 第一版规划（2026-08）

> **性质**：本文档为 2026-08 对原计划（M1/M2/M3）复盘后制定的新版开发计划的**权威副本**，覆盖阶段 A–Z（§1–§3 阶段 A–F；§6 剩余任务消解 G–L / M–T / U–Z，由原 `mvp-gaps-plan.md` 并入）。
> **总纲与进度速览**：见根目录 [`CODEBUDDY.md`](../CODEBUDDY.md)（§6 为里程碑原表）。
> **任务执行记录**：见 [`tasks/`](./tasks/README.md)（stage-* 索引 §子任务叶子文档 + milestone-tasks/）——已按 §7.4 任务管理体系从本文档 §4 / §6.4 迁移。
> **维护规则**：每完成一项任务，需同步更新本文档与任务树的状态标识。
> **0.1.0 已收口**：阶段 A–Z 全部完成（Y 部分完成），本文件为 0.1.0 规划权威副本。
> **0.2.0 主线**：自举准备阶段计划见 [`development-plan-0.2.0.md`](./development-plan-0.2.0.md)；自举可行性评估见 [`self-hosting/feasibility.md`](./self-hosting/feasibility.md)。

---

## 1. 背景：原计划进度盘点（2026-08 复盘）

### 1.1 完成情况

| 里程碑 | 状态 | 说明 |
|--------|------|------|
| M1 编译器 MVP | ✅ 100% | M1.1–M1.9 全部完成（词法/语法/AST→HIR/类型检查/借用检查/区域系统/MIR+优化/LLVM 后端/hello-world） |
| M2 生产可用 | 🔧 约 96% | M2.1 Actor 运行时、模块系统、M2.2 Dagon、M2.3 增量编译、M2.5 标准库主体、M2.8 分配器侧 已完成；M2.4 LSP（F1）、M2.6 交叉编译 macOS 双架构 + 平台内建（E1）、M2.7 WASM（E2）、M2.8 PGO 回灌（F2）已完成；剩余 Windows/ARM 工具链（待对应环境）与 M3 生态 |
| M3 生态繁荣 | ⏳ 0% | 未开始 |

### 1.2 遗留问题清单（13 项）

| # | 遗留问题 | 归属阶段 | 状态 |
|---|---------|---------|------|
| 1 | 用户级 match 解构具体实例化枚举聚合载荷失败（`expected String, found T`） | A1 | ✅ 已修复 |
| 2 | 裸 `Result::Err(7).unwrap_or(100)` 返回 `_`（`Infer` 占位未定型） | A2 | ✅ 已修复 |
| 3 | `String` 拼接 `+` 为原地追加 + 共享缓冲，存在别名隐患（需评审改拷贝语义） | A3 | ✅ 已修复 |
| 4 | 无通用 FFI，io/net 绑定层（NIO/sendfile 已有 Rust 侧但接不进 Rlyeh）无法接线 | A4 | ✅ 已修复 |
| 5 | 标准库 time 模块缺失（`Duration`/`Instant`） | B1 | ✅ 已完成 |
| 6 | 标准库 io 模块缺失（File 读写、stdin/stdout、`read_to_string` 等） | B2 | ✅ 已完成（执行情况见 [`b2-io-module.md`](tasks/leaf/b2-io-module.md)） |
| 7 | 标准库 net 模块缺失（TCP/UDP + NIO/sendfile 绑定层接线） | B3 | ✅ 已完成（执行情况见 [`b3-net-module.md`](tasks/leaf/b3-net-module.md)、[`b3-bitwise.md`](tasks/leaf/b3-bitwise.md)） |
| 8 | 标准库 sync 模块缺失（Mutex/RwLock/Condvar 等） | B4 | ✅ 已完成（执行情况见 [`b4-sync-module.md`](tasks/leaf/b4-sync-module.md)） |
| 9 | `Vec<T>`/`HashMap` 常用方法缺失（`first`/`last`/`reverse`/`swap`/`binary_search`；`len`/`is_empty` 重复定义与固化） | B5 | ✅ 已完成（执行情况见 [`b5-vec-hashmap-methods.md`](tasks/leaf/b5-vec-hashmap-methods.md)） |
| 10 | `rlyeh test` 子命令未实现（测试目前经 `cargo test` 驱动） | D | ✅ 已完成（执行情况见 [`d1-rlyeh-test.md`](tasks/leaf/d1-rlyeh-test.md)） |
| 11 | 语言级 `actor`/`spawn` 语法与标准库集成未接线（`rlyeh-actor-runtime` 已就绪） | C | ✅ 已完成（执行情况见 [`c1-actor-desugar.md`](tasks/leaf/c1-actor-desugar.md)） |
| 12 | M2.4 LSP 服务器、M2.8 PGO 数据回灌编译流程 | F | ✅ 已完成（执行情况见 [`f1-lsp.md`](tasks/leaf/f1-lsp.md)、[`f2-pgo.md`](tasks/leaf/f2-pgo.md)） |
| 13 | M2.6 交叉编译、M2.7 WASM、工具链其余命令（fmt/check/doc/bench/publish）、M3 生态（数据库/HTTP/序列化/嵌入式/GPU/教程） | E 及后续 | 🔧 大部分完成（E1 双架构 + 平台内建、E2 WASM、E3 发布 + 工具链命令；Windows/ARM 待环境；M3 未开始；执行情况见 [`stage-a-f.md`](tasks/stage-a-f.md) E/F 叶子） |
| 14 | `String::from` 暂仅支持字面量（非字面量 Str 长度表达未实现）、范围切片仅支持 String（数组/Vec 动态切片待实现） | A/B 遗留 | ✅ 已修复（执行情况见 [`legacy-misc.md`](tasks/leaf/legacy-misc.md)） |

---

## 2. 计划总览（阶段 A–Z）

> 每个阶段详情为独立文档（`docs/stages/`），含任务列表 + 各任务详情文档（任务树叶子）链接。


| 阶段 | 主题 | 关键交付 / 子任务 | 状态 | 详情文档 |
|------|------|---------|------|----------|
| **A** | 编译器加固（泛型/Infer/FFI/字符串语义） | A1–A4 | ✅ 全部完成 | [`A.md`](stages/A.md) |
| **B** | 标准库完善（time/io/net/sync/collections） | B1–B5 | ✅ 全部完成 | [`B.md`](stages/B.md) |
| **C** | Actor 语言级接线（`actor`/`spawn`/`.await`） | C1–C3 | ✅ 全部完成 | [`C.md`](stages/C.md) |
| **D** | 工具链（`rlyeh test`/`fmt`/`check`/`doc`/`bench`） | D1–D3 | ✅ 全部完成 | [`D.md`](stages/D.md) |
| **E** | 多目标与发布（交叉编译/WASM/发布流程） | E1–E3 | ✅ E1 大部分完成（macOS 双架构 + 平台内建；Windows/ARM 待环境）；E2/E3 完成 | [`E.md`](stages/E.md) |
| **F** | 编译器深度（LSP、PGO 回灌） | F1–F2 | ✅ 全部完成 | [`F.md`](stages/F.md) |
| **G** | 引用与借用（L0 完整化） | `&T`/`&mut T`、`*` 解引用、`str` 切片、裸指针、生命周期 `'a` | ✅ G1–G4（G4 为语法接受 MVP，borrowck 生命周期检查规划中） | [`G.md`](stages/G.md) |
| **H** | 一等函数 | `fn(A) -> B` 函数类型、闭包（捕获 + `move`）、`dyn Trait` | ✅ H1–H5（H3/H4/H5 为 MVP） | [`H.md`](stages/H.md) |
| **I** | 宏系统与格式化 | `macro_rules!` 声明式宏、`Display`/`Debug`、`println!`/`format!` | ✅ 已完成 | [`I.md`](stages/I.md) |
| **J** | 迭代器与集合协议 | 数组迭代、`Iterator` trait、`map`/`filter`/`fold`/`collect` | ✅ J1–J3 | [`J.md`](stages/J.md) |
| **K** | 错误传播与所有权层级 | `?` 运算符、`Box<T>`、`Rc<T>`/`Arc<T>`、`Gc<T>` | ✅ K1–K4 | [`K.md`](stages/K.md) |
| **L** | 生态模块与平台收尾 | `async fn`/`await`、`serde`、region `strategy (bump)`、WASI net / Actor 交叉编译 | ✅ L1–L4 | [`L.md`](stages/L.md) |
| **M** | 错误处理基底 | IoErrorKind/IoError、Error/From/Into、std 错误约定 Result 化 | ✅ 已完成（7 子任务） | [`M.md`](stages/M.md) |
| **N** | 文件系统与 IO 对象化 | File/OpenMode、stdin/stdout/stderr、Path/fs、eprintln | ✅ 已完成（9 子任务） | [`N.md`](stages/N.md) |
| **O** | 网络对象化 | SocketAddr/TcpListener/TcpStream、字节读写、HTTP 同步 MVP | ✅ 已完成（6 子任务） | [`O.md`](stages/O.md) |
| **P** | 并发通道与同步 | Channel、锁 guard 语义、Condvar/Barrier | ✅ 已完成（6 子任务） | [`P.md`](stages/P.md) |
| **Q** | 序列化与格式化 trait | serde trait/derive、json 泛型 API、Display/Debug+Formatter、TOML | ✅ 已完成（8 子任务） | [`Q.md`](stages/Q.md) |
| **R** | 高性能 IO | Interest/Event/Poller、非阻塞、sendfile | ✅ 已完成（4 子任务） | [`R.md`](stages/R.md) |
| **S** | 异步运行时 | 线程、Future/Poll/block_on、join_all/timeout/sleep、async channel/http | ✅ 已完成（13 子任务） | [`S.md`](stages/S.md) |
| **T** | 集合与迭代器收尾 | Vec/String/HashMap API、Iterator trait、智能指针收尾 | ✅ 已完成（6 子任务） | [`T.md`](stages/T.md) |
| **U** | 编译器地基（std 完整化前置） | 作用域栈、关联类型、泛型约束、`-> Self`、AddrOf、Cast IR、方法级泛型、泛型结构体 | ✅ U1–U8 全部完成 | [`U.md`](stages/U.md) |
| **V** | 集合与迭代器完整化 | 借用迭代器、String 码点迭代器、Iterator 默认方法+适配器、get_mut、新集合 | ✅ 已完成（V1/V2/V3/V4/V5 全部完成；V3 数组/`Vec` 适配器因数组非命名类型保留内建 desugar，记为已知语言限制） | [`V.md`](stages/V.md) |
| **W** | 异步运行时完整化 | Future 泛型化、await 状态机、事件驱动 executor、join_all/timeout、recv_async、async 泛型/递归 | ✅ W1–W6 全部完成 | [`W.md`](stages/W.md) |
| **X** | 序列化/格式化/时间完整化 | 时间 API、标准 TOML、Deserialize trait、Formatter 完整化 | ✅ 全部完成（X1/X2/X3/X4 全部完成，2026-08-30） | [`X.md`](stages/X.md) |
| **Y** | IO/网络/并发/智能指针收尾 | File API、NIO 后端、HTTP 复用、锁/Channel 泛型化、Box::leak、Error::source、UDP、stack_size | 🔧 部分完成（Y2/Y5/Y6/Y7/Y8 ✅；Y1/Y3/Y4 部分） | [`Y.md`](stages/Y.md) |

> **总览说明**：阶段 A–L 为编译器与工具链 + 能力补齐（§3 阶段详情，G–L 依赖：G 无、H 依赖 G、I 弱依赖、J 依赖 H、K 依赖 G、L 依赖 I/G）；阶段 M–T 为标准库深度完善（§6.3b 阶段详情，推荐路线见下方）；阶段 U–Z 为目标 API 对齐与编译器能力补齐（§6.3c，推荐路线见 §6.3c.1）。
>
> **阶段 G–L 推荐执行路线**：
> - **保守路线（依赖驱动）**：G → H → J → K → L，I 按需插入（宏展开器在 parse 后、typecheck 前，与 G/H 解耦，可随时并行）。
> - **快赢路线（体验驱动）**：先做 I1/I2（宏 + `println!` 格式化，开发者日常收益最大、不依赖引用系统）与 G 同步推进；J1（数组迭代）独立且极小，可先行交付。
> - **优先级建议**：G1–G2（引用地基 + str）> I1–I2（宏 + 格式化）> H1–H2（函数指针 + 无捕获闭包）> K1（`?` 运算符）> J 全阶段 > 其余。
>
> **阶段 M–T 推荐执行路线**（任务粒度：59 个子任务，字母后缀 a/b/c 子任务按序完成）：
> - **快赢线**（绑定已就绪 / 独立性强，可先行交付）：M1a/M1b（错误类型）→ M2a（`Error` trait）→ Q1a（serde trait 定义）→ R1a/R1b（Poller 封装）→ P1a/P1b（Channel 绑定 + 对象化）。
> - **主线（依赖驱动）**：M（M1a→M1b→M2a→M2b→M3a→M3b→M3c）→ N（N1a→N1b→N1c→N2a→N2b→N3a→N3b→N3c→N4）→ O（O1a→O1b→O1c→O2→O3a→O3b）→ R（R1a→R1b→R2→R3）→ S0（S0a→…→S0e）→ S1–S3（S1a/S1b → S2a/S2b/S2c → S3a/S3b，S1c 状态机独立排期）。P、Q 与主线并行（依赖交集小），T 随时插入。
> - **优先级建议**：M1a/M1b（错误类型，全部 std 的地基）> N1a（`File` 绑定层，日常收益大）> P1a（Channel 绑定层）> Q1a（serde trait 定义）> R1a（Interest/Event 类型）> O1a（SocketAddr）> S0a（线程绑定层）> 其余。

---

## 3. 阶段详情（A–L）

> 每个阶段详情为独立文档（`docs/stages/`），含任务列表 + 各任务详情文档（任务树叶子）链接。

| 阶段 | 阶段详情文档 | 任务详情文档 |
|------|-------------|-------------|
| **A** | [`A.md`](stages/A.md) | 见 [`A.md`](stages/A.md) 任务表「详情」列 |
| **B** | [`B.md`](stages/B.md) | 见 [`B.md`](stages/B.md) 任务表「详情」列 |
| **C** | [`C.md`](stages/C.md) | 见 [`C.md`](stages/C.md) 任务表「详情」列 |
| **D** | [`D.md`](stages/D.md) | 见 [`D.md`](stages/D.md) 任务表「详情」列 |
| **E** | [`E.md`](stages/E.md) | 见 [`E.md`](stages/E.md) 任务表「详情」列 |
| **F** | [`F.md`](stages/F.md) | 见 [`F.md`](stages/F.md) 任务表「详情」列 |
| **G** | [`G.md`](stages/G.md) | 见 [`G.md`](stages/G.md) 任务表「详情」列 |
| **H** | [`H.md`](stages/H.md) | 见 [`H.md`](stages/H.md) 任务表「详情」列 |
| **I** | [`I.md`](stages/I.md) | 见 [`I.md`](stages/I.md) 任务表「详情」列 |
| **J** | [`J.md`](stages/J.md) | 见 [`J.md`](stages/J.md) 任务表「详情」列 |
| **K** | [`K.md`](stages/K.md) | 见 [`K.md`](stages/K.md) 任务表「详情」列 |
| **L** | [`L.md`](stages/L.md) | 见 [`L.md`](stages/L.md) 任务表「详情」列 |

## 4. 执行记录

> 阶段 A–F 的任务具体执行情况已归档至任务树：[`tasks/stage-a-f.md`](./tasks/stage-a-f.md)（§子任务叶子文档）。
> 本文件保留计划主体（§3 阶段详情）与状态标识；详细实现流水见任务树 / git 历史。

**状态摘要**：阶段 A–F 全部完成（见 §2 总览）。

---

## 5. 跟踪与验收约定

1. 每个阶段/任务完成须满足：`cargo test --workspace` 全绿 + `cargo clippy --workspace --all-targets` 0 警告。
2. 涉及运行时/并发类测试（Actor、通道、锁、`join`）须按 `design/00_项目总览.md` 硬性规则带超时保护，挂起即视为失败。
3. 任务完成后同步更新：本文档状态标识 + 任务树（`tasks/stage-*.md` 进度与 §子任务叶子文档）+ `CODEBUDDY.md`（§6 里程碑）。
4. 优先交付顺序：B2/B3（io/net）→ C（Actor 接线）→ D（工具链）→ E/F（多目标/深度）。

---

## 附录 A：实现纪要（编译器后端与工具链）

> 本附录的历史实现纪要（MIR 中间表示 / 增量编译引擎 / 包管理器 Dagon / LLVM 后端与代码生成）已归档至任务树对应里程碑单任务文档：
> - MIR 中间表示（P011 / M1.7）：[`milestone-tasks/m1-7.md`](tasks/milestone-tasks/m1-7.md)
> - 增量编译引擎（P007 / M2.3）：[`milestone-tasks/m2-3.md`](tasks/milestone-tasks/m2-3.md)
> - 包管理器 Dagon（P008 / M2.2）：[`milestone-tasks/m2-2.md`](tasks/milestone-tasks/m2-2.md)
> - LLVM 后端与代码生成（P013 / M1.8、M1.9）：[`milestone-tasks/m1-8.md`](tasks/milestone-tasks/m1-8.md)、[`milestone-tasks/m1-9.md`](tasks/milestone-tasks/m1-9.md)
> 
> 详细实现（含 ADR 设计决策）见上述单任务文档；此处不再重复。

---
---

## 6. 剩余任务消解（阶段 G–Z）

> **本部分由原 `mvp-gaps-plan.md` 并入**（阶段 G–L：编译器能力补齐；阶段 M–T：标准库深度完善；阶段 U–Z：目标 API 对齐与编译器能力补齐）。
> **任务执行记录**：见任务树 [`stage-g-l.md`](tasks/stage-g-l.md) / [`stage-m-t.md`](tasks/stage-m-t.md) / [`stage-u-z.md`](tasks/stage-u-z.md) 各子任务叶子文档。
> **§6.2（阶段 G–L 计划总览）与 §6.3（阶段 G–L 详情）**已并入上文 §2 与 §3，故本节直接从 §6.3b（阶段 M–T）开始。

### 6.1. 背景：MVP 已知限制盘点

#### 6.1.1 规划中 / 未实现特性（7 项）

| # | 限制（guide/13-references-limits.md §13） | 编译器现状依据 | 归属阶段 |
|---|---------------------|---------------|---------|
| 1 | **宏系统**：✅ 已解决——I1 声明式宏 / I2 内置格式化宏 / I3 集合宏均已实现 | grammar.md §2.14 `macro_rules!` EBNF（新 crate `rlyeh-macro`）；std-lib.md §8 `Display`/`Debug` 仍为规划 API | **I**（I1/I2/I3 ✅） |
| 2 | **引用与借用**：`&x` 表达式、`&T` 参数类型、`str` 类型、解引用 `*`、裸指针均未实现；仅方法接收者 `&self`/`&mut self` 可用 | grammar.md Type 规则含 `'&' Lifetime? 'mut'? Type`、UnaryExpr 含 `'*' | '&' 'mut'?`、Pattern 含 `'ref'`（均已定义未实现）；typecheck `UnaryOp::Deref/AddrOf/AddrOfMut` → Unsupported（check_expr.rs）；borrowck crate 仅服务 `&self` 接收者 | **G**（G1–G4 ✅） |
| 3 | **闭包**：`|x| x + 1` 语法可解析，typecheck 报 Unsupported | parser 已产出 `AstExpr::Closure`；typecheck 报 Unsupported（check_expr.rs） | **H**（H1–H5 ✅） |
| 4 | **运算符**：✅ 已解决——K1 `?` 错误传播、H1 函数指针、H4 `dyn Trait` 均已实现 | grammar.md 含 `'dyn' TraitBound` 与后缀 `'?'`；`Option`/`Result` + `expect`/`unwrap_or` 已实现 | ✅ |
| 5 | **所有权层级**：✅ 已解决——K2 `Box<T>` / K3 `Rc<T>`/`Arc<T>`/`Weak<T>` / K4 `Gc<T>`（MVP）均已实现 | memory-model.md §4（Rc/Arc）/§5（Gc）规范完备；`Box<T>` 亦为 §3 目标 API | **K** |
| 6 | **并发**：actor `async` 方法 + `.await`/`send` 已实现；普通函数 `async fn`/`await` 已支持（L1 ✅）；`json` 序列化已实现（L2 ✅） | std-lib.md §8（fmt）规划标注 / §9（serde）已部分实现（`json::stringify`/`json::parse::<T>` 内建）；§10（async 运行时）已部分实现 | fmt→**I**，serde→L2 ✅，async→L1 ✅ |
| 7 | **迭代器协议**：数值区间、`for x in vec`/`for (k, v) in map`/`for x in arr`（数组迭代 J1）可用；自定义迭代器（`next() -> Option<T>` 方法）接入 `for`（J2）；`map`/`filter`/`fold`/`collect`/`take`/`skip` 适配器可用（J3，返回 Vec） | typecheck for 循环分派（range/Vec/HashMap/数组/迭代器），适配器内建 desugar（check_expr.rs）；std-lib.md §2.3 Iterator 规划 | **J** |

#### 6.1.2 实现约束（5 项）

| # | 约束（guide/13-references-limits.md §13） | 现状 | 处置 |
|---|---------------------|------|------|
| 1 | std 模块化（core.rl + time/io/net/sync 子模块） | ✅ 已完成 | 说明性，不纳入计划 |
| 2 | net：`tcp_connect` 依赖平台 `sockaddr_in4` 布局（已双布局化）；WASI 下网络不可用 | 平台约束 | ✅ 已解决（L4：WASI 下 net 明确禁用文档化；执行情况见 [`l4-platform.md`](tasks/leaf/l4-platform.md)） |
| 3 | Actor 运行时：交叉编译 / WASM 目标下 actor 程序暂不支持 | 平台约束 | ✅ 已解决（L4：Actor 交叉编译 / WASM 支持；执行情况见 [`l4-platform.md`](tasks/leaf/l4-platform.md)） |
| 4 | `String::from(s)`：非字面量 Str（运行期内容）长度表达未实现 | typecheck 报 Unsupported | **G**（str 切片一并补齐） |
| 5 | region 选项：`adaptive`/`with_size (N)` 可用；`strategy (bump)` 等其余选项规划中 | `rlyeh-region-alloc` 已就绪（PGO 回灌 F2 已打通） | ✅ 已解决（L3：`strategy (bump)` 语法 + 运行时 C ABI 接线 + PGO 回灌） |

---

### 6.3b. 标准库深度完善计划（阶段 M–T）

> **需求来源**：[`std-lib.md`](./std-lib.md) 状态总览中标记为 📋 规划 / 🔧 部分的标准库章节（§2.3 Iterator、§4 File/标准输入输出/Path/fs/NIO/sendfile、§5 TCP 对象化/HTTP、§6.2 Channel、§8 `Display`/`Debug`、§9 `Serialize`/`Deserialize`、§10 异步运行时、§11 智能指针目标 API、§12 错误处理）。
> **前置**：阶段 G–L 已全部完成，提供能力地基——G（引用/`&str`/裸指针/严格借用）、H（函数指针/闭包/`dyn Trait`）、I（宏/格式化宏/集合宏）、J（迭代器）、K（`?`/Box/Rc/Arc/Gc）、L（async 同步语义/json/region 指令/WASI）。
> **Rust 绑定层策略**：延续 rlyeh-std「绑定层阶段」——每个新 std 模块先在 `crates/rlyeh-std/src/` 用 Rust 实现 C ABI 绑定（`#[no_mangle] extern "C"`），语言侧 `crates/rlyeh-std/rlyeh/*.rl` 经 FFI 调用封装；**NIO/sendfile 绑定层已就绪**（`rlyeh-std/src/nio/`：poller.rs/sendfile.rs/nonblocking.rs，三平台 epoll/kqueue/poll），阶段 R 为纯语言侧封装。
> **现状修正（std-lib.md 过时标注，规划时以实际为准）**：§8 内置格式化宏已实现（I2：`println!`/`print!`/`format!`/`dbg!` + N4 `eprintln!`/`eprint!`（stderr），`{}`/`{:?}` 占位）；§12 `?` 运算符已实现（K1）；§9 `json::stringify`/`json::parse::<T>` 已实现（L2，trait/derive 仍规划）。
>
> **任务粒度**：全部任务已拆分为「可独立实现 + 独立验收」的子任务（共 **59 个**），字母后缀（a/b/c）子任务须按序完成（后者依赖前者）；M–T 推荐执行路线见上文 §2。

---

| 阶段 | 阶段详情文档 | 任务详情文档 |
|------|-------------|-------------|
| **M** | [`M.md`](stages/M.md) | 见 [`M.md`](stages/M.md) 任务表「详情」列 |
| **N** | [`N.md`](stages/N.md) | 见 [`N.md`](stages/N.md) 任务表「详情」列 |
| **O** | [`O.md`](stages/O.md) | 见 [`O.md`](stages/O.md) 任务表「详情」列 |
| **P** | [`P.md`](stages/P.md) | 见 [`P.md`](stages/P.md) 任务表「详情」列 |
| **Q** | [`Q.md`](stages/Q.md) | 见 [`Q.md`](stages/Q.md) 任务表「详情」列 |
| **R** | [`R.md`](stages/R.md) | 见 [`R.md`](stages/R.md) 任务表「详情」列 |
| **S** | [`S.md`](stages/S.md) | 见 [`S.md`](stages/S.md) 任务表「详情」列 |
| **T** | [`T.md`](stages/T.md) | 见 [`T.md`](stages/T.md) 任务表「详情」列 |

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
| **U** | 编译器地基（std 完整化前置） | U1 作用域栈重构、U2 trait 关联类型、U3 泛型约束 where、U4 `-> Self` 返回、U5 MIR AddrOf 任意目标、U6 数值转换 Cast IR、**U7 方法级泛型参数**、**U8 泛型结构体构造 + 泛型 trait** | 8 | G、T 已知限制 | ✅ 已完成（U1–U8 全部落地；执行情况见 [`stage-u-z.md`](tasks/stage-u-z.md) 叶子文档） |
| **V** | 集合与迭代器完整化 | V1 借用迭代器（`Iter`/`IterMut`/`Iter<'_, K, V>`）、V2 String 码点迭代器（`Chars`/`Lines`）、V3 Iterator 默认方法 + 适配器迁移、V4 `get_mut` 引用语义、V5 新集合（HashSet/BTreeMap/VecDeque） | 5 | U1/U2/U3 | ✅ 已完成（V1/V2/V3/V4/V5 全部完成：`iter_ref` 引用元素 `Option<&T>` + `iter_pairs`/`KVRef` KV 引用迭代、chars/lines 目标签名升级 + char 32 位、Iterator::Item + 自定义迭代器适配器迁移均已落地；数组/`Vec` 适配器因数组非命名类型保留内建 desugar，记为已知语言限制；见 [`v2-str-view.md`](tasks/v2-str-view.md)/[`v3-iterator-adapters.md`](tasks/v3-iterator-adapters.md)/[`stage-u-z.md`](tasks/stage-u-z.md)） |
| **W** | 异步运行时完整化 | W1 Future 泛型化（Output/Pin/Context）、W2 await 状态机扩展（控制流/表达式嵌套/引用跨 await）、W3 事件驱动 executor（epoll/kqueue/io_uring + Future 挂起）、W4 `join_all`/`timeout` Future 版 + `TimeoutError`、W5 `recv_async`/HTTP async 真异步、W6 async 泛型/递归 + 闭包跨线程捕获 | 6 | U、R（Poller）、S1c | ✅ **全部完成（W1–W6）**（执行情况见 [`stage-u-z.md`](tasks/stage-u-z.md) W 叶子文档） |
| **X** | 序列化/格式化/时间完整化 | X1 Duration/Instant/SystemTime 完整 API、X2 标准 TOML（空格形式/`[section]`/注释/多行字符串）+ 解析鲁棒性、X3 `Deserialize` trait + Serializer/Deserializer 框架 + `JsonError`/`TomlError`、X4 Formatter 完整化（`Result<(), FmtError>`） | 4 | U3/U4、Q | 🔧 进行中（X1 ✅；X2/X3/X4 规划，见 [`stage-u-z.md`](tasks/stage-u-z.md) X 叶子文档） |
| **Y** | IO/网络/并发/智能指针收尾 | Y1 File `open_with`/`read(&mut [u8])`/`write(&[u8])`/Metadata、Y2 NIO 高性能后端（epoll/kqueue）、Y3 HTTP 连接复用 + sendfile Windows `TransmitFile`、Y4 Mutex/RwLock guard 完整 + Channel 泛型化/bounded、Y5 `Box::leak` 目标签名、Y6 `Error::source` + `Into::into` 自动转换、Y7 UDP（`net/udp.rl`）、Y8 `thread::Builder::stack_size` | 8 | U、O/R/P | 📋 规划 |

> **推荐执行路线**：
> - **快赢线**（独立性强、不依赖 U 全量，可先行交付）：X1（Duration 构造器/读取器补齐，纯 std）→ Y8（stack_size，driver 注入扩展）→ Y1（`File::open_with`，libc extern 扩展）→ Y3（sendfile Windows 平台分支）。
> - **主线（依赖驱动）**：U1（作用域栈，解锁引用语义）→ U2（关联类型）→ U3（泛型约束）→ U4（`Self` 返回）→ V（V1→V4→V3→V2→V5，随 U 逐项解锁）→ X3/X4（依赖 U3/U4）→ W（W1→W2→W4→W3→W5→W6，W3 事件驱动为最大单点）→ Y 收尾。
> - **优先级建议**：U1（消除已知限制、风险最低收益最广）> V4（`get_mut` 引用语义，集合 API 高频）> X1（时间 API 补全，零依赖）> U2（解锁 Iterator/Future 泛型化）> W2（await 控制流块，日常编写收益大）> W3（事件驱动 executor，收益最大但风险最高）> 其余。


| 阶段 | 阶段详情文档 | 任务详情文档 |
|------|-------------|-------------|
| **U** | [`U.md`](stages/U.md) | 见 [`U.md`](stages/U.md) 任务表「详情」列 |
| **V** | [`V.md`](stages/V.md) | 见 [`V.md`](stages/V.md) 任务表「详情」列 |
| **W** | [`W.md`](stages/W.md) | 见 [`W.md`](stages/W.md) 任务表「详情」列 |
| **X** | [`X.md`](stages/X.md) | 见 [`X.md`](stages/X.md) 任务表「详情」列 |
| **Y** | [`Y.md`](stages/Y.md) | 见 [`Y.md`](stages/Y.md) 任务表「详情」列 |

### 6.4. 执行记录

> 阶段 G–T / U–Z 的任务具体执行情况已归档至任务树：
> - 阶段 G–L：[`tasks/stage-g-l.md`](./tasks/stage-g-l.md)（§执行记录）
> - 阶段 M–T：[`tasks/stage-m-t.md`](./tasks/stage-m-t.md)（§执行记录）
> - 阶段 U–Z：[`tasks/stage-u-z.md`](./tasks/stage-u-z.md)（§执行记录）
>
> 本文件保留计划主体（§2/§3/§3b/§3c 阶段详情）与状态标识；详细实现流水见任务树 / git 历史。

**状态摘要**：阶段 G–L 全部完成、阶段 M–T 全部完成、U 全部完成、V 已完成、W 全部完成、X 全部完成（2026-08-30）、Y 部分完成（Y2/Y5/Y6/Y7/Y8 ✅；Y1/Y3/Y4 部分）。

---

### 6.5. 跟踪与验收约定

1. 每个阶段/任务完成须满足：`cargo test --workspace` 全绿 + `cargo clippy --workspace --all-targets` 0 警告。
2. 涉及运行时/并发类测试（Actor、通道、锁、join、GC 周期）须按 `design/00_项目总览.md` 硬性规则带超时保护，挂起即视为失败。
3. 任务完成后同步更新：本文档状态标识 + 任务树（`tasks/` 阶段索引进度与叶子文档）+ `guide/13-references-limits.md` §13（勾销对应限制条目，并更新 `grammar.md` 顶部实现状态标注）。
4. 每个阶段产出对应的集成测试（仿 development-plan.md 各阶段 `*_test.rs` 用例并全量回归）。
5. 优先交付顺序建议：G1/G2（引用地基 + str）→ I1/I2（宏 + 格式化）→ H1/H2（函数指针 + 无捕获闭包）→ K1（`?`）→ J 全阶段 → 其余。

---

- [x] **T 阶段：集合与迭代器收尾**（2026-08-24）：T1a Vec / T1b String / T1c HashMap API 补齐 + T2 `Iterator` trait（元素固定 i64）+ T3a `Box::leak`（裸指针退化）/ T3b Rc/Arc/Weak 核对。各任务执行情况与技术细节见任务树 [`stage-m-t.md`](tasks/stage-m-t.md) T 阶段叶子文档（`t1a`–`t3b`）。

> **维护者**：Rlyeh Language Team
> **最后更新**：2026-08-31

> **维护者**：Rlyeh Language Team
> **最后更新**：2026-08-31
