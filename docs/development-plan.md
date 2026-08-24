# 开发计划 — Zeta v2.0 第二版规划（2026-08）

> **性质**：本文档为 2026-08 对原计划（M1/M2/M3）复盘后制定的新版开发计划的**权威副本**。
> **总纲与进度速览**：见根目录 [`CODEBUDDY.md`](../CODEBUDDY.md)（§5.5 为执行记录，§6 为里程碑原表）。
> **维护规则**：每完成一项任务，需同步更新本文档与 CODEBUDDY.md 的状态标识。

---

## 1. 背景：原计划进度盘点（2026-08 复盘）

### 1.1 完成情况

| 里程碑 | 状态 | 说明 |
|--------|------|------|
| M1 编译器 MVP | ✅ 100% | M1.1–M1.9 全部完成（词法/语法/AST→HIR/类型检查/借用检查/区域系统/MIR+优化/LLVM 后端/hello-world） |
| M2 生产可用 | 🔧 约 96% | M2.1 Actor 运行时、模块系统、M2.2 Zep、M2.3 增量编译、M2.5 标准库主体、M2.8 分配器侧 已完成；M2.4 LSP（F1）、M2.6 交叉编译 macOS 双架构 + 平台内建（E1）、M2.7 WASM（E2）、M2.8 PGO 回灌（F2）已完成；剩余 Windows/ARM 工具链（待对应环境）与 M3 生态 |
| M3 生态繁荣 | ⏳ 0% | 未开始 |

### 1.2 遗留问题清单（13 项）

| # | 遗留问题 | 归属阶段 | 状态 |
|---|---------|---------|------|
| 1 | 用户级 match 解构具体实例化枚举聚合载荷失败（`expected String, found T`） | A1 | ✅ 已修复 |
| 2 | 裸 `Result::Err(7).unwrap_or(100)` 返回 `_`（`Infer` 占位未定型） | A2 | ✅ 已修复 |
| 3 | `String` 拼接 `+` 为原地追加 + 共享缓冲，存在别名隐患（需评审改拷贝语义） | A3 | ✅ 已修复 |
| 4 | 无通用 FFI，io/net 绑定层（NIO/sendfile 已有 Rust 侧但接不进 Zeta）无法接线 | A4 | ✅ 已修复 |
| 5 | 标准库 time 模块缺失（`Duration`/`Instant`） | B1 | ✅ 已完成 |
| 6 | 标准库 io 模块缺失（File 读写、stdin/stdout、`read_to_string` 等） | B2 | ✅ 已完成（stdio 方案） |
| 7 | 标准库 net 模块缺失（TCP/UDP + NIO/sendfile 绑定层接线） | B3 | ✅ 已完成（hostname + htons + socketpair + sockaddr_in4 + tcp_connect；NIO/sendfile 接线随阶段 C/D 延后） |
| 8 | 标准库 sync 模块缺失（Mutex/RwLock/Condvar 等） | B4 | ✅ 已完成（Mutex/RwLock；Condvar/Barrier 待线程支持） |
| 9 | `Vec<T>`/`HashMap` 常用方法缺失（`first`/`last`/`reverse`/`swap`/`binary_search`；`len`/`is_empty` 重复定义与固化） | B5 | ✅ 已完成 |
| 10 | `zeta test` 子命令未实现（测试目前经 `cargo test` 驱动） | D | ✅ 已完成（D1，13 用例 + cargo 矩阵） |
| 11 | 语言级 `actor`/`spawn` 语法与标准库集成未接线（`zeta-actor-runtime` 已就绪） | C | ✅ 已完成 |
| 12 | M2.4 LSP 服务器、M2.8 PGO 数据回灌编译流程 | F | ✅ 已完成（F1 `zeta lsp` + zeta-lsp crate；F2 `zeta profile` + `zeta build --profile`） |
| 13 | M2.6 交叉编译、M2.7 WASM、工具链其余命令（fmt/check/doc/bench/publish）、M3 生态（数据库/HTTP/序列化/嵌入式/GPU/教程） | E 及后续 | 🔧 大部分完成（E1 macOS 双架构 + 平台内建、E2 WASM、E3 发布流程 + fmt/check/doc/bench/publish/new 命令已完成；Windows/ARM 工具链待环境；M3 生态未开始） |
| 14 | `String::from` 暂仅支持字面量（非字面量 Str 长度表达未实现）、范围切片仅支持 String（数组/Vec 动态切片待实现） | A/B 遗留 | ✅ 已修复（local_inits 追踪字面量绑定；`Vec<T>` slice 方法 + 数组切片展开；`dynamic_slice_test.rs` 7 用例） |

---

## 2. 新版计划总览（阶段 A–F）

| 阶段 | 主题 | 状态 |
|------|------|------|
| **A** | 编译器加固（泛型/Infer/FFI/字符串语义） | ✅ A1–A4 全部完成 |
| **B** | 标准库完善（time/io/net/sync/collections） | ✅ 全部完成（B1–B5） |
| **C** | Actor 语言级接线（`actor`/`spawn`/`.await`） | ✅ 全部完成（C1–C3） |
| **D** | 工具链（`zeta test`/`fmt`/`check`/`doc`/`bench`） | ✅ D1–D3 全部完成 |
| **E** | 多目标与发布（交叉编译/WASM/发布流程） | ✅ E1 大部分完成（macOS 双架构 + 平台内建；Windows/ARM 待环境）；E2 完成；E3 完成 |
| **F** | 编译器深度（LSP、PGO 回灌） | ✅ F1–F2 全部完成 |

> **执行优先级**：A（已全部完成）→ B → C → D → E → F（已完成）。

---

## 3. 阶段详情

### 阶段 A — 编译器加固

| 任务 | 内容 | 状态 |
|------|------|------|
| A1 | 用户级 match 解构具体实例化枚举聚合载荷：`check_pattern` Enum 分支合并「当前 generic_subst + `pat_ty` 类型参数 ↔ `enum_def.type_params` 新映射」，嵌套泛型 `Option<Vec<T>>` 等递归定型 | ✅ 完成 |
| A2 | Infer 枚举自动定型：`check_method_call` 参数检查时对含 `_` 的期望类型用实参 unify 回填 subst（`unify` 新增 `Type::Infer` 分支），回填后重算签名再实例化 | ✅ 完成 |
| A3 | `String` 拼接 `+` 语义评审：当前为原地追加 + 共享缓冲的别名隐患，评估改为拷贝语义（`let __s = a; __s.push_str(b)` 形态） | ✅ 完成（拷贝语义：`let __s = a.clone(); __s.push_str(b)`） |
| A4 | 通用 FFI `extern fn` 声明：打通 parser→typecheck→HIR→MIR→LIR→LLVM→链接全链路，codegen 生成 `declare` 而非 `define`，符号由链接器解析 | ✅ 完成 |

### 阶段 B — 标准库完善

| 任务 | 内容 | 状态 |
|------|------|------|
| B1 | 时间模块：`Duration { micros: i64 }` + `Instant { start: i64 }`，底层 libc `clock()` extern（首例 extern 驱动标准库模块） | ✅ 完成 |
| B2 | io 模块：libc stdio 文件 IO（`fopen`/`fread`/`fwrite`/`fclose`/`fseek`/`ftell`）+ `read_file`/`write_file`/`append_file`/`c_str`/`read_line`；编译器 extern `String` 参数取 data 指针 | ✅ 完成 |
| B3 | net 模块：`hostname()` + `htons` + `socketpair_stream`/`fd_at`（AF_UNIX 全双工字节流 + int32 字节解释）+ `send_all`/`recv_some` + `sockaddr_in4`（macOS 布局字节打包）/`tcp_connect`（socket→sockaddr→connect，失败 close 返回 -1）；**依赖本轮位运算全链路**（`&`/`|`/`^`/`<<`/`>>`，含算术右移 ashr）；NIO/sendfile 绑定层接线随阶段 C/D 延后 | ✅ 完成 |
| B4 | sync 模块：`Mutex`/`RwLock`（pthread extern + calloc 承载，try 系列依赖 extern `i32` 返回支持）；Condvar/Barrier 骨架留注释（待函数指针/线程创建） | ✅ 完成 |
| B5 | collections 补全：`Vec` 的 `first`/`last`/`reverse`/`swap`/`binary_search`；`HashMap` `len`/`is_empty` 固化与去重 | ✅ 完成 |

### 阶段 C — Actor 语言级接线

| 任务 | 内容 | 状态 |
|------|------|------|
| C1 | `actor` 语法解析 + 语义化，桥接 `zeta-actor-runtime`（ActorRef/Runtime API 已就绪） | ✅ 完成 |
| C2 | `spawn`/`.await` 调用语法与标准库集成（`Counter::new()` → `counter.increment(10).await` 形态） | ✅ 完成 |
| C3 | Actor 示例（ping-pong、Supervisor 恢复）与集成测试固化 | ✅ 完成 |

执行记录（2026-08，`crates/zeta-driver/tests/actor_test.rs` 8 用例全绿 + 全量回归通过）：

- **C1 — actor desugar 全链路**：`actor` 语法经 `zeta-typecheck` `expand_actor` 展开为普通结构 + 4 个生成函数：`<Actor>::__state_new`（分配 N 槽 calloc 缓冲 + 字段初值）、`__m<i>`（第 i 个方法：`self` 槽数组 + 消息槽参数，返回 i64）、`__handle`（按 kind 分发到 `__m<i>`，槽 4/5 为 kind 与返回槽）、`__handle_message` 函数返回 `__handle(...)` 槽 5 结果；`zeta-actor-runtime` 侧 `CallbackActor` 承载（状态指针 + handler fn 指针，统一 u64 槽语义）；方法返回 `-1` 视为崩溃信号（`u64::MAX` → `ActorError::Panic`）；编译器为含 actor 程序自动生成 `zeta_actor_spawn`/`zeta_actor_spawn_supervised`/`zeta_actor_ask`/`zeta_actor_send` extern 声明（**检测用户显式声明则跳过生成**，避免 LLVM 重复 declare 冲突；`zeta_actor_stop`/`zeta_actor_shutdown` 需用户显式声明）
- **C1 排障**：① 用户级 actor 方法返回 -1 误触崩溃协议（冒烟测试用手写 handle 未暴露）→ Panic 前先 `reply(0u64)` 让 ask 立即失败，否则 ask 干等满 ASK_TIMEOUT；`reply(0)` 裸字面量被推断为 i32 致 `WrongReplyType`，需显式 `0u64` ② supervisor 重启竞态：崩溃时旧 state 放回 handle 且 `running=false`，`reply(0)` 使 ask 提前返回、后续消息被其他 Worker 用旧 state 处理 → 修复为崩溃时不放回旧 state、保持 `running=true` 直到重启完成，重启后重置并重新调度 ③ staticlib 产物陈旧：改 runtime 后需显式 `cargo build -p zeta-actor-runtime`（`cargo run` 不重建 `.a`）
- **C2 — 语言级受监督 spawn**：构造函数分支扩展 `Actor::new_supervised(strategy)`（strategy i64：0=OneForOne 1=AllForOne 2=RestartForOne）→ `zeta_actor_spawn_supervised("<__handle>", "<__state_new>", strategy)`（factory 传符号名字符串，runtime 内部 `dlsym` 解析）；`supervisor_restart` 测试不再依赖手工 extern 声明，验证编译器自动生成 extern 全链路
- **C2 修复 — `String::from` NUL 终止 bug**：`String::from` 展开原为 `alloc_bytes(len)` + `copy_bytes(len)`，缓冲末尾无 NUL；runtime 侧 `CStr::from_ptr` 按 NUL 扫描读超界（16 字节的 `Wobbly::__handle` 恰在分配块边界读到相邻堆垃圾 `Wobbly::__handleP`，17 字节的 `Counter::__handle` 靠对齐运气幸存）→ 改为 `alloc_bytes(len+1)` + `copy_bytes(len+1)`（LLVM 字符串常量自带 `\00` 一并拷入），`cap` 字段保持 `len` 不扰动扩容路径
- **C3 — 示例与测试固化**：`examples/actor-ping-pong.zeta`（ask 往返 + send 异步 + FIFO，输出 11/12/2/4）、`examples/actor-supervisor.zeta`（`new_supervised(0)` 崩溃恢复，输出 5/0/3）；集成测试补 `crash_without_supervisor_stops_actor`（无监督崩溃 → actor 停止 → 后续 ask 返回 0）、`send_fifo_order`（连续 send 后 ask 可见全部累积）

### 阶段 D — 工具链

| 任务 | 内容 | 状态 |
|------|------|------|
| D1 | `zeta test`：接入 `tests/` 目录（compile-pass/compile-fail/run-pass）与 cargo 测试矩阵 | ✅ 完成 |
| D2 | `zeta fmt` + `zeta check`（`tools/` 下 crate 已有骨架） | ✅ 完成 |
| D3 | `zeta doc`（`///` 注释提取）+ `zeta bench`（criterion 基准） | ✅ 完成 |

执行记录（2026-08，`zeta test tests` 13/13 全绿 + `zeta fmt`/`zeta check` + 全量回归通过）：

- **D2 — `zeta fmt` 格式化器**：新增 `tools/zeta-fmt`（`FmtOptions` 缩进默认 4；`format_source`/`format_program` 解析为 AST 后按统一规范重建：顶层项空一行、块类表达式多行展开、表达式按**优先级表**重排括号保证语义不变（Assign<Or<And<BitOr<BitXor<BitAnd<Compare<Shift<Add<Mul<Cast<Unary<Postfix，右侧同优先级补括号）、字符串/字符按 lexer 转义规则重编码（`\"`/`\\`/`\n`/`\xNN`）、浮点字面量强制保留小数点、时间字面量还原（`9am`/`6pm`/`12:00`/`1:05pm`）；`AstStmt::Expr`=带分号语句 / `AstStmt::Semi`=无分号块后语句（与 parser 语义一致）；self 接收者打印 `&self`/`&mut self` 特例（`self: &Self` 无法再解析）；注释暂不保留（MVP 基于 AST 重建，词法阶段即丢弃）；CLI `zeta fmt <file> [--check] [-w] [--indent N]`
- **D2 — `zeta check` 静态分析器**：新增 `tools/zeta-check`，4 条 AST lint 规则 + parse-error：`unused-variable`（块/函数/闭包/for 作用域栈 + 遮蔽查找，`_` 通配与 self 接收者豁免）、`constant-condition`（if/while 条件为字面量或 `!字面量`）、`redundant-compare`（`==`/`!=` 两侧均为字面量）、`unreachable-code`（return/break/continue 后语句，含 `loop` 体内 break 后）；诊断格式 `line:col: level[rule]: message`；CLI `zeta check <file>`（有诊断 exit 1）
- **D2 — driver 挂载**：`zeta-driver` 增加 `fmt`/`check` 子命令（复用 tools 库，`check_source_file` 仅静态分析不跑流水线）；两个 tools crate 各带独立 binary + 根 workspace 依赖表登记
- **D2 测试固化**：`zeta-fmt` 9 单测（round-trip 再解析 / 幂等 / 顶层空行 / 优先级括号 / 浮点小数点 / 字符串转义 / 时间字面量 / parse-error / actor+region）、`zeta-check` 12 单测（每规则正反用例 + 遮蔽 + 参数 + self 豁免 + 干净代码零诊断）、`crates/zeta-driver/tests/fmt_check_test.rs` 5 集成（真实源码格式化 round-trip 幂等、格式化后**编译运行语义不变**、干净代码零诊断、规则报告与行号、parse-error）；`zeta test` 13/13 全绿 + 全量回归通过
- **D3 — `zeta doc` 文档生成器**：新增 `tools/zeta-doc`（`DocOptions { title }`；`doc_source` 解析源码为 AST 后按源码位置提取 `///` 文档注释——连续行合并为一个文档块，允许以空行/普通注释间隔，紧邻在顶层项之前即关联；`//!` 行作为文件级文档渲染在标题下方；无注释的项仍以签名+位置出现在目录与正文，保证 API 目录完整）；输出 Markdown：标题 + 文件级文档 + 目录（分类标签 + 简短标题）+ 按类别分组正文（函数/结构体/枚举/Trait/impl/Actor/常量/模块/其他），每项渲染「标题、```zeta 签名代码块、`> 位置: line:col`、文档正文」；聚合类型附成员列表（trait/impl 方法签名、actor 字段与方法、mod 子项）；**签名重建**独立实现（`item_signature`/`fn_signature`/`fmt_type`/`fmt_param`，`pub`/`async`/`extern` 修饰符、泛型参数、self 接收者 `&self`/`&mut self` 特判——`self: &Self` 无法再解析）；CLI `zeta doc <file.zeta> [--out <file.md>] [--title <标题>]`
- **D3 — `zeta bench` 基准测试框架**：新增 `tools/zeta-bench`（纯 std，无外部依赖；`BenchOptions { warmup, runs, quiet }` 预热轮数不计统计 + 测量轮数；`bench_executable` 多次启动进程 `Instant` 计时，非零退出码报错；`bench_source` 接受编译回调先编译再计时）；`BenchReport` 统计平均/中位数/最小/最大/样本标准差/吞吐（ops/s），`Display` 输出类 criterion 摘要；CLI `zeta-bench <file.zeta|exe> [--runs N] [--warmup N] [--out <路径>] [--quiet]`，源码模式自定位 `zeta build`（复用当前二进制或 PATH）编译到临时目录
- **D3 — driver 挂载**：`zeta-driver` 增加 `doc`/`bench` 子命令（`doc_source_file` 仅做文档生成不跑编译流水线；`zeta bench <file.zeta> [-o <out>] [--runs N] [--warmup N] [--cache-dir <dir>] [--force]` 复用 `build_file` 编译后交 `zeta_bench::bench_executable` 计时）；`DriverError` 新增 `Doc` 变体；两个 tools crate 各带独立 binary + 根 workspace 依赖表登记
- **D3 测试固化**：`zeta-doc` 5 单测（相邻 `///` 合并 / `//!` 与 `///` 分离 / 空白与普通注释行可间隔关联 / 代码行阻断关联 / self 参数签名）、`zeta-bench` 3 单测（统计计算含中位数偶数样本 / 单样本与空样本 / 时长格式化）、`crates/zeta-driver/tests/doc_bench_test.rs` 9 集成（doc 注释+签名+位置提取、无注释目录、parse-error 报告、driver 文件 API 正反用例、bench 统计合理性、编译产物计时、`bench_source` 编译回调链路、缺失产物报错）；`zeta doc` 对真实标准库 `core.zeta` 试运行输出完整目录（String/Option/Result/Vec/HashMap/Duration/Instant/io/FFI 符号）；全量回归通过
- **E1 — `--target` 交叉编译（macOS 双架构基础）**：编译流水线保持目标无关（LLVM IR 文本 → clang 汇编/链接），交叉编译 = 在链接阶段注入目标 triple；`zeta-driver` 新增 `host_triple()`（本机 LLVM triple，macOS/Linux/Windows 三平台映射）/`target_arch()`（`arm64`/`aarch64` 归一）/`is_cross_target()`；`assemble` 接受 `Option<&str>` target——有 target 时 clang 加 `--target=<t>`；**Actor 运行时跨架构跳过**：本机 staticlib 无法链接到其他架构（`is_cross_target` 判断 + stderr 提示，actor 程序暂不支持交叉编译）；公共 API 新增 `build_executable_with_target`/`build_executable_file_with_target`（旧 API 委托 None 保持兼容）；`IncrementalDriver.with_target`；CLI `zeta build <file> --target <triple>`（`zeta run` 拒绝 `--target`——交叉产物无法本机运行）；验证：同一 `hello-world.zeta` 编译出 `arm64-apple-macosx`/`x86_64-apple-macosx` 两个 Mach-O，arm64 本机运行 + x86_64 Rosetta 运行均输出 `Hello, Zeta!`，`file` 检查架构正确；Windows/ARM 目标（链接器与库路径）与 `sockaddr_in4` 平台布局（Linux 无 sin_len）留待后续
- **E1 测试固化**：`crates/zeta-driver/tests/cross_compile_test.rs` 6 集成（`target_arch`/`host_triple`/`is_cross_target` 归一化正反用例、本机 target 编译+运行验证、无 target 兼容性、无效 target 编译失败、macOS 双架构产物 `file` 校验、带 std 特性程序（Vec）交叉编译+Rosetta 运行）；全量回归通过
- **E1 平台内建（消除平台相关假设）**：Zeta 语言无 `#[cfg]` 属性机制（词法层无 `#` token），改用**驱动注入平台内建**：`zeta-driver` 新增 `target_os_code(target)`（triple→OS 码：0=未知 1=linux 2=macos 3=windows 4=freebsd，`None`=主机）+ `platform_builtin_ir` 在 assemble 写盘前注入 `define internal i32 @__zeta_target_os()`（ret 目标 OS 码）；**codegen 对 `__zeta_` 前缀 extern 跳过 declare 生成**（避免同符号 declare+define 冲突）；`core.zeta` 声明 `extern fn __zeta_target_os() -> i32` 并把 `sockaddr_in4` 重构为 `sockaddr_in4_with_layout(has_sin_len, ...)` 双布局（macOS：0=sin_len(16)+1=AF_INET(2)；Linux：0-1=sin_family 小端 0x0002，无 sin_len），`sockaddr_in4` 按 `__zeta_target_os() == 2` 自适应——消除 CODEBUDDY.md 标注的「Linux 无 sin_len 待 cfg 支持」遗留；验证：双布局字节序列（16,2,31,144,... / 2,0,31,144,...）、平台内建 host 端到端调用返回主机 OS 码、现有 `net_socket_test` macOS 布局不变
- **E1 测试固化**：`crates/zeta-driver/tests/platform_builtin_test.rs` 4 集成（target→OS 码映射含主机/主机 triple、平台内建端到端接线、`sockaddr_in4_with_layout` 双布局字节序列、双布局尾部字节一致性）；全量回归 104 套件通过

- **D1 — `zeta test` 子命令**：新增 `zeta-driver::test_runner` 模块（`run_test_suite(root)` 扫描 `compile-pass/`/`compile-fail/`/`run-pass/` 三个子目录：compile-pass 编译到 LLVM IR 成功即过；compile-fail 要求编译失败，源内 `// expect: <片段>` 注释断言错误消息包含片段；run-pass 编译运行成功，同名 `.out` 文件作为期望 stdout 精确对比；用例目录可选缺失即跳过、文件按名排序保证结果确定性）；CLI `zeta test [<tests-dir>]`（默认 `./tests`）逐用例打印通过/失败 + 汇总 + 失败时非零退出码
- **D1 用例**：`tests/compile-pass/`（hello / arith / struct-trait / modules / actor 共 5 个）、`tests/compile-fail/`（type-mismatch / undefined-var / unknown-field / undefined-fn / no-method 共 5 个，均带 `// expect:` 断言）、`tests/run-pass/`（hello / arith / actor-ping-pong 共 3 个 + `.out` 期望文件，字节精确写入）
- **D1 cargo 测试矩阵**：`crates/zeta-driver/tests/suite_test.rs`（`zeta_test_suite_all_pass` 断言 `tests/` 全部通过；`zeta_test_suite_covers_all_kinds` 断言三类用例均有覆盖），`cargo test` 自动驱动目录用例
- **D1 连带修复 ① — 模块项 pub 可见性**：`parse_item` 的 `Pub` 分支原仅识别 `pub mod`，其余一律按函数解析（`pub const PI` 报 `expected 'fn', found Const`）→ 改为按实际关键字分派到 mod / const / static / struct / enum / trait / impl / use / actor；`pub fn`（含 async/unsafe/extern 前缀）**不预消费 `pub`**、交回 `parse_fn` 自行处理以正确记录 `is_pub`（防止 `test_complete_function` 回归）
- **D1 连带修复 ② — use 导入常量短名解析**：`lookup_constant` 原仅按裸名查表（`use math::PI; println(PI)` 报 `undefined variable `PI``）→ 支持短名 → 完整名解析（裸名失败后回退 `use_aliases`），`resolve_full_name` 增加 `constants` 直接命中分支（与 struct/actor 一致）

### 阶段 E — 多目标与发布

| 任务 | 内容 | 状态 |
|------|------|------|
| E1 | 交叉编译：macOS / Windows / ARM 目标（消除平台相关假设） | 🔧 大部分完成（`--target` 注入 clang + 双架构验证 + 平台内建消除 `sockaddr_in4` 布局假设；Windows/ARM 链接器与库路径待补） |
| E2 | WASM 目标支持（M2.7） | ✅ 完成（`wasm_target_test.rs` 3 用例全绿：triple 判定 + hello world + std 特性数组/String/位运算；工具链 wasm-ld/wasmtime/wasi-libc 已装；`zeta build --target wasm32-wasi` → wasmtime 运行与主机目标输出一致） |
| E3 | 发布流程：`zep` 注册表版本信息、CI 多平台产物（`.github/workflows/release.yml`） | ✅ 完成（`zep publish` 重复版本保护 + `zeta publish` CLI + release.yml 四平台矩阵 + `CHANGELOG.md` v0.1.0） |

### 阶段 F — 编译器深度

| 任务 | 内容 | 状态 |
|------|------|------|
| F1 | LSP 服务器（M2.4）：IDE 支持 | ✅ 完成（MVP：新 crate `zeta-lsp`，JSON-RPC over stdio + full 文档同步 + 诊断推送；`zeta lsp` 命令） |
| F2 | PGO 数据回灌编译流程（M2.8 后半）：`.zeta_profile` → 区域大小预测 | ✅ 完成（`zeta profile` 命令 + `zeta build --profile` 编译期注入：加载画像 → PgoAdvisor p95×1.1 建议 → CompilerInterface 报告；语言级 region 接线后可回灌 `region 'r adaptive` 初始容量） |

---

## 4. 执行记录

### 2026-08（阶段 A + B1/B5）

- [x] **A1** 用户级 match 泛型聚合载荷修复（`option_result_test.rs::user_level_match_string_payload` 8 断言）
- [x] **A2** Infer 枚举自动定型（`option_result_test.rs::bare_enum_infer` 5 断言）
- [x] **A4** 通用 FFI `extern fn`（`ffi_extern_test.rs` 3 用例）
- [x] **B1** `Duration`/`Instant` 时间模块（`time_test.rs` 3 用例）
- [x] **B5** `Vec` 补充方法 + `HashMap` `len`/`is_empty` 固化（`vec_more_ops_test.rs` 5 用例 + `hashmap_len_test.rs` 4 用例）
- [x] **B2** io 模块：libc stdio 文件 IO + `read_file`/`write_file`/`append_file`/`c_str`/`read_line`；编译器 extern `String` 参数取 data 指针（`io_file_test.rs` 9 用例）
- [x] **位运算全链路**（B3 前置依赖）：HIR `HirBinaryOp` 5 新变体 + typecheck 真正映射（原为 placeholder 显式 Unsupported）+ MIR const_fold 折叠 + LIR 类型推断统一 I64 + codegen `and`/`or`/`xor`/`shl`/`ashr`；`bitwise_test.rs` 6 用例
- [x] **B3** net 模块：`hostname()` + `htons` + `socketpair_stream`/`fd_at`/`send_all`/`recv_some`/`sockaddr_in4`/`tcp_connect`（`send`/`recv` 为 actor 保留字，extern 用 `r#` 原始标识符）；`net_socket_test.rs` 6 用例
- [x] **B4** sync 模块：`Mutex`/`RwLock`（pthread extern + `calloc` 承载，`trylock` 系列返回 `int` 依赖编译器 extern `i32` 返回支持）；顺带修复内联 pass 局部变量重命名 bug（`map_local` 对非参数名字一律重命名）；`sync_test.rs` 6 用例
- [x] **E2** WASM 目标支持（M2.7）：工具链安装（Homebrew `lld` 22.1.8 含 `wasm-ld`、`wasmtime` 48.0.0、`wasi-libc` sysroot 于 `/opt/homebrew/opt/wasi-libc/share/wasi-sysroot`）；`zeta build --target wasm32-wasi` 全链路验证（hello-world / arith-print 与主机目标输出一致）；`wasm_target_test.rs` 3 用例（triple 判定 + hello world + std 特性数组/String/位运算）；关键适配：WASI 入口重命名 `main` → `__main_argc_argv(i32, i8**)`（crt1 链 `_start` → `__main_void` → `__main_argc_argv`）、`-nostdlib` 手动链接 `crt1.o` + `libc.a`、wasi-libc 33 多目标布局、`adapt_wide_int_args` malloc/memcmp 位宽适配（`i64` → `i32`，寄存器实参前插 `trunc i64→i32` 指令——LLVM 禁止 call 实参内嵌 trunc 表达式）
- [x] **E3** 发布流程：`zep publish` 重复版本保护（`PackageIndex::find` 命中即报错，阻止静默覆盖已发布版本；实测：0.1.0 发布 → 重复发布报错 → 提升 0.2.0 再发布成功）；`zeta publish` CLI（`zeta-driver` 依赖 zep lib 委托 `cmd_publish`，`--registry`/`--verbose`，重复版本拦截 exit=1）；`.github/workflows/release.yml` Windows x86_64 平台（`x86_64-pc-windows-msvc` + `.exe` + `shell: bash`），矩阵 4 平台；`CHANGELOG.md` v0.1.0 首个版本记录
- [x] **F1** LSP 服务器（M2.4）：新 crate `zeta-lsp` + `zeta lsp` CLI（JSON-RPC 2.0 over stdio，自研 `jsonrpc.rs` 无外部 LSP 依赖；full 文本同步 didOpen/didChange/didClose + 诊断推送复用 zeta-check；initialize 声明 capabilities，shutdown/exit 生命周期，未知请求 -32601）；`lsp_e2e_test.rs` 2 进程测试全协议往返
- [x] **F2** PGO 数据回灌（M2.8 后半）：`zeta profile <file.zeta_profile>`（加载画像 JSON → `PgoAdvisor::recommend_size` p95×1.1 下限 64KiB → `CompilerInterface` 报告）+ `zeta build --profile <file>` 编译期注入（失败仅告警不阻断）；`profile_cmd_test.rs` 6 用例
- [x] **遗留修复**：`zeta new <name> [--lib]` 命令（委托 zep `cmd_new`，Zeta.toml + src/main.zeta|lib.zeta）；`Vec<T>` 动态切片 `v[lo..<hi]`/`v[lo...hi]`/`v[lo<..hi]`（std 泛型方法 `Vec::slice` + check_slice 实例化，越界 clamp）；数组 `[T; N]` 动态切片（typecheck 展开为 Vec 拷贝循环 + push 实例化 + 边界 clamp）；`String::from(s)` 支持绑定字面量的变量（`TypeContext.local_inits` 追踪）；`dynamic_slice_test.rs` 7 用例 + 全量回归 111 套件全绿

**验证基线**：`cargo test --workspace` 111 套件全绿；`cargo clippy --workspace --all-targets` 0 警告。

---

## 5. 跟踪与验收约定

1. 每个阶段/任务完成须满足：`cargo test --workspace` 全绿 + `cargo clippy --workspace --all-targets` 0 警告。
2. 涉及运行时/并发类测试（Actor、通道、锁、`join`）须按 `design/00_项目总览.md` 硬性规则带超时保护，挂起即视为失败。
3. 任务完成后同步更新：本文档状态标识 + `CODEBUDDY.md`（§5.5 执行记录 / §6 里程碑）。
4. 优先交付顺序：B2/B3（io/net）→ C（Actor 接线）→ D（工具链）→ E/F（多目标/深度）。

---

## 附录 A：实现纪要（编译器后端与工具链）

> **说明**：本节提炼自开发任务书（原 `prompts/P007`/`P008`/`P011`/`P013`，2026-08-24 归档至
> [`design/prompts/`](design/prompts/)），记录编译器后端与工具链的落地状态、关键决策与遗留问题。
> 相关模块设计稿见 [`design/08_编译器后端与代码生成.md`](design/08_编译器后端与代码生成.md) 与
> [`design/09_工具链设计.md`](design/09_工具链设计.md)。

### A.1 MIR 中间表示（对应 P011，2026-08-20 ✅）

- **落地**：`zeta-mir` 从占位 crate 落地为真实实现——`lib.rs`（数据结构：`MirProgram`/`MirFunction`/
  `BasicBlock`/`MirStmt` 三地址指令/`MirTerminator`）+ `lower.rs`（HIR → CFG）+ `passes/`（优化流水线
  **常量折叠 → DCE → 小函数内联 → DCE**）。
- **区域操作显式化**：`RegionEnter`/`RegionExit`/`AllocInRegion`/`Transfer` 为一等指令。
- **关键 bug 修复**：`new_block()` 切换当前块导致分支块创建后终止符发射错位——修复为创建前记录 entry、
  创建完毕切回再发射终止符；DCE 误删副作用 `Call`（`print`/`println`）——修复为 Call 永不删除、参数标记活跃。
- **落地偏差 / 已知限制**：
  - **简化 SSA**：未引入 φ 节点，各分支写同一结果变量、merge 块读取，模拟 φ 语义；
  - `break`/`continue` 边界未校验（非循环上下文 `break` 会跳转到失效块 id，待 typecheck 补上下文校验）；
  - `transfer` 后仍可使用变量（use-after-transfer 依赖借用/区域检查器，未接线）；
  - 循环条件含 `?` 提前返回会使 entry 块未终止（边缘情况，MVP 接受）；
  - **MIR 未接线到 driver 主流水线**（M1.8 代码生成阶段补齐，见 A.4）。

### A.2 增量编译引擎（对应 P007，2026-08-20 ✅，务实版）

按**当前编译器实际能力**（单文件编译模型）落地为"务实版增量编译"，与原始设计偏差如下：

| 本文档设计 | 实际落地 | 说明 |
|-----------|---------|------|
| 模块级缓存（`modules/`，HIR/MIR 序列化） | 单文件产物缓存（`artifacts/{hash}.ll`，缓存 LLVM IR 文本） | 序列化全 IR 类型成本高；缓存最终产物同样跳过全流水线 |
| 缓存 HIR/MIR/LIR 各级 IR | 只缓存 LLVM IR 最终产物 | 命中时跳过 parse/typecheck/borrowck/regionck/MIR/LIR/codegen 全部环节 |
| 并行编译（rayon） | 未引入 rayon | 单文件模型无并行对象；依赖图与受影响传播已实现并单测，待多模块接线 |
| `use` 依赖提取 | `deps.json` 暂未生成 | 依赖图 API 已就绪（`add_edge`），依赖提取待模块系统落地 |
| 可执行文件缓存 | 只缓存 LLVM IR | 命中后仍执行 clang（毫秒级）；exe 缓存为后续优化项 |

**实际能力**：源码 SHA-256 + 模块接口哈希（函数签名，忽略函数体变化）；多版本产物缓存（ccache 语义）；
跨进程持久化（`index.json` + `artifacts/`）；损坏自动全量重建；`clean_stale` 过期清理；依赖图拓扑排序/
受影响传递闭包/循环检测；CLI `zeta run|build <file> [--cache-dir|--force|--verbose]`。
**实测**：`arith-print.zeta` 全量 152ms → 增量命中 67ms（2.3x）。
**实现文件**：`crates/zeta-driver/src/incremental/{hash,depgraph,cache}.rs`。

### A.3 包管理器 Zep（对应 P008，2026-08-20 ✅）

- **落地**：`zep/` crate——`zeta new` 项目初始化（`Zeta.toml`：`[package]`/`[dependencies]`/`[dev-dependencies]`/
  `[build-dependencies]`/`[profile.*]`/`[workspace]`，支持 `path` 本地依赖）+ **PubGrub 依赖解析** +
  本地/HTTP 注册表 + 打包解包 + 构建驱动。
- **配置要点**：版本约束 `serde = "1.0"`；feature 声明 `tokio = { version = "2.0", features = ["full"] }`。
- **质量**：29 项测试全过。

### A.4 LLVM 后端与代码生成（对应 P013，2026-08-20 ✅，M1.8/M1.9）

- **设计决策（ADR）**：

| ADR | 决策 | 理由 |
|-----|------|------|
| ADR-LLVM-001 | 非 SSA 槽式存储（`alloca` + `load`/`store`） | 天然支持分支合并，LLVM `-O1` 自动 mem2reg 提升 |
| ADR-LLVM-002 | LLVM IR 文本直出（不经 inkwell C API） | 零额外依赖、编译期快、错误定位直接 |
| ADR-LLVM-003 | 临时值一律命名寄存器 `%rN` | 规避裸数字与命名寄存器编号冲突 |
| ADR-LLVM-004 | `main` 特化为 `define i32 @main()`，`ret i32 0` | C 风格入口需要；MVP 不支持退出码 |
| ADR-LLVM-005 | 区域指令后端忽略 | MVP 分配语义由 `zeta-region-alloc` 运行时提供 |

- **流水线**：`compile_to_llvm`（lexer→parser→typecheck→borrowck→regionck→MIR(优化)→LIR→LLVM IR）→
  `build_executable`（IR 落盘 → clang `-O1` 汇编链接）→ `run_source`（执行并捕获输出）。
- **内建打印**：`print`/`println` → `printf`，格式串按类型分派（`%s`/`%lld`/`%f`/`%c`）；
  布尔经 `select` 选 `"true"`/`"false"`；字符 `zext i8 → i32`。
- **遗留问题**：用户函数重定义检查（当前 `sigs` 后写覆盖）；全局变量/常量支持；`&` 借用 → LLVM 指针语义；
  递归深度校验/栈溢出保护；WASM 目标（M2.7，部分已由 L4 平台加固覆盖）。

---

> **维护者**：Zeta Language Team
> **最后更新**：2026-08-22
