# 2. 工具链部署与安装

> 本章目标：把 Rlyeh 编译器装好，能跑通 `rlyeh --version`。会详细解释每一步**在做什么**，以及出了问题**怎么排查**。如果你只想要"能跑就行"，照着 §2.2 的命令复制即可。

---

## 2.1 前置依赖

Rlyeh 的编译器（前端）是用 Rust 写的，代码生成走 LLVM。所以你需要先有这两样工具：

| 依赖 | 用途 | 检查命令 |
|------|------|----------|
| Rust 工具链（`cargo`/`rustc` ≥ 1.75） | 用来**编译 Rlyeh 编译器本身** | `cargo --version && rustc --version` |
| LLVM / Clang | Rlyeh 把代码生成成 LLVM IR 后，由它汇编、链接成机器码 | `clang --version` |
| git | 读取仓库版本信息 | `git --version` |

安装 Rust（如果还没装）：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# 按提示完成后重开终端或执行：
source "$HOME/.cargo/env"
```

安装 LLVM / Clang：

- **macOS**：`xcode-select --install`（命令行工具自带 clang），或 `brew install llvm`
- **Linux（Debian/Ubuntu）**：`sudo apt install llvm clang`
- **Windows**：安装 Visual Studio 的"使用 C++ 的桌面开发"工作负载，或 `winget install LLVM.LLVM`

> **C 程序员的视角**：这一步等价于"装好 gcc/clang 工具链"。你平时编译 C 用 clang，Rlyeh 编译 `.rl` 时内部也是调用 clang/LLVM 来产出最终机器码。Rust 工具链只是用来构建 Rlyeh 编译器自己的——你不需要会写 Rust。

---

## 2.2 一键构建工具链（推荐）

仓库根目录的 `toolchains/` 目录承载了完整的"构建 → 测试 → 发布 → 冒烟 → 归档"流程。最省事的方式是一键执行：

```bash
cd rlyeh-language
./toolchains/build.sh            # 完整流程：环境检查 → release 构建 → 测试 → 发布 → 冒烟 → 归档
```

常用变体（迭代开发时很有用）：

```bash
./toolchains/build.sh --no-test              # 跳过测试，快速拿到编译器
./toolchains/build.sh --no-install           # 只构建 + 归档，不发布到系统
./toolchains/build.sh --prefix /opt/rlyeh     # 自定义安装前缀
```

构建完成后，产物归档在：

```
toolchains/dist/rlyeh-toolchain-<ver>-<os>-<arch>.tar.gz
```

这个 tar 包可以拷到任意同架构机器上解压即用（包内 `bin` 与 `std` 同级、可重定位，无需固定安装路径）。

> **说明**：首次构建会编译大量 Rust 依赖，可能需要几分钟到十几分钟（取决于机器）。后续增量构建会快很多。这是"构建编译器"的耗时，和你之后编译 `.rl` 程序是两码事——你写 Rlyeh 程序时编译是秒级的。

---

## 2.3 本地发布（install.sh）

构建好的工具链需要放到一个固定位置，并把 `bin` 加进 `PATH`：

```bash
./toolchains/install.sh                # 发布到默认 ~/.rlyeh
RLYEH_PREFIX=/opt/rlyeh ./toolchains/install.sh   # 自定义前缀
```

安装后的目录布局：

```
<prefix>/
├── bin/              # rlyeh / rlyeh-driver / rlyeh-fmt / rlyeh-check / rlyeh-doc / rlyeh-bench / dagon
├── std/             # 标准库源码（module.rl + core/ + time/io/net/sync/fs 模块）
└── registry/         # 本地 dagon 注册表（发布第三方包的目标）
```

各命令的作用（后面章节会逐个用到）：

| 命令 | 作用 |
|------|------|
| `rlyeh` | 主驱动：build / run / test / fmt / check / doc / publish / bench / lsp |
| `rlyeh-fmt` | 代码格式化（自动排版，符合编译器要求的缩进风格） |
| `rlyeh-check` | 静态分析（找未用变量、恒常条件等） |
| `rlyeh-doc` | 从 `///` 文档注释生成 Markdown 文档 |
| `rlyeh-bench` | 基准测试 |
| `dagon` | 包管理器（添加/发布依赖） |

---

## 2.4 配置 PATH 并验证

把 `bin` 目录加进环境变量，这样在任意目录都能直接敲 `rlyeh`：

```bash
export PATH="$HOME/.rlyeh/bin:$PATH"    # 建议写入 ~/.zshrc / ~/.bashrc 永久生效
rlyeh --version                          # 验证：应显示 rlyeh 0.1.0
```

如果你看到版本号，恭喜，**工具链装好了**。如果提示 `command not found`，检查：PATH 是否写对、install.sh 是否成功执行、`<prefix>/bin` 下是否真有 `rlyeh` 可执行文件。

---

## 2.5 交叉编译环境（可选）

如果你想给别的平台编译（比如给 WebAssembly 编译一个 actor 程序），需要额外装目标：

```bash
# WASM 目标（actor 程序也支持）
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1 -p rlyeh-actor-runtime
brew install wasi-libc lld             # WASI sysroot 与 wasm-ld（Linux 用对应包管理器）
```

> 这是进阶内容，初学可以跳过。普通本机编译不需要任何额外配置。

---

## 2.6 故障排查速查

| 症状 | 可能原因 | 处理 |
|------|----------|------|
| `cargo: command not found` | Rust 没装或没 source env | 重装 Rust 并执行 `source "$HOME/.cargo/env"` |
| `clang: command not found` | LLVM 没装 | 按 §2.1 装 LLVM/Clang |
| `rlyeh: command not found` | PATH 没配 / 没 install | 检查 §2.4 |
| 构建时 LLVM 链接报错 | LLVM 版本不兼容 | 安装较新的 LLVM（≥ 14） |
| `rlyeh --version` 版本不对 | 系统里有旧的 rlyeh | `which rlyeh` 看加载的是哪个，调整 PATH 顺序 |

---

[← 上一章：认识 Rlyeh](./01-what-is-rlyeh.md) | [返回教程目录](./index.md) | [下一章：第一个程序 →](./03-first-program.md)
