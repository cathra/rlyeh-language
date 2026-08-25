# Zeta Toolchains（工具链构建与发布）

本目录承载 Zeta 工具链的**构建、测试、发布、归档**全流程脚本与文档。

工具链组成：`zeta` 编译器命令（run/build/test/fmt/check/doc/bench/new/publish/lsp/profile）+ 独立工具（`zeta-fmt` / `zeta-check` / `zeta-doc` / `zeta-bench`）+ 包管理器 `zep` + 标准库 `zeta-std` + zeta-language 技能（`SKILL.md` + `references/`，随工具链发布）。

## 目录结构

```
toolchains/
├── build.sh               # 一键构建脚本（构建 → 测试 → 发布 → 冒烟 → 归档）
├── install.sh             # 本地发布脚本（发布到 ~/.zeta，可单独运行）
├── README.md              # 本文件
├── docs/
│   ├── REBUILD.md         # 重新构建教程（从源码重建 toolchain 的完整指南）
│   └── MANUAL.md          # 工具链使用手册（全部命令参考 + 工作流）
└── dist/                  # 构建产物归档（自动生成）
    └── zeta-toolchain-<version>-<os>-<arch>.tar.gz
```

## 快速开始

```bash
# 1. 构建 + 测试 + 本地发布 + 归档（约 1-2 分钟）
./build.sh

# 2. 加入 PATH
export PATH="$HOME/.zeta/bin:$PATH"

# 3. 验证
zeta --version
zeta new hello && cd hello && zeta run src/main.zeta
```

## 常用命令

| 命令 | 说明 |
|------|------|
| `./build.sh` | 完整构建流程 |
| `./build.sh --no-test` | 跳过测试（快速迭代） |
| `./build.sh --no-install` | 仅构建 + 归档，不发布到本地 |
| `./build.sh --prefix /opt/zeta` | 自定义安装前缀 |
| `./install.sh` | 仅发布（复用已有 release 产物） |

## 构建产物

- **本地安装**：`~/.zeta/`（bin/ + std/ + skills/ + registry/，可重定位）
- **归档**：`toolchains/dist/zeta-toolchain-<ver>-<os>-<arch>.tar.gz`（解压后 `bin` 加入 PATH 即可用，wrapper 自动定位同目录 std；`skills/zeta-language/` 为项目级技能，可被 CodeBuddy 等 IDE 加载）

## 环境要求

| 依赖 | 用途 |
|------|------|
| Rust 工具链（`cargo`/`rustc`） | 编译器自举构建 |
| LLVM/Clang（`clang`） | 汇编与链接后端 |
| `git` | 版本信息 |

## 文档导航

- [重新构建教程](docs/REBUILD.md) —— 前置依赖、一键/手动构建、交叉编译、故障排查
- [使用手册](docs/MANUAL.md) —— `zeta`/工具/`zep` 全命令参考与工作流
