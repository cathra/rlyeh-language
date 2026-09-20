# Rlyeh 自举可行性评估报告（0.1.0 → 0.2.0 准备阶段）

> **文档性质**：自举（self-hosting）主题权威评估报告。回答一个核心问题——**当前 Rlyeh 0.1.0 工具链（全部用 Rust 实现）能否用 Rlyeh 语言自身实现？**
> **评估范围**：全栈编译器（前端 → 后端含 LLVM 代码生成）、标准库（`rlyeh-std`）、工具（`tools/`）。
> **结论先行**：前端（词法/语法/AST/宏/工具）可行性高；类型检查/驱动属中高难度但可攻克；**真正的自举死结在运行时三件套（Actor / Region / GC）与 `rlyeh-std` 的 C FFI 绑定层**——它们依赖 Rlyeh 0.1.0 完全没有的 `unsafe`、跨函数边界闭包、`dyn Protocol`+`Self`+`Any`、`Arc<Mutex>`。
> **版本边界（2026-09-01 修正）**：0.2.0 定位为**自举能力补齐阶段**——P0 语言级特性（`unsafe`/跨边界闭包/`dyn`+`Self`+`Any`/并发原语）**均在 0.2.0 落地**，使语言具备自举表达力；运行时三件套与 `rlyeh-std` 绑定层的**重写**属 0.3.0 工作，可长期保留 Rust 经 FFI 调用。即 0.2.0 补"能力"、0.3.0 做"自举"。详见 [`development-plan-0.2.0.md`](./development-plan-0.2.0.md)。
> **事实依据**：所有结论来自对 `crates/`、`tools/`、`dagon/` 实际源码的核查（Rust→Rlyeh 可替代性映射见 §3），并与 `docs/guide/13-references-limits.md`、`CODEBUDDY.md` 的 MVP 实测限制对照。

---

## 1. 背景与定义

Rlyeh 的定位（README / CODEBUDDY.md）：*"实现语言：Rust（自举编译器，bootstrap 阶段用 Rust 实现）"*。即 **0.1.0 是 bootstrap 阶段**——工具链用宿主语言（Rust）编写，待语言成熟后再用 Rlyeh 自身重写（自举）。

本评估定义"自举"为：**用 Rlyeh 源码重写工具链的逻辑主体**，使其能编译 Rlyeh 程序（含自身）。不要求一次性全栈切换，按层逐步推进。

评估方法——**逐层可行性矩阵**：对每一组件，对照 Rlyeh 0.1.0 实测限制逐一判定：

| 判定 | 含义 |
|------|------|
| ✅ 可直接实现 | Rlyeh 0.1.0 已有等价能力，逻辑可平移 |
| 🔧 需小幅补齐 | 现有能力边界内扩展（如类型推断、错误类型），不引入新语言特性 |
| ⚠️ 需重大能力 | 必须新增语言/标准库特性（如 `unsafe`、嵌套模块、泛型 protocol impl） |
| ❌ 当前不可行 | 依赖 Rlyeh 完全缺失的能力（如 `unsafe` 块、类型擦除 `Any`） |

---

## 2. 关键事实（先读这条）

1. **后端不依赖 LLVM 绑定库**。工作区 `Cargo.toml` 虽声明了 `inkwell`/`llvm-sys`/`Cranelift`，但 `rlyeh-codegen/Cargo.toml` 完全未引用，仅依赖 `rlyeh-lir`。
2. **codegen 是"字符串拼接输出 LLVM IR 文本"**，再由 `rlyeh-driver` 用 `std::process::Command` 调 **`clang -O3`** 汇编链接（`crates/rlyeh-driver/src/lib.rs` 的 `assemble()`）。所以自举后端的难点不在"调用 LLVM API"，而在 (a) 生成语法正确的 LLVM IR 文本、(b) 调用外部 C 工具链。
3. **标准库的 `.rl` 源码部分已用 Rlyeh 编写**（`crates/rlyeh-std/rlyeh/*.rl`），但**绑定层**（`crates/rlyeh-std/src/`，NIO/poller 等 C FFI 封装）是 Rust + `unsafe`，无法用 Rlyeh 重写。
4. 工作区共 **19 个 Rust crate** + `dagon/` 包管理器 + `tools/` 四个工具二进制（同时被 `rlyeh-driver` 链接进 `rlyeh` CLI）。

---

## 3. 分层可行性矩阵

> 下表"**具体约束（细化）"列展开每个组件*到底卡在哪*；"**关联任务**"列将约束逐一链接到 [`../tasks/`](../tasks/self-hosting.md) 能力缺口叶子的 `SH-Px` 与 0.2.0 阶段（`A`–`Y`，见 [`../development-plan-0.2.0.md`](../development-plan-0.2.0.md)）。`✅/🔧/⚠️/❌` 含义见 §1。

### 3.1 编译器前端（front-end）

| 组件 | 职责 | ≈LoC | 可行性 | 具体约束（细化） | 关联任务 |
|------|------|------|--------|------------------|----------|
| `rlyeh-lexer` | 词法分析、Token/Span/错误 | ~46K | ✅ 可直接 | ① `unsafe`×2（UTF-8 解码/位置推进的小范围字节操作，可改安全写法但需 `unsafe` 兜底越界）；② `derive(Debug/Clone/PartialEq)` 需手写或等 derive 宏；③ 词法主循环 `while let Some(tok)=next()` 依赖模式控制流；④ 字符分类用 `match` 守卫/范围模式 | [SH-P0-1](../tasks/leaf/sh-p0-1-unsafe.md)（E）、[SH-P1-2](../tasks/leaf/sh-p1-2-derive.md)（C）、[SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)（O）、[SH-P0-7](../tasks/leaf/sh-p0-7-match-guard.md)（P） |
| `rlyeh-parser` | 递归下降语法分析、AST 构建、宏解析 | ~230K | ✅ 可直接 | ① `Box` 深 AST 树（Rlyeh 已有 `Box<T>` ✅）；② `unsafe`×3 小范围；③ 解析器骨架需 `(token, rest)` **元组多返回**（当前 Rlyeh 元组类型层就绪但值构造/解构缺失）；④ 重度 `if let`/`while let` 处理 `Option` 返回值；⑤ `match` 守卫/范围模式做字符分类与判别分支；⑥ 闭包表达式解析需闭包值；⑦ AST 节点 `derive` | [SH-P0-5](../tasks/leaf/sh-p0-5-tuple-value.md)（N）、[SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)（O）、[SH-P0-7](../tasks/leaf/sh-p0-7-match-guard.md)（P）、[SH-P0-1](../tasks/leaf/sh-p0-1-unsafe.md)（E）、[SH-P1-2](../tasks/leaf/sh-p1-2-derive.md)（C）、[SH-P0-2](../tasks/leaf/sh-p0-2-closure.md)（F）、[SH-P1-1](../tasks/leaf/sh-p1-1-generic-protocol.md)（A） |
| `rlyeh-ast` | AST 节点定义 | ~21K | 🔧 需小幅补齐 | ① `derive`×33（Debug/Clone/PartialEq 重度），无 derive 需手写或提供等价机制；② 枚举负载字段 `f0/f1` 命名（已实现）；③ 枚举 payload 已用元组类型 | [SH-P1-2](../tasks/leaf/sh-p1-2-derive.md)（C）、[SH-P0-5](../tasks/leaf/sh-p0-5-tuple-value.md)（N）、[SH-P0-7](../tasks/leaf/sh-p0-7-match-guard.md)（P） |
| `rlyeh-macro` | 声明式宏展开（`macro_rules!` 自研 token 级） | ~42K | ✅ 可直接 | ① 纯 token 算法 + HashMap + 枚举树，无 proc-macro；② 遍历 token 时 `if let` 处理节点 | [SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)（O）（基本无阻塞，✅ 可直接） |
| `rlyeh-hir` | 高级 IR（比较链/`in`/range 展开） | ~13K | 🔧 需小幅补齐 | ① `derive`×15；② 表达式脱糖重写需 `match` 守卫/元组聚合表示 | [SH-P1-2](../tasks/leaf/sh-p1-2-derive.md)（C）、[SH-P0-7](../tasks/leaf/sh-p0-7-match-guard.md)（P）、[SH-P0-5](../tasks/leaf/sh-p0-5-tuple-value.md)（N） |
| `rlyeh-mir` | 中级 IR（类型标注 + lowering + 优化 pass） | ~86K | ⚠️ 需重大能力 | ① 泛型函数极多，需 Rlyeh **泛型 protocol/impl** 支撑（鸡生蛋：注释 typecheck 自身即依赖）；② MIR 清理需 `Drop`/`RAII`；③ 值构造/解构用元组；④ `if let` 查询 MIR 结果 | [SH-P1-1](../tasks/leaf/sh-p1-1-generic-protocol.md)（A）、[SH-P0-8](../tasks/leaf/sh-p0-8-drop.md)（Q）、[SH-P0-5](../tasks/leaf/sh-p0-5-tuple-value.md)（N）、[SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)（O） |
| `rlyeh-lir` | 低级 IR（三地址码 + 类型推断） | ~60K | ⚠️ 需重大能力 | ① `derive`×7；② 泛型 lowering；③ 元组值/解构、值语义 `Copy`/`Clone` | [SH-P1-1](../tasks/leaf/sh-p1-1-generic-protocol.md)（A）、[SH-P1-2](../tasks/leaf/sh-p1-2-derive.md)（C）、[SH-P0-5](../tasks/leaf/sh-p0-5-tuple-value.md)（N）、[SH-P1-5](../tasks/leaf/sh-p1-5-copy-clone.md)（S） |
| `rlyeh-desugar` | 语法糖脱糖（async→状态机、guard） | ~140K | ⚠️ 需重大能力 | ① 大量 `Box` AST 重写 + `derive`×4；② 需**泛型 + 嵌套模块**组织；③ 嵌套 AST 变换需**元组多返回**；④ `if let`/`match` 守卫驱动脱糖分支；⑤ 作用域清理需 `Drop`；⑥ IR 重写需 `mem::swap`/`replace` 免借用冲突；⑦ 构造 AST 用 `..` 更新/字段简写 | [SH-P1-1](../tasks/leaf/sh-p1-1-generic-protocol.md)（A）、[SH-P1-2](../tasks/leaf/sh-p1-2-derive.md)（C）、[SH-P1-3](../tasks/leaf/sh-p1-3-nested-module.md)（B）、[SH-P0-5](../tasks/leaf/sh-p0-5-tuple-value.md)（N）、[SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)（O）、[SH-P0-7](../tasks/leaf/sh-p0-7-match-guard.md)（P）、[SH-P0-8](../tasks/leaf/sh-p0-8-drop.md)（Q）、[SH-P2-8](../tasks/leaf/sh-p2-8-mem-swap.md)（U）、[SH-P2-11](../tasks/leaf/sh-p2-11-struct-update.md)（X） |
| `rlyeh-borrowck` | 借用检查（move/borrow 分析） | ~34K | 🔧 需小幅补齐 | ① `dyn`×1（protocol 对象查询）；② `unsafe`×1（可改）；③ MIR borrow 查询用 `if let`；④ IR 重写需 `mem::swap` | [SH-P0-1](../tasks/leaf/sh-p0-1-unsafe.md)（E）、[SH-P0-3](../tasks/leaf/sh-p0-3-dyn-any.md)（G）、[SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)（O）、[SH-P2-8](../tasks/leaf/sh-p2-8-mem-swap.md)（U） |
| `rlyeh-regionck` | 区域所有权/转移检查 | ~17K | 🔧 需小幅补齐 | ① `unsafe`×1；② `#![warn(unsafe_code)]` 属性 | [SH-P0-1](../tasks/leaf/sh-p0-1-unsafe.md)（E） |

**前端小结**：`lexer`/`parser`/`ast`/`macro` 逻辑可平移，但解析器骨架强依赖 [SH-P0-5 元组值/解构](../tasks/leaf/sh-p0-5-tuple-value.md)（N）、[SH-P0-6 `if let`](../tasks/leaf/sh-p0-6-if-let.md)（O）、[SH-P0-7 `match` 守卫](../tasks/leaf/sh-p0-7-match-guard.md)（P）——这三项即 **0.2.0 前端 PoC 的直接前提**（复审补遗，原评估漏判）；`hir`/`borrowck`/`regionck` 仅需手写 `derive`（[SH-P1-2](../tasks/leaf/sh-p1-2-derive.md) C）或小幅补齐；`mir`/`lir`/`desugar` 需要 **泛型 protocol/impl（[SH-P1-1](../tasks/leaf/sh-p1-1-generic-protocol.md) A）+ 嵌套模块（[SH-P1-3](../tasks/leaf/sh-p1-3-nested-module.md) B）+ derive 宏（[SH-P1-2](../tasks/leaf/sh-p1-2-derive.md) C）+ `Drop`（[SH-P0-8](../tasks/leaf/sh-p0-8-drop.md) Q）+ `mem::swap`（[SH-P2-8](../tasks/leaf/sh-p2-8-mem-swap.md) U）**。整体前端自举在补齐上述 P0/P1 能力后可行，且可用 Rlyeh 重写 `lexer`+`parser` 作为 0.2.0 的 PoC 验证。

### 3.2 编译器后端（back-end / codegen）

| 组件 | 职责 | ≈LoC | 可行性 | 具体约束（细化） | 关联任务 |
|------|------|------|--------|------------------|----------|
| `rlyeh-codegen` | LIR → LLVM IR **文本**（字符串拼接） | ~41K | ⚠️ 需重大能力 | ① 纯字符串逻辑（Rlyeh 字符串 API 已较全：split/format/拼接/索引）；② 难点是**生成 IR 文本的正确性**（ABI/变参/`mem2reg`/phi），非绑定；③ 实际代码大量 `if let`、`match`、元组聚合表示；④ IR 重写需 `mem::swap`/`replace` 免借用冲突；⑤ 值语义依赖 `Copy`/`Clone` | [SH-P0-5](../tasks/leaf/sh-p0-5-tuple-value.md)（N）、[SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)（O）、[SH-P0-7](../tasks/leaf/sh-p0-7-match-guard.md)（P）、[SH-P2-8](../tasks/leaf/sh-p2-8-mem-swap.md)（U）、[SH-P1-5](../tasks/leaf/sh-p1-5-copy-clone.md)（S） |
| `rlyeh-driver` 的 `assemble()` + 线程模型 | 调 `clang`/`rust-lld`/`wasm-ld` 汇编链接；64MB 栈线程 `spawn` | — | ⚠️ 需重大能力 | ① 必须能 `system()`/`exec` 外部工具链 → 依赖 **进程调用 / 外部工具链 FFI**（低阶 `extern "C"` 已支持，进程调用需确认）；② 64MB 栈线程 `spawn(move)` → 依赖**并发原语 + 跨边界闭包 + `move` + `'static`**；③ C ABI 符号约定需 `unsafe`/`#[repr(C)]`；④ 运行时全局符号表（如 `rlyeh_actor_resolve`）需 `const`/`static` 全局项；⑤ 跨线程传 `Arc<Mutex>` 需 `Send`/`Sync` 约束 | [SH-P2-2](../tasks/leaf/sh-p2-2-process-ffi.md)（D）、[SH-P0-4](../tasks/leaf/sh-p0-4-concurrency.md)（H）、[SH-P0-2](../tasks/leaf/sh-p0-2-closure.md)（F）、[SH-P0-1](../tasks/leaf/sh-p0-1-unsafe.md)（E）、[SH-P2-9](../tasks/leaf/sh-p2-9-const-static.md)（V）、[SH-P3-1](../tasks/leaf/sh-p3-1-send-sync.md)（Y） |

**后端小结**：后端"发射 LLVM IR 文本"本身是纯字符串逻辑，Rlyeh 0.1.0 的字符串能力足以承载；真正的阻塞点是 (a) IR 文本正确性工程量大（[SH-P0-5/6/7](../tasks/leaf/sh-p0-5-tuple-value.md) 支撑值/模式表达）、(b) 必须调用外部 `clang`/链接器——依赖 [SH-P2-2 进程调用 FFI](../tasks/leaf/sh-p2-2-process-ffi.md)（D）与 [SH-P0-1 `unsafe`](../tasks/leaf/sh-p0-1-unsafe.md)（E）的 C ABI 约定、[SH-P2-9 `const`/`static`](../tasks/leaf/sh-p2-9-const-static.md)（V）的全局符号表。**LLVM 绑定不是障碍**（当前未使用）。推荐路径：保留"生成 IR 文本 + 调 clang"策略（路径 A），Rlyeh 侧只实现文本发射器。

### 3.3 标准库（std）

| 部分 | 实现语言 | 可行性 | 具体约束（细化） | 关联任务 |
|------|----------|--------|------------------|----------|
| `crates/rlyeh-std/rlyeh/*.rl`（核心库源码：Vec/HashMap/String/Channel/并发原语等） | **已是 Rlyeh** | ✅ 已自托管 | 用户态逻辑本身可平移；但 `Vec`/`HashMap` 析构需 `Drop`（[SH-P0-8](../tasks/leaf/sh-p0-8-drop.md) Q）、`MutexGuard` 透传需 `Deref`（[SH-P1-4](../tasks/leaf/sh-p1-4-deref.md) R）、拷贝语义需 `Copy`/`Clone`（[SH-P1-5](../tasks/leaf/sh-p1-5-copy-clone.md) S）、分层错误 `?` 需 `From`（[SH-P1-6](../tasks/leaf/sh-p1-6-question-from.md) T）、内部断言需 `panic!`/`assert!`（[SH-P2-10](../tasks/leaf/sh-p2-10-assert-macros.md) W） | [SH-P0-8](../tasks/leaf/sh-p0-8-drop.md)（Q）、[SH-P1-4](../tasks/leaf/sh-p1-4-deref.md)（R）、[SH-P1-5](../tasks/leaf/sh-p1-5-copy-clone.md)（S）、[SH-P1-6](../tasks/leaf/sh-p1-6-question-from.md)（T）、[SH-P2-10](../tasks/leaf/sh-p2-10-assert-macros.md)（W） |
| `crates/rlyeh-std/src/`（NIO/poller/sendfile **C FFI 绑定层**） | Rust + `unsafe`×16 | ❌ 当前不可行 | ① `unsafe`×16 系统调用封装；② `libc` 直接调用；③ FFI 全局状态需 `const`/`static`；④ `MutexGuard` 作用域尾自动解锁需 `Drop` + `Deref`；⑤ 跨线程共享需 `Send`/`Sync` | [SH-P0-1](../tasks/leaf/sh-p0-1-unsafe.md)（E）、[SH-P2-2](../tasks/leaf/sh-p2-2-process-ffi.md)（D）、[SH-P2-9](../tasks/leaf/sh-p2-9-const-static.md)（V）、[SH-P0-8](../tasks/leaf/sh-p0-8-drop.md)（Q）、[SH-P1-4](../tasks/leaf/sh-p1-4-deref.md)（R）、[SH-P3-1](../tasks/leaf/sh-p3-1-send-sync.md)（Y） |

**std 小结**：标准库的"用户态逻辑"已用 Rlyeh 编写，自举命题对 std 而言主要是**绑定层**——这部分必须保留 Rust（或待 Rlyeh 获得 `unsafe`）。也即：std 自举**部分已完成**，剩余是运行时绑定，与 §3.4 的 runtime 死结同源；核心 `.rl` 库在补齐 [SH-P0-8](../tasks/leaf/sh-p0-8-drop.md)/[SH-P1-4](../tasks/leaf/sh-p1-4-deref.md)/[SH-P1-5](../tasks/leaf/sh-p1-5-copy-clone.md)/[SH-P1-6](../tasks/leaf/sh-p1-6-question-from.md)/[SH-P2-10](../tasks/leaf/sh-p2-10-assert-macros.md) 后可进一步自洽。

### 3.4 运行时与工具（runtime / tools / dagon）

| 组件 | 职责 | ≈LoC | 可行性 | 具体约束（细化） | 关联任务 |
|------|------|------|--------|------------------|----------|
| `rlyeh-actor-runtime` | Actor 调度/邮箱/supervisor/FFI | ~140K | ❌ 当前不可行 | ① `unsafe`×36（`extern "C"`/`dlsym` 回调注册）；② `dyn`×27（消息分发经 vtable，含 `Self` 方法）；③ `Arc<Mutex<...>>`/`Weak` 共享状态；④ `Box<dyn ActorState>` 类型擦除对象；⑤ `std::any::Any` 类型擦除/downcast；⑥ `spawn(move || ...)` 跨边界闭包 + `'static` 约束；⑦ supervisor 重启重建状态需 `Drop`；⑧ 跨线程传 `Arc<Mutex>` 需 `Send`/`Sync` | [SH-P0-1](../tasks/leaf/sh-p0-1-unsafe.md)（E）、[SH-P0-2](../tasks/leaf/sh-p0-2-closure.md)（F）、[SH-P0-3](../tasks/leaf/sh-p0-3-dyn-any.md)（G）、[SH-P0-4](../tasks/leaf/sh-p0-4-concurrency.md)（H）、[SH-P3-1](../tasks/leaf/sh-p3-1-send-sync.md)（Y）、[SH-P1-4](../tasks/leaf/sh-p1-4-deref.md)（R）、[SH-P0-8](../tasks/leaf/sh-p0-8-drop.md)（Q） |
| `rlyeh-region-alloc` | L1 区域分配器（bump + 智能） | ~100K | ❌ 当前不可行 | ① `unsafe`×36（裸指针算移/链表 intrusive）；② 手动链表内存管理；③ `#[repr(C)]` 布局约定；④ `libc` 分配；⑤ arena 越界自动释放需 `Drop` | [SH-P0-1](../tasks/leaf/sh-p0-1-unsafe.md)（E）、[SH-P2-9](../tasks/leaf/sh-p2-9-const-static.md)（V）、[SH-P0-8](../tasks/leaf/sh-p0-8-drop.md)（Q）、[SH-P2-2](../tasks/leaf/sh-p2-2-process-ffi.md)（D） |
| `rlyeh-gc-runtime` | K4 追踪 GC（标记-清除，保守扫描） | ~10K | ❌ 当前不可行 | ① `unsafe`×16；② `*mut i64` 裸指针扫描栈；③ `UnsafeCell`；④ `#[repr(C)]`；⑤ `libc::malloc` | [SH-P0-1](../tasks/leaf/sh-p0-1-unsafe.md)（E）、[SH-P2-9](../tasks/leaf/sh-p2-9-const-static.md)（V） |
| `rlyeh-std/src/nio/*` | 高性能 IO 绑定层（poller/sendfile） | ~26K | ❌ 当前不可行 | ① `unsafe`×16；② `libc` 系统调用封装；③ 事件循环跨线程需 `Arc`/并发原语 | [SH-P0-1](../tasks/leaf/sh-p0-1-unsafe.md)（E）、[SH-P2-2](../tasks/leaf/sh-p2-2-process-ffi.md)（D）、[SH-P2-9](../tasks/leaf/sh-p2-9-const-static.md)（V）、[SH-P0-4](../tasks/leaf/sh-p0-4-concurrency.md)（H） |
| `tools/rlyeh-fmt` | AST 重建格式化 | ~44K | ✅ 可直接 | 纯 front-end 逻辑，无 `unsafe`；但 AST 遍历需 `derive`（[SH-P1-2](../tasks/leaf/sh-p1-2-derive.md) C）、`if let`/`match` 遍历（[SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)/[SH-P0-7](../tasks/leaf/sh-p0-7-match-guard.md)）、元组（[SH-P0-5](../tasks/leaf/sh-p0-5-tuple-value.md)） | [SH-P1-2](../tasks/leaf/sh-p1-2-derive.md)（C）、[SH-P0-5](../tasks/leaf/sh-p0-5-tuple-value.md)（N）、[SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)（O）、[SH-P0-7](../tasks/leaf/sh-p0-7-match-guard.md)（P） |
| `tools/rlyeh-check` | 静态分析 lint | ~24K | ✅ 可直接 | 纯 front-end 逻辑（同上 AST 遍历约束） | [SH-P1-2](../tasks/leaf/sh-p1-2-derive.md)（C）、[SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)（O）、[SH-P0-7](../tasks/leaf/sh-p0-7-match-guard.md)（P） |
| `tools/rlyeh-doc` | `///` → Markdown | ~27K | ✅ 可直接 | 纯 front-end 逻辑（同上） | [SH-P1-2](../tasks/leaf/sh-p1-2-derive.md)（C）、[SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)（O） |
| `tools/rlyeh-bench` | 编译+计时基准框架 | ~11K | ✅ 可直接 | 纯 front-end 逻辑 | [SH-P1-2](../tasks/leaf/sh-p1-2-derive.md)（C）、[SH-P0-6](../tasks/leaf/sh-p0-6-if-let.md)（O） |
| `dagon/` | 包管理器（manifest/registry/resolve/sandbox） | ~95K | ❌ 当前不可行 | ① 重度外部 crate：`pubgrub`（版本求解）、`tar`/`flate2`（压缩）、`libc`（沙箱系统调用）、`clap`/`serde`/`toml`；② 算法可重写但系统调用/压缩需 FFI；③ 全局配置/注册表状态需 `const`/`static` | [SH-P2-1](../tasks/leaf/sh-p2-1-external-crates.md)（P2-1 长期保留 Rust）、[SH-P2-2](../tasks/leaf/sh-p2-2-process-ffi.md)（D）、[SH-P2-9](../tasks/leaf/sh-p2-9-const-static.md)（V） |

**运行时/工具小结**：四个前端工具（fmt/check/doc/bench）可完全用 Rlyeh 重写（仅受 [SH-P1-2 derive](../tasks/leaf/sh-p1-2-derive.md) C 与 [SH-P0-5/6/7](../tasks/leaf/sh-p0-5-tuple-value.md) 约束，补齐后无障碍）；但 **actor/region/gc 三大运行时 + std 绑定层 + dagon** 依赖 `unsafe`（[SH-P0-1](../tasks/leaf/sh-p0-1-unsafe.md) E）/`dyn`+`Self`+`Any`（[SH-P0-3](../tasks/leaf/sh-p0-3-dyn-any.md) G）/跨边界闭包（[SH-P0-2](../tasks/leaf/sh-p0-2-closure.md) F）/并发原语（[SH-P0-4](../tasks/leaf/sh-p0-4-concurrency.md) H）/`Drop`（[SH-P0-8](../tasks/leaf/sh-p0-8-drop.md) Q）/`Deref`（[SH-P1-4](../tasks/leaf/sh-p1-4-deref.md) R）/`const`/`static`（[SH-P2-9](../tasks/leaf/sh-p2-9-const-static.md) V）/`Send`/`Sync`（[SH-P3-1](../tasks/leaf/sh-p3-1-send-sync.md) Y）——**在 Rlyeh 获得 `unsafe` 与完整并发原语前完全不可行**，应长期保留 Rust 实现；其*重写*属 0.3.0 工作。

---

## 4. 能力缺口分级（Rlyeh 0.1.0 缺失 → 自举阻塞度）

### P0 — 阻塞全栈自举（Rlyeh 0.1.0 完全没有）
1. **`unsafe` 块 / 裸指针** — `gc-runtime`（裸指针+`UnsafeCell`）、`region-alloc`（手动链表）、`actor-runtime`（`unsafe extern "C"`/`dlsym`）、`rlyeh-std/nio`（libc 封装）全部依赖。Rlyeh 0.1.0 无 `unsafe` 块 → 运行时 100% 无法自托管。
2. **闭包跨函数边界 + `move` + `'static`** — `actor-runtime`（`spawn(move || ...)`、`Box<dyn Fn() + Send + Sync>`）、`driver`（64MB 栈线程 `spawn(move)`）。Rlyeh 0.1.0「闭包不跨函数边界 / `move` 被忽略 / 无生命周期」→ 调度器与线程模型核心逻辑无法表达。
3. **`dyn Protocol` 调用含 `Self` 的方法 + `std::any::Any` 类型擦除/downcast** — actor 消息协议 `ActorState: Any + Send + Sync`、`Box<dyn Any + Send>`、`handle_message(&mut self, ...)` 正是 `Self` 方法；Rlyeh 0.1.0「`dyn Protocol` 不可调用含 `Self` 签名方法」且**无类型擦除** → 无法表达。

### P1 — 阻塞前端自举（需新增语言/标准库特性）
4. **泛型 protocol + impl** — `typecheck` 自身（700K+ LoC 最大 crate）表达泛型 protocol/impl（`types.rs:440`）。Rlyeh 0.1.0「protocol+impl 必须非泛型」→ 注释编译器自身即鸡生蛋问题，自举前 Rlyeh 须先支持泛型 protocol。
5. **`derive` 宏 / 自动 protocol 派生** — 全代码库重度（`ast`×33、`hir`×15、`typecheck`×10）。Rlyeh 0.1.0 无 derive 宏 → AST/HIR/MIR/LIR 的 `Debug/Clone/PartialEq` 需手写或提供等价机制。
6. **嵌套模块 / `pub use` / `super` / `crate::`** — 大型 crate 普遍多级模块（`driver`/`typecheck` 32 文件）。Rlyeh 0.1.0「无嵌套模块 / `pub use` / `super`」→ crate 内部组织需在 Rlyeh 侧扁平化或提供命名空间方案。

### P2 — 可行但需重写 / 外部依赖
7. **外部 crate 无 Rust 等价** — `pubgrub`（dagon 版本求解）、`crossbeam`/`dashmap`（actor 无锁并发）、`clap`/`serde`/`toml`/`tar`/`flate2`（dagon+driver）、`libc`（所有 runtime FFI）。需 Rlyeh FFI 调 C 库或重写算法。
8. **进程调用 / 外部工具链** — `driver` 的 `assemble()` 调 `clang`/`rust-lld`/`wasm-ld`。自托管后端须能 `system()`/`exec` 外部链接器（或 Rlyeh 自带链接器）。
9. **`Box` 深树 + `Rc`/`Arc`/`RefCell` 内部可变性** — AST/HIR/MIR/LIR 树形 IR 可变遍历。Rlyeh 有 `Box`/`Rc`/`Arc`/`Weak`，但无 `RefCell`；可选用 arena/索引式表示（Rlyeh 文档已知倾向索引式，数组/Vec 适配器保留内建 desugar）。

---

## 5. 风险

| 风险 | 等级 | 说明 |
|------|------|------|
| 运行时三件套无法自举 | 🔴 高 | `unsafe`/`dyn`/`Arc`/`Any`/跨边界闭包是系统性缺口，非单点修复；即便 0.2.0 引入 `unsafe`，actor 运行时的 `Any` 类型擦除 + 跨线程 `dyn` 调度仍是语言级难题 |
| **前端 PoC 前置语言特性缺失** | 🔴 高 | **复审补遗**：0.2.0 原计划漏判 `if let`/`while let`、元组值构造/解构、`match` 守卫/范围模式、`Drop`/RAII——这些是「用 Rlyeh 重写 `lexer`/`parser`/`typecheck`」的**直接前提**（解析器 `(tok,rest)`/`if let`、字符分类守卫、`MutexGuard` 自动释放），已补入 0.2.0 阶段 N–Q 并拆分中/低危子任务 |
| LLVM IR 文本正确性 | 🟠 中 | 文本发射器工程量大、易错（ABI/phi/mem2reg），需完整测试对拍 |
| 泛型 protocol/impl 自举鸡生蛋 | 🟠 中 | 注释 typecheck 需泛型 protocol，但 Rlyeh 须先有泛型 protocol——需 staged bootstrap（先用 Rust 版编译器验证 Rlyeh 侧泛型 protocol 实现） |
| 差分/对拍工程量大 | 🟠 中 | 需长期保留 Rust 参考编译器作对拍基准（K 阶段 harness），覆盖 IR/行为/诊断三维度 |
| 外部依赖（dagon/pubgrub/压缩） | 🟡 低-中 | 算法可移植，但系统调用与压缩需 FFI；可长期保留 Rust 实现 |
| 模块扁平化导致代码组织困难 | 🟡 低 | 大型编译器在扁平命名空间下可读性下降，可用前缀约定缓解（B 嵌套模块落地后缓解） |

---

## 6. 推荐自举路径（两阶段：0.2.0 能力 / 0.3.0 自举）

**总策略：先补能力、再自举；运行时可经 FFI 长期保留 Rust。**

- **路径 A（推荐后端）— 保留"生成 LLVM IR 文本 + 调 clang"**：Rlyeh 侧只实现文本发射器（纯字符串逻辑），由宿主提供 `clang` 链接。避免 LLVM 绑定自举，工程量最小、风险最低。
- **路径 B — 发射 C 源码交 `clang` 编译**：若 IR 文本正确性风险过高，可改发 C；但需 Rlyeh 侧重写代码生成目标，工作量大于 A。
- **路径 C — 接入 Cranelift**：需 Rlyeh FFI 调 Cranelift C API，且 Cranelift 当前未实际使用，优先级低。

**阶段划分（2026-09-01 修正）**：
1. **0.2.0 — 自举能力补齐（本计划）**：落地全部自举所需语言/标准库能力——P0 语言级特性（`unsafe`/跨边界闭包/`dyn`+`Self`+`Any`/并发原语）、P1（泛型 protocol/impl、嵌套模块、derive）、P2-2（进程 FFI）、P2-3（arena/内部可变性）；并交付**前端自举 PoC**（Rlyeh 写 lexer/parser 经 Rust driver 编译）+ **FFI/ABI 链接桥** + **差分测试基础设施**验证能力地基。
2. **0.3.0 — 工具链自举（用 Rlyeh 重写工具链）**：在 0.2.0 能力之上，逐步用 Rlyeh 重写 lexer→parser→…→codegen→driver→tools→std（见 `development-plan-0.3.0.md` 规划）。运行时三件套（actor/region/gc）与 `rlyeh-std` 绑定层的*重写*在 0.3.0 进行（依赖 0.2.0 的 `unsafe` 等能力），亦可持续保留 Rust 经 FFI 调用。

---

## 7. 对 0.2.0 的判断（修正后）

- **0.2.0 = 自举能力补齐**：必须交付 P0/P1/P2 中"语言/标准库能力"全部缺口（含原 §4 P0 三项 + 并发原语），使语言具备自举所需表达力；并交付前端自举 PoC + 链接桥 + 差分测试，证明能力可落地。
- **0.2.0 不负责**：工具链*重写本身*（那是 0.3.0）；`dagon` 包管理器可长期保留 Rust。
- **0.2.0 主线定位**：**自举能力补齐阶段**——为 0.3.0 自举 Rlyeh 语言铺路，而非仅前端 PoC。
- **2026-09-01 复审补遗**：0.2.0 缺口已扩展——除原 P0/P1/P2 外，新增**元组值构造/解构**（`if let`/`while let`、`match` 守卫/范围/或模式、`Drop`/RAII 四项为**前端 PoC 直接前提**）、`Deref`/`DerefMut`、`Copy`/`Clone`、`?` 经 `From` 转换、`mem::swap`/`replace`、`const`/`static`、断言宏、结构体 `..` 更新、`Send`/`Sync`（共 12 项，任务树 `tasks/leaf/sh-*`）。各缺口均含**风险分解（高危→中/低危）**，详见 [`../development-plan-0.2.0.md`](../development-plan-0.2.0.md) §7。

> 详细能力排期见 [`../development-plan-0.2.0.md`](../development-plan-0.2.0.md)。

---

## 8. 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新建自举可行性评估报告（§1–§7） |
| 2026-09-01 | 修正版本边界为「0.2.0 能力 / 0.3.0 自举」两阶段；风险表补「前端 PoC 前置语言特性缺失」；§7 补复审补遗 |
| 2026-09-01 | **细化 §3 分层可行性矩阵**：各组件「主要约束」展开为「具体约束（细化）」，新增「关联任务」列，逐一链接 `SH-Px` 叶子与 0.2.0 阶段（A–Y） |

---

> **维护者**：Rlyeh Language Team
> **最后更新**：2026-09-01
