# Changelog

本项目所有重要变更均记录于此文件。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased] - 2026-08-23

### 新增

- **region 选项接线（L3）**：语言级 `region` 选项完整语法（`strategy (bump)`，AST `RegionStrategy`/parser `region.rs`）+ region 指令（`RegionEnter`/`AllocInRegion`/`RegionExit`/`Transfer`）从"LLVM 后端忽略"转变为调用 `zeta-region-alloc` C ABI 运行时（`zeta_region_enter`/`zeta_region_alloc`/`zeta_region_transfer`/`zeta_region_exit`，crate 增 `crate-type = ["rlib", "staticlib"]`；`InRegion.size` = `type_slot_count × 8`，聚合对象 bump 分配 + memcpy 值镜像浅拷贝，区域退出批量释放）；PGO 画像回灌 `adaptive` 初始容量（`--profile` → `region_hints` 注入）；driver 链接 region 运行时静态库。测试 `tests/run-pass/region_alloc.zeta`（10 输出：标量/聚合 `in 'r`、`with_size`/`adaptive`/`strategy (bump)`、transfer 移出、跨区域引用）；全量用例全绿 + cargo test 全绿。关键修复：Apple clang 21 不认 `%r = getelementptr inbounds ([N x i8], ...)` 独立指令（报 `expected type`）→ 独立取地址改用 `emit_string_global_ptr`（bitcast 指令形式）。
- **可选 GC `Gc<T>`（K4，MVP 保守标记-清除）**：`Gc::new` 编译器内建 + `gc_region` 生命周期块（desugar 为 `zeta_gc_region_begin`/`zeta_gc_alloc`/`zeta_gc_escape`/`zeta_gc_collect`）+ 逃逸对象 root 登记 + 嵌套块存活链式提升 + 字段/方法/索引自动剥层（与 `Box` 同构）；独立运行时 crate `zeta-gc-runtime`（对象 = `slot_count(T)` 个 8 字节槽连续堆块，`T` 值区自堆首槽起，与 `Box<T>` 同构布局；epoch 分层块生命周期，块外对象永不回收——MVP 泄漏语义）。测试 `tests/run-pass/gc_region.zeta`（15 行输出）+ `tests/compile-fail/gc_bad.zeta`（`Gc::new()` 参数个数断言）；全量 36 用例全绿 + cargo test 全绿。
- **trait 对象 `dyn Trait`（H4，MVP）**：`dyn Trait` 类型（parser `dyn` 关键字分支 + AST `AstType::Dyn` + typecheck `Type::Dyn`）+ `&T` → `dyn Trait` 强制转换（`coerce_to_dyn`：运行时构造 vtable——drop/size/align 槽 MVP 置 0 + 方法表按 trait 声明序入表，+ 2 槽胖指针 = 数据指针 + vtable 指针）+ 方法调用 vtable 间接分派（`FieldGet` 数据/表指针 + `Index(3+idx)` + `CallIndirect`，同一签名分派到不同 impl）；`dyn Trait` 作形参/局部/胖指针拷贝可用；MVP 限制：trait/impl 非泛型、含 `Self` 签名方法不可经 dyn 调用。测试 `tests/run-pass/dyn_trait.zeta`（6 输出：转换/多态分派/胖指针拷贝/带参方法）；全量 40 用例全绿 + cargo test 全绿。
- **闭包剩余功能（H5 补全）**：① **返回闭包的函数**——`fn make() -> fn(i64) -> i64 { |x| x + 1 }`（尾闭包按 H2 无捕获闭包签名检查生成函数指针返回，`check_block_with_expected_final` 传入预期 final 类型）；无捕获/未固化闭包值变量作返回值（`try_closure_value_as_fn` 降级为 fn 指针 / `fix_deferred_closure_with_sig` 按返回签名固化参数类型）。② **非注解/半注解闭包值绑定**——`let f = |x| x + 1; f(41);`（绑定处注册延迟闭包 `ctx.deferred_closures`，参数类型规则：有注解用注解类型、无注解由首次调用点实参推断；捕获对象内联构造并重绑定变量，完整闭包类型回写，从未调用则惰性不检查）。③ **无捕获闭包值作 fn 实参**——`apply(f, 41)`（降级/固化为函数指针）；有捕获闭包值经 fn 签名传递报 Unsupported/类型不匹配（跨函数边界 MVP 限制）。测试 `tests/run-pass/closure_fn_return.zeta`、`closure_value_half_anno.zeta`（+`.out`）+ `tests/compile-fail/closure-value-{anno-conflict,capture-fnarg}.zeta`；全量 62 用例全绿 + cargo build 无警告。
- **闭包测试补充与全量验证**：新增 4 用例——`tests/run-pass/closure_lazy.zeta`（+`.out`：从未调用闭包惰性不检查，闭包体含未定义变量/类型错误也通过）+ `tests/compile-fail/closure-value-{capture-return,capture-return-unfixed,body-undefined}.zeta`（已固化捕获闭包值作返回值 → TypeMismatch / 未固化捕获闭包值作返回值 → Unsupported 跨函数边界 / 被调用闭包体内未定义变量报错，与惰性形成对照）。集成测试 `zeta-driver test tests` 66/66 全绿；`cargo test --workspace -- --test-threads=1` 118 个 test target 全绿（driver 集成测试并行 spawn clang 会进程资源耗尽 os error 35 导致偶发失败，CI 同步改为串行测试线程）。

- **严格借用检查（G1 收尾，zeta-borrowck）**：借用排他性规则启用（此前为防御性变体）——`&mut` 与任何活跃借用互斥、多个 `&mut` 互斥、活跃可变借用期间写入被借用变量报 `BorrowConflict`（Rust E0502/E0499 对应）；`&mut` 要求 `let mut` 绑定报 `BorrowMutImmutable`（E0596）；局部引用逃逸函数（尾表达式 / `return` 返回 `&x` 或绑定引用变量）报 `DanglingReference`（E0597，参数来源引用允许返回）。NLL 近似：借用存活范围 = 语句粒度 `born..=last_use`，`last_use` 由预扫描阶段按同一遍历顺序预先收集。语义有意宽松：读取被借用变量与经 `*p` 写入允许（裸指针别名合法），共享借用（多个 `&`）可共存，仅直接赋值被借用变量触发冲突。测试 `tests/compile-fail/borrow-mut-immut.zeta`、`borrow-conflict-{assign,mutmut,shared-mut,mut-shared}.zeta`、`dangling-return-{ref,var}.zeta`（7 用例）+ `tests/run-pass/borrow_pass.zeta`（10 输出）；全量用例全绿 + cargo test 全绿。

- **发布级优化提升（性能基准）**：汇编管线 native 与 WASM 统一 `clang -O3`（此前 -O2；-O3 = -O2 + 更强内联/循环变换，生成 IR 不含 noalias/nsw/nuw 标注，优化保守安全）。`String::push_str(字面量)` 方法级整体特判（`check_method_call`）：改调新增 `String::push_bytes(src, n)` 快速路径（`core.zeta`：`while cap - len < n { grow() }` + 逐字节拷贝），免去字面量实参每次 `alloc_bytes` + 拷贝 + 对象分配的深拷贝；`src` 实参传 `&__lit`（Str 标量槽地址，AddrOf 标量分支生成「指向 data 指针槽的指针」= 单槽伪对象头，与 `&str` 的 String 对象指针表示在 `check_index` 的 FieldGet 槽 0 语义下完全一致）。基准效果（runs=7，Zeta vs C）：strcat 2.43ms vs 3.03ms（反超 C；此前深拷贝路径慢数量级）、matmul 持平/反超 C、sort 持平、fib 1.07x；`ZETA_KEEP_TMP=1` 保留汇编临时目录（native + WASM 路径，调试用）。
- **WASM 目标 `calloc` 位宽适配（driver 汇编管线）**：Zeta IR 以 `i64 @calloc(i64, i64)` 声明/调用，而 wasi-libc 的 `calloc` 为 32 位指针签名（`i32 @calloc(i32, i32)`），此前仅 `malloc`/`memcmp` 被适配——凡用聚合分配（`LirStmt::Alloc`/`alloc_array`/`alloc_bytes`）或标准库直调（sync 模块 `calloc(1, N)`）的 WASM 程序，clang 编译时对「i64 结果 ← i32 返回」插入 bitcast 非法陷阱（`Lcalloc_bitcast_invalid`，wasmtime trap）。新增 `adapt_calloc_wasm32`：声明与调用点结果类型降为 i32、两参数降宽（寄存器实参经 `trunc` 包装）、紧随 `inttoptr i64 %r to i8*` 降为 i32、标准库直调的 i64 槽存储经 `zext i32 → i64` 回填；修复 `tests/run-pass/actor-ping-pong.zeta` 等 3 个 WASM 用例（此前因缺 wasm actor runtime 库而跳过、工具链补齐后暴露）。
- **测试断言更新（zeta-driver `agg_test.rs`）**：聚合对象分配自 `calloc` 清零优化后由 `call i8* @malloc` 变为 `call i64 @calloc`，`compile_agg_llvm` 断言同步更新。
- **region 分配内联化与字面量直接构造（L3 性能）**：① **内联 bump 快路径**——`zeta-region-alloc` 的 `Region` 改为 `#[repr(C)]` 首部固定偏移快路径字段（`base`+0 / `cursor`+8 / `limit`+16），bump 状态权威上提到 Region（`MemoryBlock`/`BumpAllocator` 瘦身为纯块管理，`Region::try_bump` 与 codegen 公式逐位一致）；codegen 对 `in 'r` 聚合分配内联生成 `load cursor/limit → 对齐 → 越界分支`，仅当前块空间不足才 `call @zeta_region_alloc` 扩容（LLVM label 分支 + entry 预分配结果槽跨块汇合，规避 SSA phi）。② **字面量直接构造**——新增 LIR 语句 `AllocInRegionDirect` + codegen 变换 `inline_region_literal`：`Alloc + FieldSet* + AllocInRegion`（含别名链与字段值计算穿插）重写为「区域指针上直接逐字段构造」，消除每次分配的中间堆临时（malloc）与值镜像 memcpy，同时修复 `in 'r` 变量仍指向堆临时对象的低效（`zeta_region_stats` 的 allocs 语义改为慢路径 + Rust API 计数，快路径不维护统计换取热路径性能）。基准 region_alloc（100 万次 32B 分配）：Zeta 14.21ms → ~7.2ms（vs C 2.4–3.7ms，比值 5.92x → ~2.5x，提升 ~50%）；`tests/run-pass/region_alloc.zeta` 与全量套件全绿。

### 修复

- **引用返回函数被误判为 `i64`（zeta-lir）**：`DerefRead`/`DerefWrite` 类型推断未将 `base` 注册为 `Ptr`，引用返回函数（如 `max_ref`）被声明为 `i64`，调用点解引用产生垃圾值；现 `DerefRead`/`DerefWrite` 推断同时注册 `base` 为 `LirType::Ptr`。
- **`print`/`println` 内建对引用参数不剥层**（zeta-typecheck `check_call`）：`println(r)` 直接打印引用地址而非解引用值；现引用参数自动剥一层（`Type::Ref` → `HirExpr::Deref`），且非 String 实参在剥层后提前返回，避免落入通用路径重新推断丢失剥层结果。
- **`zeta-gc-runtime` 在 C 主程序环境的崩溃（SIGKILL / `_os_unfair_lock_unowned_abort`）**：运行时全部动态内存改用 `libc::malloc`/`libc::free`（对象块 + header/root 元数据链表），弃用 Rust 堆分配（`RawVec`/`Vec` 扩容）与 `libc::realloc`——macOS 实测这些分配在先前 `libc::malloc` 之后调用会触发 `libsystem_malloc` 的 `mfm_alloc` 内部锁崩溃；全局状态由 Mutex 改为 `SyncUnsafeCell` 单线程无锁调用约定。
- **`zeta_gc_escape` 传参错误致逃逸对象被误回收**（编译器 `check_gc_region`）：escape 参数由 Gc 包装指针改为经 `heap_ptr_hir` 解包装的对象基址（与 `zeta_gc_alloc` 注册一致），修复 `mark` 线性查找失配导致的悬垂读取。
- **嵌套 `gc_region` 中外层逃逸对象被误回收**（`collect` 存活提升）：提升条件由 `marked && epoch == s.epoch` 改为全部被标记对象，确保嵌套块释放旧 root 后外层逃逸对象（`epoch < s.epoch` 存活但未标记可达）仍受保护。
- **按值（栈内联）聚合判定不安全致 `tcp_addr` SIGSEGV**（zeta-typecheck `check_expr.rs`）：`struct_by_value`/`enum_by_value` 此前仅查槽数（struct ≤2 字段 / enum ≤2 槽），不查字段类型——按值对象以 `[2 x i64]` 栈槽存储，当其作为另一聚合（enum/struct）的字段/payload 时以"对象地址"语义写入外层 Ptr 槽；若该对象含聚合字段（如 `SocketAddr { ip: String, port: i64 }`，`Result<SocketAddr, _>::Ok(sa)`），写入的是**内部对象栈地址**，函数返回/跨调用后悬垂 → 后续 String 操作读垃圾指针（`strnlen` 崩溃于 0x200）。修复：按值判定增加安全约束——① 对象所有字段须为**具体标量槽**（`field_is_scalar_slot`：非泛型/未推断 + `field_scalar_of != Ptr`）；② **泛型枚举（`Option`/`Result` 等）保守禁用按值**——定义时无法预知类型实参是否聚合，且同一枚举 `None`/`Some` 等各构造路径须判定一致（退化回 calloc 堆分配，指针稳定）。`Ipv4Octets`（4 标量字段）等纯标量聚合不受影响；全量 `suite_test` 回归全绿；基准重测无性能退化（hashmap 11.09 / hashmap_str 5.30 / strcat 3.29 ms，与修复前持平或更好）。

## [0.1.0] - 2026-08-22

首个公开版本。Zeta 编译器、标准库与工具链的里程碑能力汇总。

### 新增

- **语言核心**
  - 完整编译管线：lexer → parser → typecheck → borrowck → regionck → MIR → LIR → LLVM 代码生成
  - 类型系统：结构体/枚举/泛型/泛型方法、Option/Result 集成、类型推断
  - 借用检查与区域检查、智能区域分配器（静态大小推断 + PGO 画像/推荐 + EWMA 自适应扩容）
  - Actor 语言级接线：`actor` / `spawn` / `.await`（`zeta-actor-runtime`）
  - 通用 FFI：`extern fn` 声明（全链路打通）
  - 宏系统：`macro_rules!` 声明式宏（`$x:expr`/`ident`/`ty`/`tt` + `$(`...`)` 重复，parse 期 AST 展开，新 crate `zeta-macro`）+ 内置格式化宏 `println!` / `print!` / `format!` / `dbg!`（`{}` 占位、`{:?}` 同构、`{{`/`}}` 转义，typecheck desugar 为 String 拼接 + 内建打印）
  - 函数一等值（H1）：`fn(T) -> R` 函数类型、`let f = add` 函数值绑定、`f(args)` 间接调用（typecheck `Type::Fn` → HIR/MIR/LIR `CallIndirect` → LLVM `i8*` 槽 + 按签名 `bitcast` + 间接 `call`），函数值可作实参/返回值/重新绑定/类型注解
  - 无捕获闭包（H2）：`|x, y| expr` desugar 为匿名函数（`__closure_N`）+ 函数指针（复用 H1 全链路，零运行时开销）；fn 形参实参 / `let f: fn(..) = |..| ..` 注解绑定驱动参数类型推断；捕获外部变量报错（H3 规划）
  - `?` 错误传播运算符（K1）：`expr?` 在 Option/Result 上下文 desugar 为 `match` + `return` 早返回（复用 check_match 的 if-else 链 + tag 比较，零新增 HIR 节点）；支持表达式中间嵌套 `?`；裸无参变体值表达式（`return None;`）可用；非 Option/Result 类型报 Unsupported
  - 堆分配装箱 `Box<T>`（K2）：`Box::new` 编译器内建（栈 1 指针槽 + 堆 `slot_count(T)` 个 8 字节槽，标量 `DerefSet` 写堆首槽 / 聚合 `array_copy` 整槽区浅拷贝）+ `*` 解引用（标量 load / 聚合指针拷贝，与 `&T` 同构）+ 字段/方法/索引自动剥层（`Box<String>` 的 len/索引、`Box<Vec<i64>>` 的 push、`Box<Point>` 字段、嵌套 `**bb`、Box 赋值指针共享）；`Box<T>` 可作函数参数与返回值类型
  - 引用计数装箱 `Rc<T>` / `Arc<T>`（K3）：编译器内建（堆 `RcInner` 的 `T` 值区自堆首槽起 + 尾部 strong/weak 计数槽，`Rc<T>` 栈 1 槽指向 RcInner）+ `Rc::new`/`Arc::new`（值区写 + `FieldSet` 计数初始化）+ `clone`（强计数 +1 指针共享）+ `strong_count`/`weak_count`（`usize`）+ `downgrade`→`Weak<T>` + `Weak::upgrade`（强计数 > 0 返回 `Option<Rc<T>>`）+ `try_unwrap`（强计数 == 1 返回 `Result<T, Rc<T>>`）；与 `Box` 同构的解引用/字段/方法/索引剥层；`Arc` 计数槽原子性规划中
  - 迭代器与集合协议（J1–J3）：数组迭代 `for x in arr`（索引遍历，长度编译期已知）；自定义迭代器接入 `for`（`next() -> Option<T>` 方法，inherent/trait impl，desugar 为 `loop { match it.next() { Some(x) => body, None => break } }`）；适配器 `map`/`filter`/`fold`/`collect`/`take`/`skip`（数组/Vec/迭代器接收者，经 H2 无捕获闭包，内建 desugar 返回 `Vec<T>` 可链式）；parser `stmt_terminator` 补 `,`（match 臂中 `break,`/`return,`）

- **标准库**（纯 Zeta 实现，编译器注入搜索路径）
  - 集合：`Vec<T>`（索引/迭代/扩容/排序/查找）、`String`（拼接/比较/子串/查找）、`HashMap<K, V>`
  - 控制流：`Option<T>` / `Result<T, E>`
  - 并发：`Mutex` / `RwLock`（pthread 绑定）
  - IO/网络：socket / NIO / sendfile 绑定层
  - 模块化拆分：`core.zeta` 根模块 + `time` / `io` / `net` / `sync` 子模块（driver 模块展开 + use 重新导出，用户侧裸名即用；全部 extern 集中根模块保证 LLVM 链接符号一致）

- **工具链**（`zeta` CLI）
  - `zeta run` / `zeta build`（增量缓存、`--force` / `--no-std` / `--verbose`）
  - `zeta test`（`tests/*.zeta` 测试套件）
  - `zeta fmt`（AST 重建格式化，`--check` / `-w` / `--indent`）
  - `zeta check`（静态分析：未使用变量/恒常条件/冗余比较/不可达代码）
  - `zeta doc`（`///` 注释提取生成 Markdown）
  - `zeta bench`（多次计时统计，`--runs` / `--warmup`）
  - `zeta publish`（打包发布到 zep 注册表）

- **包管理**（`zep`）
  - 项目脚手架：`zep new` / `zep init`
  - 依赖解析：PubGrub 版本求解、`Zeta.lock`
  - 注册表：本地目录 + HTTP，`publish` / `search` / `download`（重复版本发布保护）

- **多目标**
  - 交叉编译：`--target` 注入 clang（macOS arm64 / x86_64 已验证）
  - WASM 目标：`--target wasm32-wasi` 编译 + wasmtime 运行验证（wasi-libc 链接、入口适配、位宽适配）

### CI / 发布

- GitHub Actions：CI 全量测试 + WASM 冒烟；Release 四平台产物（macOS ARM64 / x86_64、Linux x86_64、Windows x86_64）

### 说明

- 0.1.0 之前为开发迭代期，未单独发布版本；具体执行记录见 `CODEBUDDY.md` 与 `docs/development-plan.md`。
