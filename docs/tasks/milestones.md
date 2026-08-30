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
| **U** | 编译器地基 | [`stage-u-z.md`](./stage-u-z.md) | 8 | ✅ 全部完成（U1–U6 + U7 方法级泛型 + U8 泛型结构体/泛型 trait） |
| **V** | 集合与迭代器完整化 | [`stage-u-z.md`](./stage-u-z.md)（[V2](./v2-str-view.md) / [V3](./v3-iterator-adapters.md)） | 5（V3 细分为 12 子任务） | ✅ 已完成（V1/V2/V3/V4/V5 全部完成；V3 数组/`Vec` 适配器因数组非命名类型保留内建 desugar，记为已知语言限制） |
| **W** | 异步运行时完整化 | [`stage-u-z.md`](./stage-u-z.md) | 6 | ✅ 全部完成（W1–W6；2026-08-30 验收全绿；async_if_await/async_neg poll 死循环已于 2026-08-29 修复并解除 skip） |
| **X** | 序列化/格式化/时间完整化 | [`stage-u-z.md`](./stage-u-z.md) | 4 | ✅ 全部完成（X1/X2/X3/X4 全部完成，2026-08-30） |
| **Y** | IO/网络/并发/智能指针收尾 | [`stage-u-z.md`](./stage-u-z.md) | 8 | 🔧 部分完成（Y2/Y5/Y6/Y7/Y8 ✅；Y1/Y3/Y4 部分） |

---

## 关键里程碑

| 里程碑 | 完成时间 | 说明 |
|--------|---------|------|
| 阶段 A–F 全部完成 | — | 编译器后端 + 工具链（rlyeh test/fmt/check/doc/bench、交叉编译、WASM、发布流程、LSP、PGO） |
| 阶段 G–L 全部完成 | — | 引用/借用、一等函数、宏、迭代器协议、错误传播、生态收尾 |
| 阶段 M–T 全部完成 | — | 错误处理、文件系统、网络、并发、序列化、异步、集合收尾 |
| 阶段 U 全部完成 | 2026-08-25 | U1–U8（作用域栈、关联类型、泛型约束、`Self` 返回、AddrOf、Cast IR、方法级泛型、泛型结构体） |
| V5 新集合 | 2026-08-26 | VecDeque / HashSet / BTreeMap |
| V3 默认方法 | 2026-08-26 | trait 默认方法机制 + `Iterator` count/sum/any/all |
| V2-C 打印修复 | 2026-08-26 | `&str`（StrFat）打印链路（方案 A） |
| V2-D 函数参数 | 2026-08-26 | `&str` 函数参数 StrFat ABI |
| W1–W6 全部完成 | 2026-08-26 | async 运行时完整化 |

---

## 已知问题（阻塞/待修）

| 问题 | 阶段 | 状态 |
|------|------|------|
| `async_if_await.rl` / `async_neg.rl` 运行时 poll 死循环内存暴涨（1.2GB+） | W2 | ✅ 已修复（2026-08-29：解除 `// skip:` 跳过，suite_test 中 async_if_await/async_neg 用例全绿，2026-08-30 验收） |
| V1 `HashMap::iter` 引用迭代器 + 引用元素 `Option<&T>` | V1 | ✅ 已完成（2026-08-29：`Vec::iter_ref` 返回 `Option<&T>`、`HashMap::iter_pairs` 经 `KVRef` 零拷贝 KV 引用迭代已落地） |
| V2 `chars`/`lines` 目标签名（返回 `Chars`/`Lines` 迭代器，需 char 类型） | V2 | ✅ 已完成（2026-08-29：chars/lines 升级为迭代器、`char` 拓宽 32 位） |
| V3 数组/Vec 适配器走 trait 方法（数组非命名类型语言限制） | V3 | 📋 已知语言限制（数组 `[T;N]` 非命名类型无法 `impl Iterator`，适配器保留内建 desugar 返回 `Vec`；功能完整、全量测试通过，需语言增强后迁移，非 V 阶段阻塞项） |
| X2/X3/X4、Y1–Y8 | X/Y | 📋 规划 |

---

## 历史里程碑任务

> 各历史里程碑（M1/M2/M3）的任务按三级结构组织（CODEBUDDY §6 里程碑列表 → milestone-*.md 任务列表 → milestone-tasks/ 单任务文档）：

| 里程碑 | 范围 | 状态 | 任务列表 | 单任务文档 |
|--------|------|------|---------|-----------|
| Milestone 1 — 编译器 MVP（Month 0-4） | M1.1–M1.9 | ✅ 全部完成 | [`milestone-1.md`](./milestone-1.md) | [`milestone-tasks/`](./milestone-tasks/) |
| Milestone 2 — 生产可用（Month 4-8） | M2.1–M2.8 + 已完成语言特性 | 🔧 部分完成 | [`milestone-2.md`](./milestone-2.md) | [`milestone-tasks/`](./milestone-tasks/) |
| Milestone 3 — 生态繁荣（Month 8-12） | M3.1–M3.6 | 📋 规划 | [`milestone-3.md`](./milestone-3.md) | [`milestone-tasks/`](./milestone-tasks/) |

---

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 里程碑总览整合（A–Z + CODEBUDDY 进度） |
| 2026-08-26 | 历史里程碑执行情况迁移至 milestone-1/2/3.md，总览补充引用 |
