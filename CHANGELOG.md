# Changelog

本项目所有重要变更均记录于此文件。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

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
