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
1. 落地**全部自举所需语言/标准库能力**：P0（`unsafe`/跨边界闭包/`dyn`+`Self`+`Any`/并发原语）、P1（泛型 protocol/impl、嵌套模块、derive）、P2-2（进程 FFI）、P2-3（arena/内部可变性）。
2. 交付 **FFI/ABI 链接桥**：Rlyeh 编译产物可链接/调用现有 Rust 运行时。
3. 交付 **分阶段自举 + 差分测试基础设施**：Rust 引导器编译 Rlyeh 版组件，并与 Rust 参考实现对拍。
4. 交付**前端自举 PoC**：用 Rlyeh 重写 `lexer`+`parser`+`ast`+`macro`，经 Rust driver 编译通过 + 对拍。

**0.2.0 不负责**：工具链*重写本身*（属 0.3.0）；`dagon` 包管理器可长期保留 Rust 经 FFI。

---

## 2. 计划总览（阶段 A–M）

| 阶段 | 主题 | 对应缺口 | 关联文档 | 风险 | 状态 | 说明 |
|------|------|----------|----------|------|------|------|
| **0.2.0-A** | 泛型 protocol/impl 完整化 | P1-1 | [SH-P1-1](tasks/leaf/sh-p1-1-generic-protocol.md) | 🟠 中 | 🟢 完成 | typecheck 自举前置（A1/A3 复核为既有能力；A4 两缺口已补；A2 两处限制 2026-09-02 第二轮修复） |
| **0.2.0-B** | 嵌套模块系统 | P1-3 | [SH-P1-3](tasks/leaf/sh-p1-3-nested-module.md) | 🟠 中 | 🟢 完成 | 嵌套模块（既有）+ `pub use`/组导入/glob/嵌套组导入已实现；B3 `crate::`/`super::` 按扁平决策排除；B4 可见性暂缓 |
| **0.2.0-C** | protocol derive 宏 | P1-2 | [SH-P1-2](tasks/leaf/sh-p1-2-derive.md) | 🟠 中 | 🟢 核心落地 | struct 的 `#[derive(Clone/PartialEq/Debug)]` 已落地（C1/C2/C3/C4 框架），全量回归 252/252 |
| **0.2.0-D** | 进程调用 / 外部工具链 FFI | P2-2 | [SH-P2-2](tasks/leaf/sh-p2-2-process-ffi.md) | 🟠 中 | 🟢 核心落地 | process 模块（system/exec/output/exec_combined）+ clang FFI 印证，全量回归 253/253 |
| **0.2.0-E** | `unsafe` 块 / 裸指针 / `#[repr(C)]` | P0-1 | [SH-P0-1](tasks/leaf/sh-p0-1-unsafe.md) | 🔴 高 | 🟢 完成 | unsafe 块/裸指针/E-M1 bump 分配器 + E2 repr(C)（sub8+嵌套聚合内联）+ E3 FFI 门禁全部落地，全量 253/253 |
| **0.2.0-F** | 跨函数边界闭包 + `move` + `'static` | P0-2 | [SH-P0-2](tasks/leaf/sh-p0-2-closure.md) | 🔴 高 | 🟢 完成 | actor 调度器 / driver 线程模型地基 |
| **0.2.0-G** | `dyn Protocol` 含 `Self` + `Any` 类型擦除 | P0-3 | [SH-P0-3](tasks/leaf/sh-p0-3-dyn-any.md) | 🔴 高 | 🟢 完成 | actor 消息协议地基 |
| **0.2.0-H** | 并发原语（Arc<Mutex>/atomic/线程 spawn） | P0-4 | [SH-P0-4](tasks/leaf/sh-p0-4-concurrency.md) | 🔴 高 | 🟢 完成 | 运行时并发地基 |
| **0.2.0-I** | 内部可变性 / arena 表示 | P2-3 | [SH-P2-3](tasks/leaf/sh-p2-3-internal-mut.md) | 🟠 中 | 🟢 核心落地 | I2 arena+NodeId + I1 RefCell 验证，全量 255/255 |
| **0.2.0-J** | FFI/ABI 链接桥 | 新增 | [SH-P2-4](tasks/leaf/sh-p2-4-linkage-bridge.md) | 🔴 高 | ✅ 完成（C-ABI staticlib 链接桥） | Rlyeh 产物经 `extern "C"` 符号 + `crate-type=["rlib","staticlib"]` 链接 `librlyeh_*_runtime.a`（按需、缺失跳过），actor/gc/region 测试全绿 |
| **0.2.0-K** | 分阶段自举 + 差分测试基础设施 | 新增 | [SH-P2-5](tasks/leaf/sh-p2-5-staged-bootstrap.md) | 🔴 高 | 🟢 PoC(CD) | harness + 单编译器快照基线（C0/C1/C2）落地；三阶段自举 K-M1..K-M3 推迟 0.3.0 |
| **0.2.0-L** | 诊断信息质量对齐 | 新增 | [SH-P2-6](tasks/leaf/sh-p2-6-diagnostics.md) | 🟠 中 | 🟢 完成 | L0 harness 诊断维度 + 探针基线（12 例 check 12/0/0/0）；L1 typecheck/borrowck/regionck 用户态 span 对齐 + 语句级坐标（HIR Span 传播）；L2 结构化诊断（稳定错误码 TC/BC/RC0xx + `= help:` + 相关 span 标注，TypeError 多位置回指） |
| **0.2.0-M** | 前端自举 PoC | 新增(扩) | [SH-P2-7](tasks/leaf/sh-p2-7-driver.md) | 🔴 高 | 🟡 进行中 | M-M1a/b/c 切片1 落地：Rlyeh 版 lexer `self-host/lexer.rl`（标识符/关键字/整数/浮点/字符串含转义/运算符/注释/char/生命周期/`not in`/时间/原始字符串/原始标识符）+ 差分对拍 harness（`--emit tokens` oracle）corpus1/2/3/4/5 token 逐行一致；M-M1c 浮点 ✅（原始拼写对齐，2026-09-21）、非法字符报错 ✅（2026-09-21，负向对拍）；M-M2a 表达式 parser ✅（2026-09-21，S-表达式 AST 对拍）；M-M2b 程序/语句 parser ✅（2026-09-21，b1 语句骨架 + b2 模式/类型标注 + b3 元组/数组类型标注，程序级 S-表达式 AST 对拍，含嵌套泛型 `Vec<Result<i64,String>>`/`&mut`/扁平元组模式/元组类型 `(i64,i64)`/数组 `[T;N]`）；M-M2c/M-M3/M-M4 待推进 |
| **0.2.0-N** | 元组值构造 + 解构（多返回值） | P0-5 | [SH-P0-5](tasks/leaf/sh-p0-5-tuple-value.md) | 🔴 中高 | 🟢 完成 | **复审补遗**：PoC 解析器 `(tok,rest)` 前置；类型层已就绪 |
| **0.2.0-O** | `if let` / `while let` 模式控制流 | P0-6 | [SH-P0-6](tasks/leaf/sh-p0-6-if-let.md) | 🔴 高 | 🟢 完成 | **复审补遗**：语言完全缺失，解析器/类型检查器重写依赖 |
| **0.2.0-P** | `match` 守卫 + 范围/或模式 | P0-7 | [SH-P0-7](tasks/leaf/sh-p0-7-match-guard.md) | 🔴 中高 | 🟢 完成 | **复审补遗**：字符分类/判别分支依赖 |
| **0.2.0-Q** | `Drop` protocol / 析构 / RAII | P0-8 | [SH-P0-8](tasks/leaf/sh-p0-8-drop.md) | 🔴 高 | 🟢 完成（Q1–Q3） | **复审补遗**：MutexGuard/arena/智能指针自动释放；Q4 智能指针接入待办 |
| **0.2.0-R** | `Deref`/`DerefMut` 用户类型自动解引用 | P1-4 | [SH-P1-4](tasks/leaf/sh-p1-4-deref.md) | 🟠 中 | ✅ 完成（M1 protocol 声明 + M2 自动解引用强制 + L1 回归） | M1 `Deref`/`DerefMut` protocol 声明（2026-09-04）+ M2 字段/方法/索引解析失败回退插入 `x.deref()`（深度 16，零新增 IR，2026-09-04）+ run-pass `p1_4_autoderef.rl`；**已知限制**：`deref(&self) -> &Target` 返回引用路径受函数返回引用 codegen 缺陷影响，暂仅对返回值的 `deref`（`MutexGuard`/`RwLockGuard` 值分发）生效 |
| **0.2.0-S** | `Copy`/`Clone` 语义 + `#[derive(Copy)]` | P1-5 | [SH-P1-5](tasks/leaf/sh-p1-5-copy-clone.md) | 🟠 中 | 🟢 完成 | `protocol Copy {}` + `#[derive(Copy)]` 展开 `impl Copy for T` + `T: Copy` 约束（2026-09-04） |
| **0.2.0-T** | `?` 经 `From`/`Into` 错误自动转换 | P1-6 | [SH-P1-6](tasks/leaf/sh-p1-6-question-from.md) | 🟠 中 | 🟢 完成 | `?`+`From` 转换 P6c（2026-08-29）已落地，`check_question` 在 `E1≠E2` 时插入 `From::<E1>::from` |
| **0.2.0-U** | `mem::swap` / `mem::replace` 内建 | P2-8 | [SH-P2-8](tasks/leaf/sh-p2-8-mem-swap.md) | 🟠 中 | 🟢 完成 | M1 `mem::swap` 三次 memcpy 交换；M2 `mem::replace` desugar 复用 mem::swap 统一处理标量/聚合；M3 `mem::take` desugar 复用 mem::swap + 打通 `Default` 协议 `Self` 上下文推断（run-pass + compile-fail 已固化）；全量回归 335/335 |
| **0.2.0-V** | `const` / `static` 全局项 | P2-9 | [SH-P2-9](tasks/leaf/sh-p2-9-const-static.md) | 🟠 中 | 🟢 完成 | M1 const 折叠 / M2 static·static mut data 段符号 + unsafe 门禁 / M3 `&GLOBAL`→`&'static T` 取址；全量回归 330/330 |
| **0.2.0-W** | `panic!`/`assert!`/`unreachable!`/`todo!` 宏 | P2-10 | [SH-P2-10](tasks/leaf/sh-p2-10-assert-macros.md) | 🟡 低 | ✅ 完成（L1 运行时 panic：desugar 为内置 `panic` 调用，codegen 经 `dprintf` 输出到 stderr 后 `abort()`；L3 never 类型待办） | **复审补遗**：编译器内部断言 |
| **0.2.0-X** | 结构体 `..` 更新 + 字段简写 | P2-11 | [SH-P2-11](tasks/leaf/sh-p2-11-struct-update.md) | 🟡 低 | 🟢 完成 | L1 字段简写（须显式首字段在前）/ L2 `..base` 拷贝；run-pass 验证通过 |
| **0.2.0-Y** | `Send`/`Sync` 自动 protocol（放宽/标记） | P3-1 | [SH-P3-1](tasks/leaf/sh-p3-1-send-sync.md) | 🟠 中 | ✅ 完成（告警式并发安全基线 + L1 回归） | **复审补遗**：并发安全基线（告警式）；`Thread::start` 边界 W002 告警 + `is_send_sync` 内建谓词 + run-pass `send_sync_thread.rl` 回归 |

**推荐路线（依赖驱动）**：A/B/C（语言组织）→ E/F/G/H（运行时表达力地基）→ D/I（FFI/arena）→ J（链接桥）→ K（bootstrap+差分）→ L（诊断）→ M（PoC 串联）。
**优先级**：A1/A2 > B1/B2 > E（unsafe 地基）> C1–C3 > F/G > H > D > I > J > K > L > M。

---

## 3. 阶段详情

> A–M 见 §3.1–§3.13；N–Y 复审补遗见 §3.14–§3.25。每个阶段「关联文档」指向 `docs/tasks/leaf/` 下对应叶子（缺口细化 / 风险分解 / 受影响组件 / 验证 / 状态）。

### 3.1 A 泛型 protocol/impl 完整化（P1-1）
关联类型（U2）已在 0.1.0 落地，叠加泛型参数化。**动手前复核（2026-09-02）**修正了「protocol+impl 必须非泛型」的过时判断：
- ✅ A1 泛型 protocol 声明——**已具备**（U8）：std 已用 `protocol From<T>` / `protocol Into<T>`，测试有 `protocol Wrap<T>` / `protocol Deref<T>`。
- ✅ A2 泛型 impl——**已具备**（std 已用 `impl<T> Iterator for Iter<T>` / `impl<T> Future for RecvAsync<T>`）。**两轮待修复的两处限制已修复（2026-09-02 第二轮）**：① 同类型同泛型 protocol 的多 impl 可按 protocol 类型实参 / 实参类型选择（`check_method_call` 收集候选 impl 按签名兼容性选取）；② impl 类型参数可由实参反推（`impl<T> Wrap<T> for W` 的 `T` 不再报 `undefined type T`）；附带修复同 protocol 多 impl 单态化缓存碰撞（`instantiate_impl_method` 的 mono 键并入 `protocol_type_args`）。
- ✅ A3 含 `Self` 返回——**已具备**（SH-P0-3 G-M1 + 按值 `Self` 返回 codegen 修复）。
- ✅ A4 约束收尾——**本轮补两处真实缺口**：函数/方法级 `where` 子句（parser `parse_fn` 未接 `parse_where_clause`，而 `grammar.md` §2.3 早已写入该产生式）；impl 级约束强制校验（此前 `ImplDef.bounds` 只记录不校验，违反时在方法体内部报误导性错误）。

> **关联文档**：[SH-P1-1 泛型 protocol/impl 完整化](tasks/leaf/sh-p1-1-generic-protocol.md)

### 3.2 B 嵌套模块系统（P1-3）
- B1 嵌套模块（**既有** ✅，内联 + 多文件）/ B2 `pub use` 重导出（✅ 2026-09-02）/ 组导入 `import a::{b,c}`（✅ 2026-09-02）/ glob 导入 `import a::*`（✅ 2026-09-02）。
- **B3 `super`/`crate::` 相对路径：排除**。依据 `docs/module-system.md` v1.1 扁平名字空间决策，路径首段恒为模块名，现实行 `模块名::Item` 编码，不引入 `crate::`/`super::`/`self::`。
- **B4 模块级可见性（默认私有 + `pub` 校验）：暂缓**。当前 std 全量依赖"跨模块全部可达"，启用可见性需先为 std 全部跨模块引用补齐 `pub`，风险高、易引发大范围回归。
> **关联文档**：[SH-P1-3 嵌套模块系统](tasks/leaf/sh-p1-3-nested-module.md)

### 3.3 C protocol derive 宏（P1-2）
- C1 `Debug` / C2 `Clone` / C3 `PartialEq` / C4 derive 框架。**（🟢 2026-09-02 struct 核心落地）**
  - 机制：derive 展开**零新增 IR 节点**——`expand_derives_for_struct`（`check_item/derive.rs`）为每个受支持 protocol 合成 `AstImplBlock` 走既有 `collect_impl`，方法体调用点实例化。
  - C2 `Clone`/`C3 `PartialEq` protocol 在 `core.rl` 顶层新增；结构体 `==`/`!=` 经 `comparison.rs` desugar 为 `a.eq(&b)`。
  - C1 `Debug` 复用 `fmt` 模块既有 protocol，`dbg!`（`{:?}` 语义）输出 `Name { f: <Debug>, ... }`。
  - C4：未知 derive 名（serde `Serialize`/`Deserialize`）宽松忽略，与既有行为兼容。
  - 验证：新增 `derive_clone`/`derive_partialeq`/`derive_debug` run-pass（含 `.out`）；全量 `rlyeh test tests` **252/252 通过**。
  - 已知限制：仅 struct（enum derive 待办）；泛型字段须自身实现对应 protocol（MVP 未强制 bound）；char 字段 Debug 需 `fmt::Debug for char`（未提供）。
> **关联文档**：[SH-P1-2 protocol derive 宏](tasks/leaf/sh-p1-2-derive.md)

### 3.4 D 进程调用 / 外部工具链 FFI（P2-2）
- D1 `system`/`exec` / D2 捕获 stdout/stderr / D3 对接 driver `assemble()`（调 `clang`，保留"生成 LLVM IR 文本 + 调 clang"策略，见评估报告 §6 路径 A）。
- **（🟢 2026-09-02 核心落地）** 新增 `rlyeh-std/rlyeh/process/module.rl`：
  - D1 `system(cmd) -> i32`：经 `popen`+`pclose` 实现，返回归一化退出码 `(status >> 8) & 0xFF`。
  - D2 `exec(cmd) -> Output{status,stdout}` / `output(cmd) -> String` / `exec_combined(cmd)`（`2>&1` 合并 stderr），`fread` 块读捕获 stdout。
  - D3 印证：`process::exec("clang --version")` 返回 status 0，外部工具链 FFI 可用；Rlyeh 版 driver `assemble()` 完整自举留待 0.3.0。
  - 验证：新增 `process_exec.rl`（含 `.out`）；全量 `rlyeh test tests` **253/253 通过**。
> **关联文档**：[SH-P2-2 进程调用 / 外部工具链 FFI](tasks/leaf/sh-p2-2-process-ffi.md)

### 3.5 E `unsafe` 块 / 裸指针 / `#[repr(C)]`（P0-1）
- E1 `unsafe` 块作用域与裸指针（`*const T`/`*mut T`）读写 / E2 `#[repr(C)]` 内存布局 / E3 FFI 安全边界约定。
- **（🟢 已完成，2026-09-01）** E-M1：`unsafe { }` 块表达式 + 裸指针读写/索引（跨 ast/hir/parser/typecheck/mir/borrowck/regionck/desugar/fmt/check）；E2：`#[repr(C)]` 真布局（sub-8 标量 C 打包 + 嵌套聚合内联 + 裸指针 1 字节步长字节语义）；E3：`extern fn` 调用强制 `unsafe` 门禁（prelude 受信任 FFI 豁免）。
- 验证：Rlyeh 侧用 `unsafe` 封装手动 bump 分配器（等价于 `rlyeh-region-alloc`），证明运行时表达力地基可用；run-pass `unsafe-raw-ptr`/`unsafe-bump-allocator`/`repr-c-struct`/`repr-c-packing`/`repr-c-packing-bytes`/`repr-c-nested`/`unsafe-extern-call` + compile-fail `repr-c-sub8`/`unsafe-extern-call-outside`；全量 `rlyeh test tests` **253/253 通过**。
> **关联文档**：[SH-P0-1 `unsafe` 块 / 裸指针 / `#[repr(C)]`](tasks/leaf/sh-p0-1-unsafe.md)

### 3.6 F 跨函数边界闭包 + `move` + `'static`（P0-2）
- F1 闭包值跨函数边界传递（作 fn 实参/返回值）/ F2 `move` 所有权转移生效 / F3 `'static` 约束检查。
- 验证：Rlyeh 侧 `spawn(move || ...)` 跨线程执行（等价于 actor-runtime worker_loop）。
> **关联文档**：[SH-P0-2 跨函数边界闭包 + `move` + `'static`](tasks/leaf/sh-p0-2-closure.md)

### 3.7 G `dyn Protocol` 含 `Self` + `Any` 类型擦除（P0-3）
- ✅ G1 `dyn Protocol` 调用含 `Self` 签名方法（devirtualize 时 `Self`→具体类型；完全擦除的 `dyn` 仍按 object-unsafe 拒绝）/ ✅ G2 `Any` 类型标识存储（`dyn Any` vtable 槽 0 存 type_id）+ `downcast` 安全检查（`any_downcast_ref::<T>` → `Option<&T>`）。
- 验证：经 `dyn Protocol` 调含 `Self` 签名方法（形参 `&Self` + 按值返回 `Self`，见 `tests/run-pass/dyn_self_return.rl`）；`Any` 装箱 + `downcast` 往返（见 `tests/run-pass/any_downcast.rl`，等价于 actor 消息分发）。
- 已知限制：`Box<dyn Any + Send>` / 多 protocol 约束未实现。
> **关联文档**：[SH-P0-3 `dyn Protocol` 含 `Self` + `Any` 类型擦除](tasks/leaf/sh-p0-3-dyn-any.md)

### 3.8 H 并发原语（P0-4）
- ✅ H1 `Arc<Mutex<T>>`/`Weak` 内部可变性 + 锁原语（复核：已具备 `Mutex`/`RwLock`/守卫/`Condvar`/`Barrier`/`Channel`）/ ✅ H2 原子类型 `AtomicI64` + `Ordering` 内存序（driver 注入 LLVM `atomicrmw`/`cmpxchg`）/ ✅ H3 线程 `spawn`（复核：已由 SH-P0-2 的 `Thread::start(move || ..)` 落地）。
- 验证：Rlyeh 侧并发计数器（多线程 + `Arc<Mutex<i64>>` + `Arc<AtomicI64>`）无数据竞争（见 `tests/run-pass/concurrent_counter.rl`，两路径均 4000）。
- 已知限制：仅 `AtomicI64`；无 `fetch_update`/`fetch_max`/`compare_exchange_weak`；`AcqRel` 在 load/store 分派中归入 SeqCst。
> **关联文档**：[SH-P0-4 并发原语](tasks/leaf/sh-p0-4-concurrency.md)

### 3.9 I 内部可变性 / arena 表示（P2-3）
- I1 引入 `RefCell` 等价或 I2 arena + 整数索引（`Arena<T>` + `NodeId`）表示树形 IR（与 Rlyeh 索引式倾向一致，避免运行时引用计数开销）。
- **（🟢 核心落地，2026-09-02）** I2：泛型 `Arena<T>` + `NodeId`（`Vec<T>` 槽区 + 1-based 下标），整型二叉树 IR 经显式工作栈完成可变重写遍历；I1：`RefCell<T>` 安全内部可变性封装（`unsafe` 裸指针 + `alloc_array` 堆缓冲，`get(&self)`/`set(&self)` 经共享引用 mutate，即得 `Rc<RefCell<T>>` 共享可变状态）。两者均提供 Rlyeh 侧 IR 可变遍历 / 共享可变状态能力，替代 Rust 的 `Rc`/`Arc`/`RefCell` 内部可变性模式。
- 验证：`tests/run-pass/arena-ir-traversal.rl`（arena 可变遍历）+ `tests/run-pass/refcell.rl`（RefCell 内部可变性）；全量 `rlyeh test tests` **255/255 通过**。
- 已知限制：泛型 impl 关联静态方法 MVP 不支持（`Vec::new()` 特判在泛型关联函数内无法解析元素类型），空 `Arena` 在调用方顶层以 `Arena { slots: Vec::new() }` 构造。
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
解析器骨架 `(token, rest)` 风格需元组多返回值。类型层（`Type::Tuple`/单元 `()`）已就绪。
- ✅ N1 元组值字面量 `(a, b, c)` → 复用聚合槽布局 `f0/f1/...`（复核：已具备，见 `tests/run-pass/tuple_value.rl`）。
- ✅ N2 解构 `let (a, b) = e` / `let (a, _, c) = e`——`check_stmt` 改为返回 `(Vec<HirStmt>, Type)`，展开为「临时变量承载元组值 + 各元素按位置 `FieldGet` 绑定」（init 只求值一次）；元数不匹配 / 非元组 / 嵌套解构均有门禁。
- ✅ N3 多返回值 `fn f() -> (i64, String)` + `let (x, y) = f()`（复核：多返回本身早已可用，此前仅被 N2 阻塞）。
- ✅ N4 `for (k, v) in map` 回归确认不受影响（`iter.rs` 走独立的二元元组模式特判）。
- 已知限制：元素模式仅支持标识符与 `_`（无嵌套解构、`(mut a, b)`、`..` 剩余模式）；结构体 / 枚举解构仍为 Unsupported。
> **关联文档**：[SH-P0-5 元组值构造 + 解构](tasks/leaf/sh-p0-5-tuple-value.md)

### 3.15 O `if let` / `while let` 模式控制流（P0-6）
语言此前完全缺失；解析器/类型检查器重写依赖。地基复核完备（`match` / `Option`+`?` / `loop`+`break`+`continue` 均已具备），故实现降为 **parser 层 desugar、零新增 IR 节点（typecheck / codegen 无改动）**。
- ✅ O1 `if let Pat = e { A } else { B }` ⟶ `match e { Pat => { A }, _ => { B } }`（缺 `else` 兜底空块求值 `()`）；`else if` / `else if let` 链由 `if` 解析递归处理。
- ✅ O2 `while let Pat = e { A }` ⟶ `loop { match e { Pat => { A }, _ => break } }`（每轮重求值；体内 `break` / `continue` 落在 `loop` 上）。
- ✅ O3 嵌套 `if let`（递归自然支持）+ 作表达式使用（`let v = if let .. { x } else { 0 };`，因 desugar 结果即 `match`）。
- ❌ O4 **let 链**（`if let a = .. && let b = ..`）按「先单模式」暂不支持；元组 / 结构体模式受限（继承 `match` 臂能力）。
- ⏳ O-L1 差分对拍 Rust 参考（随 K 阶段 harness 落地后补）。
> **关联文档**：[SH-P0-6 `if let` / `while let`](tasks/leaf/sh-p0-6-if-let.md)

### 3.16 P `match` 守卫 + 范围/或模式（P0-7）
字符分类/判别分支依赖。
- ✅ P1 守卫表达式——**语法与 typecheck 本已存在，但存在缺陷**：绑定位于臂体块内、晚于条件求值，致引用绑定的守卫读到未初始化值。修复为以 `if <模式条件> { <绑定>; <守卫> } else { false }` 作条件（MIR 的 `&&` 非短路，故不可直接 And 合并）。
- ✅ P2 范围模式——`AstPattern::Range` 的 AST 变体与 parser 本已具备，补齐 `check_pattern` 分支；复用比较链的 `check_comparison` / `compare_hir`，与 `in` 区间语义完全一致。
- ✅ P3 或模式——新增 `AstPattern::Or` + `parse_or_pattern`（不并入 `parse_pattern`，避免误食闭包参数列表的 `|`）+ typecheck 分支（绑定取首个备选，其余在临时作用域内只取条件，源码名序须一致）+ `rlyeh-fmt` / `rlyeh-check` 适配。
- ✅ P4 组合——守卫 + 范围 / 或模式由上述分支天然合流。
- ⏳ 差分对拍 Rust 参考（随 K 阶段 harness 落地后补）。
- 连带修复：codegen `Assign` 未按目标槽类型转换（bool 槽 8 字节 → 1 字节写穿）。
> **关联文档**：[SH-P0-7 `match` 守卫 + 范围/或模式](tasks/leaf/sh-p0-7-match-guard.md)

### 3.17 Q `Drop` protocol / 析构 / RAII（P0-8）
`MutexGuard` 自动解锁、`arena` 自动释放需 `Drop`/RAII。
- ✅ Q1 `Drop` **内置** protocol（`is_drop_protocol`，同 `Any`，不依赖 `protocol Drop` 声明）+ `has_drop_impl` 按类型查 `impl_defs`。复核修正：原注「需 A 泛型 protocol 落地后接」非阻塞——`Drop` 是非泛型 protocol，仅泛型类型（如 `Vec<T>`）的 Drop 才依赖 A。
- ✅ Q2 作用域尾插入 `x.drop()`——**逆声明序**（新增 `Scope.decl_order`，`vars` 是 HashMap 无序）、仅拥有所有权绑定（引用跳过）、块值先求后析构；调用经 `check_stmt` 走常规方法解析，**零新增 IR**。复核修正：块尾注入机制本已存在（`rlyeh-desugar/src/guard.rs`），但按方法名 `lock_guard` 硬编码，本项将其泛化为按类型 / Drop impl。
- ✅ Q3 字段级递归（`build_drop_glue`）：先 `T::drop()` 再逆字段序递归，仅具名 struct 参与，深度上限 4 防自引用无限展开。
- ⏳ Q4 智能指针接入（`Box` 释放堆 / `Rc`/`Arc` 计数-1 / `Gc` 逃逸登记）——需先定堆释放与计数递减的调用约定，且泛型类型 Drop 依赖 A 阶段。
- ⏳ Q-L1 差分对拍 Rust 参考。
- **零影响存量**：无 `Drop` 实现的类型不产生任何语句，std 当前无 `Drop` 实现，故既有代码行为不变。
> **关联文档**：[SH-P0-8 `Drop` / 析构 / RAII](tasks/leaf/sh-p0-8-drop.md)

### 3.18 R `Deref`/`DerefMut` 用户类型自动解引用（P1-4）
`MutexGuard`/`Box<dyn Protocol>` 透传需 `Deref` 自动解引用。
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

### 3.25 Y `Send`/`Sync` 自动 protocol（放宽/标记）（P3-1）
并发安全基线（告警式放宽，非硬阻塞）。
> **关联文档**：[SH-P3-1 `Send`/`Sync`](tasks/leaf/sh-p3-1-send-sync.md)

---

## 4. 跟踪与验收约定

1. 每个阶段完成须满足：`cargo test --workspace` 全绿 + `cargo clippy --workspace --all-targets` 0 警告（编译器改动回归）。
2. 能力类任务须附 Rlyeh 侧**单元/集成用例**（泛型 protocol 声明、嵌套模块访问、derive 后 `dbg!`、进程调用 `echo`、unsafe bump 分配器、`spawn(move)` 跨线程、`dyn`+`Self`/`Any` 往返、`Arc<Mutex>` 并发计数）。
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
| **Q** `Drop`/RAII | 🔴 高 | `Drop` protocol 声明 → 作用域尾插入 `drop` → 字段递归 → 智能指针接入 | 离开作用域自动释放 | [SH-P0-8](tasks/leaf/sh-p0-8-drop.md) |

> 中/低危阶段（A/B/C/D/I/L/R/S/T/U/V/W/X/Y）按叶子文档子任务推进，均有独立单测/集成用例兜底，不再单列分解。

### 7.1 E `unsafe` 块 / 裸指针 / `#[repr(C)]`（SH-P0-1，🔴 高）
运行时表达力地基；解除"运行时必须保留 Rust"的死结（事实依据见 `crates/` 核查：`rlyeh-region-alloc` `unsafe`×36、`rlyeh-gc-runtime` 裸指针/`UnsafeCell`/`#[repr(C)]`、`rlyeh-actor-runtime` FFI、`rlyeh-std` nio `unsafe`×16）。
- **E-M1（设计原型）** `unsafe` 块作用域 + 手动 bump 分配器（MVP 验证，等价于 `rlyeh-region-alloc`），证明运行时表达力地基可用。
- **E-M2** 裸指针 `*const T`/`*mut T` 读写（受控操作，复用现有 G3 裸指针机制）。
- **E-M3** `#[repr(C)]` 内存布局约定（对齐/字段序，与 Rust rlib 对齐）。
- **E-M4** FFI 安全边界约定（`extern "C"` / `#[no_mangle]` 导出语义）。
- **关键 checkpoint**：unsafe bump 分配器跑通且与 `rlyeh-region-alloc` 对拍。
> **关联文档**：[SH-P0-1 `unsafe` 块 / 裸指针 / `#[repr(C)]`](tasks/leaf/sh-p0-1-unsafe.md)

### 7.2 F 跨函数边界闭包 + `move` + `'static`（SH-P0-2，🔴 高）🟢 完成
actor 调度器 / driver 线程模型地基（事实依据：`rlyeh-actor-runtime` `spawn(move || worker_loop)`、`rlyeh-driver` 线程 stack 64MB `spawn(move)`）。
- ✅ **F-M1** 无捕获闭包值跨 fn（复用 H2/H5 降级为 fn 指针）。
- ✅ **F-M2** `move` 所有权转移生效（`Type::Closure` 新增 `is_move` 字段；有捕获的非 `move` 闭包跨线程被拒）。
- ✅ **F-M3** `'static` 约束检查（`type_contains_ref` 拒绝捕获借用引用）。
- ✅ **F-M4** `Thread::start(move || ...)` 跨线程执行（复用 W6 机制：捕获环境堆分配 + `__thread_entry_N` thunk 新线程调用；注：`spawn` 为 actor 派生保留关键字，线程启动统一用 `Thread::start`）。
- **关键 checkpoint**：`Thread::start(move || ...)` 跨线程执行通过（见 `tests/run-pass/thread-spawn-move.rl` + 两个 compile-fail；完整套件 218/218 通过）。
> **关联文档**：[SH-P0-2 跨函数边界闭包 + `move` + `'static`](tasks/leaf/sh-p0-2-closure.md)

### 7.3 G `dyn Protocol` 含 `Self` + `Any` 类型擦除（SH-P0-3，🔴 高）
actor 消息协议（异构消息信封）地基（事实依据：`rlyeh-actor-runtime` `ActorState: Any + Send + Sync`、`Box<dyn Any + Send>` 信封、编译器已建模 `dyn Protocol` 为 2 槽胖指针）。
- ✅ **G-M1** `dyn` 调含 `Self` 方法（devirtualize 时 `replace_type_self` 把 `Self` 替换为绑定源具体类型，再检查实参/推导返回类型；完全擦除的 `dyn Protocol` 仍由 vtable 分支按 object-unsafe 拒绝）。
- ✅ **G-M2** `Any` 类型标识存储（TypeId 式：`type_id_of` = 类型规范字符串 FNV-1a 64 散列，`dyn Any` vtable 槽 0 存储；`any_type_id(x)` 读回）。
- ✅ **G-M3** `downcast` 安全检查（`any_downcast_ref::<T>(x)` 展开为 type_id 相等判定 + `Option<&T>`，错误类型得 `None`；非 `dyn Any` 实参 / 缺 turbofish 均报错）。
- **关键 checkpoint**：经 `dyn Protocol` 调含 `Self` 签名方法通过（形参 `&Self` + 按值返回 `Self` 聚合，见 `tests/run-pass/dyn_self_return.rl`）；`Any` 装箱 + `downcast` 往返通过（见 `tests/run-pass/any_downcast.rl` + 两个 compile-fail；完整套件 740/740 通过）。
- **附带修复**：按值返回 `Self` 聚合经 dyn 调用的 codegen 缺陷——`rlyeh-codegen/src/llvm/llvm_ctor.rs` 中**被取址函数**（vtable `FnPtr`）为保持 `i8*` 返回 ABI 被排除在 `ret_by_value` 之外，但原实现在其分支直接 `continue`，跳过了 4b-iv 连带剔除，破坏「`f ∈ ret_by_value` ⟺ 全部 `Return ∈ bvs`」不变量，致函数体按值返回 `{i64,i64}` 而声明为 `i8*`。改为以 `participates` 标志区分：不参与判定但仍执行连带剔除（含指针拷贝别名闭包），函数体改用 calloc 堆分配并返回 `i8*`，声明/函数体/调用点三方一致。
- **已知限制**：`Box<dyn Any + Send>` / 多 protocol 约束未实现（待 0.3.0 运行时重写）。
> **关联文档**：[SH-P0-3 `dyn Protocol` 含 `Self` + `Any` 类型擦除](tasks/leaf/sh-p0-3-dyn-any.md)

### 7.4 H 并发原语（SH-P0-4，🔴 高）
运行时并发地基（与 F 协同）；当前有 `Arc`/`Weak`（K3）。
- ✅ **H-M1** `Arc<Mutex<T>>`/`Weak` 内部可变性 + 锁原语——**复核确认已具备**：`Mutex<T>`/`RwLock<T>` + 三种守卫（desugar 作用域自动解锁）+ `Condvar`/`Barrier`/`Channel`（无界/有界/异步 `recv_async`）已在 `rlyeh-std/rlyeh/sync/module.rl` 落地（B4/P/P1/P5/Y4b-3/Y4c/W5）。
- ✅ **H-M2** `Atomic*` 原子类型 + 内存序——` AtomicI64` + `Ordering`。底层由 driver 注入 LLVM IR（`rlyeh-driver/src/platform_ir.rs::atomic_builtin_ir`）：RMW 族（`swap`/`fetch_{add,sub,and,or,xor}`，返回旧值）经 `atomicrmw ... seq_cst`，CAS 经 `cmpxchg`，`load`/`store` 另提供 `acquire`/`release`/`relaxed` 变体供内存序分派。原子读改写**无 C 链接符号**（C11 `<stdatomic.h>` 为泛型宏），故沿用 `__rlyeh_*` 注入机制。API：`new`/`load`/`store`/`swap`/`fetch_*`/`compare_and_swap`/`compare_exchange`/`load_with`/`store_with`。
- ✅ **H-M3** 线程 `spawn`（接收跨边界闭包，依赖 F）——**复核确认已由 SH-P0-2 F-M4 落地**：`Thread::start(move || ..)` + `join`。
- ✅ **H-M4** 并发计数器（无数据竞争）——`tests/run-pass/concurrent_counter.rl`：两线程各自增 2000 次，`Arc<AtomicI64>`（无锁）与 `Arc<Mutex<i64>>`（`lock_guard` 守卫）两路径最终计数均恰为 4000（丢失更新会偏小），连跑 3 次稳定。
- **关键 checkpoint**：Rlyeh 侧多线程 + `Arc<Mutex>` 计数器无数据竞争通过（4000/4000/1）；原子自增正确通过（`atomic_i64.rl`）；完整套件 740/740 全绿。
- **已知限制**：仅 `AtomicI64`（无 `AtomicBool`/`AtomicUsize`/`AtomicPtr`）；无 `fetch_update`/`fetch_max`/`fetch_min`/`compare_exchange_weak`；`Ordering::AcqRel` 在 `load_with`/`store_with` 分派中归入 SeqCst；原子对象无析构（同 `Mutex`）。
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
- ✅ **N-M1（中）** 元组字面量 `(a, b, c)` → 复用聚合槽布局 `f0/f1/...`——**复核确认已具备**（`tests/run-pass/tuple_value.rl`）。
- ✅ **N-M2（中）** 解构 `let (a, b) = e` / `let (a, _, c) = e`——`check_stmt` 签名改为返回 `(Vec<HirStmt>, Type)`（`HirStmt` 无 Block 变体，解构须展开为多条 `Let`），展开形态为「临时变量承载元组值（init 只求值一次）+ 各元素按位置 `FieldGet` 绑定」，与 `t.f0` 字段访问同构；元数不匹配 / 非元组类型 / 嵌套解构均有显式门禁。
- ✅ **N-M3（中）** 多返回值 `fn f() -> (i64, String)` + `let (x, y) = f()`——**多返回本身早已可用，此前仅被 N-M2 阻塞**（std `Channel` 退化为 `ChannelPair` 结构体的根因）。
- ✅ **N-M4（低）** `for (k, v) in map` 回归确认不受影响（`iter.rs` 走独立的二元元组模式特判路径）。
- **关键 checkpoint**：解构 + 多返回接收通过（`tests/run-pass/tuple_destructure.rl`）；`(tok, rest)` 风格重写 lexer/parser 主体待 M 阶段（依赖 O/P 落地）。
- **已知限制**：元素模式仅支持标识符与 `_`（无嵌套解构、`(mut a, b)`、`..` 剩余模式）；结构体 / 枚举解构仍为 Unsupported；差分对拍（N-L1）随 K 阶段 harness 落地后补。
> **关联文档**：[SH-P0-5 元组值构造 + 解构](tasks/leaf/sh-p0-5-tuple-value.md)

### 7.9 O `if let` / `while let`（SH-P0-6，🔴 高）
语言此前完全缺失；解析器/类型检查器重写依赖。
- ✅ **O-M1（中）** `if let Pat = expr { .. } else { .. }` desugar 为 **`match expr { Pat => { .. }, _ => { .. } }`**（零新增 IR；缺 `else` 兜底空块）。落点 `crates/rlyeh-parser/src/expr/control.rs::parse_if_let_expr`。
- ✅ **O-M2（中）** `while let Pat = expr { .. }` desugar 为 **`loop { match expr { Pat => { .. }, _ => break } }`**（每轮条件重评估；体内 `break` / `continue` 落在 `loop` 上，语义同 Rust）。
- ✅ **O-M3（低）** 嵌套 / `else if let` 链（`if` 解析递归自然支持）；**let 链**（`&& let`）暂不支持。
- ⏳ **O-L1（低）** 差分对拍 Rust 参考（随 K 阶段 harness 落地后补）。
- **关键 checkpoint**：`tests/run-pass/if_while_let.{rl,out}` 10 行输出全绿 + `tests/compile-fail/if-let-tuple-pattern.rl` 门禁；全量 740 用例全绿。
- **已知限制**：模式能力继承 `match` 臂（元组 / 结构体模式不支持）；无 let 链。
> **关联文档**：[SH-P0-6 `if let` / `while let`](tasks/leaf/sh-p0-6-if-let.md)

### 7.10 P `match` 守卫 + 范围/或模式（SH-P0-7，🔴 中高）
字符分类/判别分支依赖。
- ✅ **P-M1（中）** 守卫——**本已存在但有缺陷**：绑定在 `then_block` 内、晚于条件求值，致引用绑定的守卫读到未初始化值。修复为以 `if <模式条件> { <绑定>; <守卫> } else { false }` 作条件（MIR `&&` 非短路，故不可直接 And 合并；`HirExpr::If` 提供真实 CFG 分叉）。零新增 IR 节点。
- ✅ **P-M2（中）** 范围模式——AST 变体与 parser 本已具备，补 `check_pattern` 分支；复用比较链 `check_comparison` / `compare_hir`，与 `in` 区间语义一致（排序仅数值与字符）。
- ✅ **P-M3（中）** 或模式——新增 `AstPattern::Or` + `parse_or_pattern`（不并入 `parse_pattern`，避免误食闭包参数列表的 `|`）+ typecheck 分支（绑定取首个备选、其余在临时作用域内只取条件、源码名序须一致）+ `rlyeh-fmt` / `rlyeh-check` 适配。
- ✅ **P-M4（低）** 守卫 + 范围/或组合（条件合流为单一表达式，天然支持）。
- ⏳ **P-L1（低）** 差分对拍 Rust 参考（随 K 阶段 harness 落地后补）。
- **关键 checkpoint**：`tests/run-pass/match_guard_range_or.{rl,out}` 28 行输出全绿（含字符分类 `'a'...'z' | 'A'...'Z' if upper` 形态）+ 4 项 compile-fail 门禁；全量回归无失败。
- **连带修复**：codegen `LirStmt::Assign` 未按目标槽类型转换——bool 形参与具名 bool 局部槽为 `i64`（8 字节），按值推断的 bool 临时槽为 `i1`（1 字节），`store i64 .., i1*` 写穿 1 字节槽破坏相邻栈（守卫值即经此路径）。已按 I64↔Bool 插入 `icmp ne` / `zext`。
> **关联文档**：[SH-P0-7 `match` 守卫 + 范围/或模式](tasks/leaf/sh-p0-7-match-guard.md)

### 7.11 Q `Drop` protocol / 析构 / RAII（SH-P0-8，🔴 高）
MutexGuard 自动解锁、arena 自动释放需 `Drop`/RAII；Rlyeh 0.1.0 完全无析构机制。
- ✅ **Q-M1（中）** `Drop` 作为**内置** protocol（`is_drop_protocol` / `has_drop_impl`），不依赖 `protocol Drop` 显式声明，也不依赖 A 阶段（`Drop` 为非泛型 protocol）。
- ✅ **Q-M2（中）** 作用域尾自动插入 `x.drop()`：逆声明序（`Scope.decl_order`）、仅拥有所有权绑定（引用跳过）、块值先求后析构、`Never` 结尾跳过。调用经 `check_stmt` 走常规方法解析，零新增 IR。
- ✅ **Q-M3（中）** 字段级递归 `build_drop_glue`：先 `T::drop()` 再逆字段序递归；仅具名 struct 参与，`MAX_DROP_DEPTH = 4` 防自引用无限展开。
- ⏳ **Q-M4（中）** 智能指针接入（`Box` 释放堆、`Rc`/`Arc` 计数-1、`Gc` 逃逸登记）——待办，需先定调用约定且泛型 Drop 依赖 A 阶段。
- ⏳ **Q-L1（低）** 差分对拍 Rust 参考。
- **关键 checkpoint**：`tests/run-pass/drop_raii.{rl,out}` 22 行输出全绿（逆声明序 / 嵌套块 / 块值时序 / 引用跳过 / 逆字段序 / drop glue / 多层嵌套）；全量 128 个测试目标全绿。
- **已知限制**：不跟踪 move（被 return 移出的变量仍析构）；函数形参不析构（位于外层 fn 作用域）；`return`/`break` 提前退出路径不注入（同既有 `guard.rs` 约束）。
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
| 2026-09-21 | **M-M2b2 落地**：SH-P2-7 M-M2b 补齐 `let` 模式（扁平元组/`_`）与类型标注（`i64`/`&T`/`&mut T`/泛型 `@GEN@` 迭代收束，嵌套 `Vec<Result<i64,String>>` 正确）；Rust oracle 新增 `render_pattern_canonical`/`render_type_canonical`；`tests/self_host_parser.rs` 扩展 b2 用例逐字节对拍一致；实证 Rlyeh 5 项约束（`&&` 不短路、返回 `String` 禁用 `return`、if-表达式 else 返回条件值、`>>`=shr、前向调用推断 i64）写入叶子踩坑点 |

---

> **维护者**：Rlyeh Language Team
> **最后更新**：2026-09-01
