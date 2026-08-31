# 2. 工具链部署与安装

## 2.1 前置依赖

| 依赖 | 用途 |
|------|------|
| Rust 工具链（`cargo`/`rustc` ≥ 1.75） | 编译器自举构建 |
| LLVM/Clang（macOS 自带或 `xcode-select --install`） | 汇编与链接后端 |
| git | 版本信息 |

```bash
cargo --version && rustc --version && clang --version
```

## 2.2 一键构建工具链

仓库根目录的 `toolchains/` 承载完整的构建、发布、归档流程：

```bash
cd rlyeh-language
./toolchains/build.sh            # 完整流程：环境检查 → release 构建 → 测试 → 发布 → 冒烟 → 归档
```

常用变体：

```bash
./toolchains/build.sh --no-test              # 跳过测试（快速迭代）
./toolchains/build.sh --no-install           # 仅构建 + 归档，不发布
./toolchains/build.sh --prefix /opt/rlyeh     # 自定义安装前缀
```

构建产物归档在 `toolchains/dist/rlyeh-toolchain-<ver>-<os>-<arch>.tar.gz`，可分发到任意机器（解压后即可用，`bin` 与 `std` 同级、可重定位）。

## 2.3 本地发布（install.sh）

```bash
./toolchains/install.sh                # 发布到默认 ~/.rl
RLYEH_PREFIX=/opt/rlyeh ./toolchains/install.sh   # 自定义前缀
```

安装布局：

```
<prefix>/
├── bin/              # rlyeh / rlyeh-driver / rlyeh-fmt / rlyeh-check / rlyeh-doc / rlyeh-bench / dagon
├── std/              # 标准库源码（core.rl + time/io/net/sync/fs 模块）
└── registry/         # 本地 dagon 注册表（publish 目标）
```

## 2.4 配置 PATH

```bash
export PATH="$HOME/.rl/bin:$PATH"    # 建议写入 ~/.zshrc / ~/.bashrc
rlyeh --version                          # 验证：rlyeh 0.1.0
```

## 2.5 交叉编译环境（可选）

```bash
# WASM 目标（actor 程序亦支持）
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1 -p rlyeh-actor-runtime
brew install wasi-libc lld             # WASI sysroot 与 wasm-ld
```

---

[← 上一章：认识 Rlyeh](./01-what-is-rlyeh.md) | [返回教程目录](./index.md) | [下一章：第一个程序 →](./03-first-program.md)
