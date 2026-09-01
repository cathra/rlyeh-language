# 4. 实战：创建并发布一个项目

> 本章目标：用 `rlyeh new` 建立一个**真正的项目**（而不是单文件脚本），学会项目结构、代码格式化/检查/文档、基准测试、依赖管理与发布。读完你就能从零开始维护一个可发布的 Rlyeh 工程。

---

## 4.1 为什么需要"项目"而不是单文件

[上一章](./03-first-program.md)我们用单文件 `hello-world.rl` 跑了程序。但实际工程会拆成多个模块、有第三方依赖、要测试、要发布版本。Rlyeh 用 `rlyeh new` 生成标准项目骨架：

```bash
rlyeh new myapp          # 生成 Rlyeh.toml + src/main.rl
cd myapp
rlyeh run src/main.rl    # 编译并运行
```

生成的结构：

```
myapp/
├── Rlyeh.toml           # 项目清单（类似 C 的 Makefile/CMakeLists，或 Rust 的 Cargo.toml）
├── src/
│   └── main.rl          # 程序入口（默认内容就是一个 main 函数）
└── (tests/)             # 测试目录（可手动建）
```

> **C 程序员的视角**：`rlyeh new` 相当于"给我生成一个最小可编译的 C 工程 + 一个构建配置"。你不用自己手写 Makefile；`Rlyeh.toml` 就是这个工程的元信息（名字、版本、依赖）。

如果要写**库**（给别人 `import` 用，而不是可执行程序），加 `--lib`：

```bash
rlyeh new mylib --lib    # 生成 src/lib.rl（入口是 lib 根，而非 main）
```

---

## 4.2 Rlyeh.toml 清单长什么样

`Rlyeh.toml` 是项目的"身份证 + 配置"。一个典型内容（具体字段以你生成的为准）：

```toml
[package]
name = "myapp"
version = "0.1.0"
edition = "2026"

[dependencies]
# foo = "^1.0"     # 依赖在这里声明（见 §4.4）
```

> 初学阶段你基本不用改它，知道它存在即可。等你发布或加依赖时才会动到 `[dependencies]`。

---

## 4.3 格式化、检查与文档

写代码过程中有三个高频命令，对应 C 工程里的 `clang-format` / 静态分析 / 文档生成：

```bash
rlyeh fmt src/main.rl -w          # 格式化并写回（-w = write，否则只显示差异）
rlyeh fmt src/main.rl --check     # 仅检查格式（不修改，CI 里用来卡格式）
rlyeh check src/main.rl           # 静态分析：未用变量 / 恒常条件 / 冗余比较 / 不可达代码
rlyeh doc lib.rl --out docs/api.md   # 从 /// 注释生成 API 文档
```

### 关于格式：Rlyeh 强制缩进风格

和第 3 章提过的一致，Rlyeh 要求规范缩进（类似 `gofmt`）。`rlyeh fmt` 会帮你自动排版，**强烈建议保存前跑一次**。例如它会把：

```rlyeh
fn f(){let x=1;println(x)}
```

整理成：

```rlyeh
fn f() {
    let x = 1;
    println(x)
}
```

> **为什么强制格式**：统一风格减少代码评审噪音，也让编译器能更简单地解析缩进（Rlyeh 的块边界部分依赖缩进提示）。

### 文档注释 `///`

函数/类型上方写 `///` 开头的注释，会被 `rlyeh doc` 提取成文档：

```rlyeh
/// 计算两个数的和。
/// 参数 a、b 为加数，返回它们的和。
fn add(a: i64, b: i64) -> i64 {
    a + b
}
```

---

## 4.4 基准测试

想衡量某段代码性能（比如对比两种算法），用 `bench`：

```bash
rlyeh bench fib.rl --runs 10 --warmup 2   # 编译并基准计时（warmup 预热 2 次，正式跑 10 次取中位数）
# 或独立工具
rlyeh-bench fib.rl --runs 5
```

> **C 程序员的视角**：等价于你手写 `clock()`/`gettimeofday()` 包一层循环取中位数。Rlyeh 直接内置了"预热 + 多次采样"的统计逻辑，避免你被单次测量的噪声误导。

---

## 4.5 依赖管理（dagon 包管理器）

当你的项目需要复用别人写好的库时，用 `dagon`（Rlyeh 的包管理器，名字致敬《克苏鲁神话》的"达贡"）：

```bash
dagon add foo@^1.0       # 添加依赖（解析出版本，写入 Rlyeh.lock 锁定）
dagon update             # 重新解析（升级到满足约束的最新版）
dagon build && dagon run  # 按清单拉依赖并构建运行
```

> **C 程序员的视角**：`dagon` ≈ 一个极简的 `apt`/`vcpkg` + `git submodule` 混合体。它负责把依赖下载、版本解析、接入编译。MVP 阶段注册表规模有限，但机制已可用。

---

## 4.6 发布到本地注册表

当你写好一个可复用的库，想发布出去（或只是存档）：

```bash
rlyeh publish           # 默认发布到 ~/.rlyeh/registry
dagon search myapp       # 在注册表中搜索（验证是否发布成功）
```

> 重复发布同一版本会被拦截（避免覆盖已发布的版本），需要升版本号后再发。

---

## 4.7 测试用例目录

Rlyeh 有一套约定式的测试组织。`rlyeh test [<tests-dir>]` 会运行目录下三类用例：

| 目录 | 含义 | 期望 |
|------|------|------|
| `compile-pass/` | 应当编译通过的用例 | 编译成功 |
| `compile-fail/` | 应当编译**失败**的用例（测编译器报错是否正确） | 编译报错且报错信息匹配 |
| `run-pass/` | 应当编译并正确运行的用例 | 运行输出符合预期 |

仓库 `tests/` 已内置 194 个用例（run-pass 149 + compile-pass 12 + compile-fail 33），你可以参考它们的写法来组织自己的测试。

```bash
rlyeh test               # 跑当前项目/仓库的测试目录
rlyeh test tests/run-pass  # 只跑某一类
```

---

## 4.8 一个最小完整工作流示例

```bash
# 1. 建项目
rlyeh new myapp && cd myapp

# 2. 写代码（编辑 src/main.rl）

# 3. 格式化 + 静态检查
rlyeh fmt src/main.rl -w
rlyeh check src/main.rl

# 4. 跑
rlyeh run src/main.rl

# 5. 加依赖、测试、发布
dagon add some-lib@^1.0
rlyeh test
rlyeh publish
```

---

## 4.9 下一步

你已经走完"安装 → 第一个程序 → 发布项目"全流程。接下来建议：

- **系统学语言** → [编程语言指南](../guide/index.md)（每个知识点都有详细讲解与可运行示例）
- **按功能速查** → [语言手册](../manual/index.md)
- **看真实可运行范例** → 仓库 `examples/`、`tests/run-pass/`

> **C 程序员的建议**：不要急着一口气读完所有语言特性。先照着 [指南 §3 基础语法](../guide/03-basic-syntax.md) 把"变量、函数、控制流、数组"这几块跑熟，它们和 C 最像；再去看"所有权/借用/区域"这类 C 没有的概念——那时你会更感激编译器的保护。

---

[← 上一章：第一个程序](./03-first-program.md) | [返回教程目录](./index.md)

## 下一步

- 完整学习语言 → [编程语言指南](../guide/index.md)
- 语言速查 / 词法细节 / 命令参考 → [语言手册](../manual/index.md)
- 工具链构建与故障排查 → [../../toolchains/README.md](../../toolchains/README.md)、[../../toolchains/docs/REBUILD.md](../../toolchains/docs/REBUILD.md)
- 跨语言性能对比 → [../../examples/projects/benchmarks/README.md](../../examples/projects/benchmarks/README.md)
