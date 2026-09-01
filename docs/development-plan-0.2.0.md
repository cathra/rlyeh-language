# 开发计划 — Rlyeh 0.2.0（自举能力补齐阶段）

> **性质**：0.2.0 主线权威文档。0.1.0 已收口（阶段 A–Z 完成）。**0.2.0 = 自举能力补齐阶段**——完善 Rlyeh 语言自身使其具备自举所需表达力，为 **0.3.0 用 Rlyeh 自举语言/工具链**做准备。
> **与 0.3.0 的边界**：0.2.0 交付**语言/标准库能力** + 前端自举 PoC + 链接桥 + 差分测试；0.3.0 交付**工具链重写本身**（用 0.2.0 的能力把编译器/工具/std 改写为 Rlyeh）。
> **前置评估**：[`self-hosting/feasibility.md`](./self-hosting/feasibility.md)（逐层可行性矩阵 + 缺口 P0/P1/P2）。
> **能力缺口任务树**：[`tasks/self-hosting.md`](../tasks/self-hosting.md)（P0/P1/P2 分级索引 + `leaf/sh-*`）。
> **维护规则**：任务完成同步更新本文档状态 + `tasks/` 树 + `CODEBUDDY.md` 版本段。

---

## 1. 背景与定位

| 版本 | 状态 | 说明 |
|------|------|------|
| 0.1.0 (MVP) | ✅ 收口 | 阶段 A–Z 全部完成；工具链用 Rust 实现（bootstrap 阶段） |
| **0.2.0 (自举能力补齐)** | 🔧 规划中 | 落地全部自举所需**语言/标准库能力**（含原 P0 语言特性），交付前端自举 PoC + FFI 链接桥 + 差分测试，使 0.3.0 可自举 |
| **0.3.0 (工具链自举)** | 📋 待规划 | 用 0.2.0 能力把 lexer→…→codegen→driver→tools→std 重写为 Rlyeh（运行时可选保留 Rust 经 FFI） |

**0.2.0 主线目标（单一、可验证）**：
1. 落地**全部自举所需语言/标准库能力**：P0（`unsafe`/跨边界闭包/`dyn`+`Self`+`Any`/并发原语）、P1（泛型 trait/impl、嵌套模块、derive）、P2-2（进程 FFI）、P2-3（arena/内部可变性）。
2. 交付 **FFI/ABI 链接桥**：Rlyeh 编译产物可链接/调用现有 Rust 运行时。
3. 交付 **分阶段自举 + 差分测试基础设施**：Rust 引导器编译 Rlyeh 版组件，并与 Rust 参考实现对拍。
4. 交付**前端自举 PoC**：用 Rlyeh 重写 `lexer`+`parser`+`ast`+`macro`，经 Rust driver 编译通过 + 对拍。

**0.2.0 不负责**：工具链*重写本身*（属 0.3.0）；`dagon` 包管理器可长期保留 Rust 经 FFI。

---

## 2. 计划总览（阶段 A–M）

| 阶段 | 主题 | 对应缺口 | 关联文档 | 风险 | 状态 | 说明 |
|------|------|----------|----------|------|------|------|
| **0.2.0-A** | 泛型 trait/impl 完整化 | P1-1 | [SH-P1-1](tasks/leaf/sh-p1-1-generic-trait.md) | 🟠 中 | ⏳ 规划 | typecheck 自举前置 |
| **0.2.0-B** | 嵌套模块系统 | P1-3 | [SH-P1-3](tasks/leaf/sh-p1-3-nested-module.md) | 🟠 中 | ⏳ 规划 | 大型 crate 组织 |
| **0.2.0-C** | trait derive 宏 | P1-2 | [SH-P1-2](tasks/leaf/sh-p1-2-derive.md) | 🟠 中 | ⏳ 规划 | 消除 AST 样板 |
| **0.2.0-D** | 进程调用 / 外部工具链 FFI | P2-2 | [SH-P2-2](tasks/leaf/sh-p2-2-process-ffi.md) | 🟠 中 | ⏳ 规划 | 后端 assemble 自举 |
| **0.2.0-E** | `unsafe` 块 / 裸指针 / `#[repr(C)]` | P0-1 | [SH-P0-1](tasks/leaf/sh-p0-1-unsafe.md) | 🔴 高 | ⏳ 规划 | 运行时表达力地基 |
| **0.2.0-F** | 跨函数边界闭包 + `move` + `'static` | P0-2 | [SH-P0-2](tasks/leaf/sh-p0-2-closure.md) | 🔴 高 | ⏳ 规划 | actor 调度器 / driver 线程模型地基 |
| **0.2.0-G** | `dyn Trait` 含 `Self` + `Any` 类型擦除 | P0-3 | [SH-P0-3](tasks/leaf/sh-p0-3-dyn-any.md) | 🔴 高 | ⏳ 规划 | actor 消息协议地基 |
| **0.2.0-H** | 并发原语（Arc<Mutex>/atomic/线程 spawn） | P0-4 | [SH-P0-4](tasks/leaf/sh-p0-4-concurrency.md) | 🔴 高 | ⏳ 规划 | 运行时并发地基 |
| **0.2.0-I** | 内部可变性 / arena 表示 | P2-3 | [SH-P2-3](tasks/leaf/sh-p2-3-internal-mut.md) | 🟠 中 | ⏳ 规划 | IR 可变遍历 |
| **0.2.0-J** | FFI/ABI 链接桥 | 新增 | [SH-P2-4](tasks/leaf/sh-p2-4-linkage-bridge.md) | 🔴 高 | ⏳ 规划 | Rlyeh 产物链接 Rust 运行时 |
| **0.2.0-K** | 分阶段自举 + 差分测试基础设施 | 新增 | [SH-P2-5](tasks/leaf/sh-p2-5-staged-bootstrap.md) | 🔴 高 | ⏳ 规划 | 引导器 + 对拍验证 |
| **0.2.0-L** | 诊断信息质量对齐 | 新增 | [SH-P2-6](tasks/leaf/sh-p2-6-diagnostics.md) | 🟠 中 | ⏳ 规划 | span 诊断复刻 |
| **0.2.0-M** | 前端自举 PoC | 新增(扩) | [SH-P2-7](tasks/leaf/sh-p2-7-driver.md) | 🔴 高 | ⏳ 规划 | 交付物（dogfood） |
| **0.2.0-N** | 元组值构造 + 解构（多返回值） | P0-5 | [SH-P0-5](tasks/leaf/sh-p0-5-tuple-value.md) | 🔴 中高 | ⏳ 规划 | **复审补遗**：PoC 解析器 `(tok,rest)` 前置；类型层已就绪 |
| **0.2.0-O** | `if let` / `while let` 模式控制流 | P0-6 | [SH-P0-6](tasks/leaf/sh-p0-6-if-let.md) | 🔴 高 | ⏳ 规划 | **复审补遗**：语言完全缺失，解析器/类型检查器重写依赖 |
| **0.2.0-P** | `match` 守卫 + 范围/或模式 | P0-7 | [SH-P0-7](tasks/leaf/sh-p0-7-match-guard.md) | 🔴 中高 | ⏳ 规划 | **复审补遗**：字符分类/判别分支依赖 |
| **0.2.0-Q** | `Drop` trait / 析构 / RAII | P0-8 | [SH-P0-8](tasks/leaf/sh-p0-8-drop.md) | 🔴 高 | ⏳ 规划 | **复审补遗**：MutexGuard/arena/智能指针自动释放 |
| **0.2.0-R** | `Deref`/`DerefMut` 用户类型自动解引用 | P1-4 | [SH-P1-4](tasks/leaf/sh-p1-4-deref.md) | 🟠 中 | ⏳ 规划 | **复审补遗**：智能指针/MutexGuard 透传 |
| **0.2.0-S** | `Copy`/`Clone` 语义 + `#[derive(Copy)]` | P1-5 | [SH-P1-5](tasks/leaf/sh-p1-5-copy-clone.md) | 🟠 中 | ⏳ 规划 | **复审补遗**：拷贝模型对齐 |
| **0.2.0-T** | `?` 经 `From`/`Into` 错误自动转换 | P1-6 | [SH-P1-6](tasks/leaf/sh-p1-6-question-from.md) | 🟠 中 | ⏳ 规划 | **复审补遗**：分层错误传播 |
| **0.2.0-U** | `mem::swap` / `mem::replace` 内建 | P2-8 | [SH-P2-8](tasks/leaf/sh-p2-8-mem-swap.md) | 🟠 中 | ⏳ 规划 | **复审补遗**：IR 重写免借用冲突 |
| **0.2.0-V** | `const` / `static` 全局项 | P2-9 | [SH-P2-9](tasks/leaf/sh-p2-9-const-static.md) | 🟠 中 | ⏳ 规划 | **复审补遗**：运行时 FFI 全局状态 |
| **0.2.0-W** | `panic!`/`assert!`/`unreachable!`/`todo!` 宏 | P2-10 | [SH-P2-10](tasks/leaf/sh-p2-10-assert-macros.md) | 🟡 低 | ⏳ 规划 | **复审补遗**：编译器内部断言 |
| **0.2.0-X** | 结构体 `..` 更新 + 字段简写 | P2-11 | [SH-P2-11](tasks/leaf/sh-p2-11-struct-update.md) | 🟡 低 | ⏳ 规划 | **复审补遗**：AST 构造样板消减 |
| **0.2.0-Y** | `Send`/`Sync` 自动 trait（放宽/标记） | P3-1 | [SH-P3-1](tasks/leaf/sh-p3-1-send-sync.md) | 🟠 中 | ⏳ 规划 | **复审补遗**：并发安全基线（告警式） |

**推荐路线（依赖驱动）**：A/B/C（语言组织）→ E/F/G/H（运行时表达力地基）→ D/I（FFI/arena）→ J（链接桥）→ K（bootstrap+差分）→ L（诊断）→ M（PoC 串联）。
**优先级**：A1/A2 > B1/B2 > E（unsafe 地基）> C1–C3 > F/G > H > D > I > J > K > L > M。

---

## 3. 阶段详情

> A–M 见 §3.1–§3.13；N–Y 复审补遗见 §3.14–§3.25。每个阶段「关联文档」指向 `docs/tasks/leaf/` 下对应叶子（缺口细化 / 风险分解 / 受影响组件 / 验证 / 状态）。

### 3.1 A 泛型 trait/impl 完整化（P1-1）
- A1 泛型 trait 声明 / A2 泛型 impl / A3 含 `Self` 返回 / A4 约束 `where`/`:` 收尾。关联类型（U2）已在 0.1.0 落地，叠加泛型参数化。
> **关联文档**：[SH-P1-1 泛型 trait/impl 完整化](tasks/leaf/sh-p1-1-generic-trait.md)

### 3.2 B 嵌套模块系统（P1-3）
- B1 嵌套模块 / B2 `pub use` / B3 `super`/`crate::` / B4 可见性细化。补齐层级化命名空间（见 `docs/guide/13-references-limits.md` §模块）。
> **关联文档**：[SH-P1-3 嵌套模块系统](tasks/leaf/sh-p1-3-nested-module.md)

### 3.3 C trait derive 宏（P1-2）
- C1 `Debug` / C2 `Clone` / C3 `PartialEq` / C4 derive 框架。扩展 `macro_rules!`（I1）为属性宏 + 编译期 trait 自动实现，消除 `derive`×70+ 样板。
> **关联文档**：[SH-P1-2 trait derive 宏](tasks/leaf/sh-p1-2-derive.md)

### 3.4 D 进程调用 / 外部工具链 FFI（P2-2）
- D1 `system`/`exec` / D2 捕获 stdout/stderr / D3 对接 driver `assemble()`（调 `clang`，保留"生成 LLVM IR 文本 + 调 clang"策略，见评估报告 §6 路径 A）。
> **关联文档**：[SH-P2-2 进程调用 / 外部工具链 FFI](tasks/leaf/sh-p2-2-process-ffi.md)

### 3.5 E `unsafe` 块 / 裸指针 / `#[repr(C)]`（P0-1）
- E1 `unsafe` 块作用域与裸指针（`*const T`/`*mut T`）读写 / E2 `#[repr(C)]` 内存布局 / E3 FFI 安全边界约定。
- 验证：Rlyeh 侧用 `unsafe` 封装手动 bump 分配器（等价于 `rlyeh-region-alloc`），证明运行时表达力地基可用。
> **关联文档**：[SH-P0-1 `unsafe` 块 / 裸指针 / `#[repr(C)]`](tasks/leaf/sh-p0-1-unsafe.md)

### 3.6 F 跨函数边界闭包 + `move` + `'static`（P0-2）
- F1 闭包值跨函数边界传递（作 fn 实参/返回值）/ F2 `move` 所有权转移生效 / F3 `'static` 约束检查。
- 验证：Rlyeh 侧 `spawn(move || ...)` 跨线程执行（等价于 actor-runtime worker_loop）。
> **关联文档**：[SH-P0-2 跨函数边界闭包 + `move` + `'static`](tasks/leaf/sh-p0-2-closure.md)

### 3.7 G `dyn Trait` 含 `Self` + `Any` 类型擦除（P0-3）
- G1 `dyn Trait` 调用含 `Self` 签名方法（vtable 签名恢复）/ G2 `Any` 类型标识存储与 `downcast` 安全检查。
- 验证：经 `dyn Trait` 调含 `Self` 返回方法；`Any` 装箱 + `downcast` 往返（等价于 actor 消息分发）。
> **关联文档**：[SH-P0-3 `dyn Trait` 含 `Self` + `Any` 类型擦除](tasks/leaf/sh-p0-3-dyn-any.md)

### 3.8 H 并发原语（P0-4）
- H1 `Arc<Mutex<T>>`/`Weak` 内部可变性 + 锁原语 / H2 原子类型（`Atomic*`）/ H3 线程 `spawn`（与 F 协同）。
- 验证：Rlyeh 侧并发计数器（多线程 + `Arc<Mutex>`）无数据竞争。
> **关联文档**：[SH-P0-4 并发原语](tasks/leaf/sh-p0-4-concurrency.md)

### 3.9 I 内部可变性 / arena 表示（P2-3）
- I1 引入 `RefCell` 等价或 I2 arena + 整数索引（`Arena<T>` + `NodeId`）表示树形 IR（与 Rlyeh 索引式倾向一致，避免运行时引用计数开销）。
> **关联文档**：[SH-P2-3 内部可变性 / arena 表示](tasks/leaf/sh-p2-3-internal-mut.md)

### 3.10 J FFI/ABI 链接桥（新增，关键）
- J1 **符号兼容**：Rlyeh codegen 发射的 LLVM IR 符号命名 / 调用约定须与 Rust 运行时 rlib 对齐（C ABI 边界）。
- J2 **链接编排**：Rlyeh 编译产物（`.o`/IR）在 `assemble()` 阶段与 Rust 运行时（`actor-runtime`/`region-alloc`/`gc-runtime`/`rlyeh-std` 绑定层）rlib 一并交给 `clang` 链接。
- J3 **运行时入口约定**：Rlyeh 程序 `fn main` 与 Rust 运行时初始化（GC/region/actor 引导）的衔接。
- 验证：Rlyeh 写的"hello world" 经 Rlyeh codegen + Rust 运行时链接，运行输出正确。
> **关联文档**：[SH-P2-4 FFI/ABI 链接桥](tasks/leaf/sh-p2-4-linkage-bridge.md)

### 3.11 K 分阶段自举 + 差分测试基础设施（新增，关键）
- K1 **引导器（Rust driver）编译 Rlyeh 版组件**：Rust 版编译器始终作为 bootstrap 编译器，先编译 Rlyeh 写的 `lexer`/`parser`/…/`typecheck`/`codegen`/`driver`。
- K2 **三阶段 bootstrap 校验**：① Rust 编译 Rlyeh 编译器；② Rlyeh 编译器编译自身得 `rlyeh₂`；③ `rlyeh₂` 编译自身得 `rlyeh₃`，`rlyeh₂` 与 `rlyeh₃` 字节/行为一致（经典自举校验）。
- K3 **差分测试 harness**：同一 `.rl` 程序分别经 Rust 参考编译器与 Rlyeh 编译器编译，比较 IR 文本 / 可执行行为 / 诊断输出。
- K4 **快照测试**：AST/HIR/MIR/LIR 关键节点序列化快照，回归比对。
- 本阶段为 0.3.0 自举的工程骨架，0.2.0 内先落地 harness 与 PoC 级对拍（lexer/parser + 后续逐步扩展）。
> **关联文档**：[SH-P2-5 分阶段自举 + 差分测试基础设施](tasks/leaf/sh-p2-5-staged-bootstrap.md)

### 3.12 L 诊断信息质量对齐（新增）
- L1 span 级错误定位（文件名/行/列/长度）/ L2 结构化诊断（错误码 + 建议）/ L3 与 Rust 参考实现诊断文本对拍。
- 验证：`check`/`typecheck` 错误输出与 Rust 版语义一致。
> **关联文档**：[SH-P2-6 诊断信息质量对齐](tasks/leaf/sh-p2-6-diagnostics.md)

### 3.13 M 前端自举 PoC（交付物，扩展）
- M1–M3：用 Rlyeh 重写 `lexer`+`parser`+`ast`+`macro`（依赖 A/B/C/E 落地）。
- M4：经 Rust `rlyeh-driver` 编译通过，并与 Rust 版前端**对拍**（同 `.rl` 输入，token/AST 一致）。
- 验收即"前端逻辑主体已是 Rlyeh 源码、可被自身工具链编译"。
> **关联文档**：[SH-P2-7 前端自举 PoC（driver 自举）](tasks/leaf/sh-p2-7-driver.md)

### 3.14 N 元组值构造 + 解构（多返回值）（P0-5）
解析器骨架 `(token, rest)` 风格需元组多返回值。类型层（`Type::Tuple`/单元 `()`）已就绪，缺值字面量/解构/多返回 codegen。
- N1 元组值字面量 `(a, b, c)` → 复用聚合槽布局 `f0/f1/...`；N2 解构 `let (a, b) = e`（`_`/嵌套）；N3 多返回值 `fn f() -> (i64, String)` + `let (x,y)=f()`；N4 `for (k,v) in map` 一致化；差分对拍（K）。
> **关联文档**：[SH-P0-5 元组值构造 + 解构](tasks/leaf/sh-p0-5-tuple-value.md)

### 3.15 O `if let` / `while let` 模式控制流（P0-6）
语言完全缺失；解析器/类型检查器重写依赖。`desugar` 为 `match`/`let+if`（零新增 IR）。
- O1 语法解析 → O2 desugar（零新增 IR）→ O3 `while let` → O4 嵌套/链；typecheck `if let` 重写。
> **关联文档**：[SH-P0-6 `if let` / `while let`](tasks/leaf/sh-p0-6-if-let.md)

### 3.16 P `match` 守卫 + 范围/或模式（P0-7）
字符分类/判别分支依赖。守卫 desugar 为「绑定临时 + `if cond`」，范围复用 `in` 语义，或模式复用 union 优先级。
- P1 守卫表达式 → P2 范围模式 → P3 或模式 → P4 组合；差分对拍 Rust 参考。
> **关联文档**：[SH-P0-7 `match` 守卫 + 范围/或模式](tasks/leaf/sh-p0-7-match-guard.md)

### 3.17 Q `Drop` trait / 析构 / RAII（P0-8）
`MutexGuard` 自动解锁、`arena` 自动释放需 `Drop`/RAII。
- Q1 `Drop` 声明 → Q2 作用域尾插入 `drop` → Q3 字段递归 → Q4 智能指针接入；离开作用域自动释放。
> **关联文档**：[SH-P0-8 `Drop` / 析构 / RAII](tasks/leaf/sh-p0-8-drop.md)

### 3.18 R `Deref`/`DerefMut` 用户类型自动解引用（P1-4）
`MutexGuard`/`Box<dyn Trait>` 透传需 `Deref` 自动解引用。
> **关联文档**：[SH-P1-4 `Deref`/`DerefMut`](tasks/leaf/sh-p1-4-deref.md)

### 3.19 S `Copy`/`Clone` 语义 + `#[derive(Copy)]`（P1-5）
拷贝模型对齐；`T: Copy` 约束。
> **关联文档**：[SH-P1-5 `Copy`/`Clone`](tasks/leaf/sh-p1-5-copy-clone.md)

### 3.20 T `?` 经 `From`/`Into` 错误自动转换（P1-6）
分层错误传播；编译器多错误类型经 `?` + `From` 传播。
> **关联文档**：[SH-P1-6 `?` 经 `From`/`Into`](tasks/leaf/sh-p1-6-question-from.md)

### 3.21 U `mem::swap` / `mem::replace` 内建（P2-8）
IR 重写免借用冲突；`borrowck`/`desugar`/`regionck` 需 `mem::swap`/`replace`。
> **关联文档**：[SH-P2-8 `mem::swap` / `mem::replace`](tasks/leaf/sh-p2-8-mem-swap.md)

### 3.22 V `const` / `static` 全局项（P2-9）
运行时 FFI 全局状态；actor 解析表等需 `const`/`static`（语法已接受）。
> **关联文档**：[SH-P2-9 `const` / `static`](tasks/leaf/sh-p2-9-const-static.md)

### 3.23 W `panic!`/`assert!`/`unreachable!`/`todo!` 宏（P2-10）
编译器内部断言；开发期断言。
> **关联文档**：[SH-P2-10 断言宏](tasks/leaf/sh-p2-10-assert-macros.md)

### 3.24 X 结构体 `..` 更新 + 字段简写（P2-11）
AST 构造样板消减；构造样板 `..` 更新/字段简写。
> **关联文档**：[SH-P2-11 结构体 `..` 更新 + 字段简写](tasks/leaf/sh-p2-11-struct-update.md)

### 3.25 Y `Send`/`Sync` 自动 trait（放宽/标记）（P3-1）
并发安全基线（告警式放宽，非硬阻塞）。
> **关联文档**：[SH-P3-1 `Send`/`Sync`](tasks/leaf/sh-p3-1-send-sync.md)

---

## 4. 跟踪与验收约定

1. 每个阶段完成须满足：`cargo test --workspace` 全绿 + `cargo clippy --workspace --all-targets` 0 警告（编译器改动回归）。
2. 能力类任务须附 Rlyeh 侧**单元/集成用例**（泛型 trait 声明、嵌套模块访问、derive 后 `dbg!`、进程调用 `echo`、unsafe bump 分配器、`spawn(move)` 跨线程、`dyn`+`Self`/`Any` 往返、`Arc<Mutex>` 并发计数）。
3. J/K/L 须产出**可运行基础设施**：链接桥（J）、bootstrap + 差分测试 harness（K）、诊断对拍（L）。
4. M 阶段须产出**对拍测试**（`tests/self-host-*`），证明 Rlyeh 版前端与 Rust 版产出一致。
5. 任务完成后同步：本文档状态 + `tasks/` 树（阶段索引 + 叶子文档）+ `CODEBUDDY.md` 版本段。

---

## 5. 0.3.0 衔接（自举执行阶段概览）

0.2.0 能力就绪后，0.3.0 按 K 的三阶段 bootstrap 推进：
1. **0.3.0-α**：Rust 引导器编译 Rlyeh 写的完整编译器（lexer→…→codegen→driver→tools→std），差分测试全绿。
2. **0.3.0-β**：Rlyeh 编译器自举自身（阶段②③），产出 `rlyeh₂`/`rlyeh₃` 一致。
3. **0.3.0-γ**：运行时三件套（actor/region/gc）与 `rlyeh-std` 绑定层*可选*用 0.2.0 的 `unsafe` 等能力重写为 Rlyeh，或持续保留 Rust 经 J 链接桥调用。
4. **dagon** 持续保留 Rust 经 FFI（算法可后续重写）。

> 0.3.0 详细计划建议在本计划临近收口时新建 `development-plan-0.3.0.md`。

---

## 6. 长期保留 Rust 的项（非 0.2.0 必须）

| 项 | 说明 |
|----|------|
| `dagon` 包管理器 | 重度外部 crate（pubgrub/压缩/沙箱），持续保留 Rust 经 FFI，算法后续可重写 |
| 运行时 crate 的*重写* | actor/region/gc/std 绑定层在 0.2.0 获得 `unsafe` 等能力后可于 0.3.0 重写；0.2.0 仅交付能力，不重写 |

> 注：原 §5（P0 语言特性长期跟踪）已上移为 0.2.0 必须项（E/F/G/H），因 0.3.0 自举语言的前提是语言本身具备这些能力。

---

## 7. 高危任务风险分解（→ 中/低危）

**原则**：每个🔴高危阶段均经「设计原型 → 最小 MVP 子集 → 增量扩展 → 差分对拍 Rust 参考 → dogfood（PoC 串联）」拆为**独立可测**的中/低危子任务。各阶段「关联文档」指向 `docs/tasks/leaf/` 下对应叶子（含完整「风险分解」小节）；下表为分解路径与关键 checkpoint 速览，细化见 §7.1–§7.11。

| 高危阶段 | 风险 | 分解路径（→ 中/低危） | 关键 checkpoint（转中/低危信号） | 关联文档 |
|----------|------|----------------------|-------------------------------|----------|
| **E** `unsafe`/裸指针/`repr(C)` | 🔴 高 | 原型：手动 bump 分配器 → 裸指针读写 → `#[repr(C)]` 布局 → FFI 安全边界 | unsafe bump 分配器跑通 | [SH-P0-1](tasks/leaf/sh-p0-1-unsafe.md) |
| **F** 跨边界闭包+`move`+`'static` | 🔴 高 | 无捕获闭包值跨 fn → `move` 所有权转移 → `'static` 检查 → `spawn(move)` | `spawn(move)` 跨线程执行 | [SH-P0-2](tasks/leaf/sh-p0-2-closure.md) |
| **G** `dyn`+`Self`+`Any` | 🔴 高 | `dyn` 调含 `Self` 方法 → `Any` 装箱 → `downcast` 安全检查 | 消息分发 `Any` 往返 | [SH-P0-3](tasks/leaf/sh-p0-3-dyn-any.md) |
| **H** 并发原语 | 🔴 高 | `Arc`/`Mutex` 内部可变性 → `Atomic*` → 线程 `spawn`（与 F 协同）→ 并发计数 | 无数据竞争计数 | [SH-P0-4](tasks/leaf/sh-p0-4-concurrency.md) |
| **J** FFI/ABI 链接桥 | 🔴 高 | 单符号 C ABI 对齐 → 多符号 → rlib 链接编排 → 运行时入口约定 | Rlyeh hello world 链接运行 | [SH-P2-4](tasks/leaf/sh-p2-4-linkage-bridge.md) |
| **K** 自举+差分 | 🔴 高 | 引导器编译单组件 → 双组件 → 三阶段 bootstrap → 差分 harness → 快照 | `rlyeh₂` ≡ `rlyeh₃` | [SH-P2-5](tasks/leaf/sh-p2-5-staged-bootstrap.md) |
| **M** 前端 PoC | 🔴 高 | 先 `lexer` → `parser` → `ast` → `macro`，每步差分对拍 | 对拍 token/AST 一致 | [SH-P2-7](tasks/leaf/sh-p2-7-driver.md) |
| **N** 元组值/解构 | 🔴 中高 | 字面量 → 解构绑定 → 多返回 → codegen 发射 → 差分对拍 | `(tok, rest)` 风格重写 | [SH-P0-5](tasks/leaf/sh-p0-5-tuple-value.md) |
| **O** `if let`/`while let` | 🔴 高 | 语法解析 → desugar 为 `match`/`let+if` → `while let` → 嵌套/链 | typecheck `if let` 重写 | [SH-P0-6](tasks/leaf/sh-p0-6-if-let.md) |
| **P** `match` 守卫/范围/或 | 🔴 中高 | 守卫表达式 → 范围模式 → 或模式 → 组合 | 字符分类 `match` 重写 | [SH-P0-7](tasks/leaf/sh-p0-7-match-guard.md) |
| **Q** `Drop`/RAII | 🔴 高 | `Drop` trait 声明 → 作用域尾插入 `drop` → 字段递归 → 智能指针接入 | 离开作用域自动释放 | [SH-P0-8](tasks/leaf/sh-p0-8-drop.md) |

> 中/低危阶段（A/B/C/D/I/L/R/S/T/U/V/W/X/Y）按叶子文档子任务推进，均有独立单测/集成用例兜底，不再单列分解。

### 7.1 E `unsafe` 块 / 裸指针 / `#[repr(C)]`（SH-P0-1，🔴 高）
运行时表达力地基；解除"运行时必须保留 Rust"的死结（事实依据见 `crates/` 核查：`rlyeh-region-alloc` `unsafe`×36、`rlyeh-gc-runtime` 裸指针/`UnsafeCell`/`#[repr(C)]`、`rlyeh-actor-runtime` FFI、`rlyeh-std` nio `unsafe`×16）。
- **E-M1（设计原型）** `unsafe` 块作用域 + 手动 bump 分配器（MVP 验证，等价于 `rlyeh-region-alloc`），证明运行时表达力地基可用。
- **E-M2** 裸指针 `*const T`/`*mut T` 读写（受控操作，复用现有 G3 裸指针机制）。
- **E-M3** `#[repr(C)]` 内存布局约定（对齐/字段序，与 Rust rlib 对齐）。
- **E-M4** FFI 安全边界约定（`extern "C"` / `#[no_mangle]` 导出语义）。
- **关键 checkpoint**：unsafe bump 分配器跑通且与 `rlyeh-region-alloc` 对拍。
> **关联文档**：[SH-P0-1 `unsafe` 块 / 裸指针 / `#[repr(C)]`](tasks/leaf/sh-p0-1-unsafe.md)

### 7.2 F 跨函数边界闭包 + `move` + `'static`（SH-P0-2，🔴 高）
actor 调度器 / driver 线程模型地基（事实依据：`rlyeh-actor-runtime` `spawn(move || worker_loop)`、`rlyeh-driver` 线程 stack 64MB `spawn(move)`）。
- **F-M1** 无捕获闭包值跨 fn（复用 H2/H5 降级为 fn 指针）。
- **F-M2** `move` 所有权转移生效（捕获聚合对象按值转移）。
- **F-M3** `'static` 约束检查。
- **F-M4** `spawn(move || ...)` 跨线程执行（等价于 actor-runtime worker_loop）。
- **关键 checkpoint**：`spawn(move)` 跨线程执行通过超时保护的并发测试。
> **关联文档**：[SH-P0-2 跨函数边界闭包 + `move` + `'static`](tasks/leaf/sh-p0-2-closure.md)

### 7.3 G `dyn Trait` 含 `Self` + `Any` 类型擦除（SH-P0-3，🔴 高）
actor 消息协议（异构消息信封）地基（事实依据：`rlyeh-actor-runtime` `ActorState: Any + Send + Sync`、`Box<dyn Any + Send>` 信封、编译器已建模 `dyn Trait` 为 2 槽胖指针）。
- **G-M1** `dyn` 调含 `Self` 方法（vtable 签名恢复）。
- **G-M2** `Any` 类型标识存储（TypeId 式）。
- **G-M3** `downcast` 安全检查。
- **关键 checkpoint**：经 `dyn Trait` 调含 `Self` 返回方法；`Any` 装箱 + `downcast` 往返（等价 actor 消息分发）。
> **关联文档**：[SH-P0-3 `dyn Trait` 含 `Self` + `Any` 类型擦除](tasks/leaf/sh-p0-3-dyn-any.md)

### 7.4 H 并发原语（SH-P0-4，🔴 高）
运行时并发地基（与 F 协同）；当前有 `Arc`/`Weak`（K3），缺 `Mutex` 内部可变性、原子类型、线程 `spawn` 一等支持。
- **H-M1** `Arc<Mutex<T>>`/`Weak` 内部可变性 + 锁原语。
- **H-M2** `Atomic*` 原子类型 + 内存序。
- **H-M3** 线程 `spawn`（接收跨边界闭包，依赖 F）。
- **H-M4** 并发计数器（无数据竞争）。
- **关键 checkpoint**：Rlyeh 侧多线程 + `Arc<Mutex>` 计数器无数据竞争；原子自增正确。
> **关联文档**：[SH-P0-4 并发原语](tasks/leaf/sh-p0-4-concurrency.md)

### 7.5 J FFI/ABI 链接桥（SH-P2-4，🔴 高）
评估**完全遗漏**的关键项：使 Rlyeh 编译产物能链接现有 Rust 运行时（原评估仅规划"生成 LLVM IR + 调 clang"，未规划与 Rust rlib 的桥接）。
- **J-M1** 单符号 C ABI 对齐（调用约定 / 符号命名与 Rust `extern "C"` / `#[no_mangle]` 一致）。
- **J-M2** 多符号对齐。
- **J-M3** rlib 链接编排（`assemble()` 调 `clang` 时把 Rlyeh 产物与 `actor-runtime`/`region-alloc`/`gc-runtime`/`rlyeh-std` rlib 一并链接）。
- **J-M4** 运行时入口约定（Rlyeh `fn main` 与 Rust 运行时 GC/region/actor 引导、argv 传递衔接）。
- **关键 checkpoint**：Rlyeh 写的 hello world 经 Rlyeh codegen + Rust 运行时链接，运行输出正确。
> **关联文档**：[SH-P2-4 FFI/ABI 链接桥](tasks/leaf/sh-p2-4-linkage-bridge.md)

### 7.6 K 分阶段自举 + 差分测试基础设施（SH-P2-5，🔴 高）
0.3.0 自举的工程骨架；0.2.0 内先落地 harness 与 PoC 级对拍。
- **K-M1（中）** 引导器（Rust driver）编译 Rlyeh 版**单组件**（如 `lexer`），经差分 harness 对拍。
- **K-M2（中）** 扩展为**双组件**（lexer + parser），验证组件间接口在 Rlyeh 侧一致。
- **K-M3（高→中）** 三阶段 bootstrap：`rlyeh₂` ≡ `rlyeh₃`（字节/行为一致）。
- **K-M4（中）** 差分测试 harness：IR 文本 / 行为 / 诊断三维比对。
- **K-M5（低）** 快照测试：AST/HIR/MIR/LIR 序列化快照回归比对。
- **关键 checkpoint**：`rlyeh₂` ≡ `rlyeh₃`。
> **关联文档**：[SH-P2-5 分阶段自举 + 差分测试基础设施](tasks/leaf/sh-p2-5-staged-bootstrap.md)

### 7.7 M 前端自举 PoC（SH-P2-7，🔴 高）
交付物（dogfood）；依赖 A/B/C/E 落地，复用 K 的差分 harness 逐步对拍。
- **M-M1（中）** 用 Rlyeh 重写 `lexer`（依赖 N 元组值 / O `if let` / P `match` 守卫），对拍 token 一致。
- **M-M2（中）** 用 Rlyeh 重写 `parser`（依赖 N/O/P + 递归下降），对拍 AST 一致。
- **M-M3（中）** 用 Rlyeh 重写 `ast` + `macro`（依赖 C derive / I 内部可变性），对拍 AST 节点构造一致。
- **M-M4（中）** 串联 M1–M3，经 Rust `rlyeh-driver` 编译通过，并与 Rust 版前端**对拍**（同 `.rl` 输入，token/AST 一致）。
- **关键 checkpoint**：对拍 token/AST 一致；验收即「前端逻辑主体已是 Rlyeh 源码、可被自身工具链编译」。
> **关联文档**：[SH-P2-7 前端自举 PoC（driver 自举）](tasks/leaf/sh-p2-7-driver.md)

### 7.8 N 元组值构造 + 解构（SH-P0-5，🔴 中高）
PoC 解析器 `(tok, rest)` 前置；类型层（`Type::Tuple`/单元 `()`）已就绪。
- **N-M1（中）** 元组字面量 `(a, b, c)` → 复用聚合槽布局 `f0/f1/...`。
- **N-M2（中）** 解构 `let (a, b) = e`（`_`/嵌套）。
- **N-M3（中）** 多返回值 `fn f() -> (i64, String)` + `let (x,y)=f()`。
- **N-M4（低）** `for (k,v) in map` 一致化。
- **N-L1（低）** 差分对拍 Rust 参考（K）。
- **关键 checkpoint**：`(tok, rest)` 风格重写 lexer/parser 主体。
> **关联文档**：[SH-P0-5 元组值构造 + 解构](tasks/leaf/sh-p0-5-tuple-value.md)

### 7.9 O `if let` / `while let`（SH-P0-6，🔴 高）
语言完全缺失；解析器/类型检查器重写依赖。
- **O-M1（中）** `if let Pat = expr { .. }` desugar 为「`let Pat = expr; if 绑定成功 { .. } else { .. }`」（零新增 IR）。
- **O-M2（中）** `while let Pat = expr { .. }` 循环头绑定 + 条件重评估。
- **O-M3（低）** 嵌套/链。
- **O-L1（低）** 差分对拍 Rust 参考。
- **关键 checkpoint**：typecheck `if let` 重写。
> **关联文档**：[SH-P0-6 `if let` / `while let`](tasks/leaf/sh-p0-6-if-let.md)

### 7.10 P `match` 守卫 + 范围/或模式（SH-P0-7，🔴 中高）
字符分类/判别分支依赖。
- **P-M1（中）** 守卫表达式 desugar 为「绑定临时 + `if cond`」（零新增 IR）。
- **P-M2（中）** 范围模式（复用 `in` 区间语义）。
- **P-M3（中）** 或模式（复用 union 优先级规则）。
- **P-M4（低）** 守卫 + 范围/或组合。
- **P-L1（低）** 差分对拍 Rust 参考。
- **关键 checkpoint**：字符分类 `match` 重写。
> **关联文档**：[SH-P0-7 `match` 守卫 + 范围/或模式](tasks/leaf/sh-p0-7-match-guard.md)

### 7.11 Q `Drop` trait / 析构 / RAII（SH-P0-8，🔴 高）
MutexGuard 自动解锁、arena 自动释放需 `Drop`/RAII；Rlyeh 0.1.0 完全无析构机制。
- **Q-M1（中）** `trait Drop { fn drop(&mut self); }` 声明 + `impl Drop for T`（复用 A 泛型 trait 落地后接）。
- **Q-M2（中）** 作用域尾自动插入 `x.drop()`：仅拥有所有权栈变量、声明逆序、临时/借用跳过（与 G1 借用检查协同）。
- **Q-M3（中）** 字段级递归 drop（struct 拥有字段如 `Vec` 内部堆指针）。
- **Q-M4（中）** 智能指针接入（`Box` 释放堆、`Rc`/`Arc` 计数-1、`Gc` 逃逸登记）。
- **Q-L1（低）** 差分对拍 Rust 参考。
- **关键 checkpoint**：离开作用域自动释放（MutexGuard 解锁 / arena 释放）。
> **关联文档**：[SH-P0-8 `Drop` / 析构 / RAII](tasks/leaf/sh-p0-8-drop.md)

---

## 8. 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新建 0.2.0 计划（阶段 A–M） |
| 2026-09-01 | 修正版本边界：P0 语言特性上移为 0.2.0 必须项；新增 P0-4、P2-4~P2-7 |
| 2026-09-01 | **复审补遗**：阶段表加风险列；新增阶段 N–Y（12 项遗漏语言能力）；新增 §3.14 复审补遗、§7 高危任务风险分解；同步 `tasks/` 树（P0 4→8、P1 3→6、P2 7→11、新增 P3 级） |
| 2026-09-01 | **文档管理对齐**：§2 总览表增「关联文档」列（阶段→`tasks/leaf/sh-*` 叶子）；§3 各阶段明细补「关联文档」链接；复审补遗 N–Y 由合并 §3.14 拆分为 §3.14–§3.25 独立小节，与叶子文档 `计划` 反向链接（§3.14=SH-P0-5 … §3.25=SH-P3-1）一致；补齐缺失叶子 `sh-p0-6-if-let.md`（阶段 O） |
| 2026-09-01 | **高危任务细化**：§7 由单表扩展为「速览表 + §7.1–§7.11 子任务小节」，各高危阶段列具体 M 子任务 / 关键 checkpoint / 关联叶子；补齐缺失叶子 `sh-p2-5-staged-bootstrap.md`（K）、`sh-p2-7-driver.md`（M）；修正 `sh-p0-1`/`sh-p0-3` 归属为 0.2.0-E/G（与原「0.3.0+ 长期跟踪」矛盾） |

---

> **维护者**：Rlyeh Language Team
> **最后更新**：2026-09-01
