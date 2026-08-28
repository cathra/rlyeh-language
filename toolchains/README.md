# Rlyeh Toolchains（工具链构建与发布）

本目录承载 Rlyeh 工具链的**构建、测试、发布、归档**全流程脚本与文档。

工具链组成：`rlyeh` 编译器命令（run/build/test/fmt/check/doc/bench/new/publish/lsp/profile）+ 独立工具（`rlyeh-fmt` / `rlyeh-check` / `rlyeh-doc` / `rlyeh-bench`）+ 包管理器 `dagon` + 标准库 `rlyeh-std` + rlyeh-language 技能（`SKILL.md` + `references/`，随工具链发布）。

## 目录结构

```
toolchains/
├── build.sh               # 一键构建脚本（构建 → 测试 → 发布 → 冒烟 → 归档）
├── install.sh             # 本地发布脚本（发布到 ~/.rl，可单独运行）
├── README.md              # 本文件
├── docs/
│   ├── REBUILD.md         # 重新构建教程（从源码重建 toolchain 的完整指南）
│   └── MANUAL.md          # 工具链使用手册（全部命令参考 + 工作流）
└── dist/                  # 构建产物归档（自动生成）
    └── rlyeh-toolchain-<version>-<os>-<arch>.tar.gz
```

## 快速开始

```bash
# 1. 构建 + 测试 + 本地发布 + 归档（约 1-2 分钟）
./build.sh

# 2. 加入 PATH
export PATH="$HOME/.rl/bin:$PATH"

# 3. 验证
rlyeh --version
rlyeh new hello && cd hello && rlyeh run src/main.rl
```

## 常用命令

| 命令 | 说明 |
|------|------|
| `./build.sh` | 完整构建流程 |
| `./build.sh --no-test` | 跳过测试（快速迭代） |
| `./build.sh --no-install` | 仅构建 + 归档，不发布到本地 |
| `./build.sh --prefix /opt/rlyeh` | 自定义安装前缀 |
| `./install.sh` | 仅发布（复用已有 release 产物） |

## 构建产物

- **本地安装**：`~/.rl/`（bin/ + std/ + skills/ + examples/ + registry/，可重定位）
- **归档**：`toolchains/dist/rlyeh-toolchain-<ver>-<os>-<arch>.tar.gz`（解压后 `bin` 加入 PATH 即可用，wrapper 自动定位同目录 std；`skills/rlyeh-language/` 为项目级技能，可被 CodeBuddy 等 IDE 加载；`examples/std-demos/` 为标准库各功能点用例项目，纯代码随包分发，不编译）

## 环境要求

| 依赖 | 用途 |
|------|------|
| Rust 工具链（`cargo`/`rustc`） | 编译器自举构建 |
| LLVM/Clang（`clang`） | 汇编与链接后端 |
| `git` | 版本信息 |

## 全平台发布（CI/CD，2026-08-28）

Rlyeh 编译器（`rlyeh-driver`）**运行时经外部 `clang` 汇编 LLVM IR + 链接**（`lib.rs compile_ir`），因此每个平台的 toolchain 必须在**该平台原生构建**（交叉编译的 driver 在目标平台运行仍依赖该平台 clang）。全平台发布由 GitHub Actions 完成（[`release.yml`](../.github/workflows/release.yml)）：

| 平台 | target | 产物 |
|------|--------|------|
| macOS arm64 | `aarch64-apple-darwin` | `rlyeh-macos-arm64.tar.gz` |
| macOS x86_64 | `x86_64-apple-darwin` | `rlyeh-macos-x86_64.tar.gz` |
| Linux x86_64 | `x86_64-unknown-linux-gnu` | `rlyeh-linux-x86_64.tar.gz` |
| Windows x86_64 | `x86_64-pc-windows-gnu`（MinGW） | `rlyeh-windows-x86_64.tar.gz` |

每个归档含**完整 toolchain**：`bin/`（`rlyeh` wrapper + driver/fmt/check/doc/bench/dagon）+ `std/` + `skills/` + `examples/`，可重定位。

**触发方式**：推送 `v*` 标签到远程（Gitee `origin`）后，GitHub Actions 检测到 tag 自动构建 4 平台并上传 Release。

```bash
git tag v0.1.0
git push origin v0.1.0
```

**全平台发布前置（2026-08-28 已完成）**：
- 修 Windows POSIX 阻断：actor 运行时 `ffi.rs` `dlsym` → Windows `GetProcAddress`/`GetModuleHandleA`（`#[cfg(target_os = "windows")]`），Windows 交叉编译验证通过（`PE32+` exe 全部生成）。
- 修 Linux 编译阻断：`nio/poller.rs` epoll 分支 `f |= libc::EPOLLIN` 的 `u32 |= i32` 类型不匹配（Linux 路径在 macOS 不编译故未暴露），改 `as u32`。

## 本机构建（build-all.sh）

在单一 macOS 主机上用 [`build-all.sh`](./build-all.sh) 可构建 **5 平台**归档（`toolchains/dist/`）：
- `rlyeh-toolchain-<ver>-macos-arm64.tar.gz`（本机）
- `rlyeh-toolchain-<ver>-macos-x86_64.tar.gz`（交叉 `x86_64-apple-darwin`）
- `rlyeh-toolchain-<ver>-windows-x86_64.tar.gz`（交叉 MinGW `x86_64-pc-windows-gnu`）
- `rlyeh-toolchain-<ver>-linux-x86_64.tar.gz`（交叉 `x86_64-unknown-linux-musl`，**rust-lld** 链接）
- `rlyeh-toolchain-<ver>-linux-arm64.tar.gz`（交叉 `aarch64-unknown-linux-musl`，**rust-lld** 链接）

**交叉链接器**：Linux 用 **rust-lld**（`<rustup>/lib/rustlib/<host>/bin/rust-lld`）——2026-08-28 实测 rust-lld 与 Rust musl 的 `-static-pie`/`-Bstatic` 兼容（zig 0.13 不兼容，报 `unknown file type`）。

**Windows arm64 本机不可行**（2026-08-28 实测：缺 Windows ARM64 import 库 `kernel32.lib` 等，rust-lld 报 `unable to find library -lkernel32`）。Windows arm64 包由 CI 构建。

## `rlyeh build --target` 编译期交叉编译（Y，2026-08-28）

`rlyeh-driver` 的 `--target` 交叉编译已完善（lib.rs）：
- **`is_cross_target` 完善**：交叉 = 架构不同 **或** OS 不同（修复同架构跨 OS 误判，macOS arm64 → Linux arm64）。
- **新增 `assemble_cross_elf`**：跨 OS ELF（Linux musl）目标用 **rust-lld** + Rust musl sysroot crt/libc 链接（宿主 clang 的 ld 无法链接跨 OS 目标）。
- **kqueue/kevent 平台注入**：改为 `__rlyeh_` 前缀，driver 按目标注入（macOS/BSD 原生转发、其他平台 stub -1），修复 Linux 交叉的 kqueue undefined。
- **f128 内建**：arm64 等 long double=f128，链接 Rust `compiler_builtins` rlib + `rust_eh_personality` stub。

**已验证**（2026-08-28）：macOS arm64/x86_64、Linux x86_64（ELF static）、Linux arm64（ELF static）交叉编译成功；经 `RLYEH_CLANG=zig-cc` + zig 工具链，**RISC-V64（ELF UCB RISC-V）与 LoongArch64（ELF LoongArch）交叉编译亦验证成功**。

**工具链边界**：RISC-V64/LoongArch64 需 **Apple clang 不支持**（`unknown target triple riscv64gc-unknown-linux-musl`）——需支持这些架构的 clang/LLVM（如 zig / LLVM 官方）才能走 `--target` 交叉编译。

### 安装 RISC-V64 / LoongArch64 交叉工具链（zig）

`rlyeh-driver` 支持 `RLYEH_CLANG` 环境变量指定汇编/链接器（util.rs）。用 **zig**（自带 LLVM，`zig cc` 支持 RISC-V/LoongArch target）替代 Apple clang：

```bash
# 方式一：一键脚本（brew install zig + zig-cc wrapper，含 target 格式转换）
./toolchains/install-riscv.sh

# 方式二：手动
brew install zig
# 创建 zig-cc wrapper（关键：把 rlyeh 的 Rust target 格式 riscv64gc-unknown-linux-musl
# 转为 zig 的 gcc 格式 riscv64-linux-musl），见 install-riscv.sh 模板
rustup target add riscv64gc-unknown-linux-musl loongarch64-unknown-linux-musl

# 交叉编译（已验证成功，2026-08-28）
export RLYEH_CLANG="$HOME/.local/bin/zig-cc"
rlyeh build app.rl --target riscv64gc-unknown-linux-musl  -o app-riscv   # ELF UCB RISC-V
rlyeh build app.rl --target loongarch64-unknown-linux-musl -o app-loong  # ELF LoongArch
```

**关键**：zig/LLVM 需 gcc 格式 target（`riscv64-linux-musl`），而 rlyeh 传 Rust 格式（`riscv64gc-unknown-linux-musl`）——`zig-cc` wrapper 负责转换。`assemble_cross_elf` 已用 rust-lld + Rust musl sysroot crt/libc 链接（含 f128 内建 + `rust_eh_personality` stub），zig 只需提供目标架构的**汇编**阶段。

## LoongArch64（龙芯）与 RISC-V64 Linux

Rust target `loongarch64-unknown-linux-musl` / `riscv64gc-unknown-linux-musl` 已支持，rust-lld 能识别 LoongArch/RISC-V。`build-all.sh` 已集成两架构构建（当 target + libgcc 可用时）。

**本机限制（2026-08-28）**：
- **LoongArch64**：缺 LoongArch64 的 `libgcc_s`（rust-lld 报 `unable to find library -lgcc_s`；musl.cc 无 loongarch64 工具链，Apple clang 不支持 LoongArch）
- **RISC-V64**：`riscv64gc-unknown-linux-musl` target 未装（rustup 下载被跳过，本机网络受限）

构建方式：
1. **原生构建**：在 LoongArch64 / RISC-V64 Linux 机器/容器运行 `build.sh`
2. **交叉构建**：安装 Loongson/RISC-V 官方交叉工具链提供 libgcc_s，设置 `LOONGARCH_LIBGCC_DIR`（LoongArch）/ `RISCV_LIBGCC_DIR`（RISC-V）指向含 `libgcc_s.so` 的目录，`build-all.sh` 即构建 `linux-loongarch64` / `linux-riscv64` 包

## 文档导航

- [重新构建教程](docs/REBUILD.md) —— 前置依赖、一键/手动构建、交叉编译、故障排查
- [使用手册](docs/MANUAL.md) —— `rlyeh`/工具/`dagon` 全命令参考与工作流
