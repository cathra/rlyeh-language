# 里程碑总览（阶段 A–Z）

> **本层职责**：整合全部阶段（A–Z）的**里程碑与实现进度**（一屏总览）。各阶段任务的具体任务列表见对应阶段索引文档，具体实施见叶子文档 / 权威源文档。
> **权威来源**：`CODEBUDDY.md` §5（阶段状态）、`development-plan.md`（阶段 A–F）、`development-plan.md`（阶段 G–T / U–Z）。

---

## 一屏总览

| 阶段 | 主题 | 阶段索引 | 子任务数 | 状态 |
|------|------|---------|:---:|------|
| **A–F** | 编译器 + 工具链（基础） | [`stage-a-f.md`](./stage-a-f.md) | 若干 | ✅ 全部完成（A1–A4、B1–B5、C1–C3、D1–D3、E1–E3、F1–F2） |
| **G–L** | 编译器能力补齐 | [`stage-g-l.md`](./stage-g-l.md) | 若干 | ✅ 全部完成（G1–G4、H1–H5、I、J1–J3、K1–K4、L1–L4） |
| **M–T** | 标准库深度完善 | [`stage-m-t.md`](./stage-m-t.md) | 59 | ✅ 全部完成（M–T 各阶段） |
| **U** | 编译器地基 | [`stage-u-z.md`](./stage-u-z.md) | 8 | ✅ 全部完成（U1–U6 + U7 方法级泛型 + U8 泛型结构体/泛型 protocol） |
| **V** | 集合与迭代器完整化 | [`stage-u-z.md`](./stage-u-z.md)（[V2](./v2-str-view.md) / [V3](./v3-iterator-adapters.md)） | 5（V3 细分为 12 子任务） | ✅ 已完成（V1/V2/V3/V4/V5 全部完成；V3 数组/`Vec` 适配器因数组非命名类型保留内建 desugar，记为已知语言限制） |
| **W** | 异步运行时完整化 | [`stage-u-z.md`](./stage-u-z.md) | 6 | ✅ 全部完成（W1–W6；2026-08-30 验收全绿；async_if_await/async_neg poll 死循环已于 2026-08-29 修复并解除 skip） |
| **X** | 序列化/格式化/时间完整化 | [`stage-u-z.md`](./stage-u-z.md) | 4 | ✅ 全部完成（X1/X2/X3/X4 全部完成，2026-08-30） |
| **Y** | IO/网络/并发/智能指针收尾 | [`stage-u-z.md`](./stage-u-z.md) | 8 | ✅ 完成（Y1/Y3 降级收口；Y4 类型层/API 完成，跨线程移动待 Thread 传参） |

---

## 关键里程碑

| 里程碑 | 完成时间 | 说明 |
|--------|---------|------|
| 阶段 A–F 全部完成 | — | 编译器后端 + 工具链（rlyeh test/fmt/check/doc/bench、交叉编译、WASM、发布流程、LSP、PGO） |
| 阶段 G–L 全部完成 | — | 引用/借用、一等函数、宏、迭代器协议、错误传播、生态收尾 |
| 阶段 M–T 全部完成 | — | 错误处理、文件系统、网络、并发、序列化、异步、集合收尾 |
| 阶段 U 全部完成 | 2026-08-25 | U1–U8（作用域栈、关联类型、泛型约束、`Self` 返回、AddrOf、Cast IR、方法级泛型、泛型结构体） |
| V5 新集合 | 2026-08-26 | VecDeque / HashSet / BTreeMap |
| V3 默认方法 | 2026-08-26 | protocol 默认方法机制 + `Iterator` count/sum/any/all |
| V2-C 打印修复 | 2026-08-26 | `&str`（StrFat）打印链路（方案 A） |
| V2-D 函数参数 | 2026-08-26 | `&str` 函数参数 StrFat ABI |
| W1–W6 全部完成 | 2026-08-30 | async 运行时完整化 |
| X1–X4 全部完成 | 2026-08-30 | TOML/JSON 序列化、Deserialize、Formatter |
| Y 阶段收尾 | 2026-11-15 | Y1/Y3 降级收口；Y4 类型层/API 完成，跨线程移动移交 0.3.0 |

---

## 已知问题与限制

> 已修复项不再列于此（历史记录见 `CHANGELOG.md` 与对应叶子文档）；下表仅保留仍然生效的已知限制。

| 问题/限制 | 阶段 | 状态与影响 |
|-----------|------|------------|
| V3 数组/`Vec` 适配器走 protocol 方法（数组非命名类型语言限制） | V3 | 📋 已知语言限制：数组 `[T;N]` 非命名类型无法 `impl Iterator`，适配器保留内建 desugar 返回 `Vec`；功能完整、全量测试通过，非阻塞项，待数组成为命名类型后迁移 |
| Y3 sendfile Windows `TransmitFile` | Y3 | 📋 平台限制：macOS/Linux sendfile 已落地；Windows 分支因本机不可编译验证保留 Unsupported，待 Windows CI 验证 |

**已修复（2026-08-29/30）**：`async_if_await.rl` / `async_neg.rl` poll 死循环（W2）、V1 引用迭代器 + `Option<&T>`、V2 `Chars`/`Lines` 目标签名；Y1 切片参数化由 S3 `read_slice`/`write_slice` 解除。

### 0.2.0 移交 0.3.0 的残留项（非 0.1.0 阻塞）

| 项 | 来源 | 说明 |
|----|------|------|
| Q-M4 智能指针 Drop | 0.2.0-Q | `Box`/`Rc`/`Arc` 未接 Drop；0.2.0 Drop/RAII 已覆盖用户 struct、字段级 drop glue、guard 自动解锁 |
| W L3 `!` never | 0.2.0-W | `panic!` 当前返回 `()`，`if` 分支类型合并不把 panic 视为发散 |
| C enum derive | 0.2.0-C | 仅 struct 的 Clone/PartialEq/Debug；enum derive、bound 强制校验未做 |
| I 泛型 impl 静态方法 | 0.2.0-I | 泛型关联静态方法 MVP 不支持，Arena 需调用方显式构造 |
| Y4 跨线程移动 | 0.1.0-Y4（0.2.0 移交） | `Sender`/`Receiver` 类型层/共享完成，跨线程移动受 `Thread::start` 传参限制 |
| P2-5 K-M1..M3 | 0.2.0-K | harness + C0/C1/C2 快照基线完成；三阶段 bootstrap 未启动 |
| P2-1 外部 crate | 长期 | pubgrub/clap/serde/toml/tar/flate2/libc 长期保留 Rust 经 FFI |
| P1-3 B4 可见性 | 0.2.0-B | 模块级默认私有 + pub 校验暂缓（std 跨模块全可达依赖） |

> 完整验收记录与移交理由见 [`development-plan-0.2.0.md`](../development-plan-0.2.0.md) §1.1。

---

## 历史里程碑任务

> 各历史里程碑（M1/M2/M3）的任务按三级结构组织（CODEBUDDY §6 里程碑列表 → milestone-*.md 任务列表 → milestone-tasks/ 单任务文档）：

| 里程碑 | 范围 | 状态 | 任务列表 | 单任务文档 |
|--------|------|------|---------|-----------|
| Milestone 1 — 编译器 MVP（Month 0-4） | M1.1–M1.9 | ✅ 全部完成 | [`milestone-1.md`](./milestone-1.md) | [`milestone-tasks/`](./milestone-tasks/) |
| Milestone 2 — 生产可用（Month 4-8） | M2.1–M2.8 + 已完成语言特性 | 🟢 基本完成（M2.4 LSP MVP 已落地；剩余 Windows/ARM 工具链环境项） | [`milestone-2.md`](./milestone-2.md) | [`milestone-tasks/`](./milestone-tasks/) |
| Milestone 3 — 生态繁荣（Month 8-12） | M3.1–M3.6 | 📋 规划 | [`milestone-3.md`](./milestone-3.md) | [`milestone-tasks/`](./milestone-tasks/) |

---

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 里程碑总览整合（A–Z + CODEBUDDY 进度） |
| 2026-08-26 | 历史里程碑执行情况迁移至 milestone-1/2/3.md，总览补充引用 |
| 2026-11-15 | 修复已知问题：M2.4 LSP 状态由「待」更正为 ✅（F1 MVP 已落地），Milestone 2 改为基本完成；「已知问题（阻塞/待修）」清理为「已知问题与限制」（移除已修复项、删除误放的 Y1–Y8 汇总行），新增 0.2.0 移交 0.3.0 残留项表；补齐 X/Y 关键里程碑 |
