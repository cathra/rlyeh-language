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
