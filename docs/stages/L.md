# 阶段 L — 生态模块与平台收尾

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-*.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| L1 | **`async fn`/`await`**：普通函数异步支持（actor `async` 方法已有独立机制，抽象复用） | ✅（MVP 同步语义；执行情况见 [`l1-async-fn.md`](tasks/leaf/l1-async-fn.md | [`l1-async-fn.md`](../tasks/leaf/l1-async-fn.md) |
| L2 | **`serde` 序列化模块**：`Serialize`/`Deserialize` trait + `#[derive]` 风格宏（依赖 I） | ✅（`json::stringify`/`json::parse::<T>` 内建 + turbofish；自定义 trait/derive 仍规划；执行情况见 [`l2-serde.md`](tasks/leaf/l2-serde.md | [`l2-serde.md`](../tasks/leaf/l2-serde.md) |
| L3 | **region 选项接线**：`strategy (bump)` 等其余选项（`rlyeh-region-alloc` 已就绪，PGO 预测可直接回灌 `adaptive` 初始容量） | ✅ | [`l3-region.md`](../tasks/leaf/l3-region.md) |
| L4 | **平台加固**：WASI 下 net 支持（或明确禁用文档化）；Actor 交叉编译 / WASM 支持（消除 §13 约束 2/3） | ✅ | [`l4-platform.md`](../tasks/leaf/l4-platform.md) |

---


## 4. 执行记录

> 阶段 A–F 的任务具体执行情况已归档至任务树：[`tasks/stage-a-f.md`](../tasks/stage-a-f.md)（§子任务叶子文档）。
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
> - MIR 中间表示（P011 / M1.7）：[`milestone-tasks/m1-7.md`](milestone-tasks/m1-7.md)
> - 增量编译引擎（P007 / M2.3）：[`milestone-tasks/m2-3.md`](milestone-tasks/m2-3.md)
> - 包管理器 Dagon（P008 / M2.2）：[`milestone-tasks/m2-2.md`](milestone-tasks/m2-2.md)
> - LLVM 后端与代码生成（P013 / M1.8、M1.9）：[`milestone-tasks/m1-8.md`](milestone-tasks/m1-8.md)、[`milestone-tasks/m1-9.md`](milestone-tasks/m1-9.md)
> 
> 详细实现（含 ADR 设计决策）见上述单任务文档；此处不再重复。

---
---

## 6. 剩余任务消解（阶段 G–Z）

> **本部分由原 `mvp-gaps-plan.md` 并入**（阶段 G–L：编译器能力补齐；阶段 M–T：标准库深度完善；阶段 U–Z：目标 API 对齐与编译器能力补齐）。
> **任务执行记录**：见任务树 [`stage-g-l.md`](../tasks/stage-g-l.md) / [`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md) 各子任务叶子文档。
> **§6.2（阶段 G–L 计划总览）与 §6.3（阶段 G–L 详情）**已并入上文 §2 与 §3，故本节直接从 §6.3b（阶段 M–T）开始。

### 6.1. 背景：MVP 已知限制盘点

#### 6.1.1 规划中 / 未实现特性（7 项）

| # | 限制（guide.md §13） | 编译器现状依据 | 归属阶段 |
|---|---------------------|---------------|---------|
| 1 | **宏系统**：✅ 已解决——I1 声明式宏 / I2 内置格式化宏 / I3 集合宏均已实现 | grammar.md §2.14 `macro_rules!` EBNF（新 crate `rlyeh-macro`）；std-lib.md §8 `Display`/`Debug` 仍为规划 API | **I**（I1/I2/I3 ✅） |
| 2 | **引用与借用**：`&x` 表达式、`&T` 参数类型、`str` 类型、解引用 `*`、裸指针均未实现；仅方法接收者 `&self`/`&mut self` 可用 | grammar.md Type 规则含 `'&' Lifetime? 'mut'? Type`、UnaryExpr 含 `'*' | '&' 'mut'?`、Pattern 含 `'ref'`（均已定义未实现）；typecheck `UnaryOp::Deref/AddrOf/AddrOfMut` → Unsupported（check_expr.rs）；borrowck crate 仅服务 `&self` 接收者 | **G**（G1–G4 ✅） |
| 3 | **闭包**：`|x| x + 1` 语法可解析，typecheck 报 Unsupported | parser 已产出 `AstExpr::Closure`；typecheck 报 Unsupported（check_expr.rs） | **H**（H1–H5 ✅） |
| 4 | **运算符**：✅ 已解决——K1 `?` 错误传播、H1 函数指针、H4 `dyn Trait` 均已实现 | grammar.md 含 `'dyn' TraitBound` 与后缀 `'?'`；`Option`/`Result` + `expect`/`unwrap_or` 已实现 | ✅ |
| 5 | **所有权层级**：✅ 已解决——K2 `Box<T>` / K3 `Rc<T>`/`Arc<T>`/`Weak<T>` / K4 `Gc<T>`（MVP）均已实现 | memory-model.md §4（Rc/Arc）/§5（Gc）规范完备；`Box<T>` 亦为 §3 目标 API | **K** |
| 6 | **并发**：actor `async` 方法 + `.await`/`send` 已实现；普通函数 `async fn`/`await` 已支持（L1 ✅）；`json` 序列化已实现（L2 ✅） | std-lib.md §8（fmt）规划标注 / §9（serde）已部分实现（`json::stringify`/`json::parse::<T>` 内建）；§10（async 运行时）已部分实现 | fmt→**I**，serde→L2 ✅，async→L1 ✅ |
| 7 | **迭代器协议**：数值区间、`for x in vec`/`for (k, v) in map`/`for x in arr`（数组迭代 J1）可用；自定义迭代器（`next() -> Option<T>` 方法）接入 `for`（J2）；`map`/`filter`/`fold`/`collect`/`take`/`skip` 适配器可用（J3，返回 Vec） | typecheck for 循环分派（range/Vec/HashMap/数组/迭代器），适配器内建 desugar（check_expr.rs）；std-lib.md §2.3 Iterator 规划 | **J** |

#### 6.1.2 实现约束（5 项）

| # | 约束（guide.md §13） | 现状 | 处置 |
|---|---------------------|------|------|
| 1 | std 模块化（core.rl + time/io/net/sync 子模块） | ✅ 已完成 | 说明性，不纳入计划 |
| 2 | net：`tcp_connect` 依赖平台 `sockaddr_in4` 布局（已双布局化）；WASI 下网络不可用 | 平台约束 | ✅ 已解决（L4：WASI 下 net 明确禁用文档化；执行情况见 [`l4-platform.md`](../tasks/leaf/l4-platform.md)） |
| 3 | Actor 运行时：交叉编译 / WASM 目标下 actor 程序暂不支持 | 平台约束 | ✅ 已解决（L4：Actor 交叉编译 / WASM 支持；执行情况见 [`l4-platform.md`](../tasks/leaf/l4-platform.md)） |
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
