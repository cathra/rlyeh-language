# Zeta 教程指南（新手入门）

> **定位**：本文带你完成 Zeta 的**安装 → 第一个程序 → 发布项目**全流程，适合第一次接触 Zeta 的读者。
> **学语言**：渐进式语言教程见 [guide.md](./guide.md)（完整、示例可运行）；语言速查与词法细节见 [manual.md](./manual.md)。
> **权威规范**：`grammar.md`（EBNF）/ `semantics.md` / `memory-model.md` / `actor-model.md` / `std-lib.md`。

---

## 1. 认识 Zeta

Zeta 是一门面向未来十年基础设施的**系统级编程语言**：

| 设计目标 | 对标 | 落地形态 |
|----------|------|----------|
| 内存安全、零 GC | Rust | 分层内存模型（L0 所有权 → L1 区域 → L2 Rc → L3 GC） |
| 编译速度极快 | Go | 模块级缓存（增量编译） |
| 并发模型一等公民 | Erlang / Akka | `actor` 语言级构造 + 运行时监督 |
| 数学式语法直觉 | Python / MATLAB | 比较链 `0 < x < 10`、集合判断 `x in (1, 3, 5)` |

**一句话定位**：Zeta = Rust 的安全性 + Go 的编译速度 + Erlang 的并发模型 + 数学的自然表达。

---

## 2. 工具链部署与安装

### 2.1 前置依赖

| 依赖 | 用途 |
|------|------|
| Rust 工具链（`cargo`/`rustc` ≥ 1.75） | 编译器自举构建 |
| LLVM/Clang（macOS 自带或 `xcode-select --install`） | 汇编与链接后端 |
| git | 版本信息 |

```bash
cargo --version && rustc --version && clang --version
```

### 2.2 一键构建工具链

仓库根目录的 `toolchains/` 承载完整的构建、发布、归档流程：

```bash
cd zeta-language
./toolchains/build.sh            # 完整流程：环境检查 → release 构建 → 测试 → 发布 → 冒烟 → 归档
```

常用变体：

```bash
./toolchains/build.sh --no-test              # 跳过测试（快速迭代）
./toolchains/build.sh --no-install           # 仅构建 + 归档，不发布
./toolchains/build.sh --prefix /opt/zeta     # 自定义安装前缀
```

构建产物归档在 `toolchains/dist/zeta-toolchain-<ver>-<os>-<arch>.tar.gz`，可分发到任意机器（解压后即可用，`bin` 与 `std` 同级、可重定位）。

### 2.3 本地发布（install.sh）

```bash
./toolchains/install.sh                # 发布到默认 ~/.zeta
ZETA_PREFIX=/opt/zeta ./toolchains/install.sh   # 自定义前缀
```

安装布局：

```
<prefix>/
├── bin/              # zeta / zeta-driver / zeta-fmt / zeta-check / zeta-doc / zeta-bench / zep
├── std/              # 标准库源码（core.zeta + time/io/net/sync/fs 模块）
└── registry/         # 本地 zep 注册表（publish 目标）
```

### 2.4 配置 PATH

```bash
export PATH="$HOME/.zeta/bin:$PATH"    # 建议写入 ~/.zshrc / ~/.bashrc
zeta --version                          # 验证：zeta 0.1.0
```

### 2.5 交叉编译环境（可选）

```bash
# WASM 目标（actor 程序亦支持）
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1 -p zeta-actor-runtime
brew install wasi-libc lld             # WASI sysroot 与 wasm-ld
```

---

## 3. 第一个程序

```zeta
// hello-world.zeta
fn main() {
    println("Hello, Zeta!");
}
```

编译与运行：

```bash
zeta build hello-world.zeta     # 生成可执行文件 hello-world
./hello-world
zeta run hello-world.zeta       # 编译并运行（一步到位）
```

---

## 4. 实战：创建并发布一个项目

### 4.1 创建项目脚手架

```bash
zeta new myapp          # 生成 Zeta.toml + src/main.zeta
cd myapp
zeta run src/main.zeta  # 编译并运行
```

### 4.2 格式化、检查与文档

```bash
zeta fmt src/main.zeta -w          # 格式化并写回
zeta fmt src/main.zeta --check     # 仅检查（CI 用）
zeta check src/main.zeta           # 静态分析
zeta doc lib.zeta --out docs/api.md   # 从 /// 注释生成文档
```

### 4.3 基准测试

```bash
zeta bench fib.zeta --runs 10 --warmup 2   # 编译并基准计时
# 或独立工具
zeta-bench fib.zeta --runs 5
```

### 4.4 依赖管理（zep）

```bash
zep add foo@^1.0       # 添加依赖（解析到 Zeta.lock）
zep update             # 重新解析
zep build && zep run
```

### 4.5 发布到本地注册表

```bash
zeta publish           # 默认发布到 ~/.zeta/registry
zep search myapp       # 注册表中搜索
```

### 4.6 测试用例目录

`zeta test [<tests-dir>]` 运行目录下 `compile-pass/`、`compile-fail/`、`run-pass/` 三类用例（仓库 `tests/` 已内置 140+ 用例）。

---

## 下一步

- 完整学习语言 → [guide.md](./guide.md)
- 语言速查 / 词法细节 / 命令参考 → [manual.md](./manual.md)
- 工具链构建与故障排查 → [../toolchains/README.md](../toolchains/README.md)、[../toolchains/docs/REBUILD.md](../toolchains/docs/REBUILD.md)
- 跨语言性能对比 → [../examples/projects/benchmarks/README.md](../examples/projects/benchmarks/README.md)
