# 重新构建教程（Rebuild Guide）

从源码重新构建 Zeta 工具链的完整指南。适用于：源码更新后的重建、环境迁移、发布新版本。

## 1. 前置依赖

| 依赖 | 版本要求 | 验证 |
|------|---------|------|
| Rust 工具链 | stable（≥ 1.75） | `cargo --version && rustc --version` |
| LLVM/Clang | ≥ 15（macOS 自带即可） | `clang --version` |
| git | 任意 | `git --version` |

## 2. 一键构建（推荐）

```bash
cd zeta-language
./toolchains/build.sh
```

该脚本依次执行：

1. **环境检查**：cargo / rustc / clang / git 是否就位
2. **release 构建**：`cargo build --release`（编译 zeta-driver + 4 工具 + zep）
3. **全量测试**：`cargo test --workspace`（compile-pass/compile-fail/run-pass 全用例）
4. **本地发布**：调用 `toolchains/install.sh`，发布到 `~/.zeta/`
5. **冒烟验证**：`zeta --version` + 编译运行 hello world
6. **归档打包**：生成 `toolchains/dist/zeta-toolchain-<ver>-<os>-<arch>.tar.gz`

常用变体：

```bash
./toolchains/build.sh --no-test      # 跳过测试，快速迭代
./toolchains/build.sh --no-tar       # 不打包归档
./toolchains/build.sh --prefix /opt/zeta   # 发布到指定前缀
```

## 3. 手动逐步构建

```bash
# 1) 构建 release（workspace 全部 crate）
cd zeta-language
cargo build --release

# 2) 运行测试（可选但推荐）
cargo test --workspace

# 3) 本地发布（默认 ~/.zeta，可 ZETA_PREFIX 覆盖）
./toolchains/install.sh

# 4) 冒烟验证
~/.zeta/bin/zeta --version
~/.zeta/bin/zeta run examples/hello-world.zeta

# 5) 归档（可选）
tar -C ~/.zeta -czf toolchains/dist/zeta-toolchain.tar.gz bin std
```

## 4. 验证清单

构建完成后逐项确认：

```bash
export PATH="$HOME/.zeta/bin:$PATH"

# 编译器
zeta --version                    # → zeta 0.1.0

# 编译运行（含标准库模块）
zeta run examples/hello-world.zeta

# 独立工具
zeta-fmt --help                   # 格式化
zeta-check examples/hello-world.zeta   # 静态分析
zeta-doc examples/hello-world.zeta     # 文档生成
zeta-bench examples/hello-world.zeta --runs 3   # 基准

# 包管理器 + 本地注册表
zeta new demo && cd demo
zeta publish                      # → 已发布 demo 0.1.0
zep search demo                   # → 能查到
```

## 5. 交叉编译（可选，WASM）

actor 运行时支持 `wasm32-wasip1` 目标：

```bash
# 前置：安装 wasm 目标与 actor 运行时
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1 -p zeta-actor-runtime

# 交叉编译 Zeta 程序
zeta build app.zeta --target wasm32-wasip1 -o app.wasm
```

## 6. 故障排查

| 现象 | 原因 | 解决 |
|------|------|------|
| `缺少 release 产物` | 未先构建或构建失败 | `cargo build --release` 后重跑 install.sh |
| `模块 X 未找到` | 标准库未正确复制 | 重跑 install.sh（会重建 `~/.zeta/std`） |
| `zeta: command not found` | PATH 未配置 | `export PATH="$HOME/.zeta/bin:$PATH"` |
| clang 链接报错 | 无 LLVM/Clang | 安装 Xcode CLT：`xcode-select --install` |
| `cargo test` 个别失败 | 依赖未更新 | `cargo update` 后重试 |
| wasm 目标缺失 | 未装 target | `rustup target add wasm32-wasip1` |

## 7. 发布新版本流程

```bash
# 1) 更新版本号（Cargo.toml 与 CHANGELOG.md）
# 2) 全量构建 + 测试 + 发布
./toolchains/build.sh
# 3) 归档即发布物：toolchains/dist/zeta-toolchain-<ver>-<os>-<arch>.tar.gz
```
