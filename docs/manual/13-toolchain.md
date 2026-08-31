# 13. 工具链命令速查

> 速查所有 `rlyeh` 子命令与目标平台。详细用法与示例见 [教程 · 实战发布](../tutorial/04-publish.md)、[指南 §11 编译目标与工具链](../guide/11-targets-toolchain.md)。

---

## 13.1 命令一览

| 命令 | 功能 | 备注 / C 对照 |
|------|------|----------------|
| `rlyeh new <name> [--lib]` | 创建项目脚手架 | 生成 `Rlyeh.toml` + `src/main.rl` 或 `lib.rl`；≈ `mkdir` + 写 Makefile |
| `rlyeh build <file> [-o <out>] [--target <triple>]` | 编译为可执行文件 | ≈ `clang file.c -o out`；`--target` 交叉编译 |
| `rlyeh run <file>` | 编译并运行 | 一步到位 |
| `rlyeh test` | 运行测试目录用例 | compile-pass / compile-fail / run-pass 三类 |
| `rlyeh fmt <file>` | 代码格式化 | `--check` 仅检查 / `-w` 写回 / `--indent` 设缩进；≈ `clang-format` |
| `rlyeh check <file>` | 静态分析 | 未用变量 / 恒常条件 / 冗余比较 / 不可达代码；≈ `clang --analyze` |
| `rlyeh bench <file>` | 基准测试 | `--runs` / `--warmup` 采样；≈ 手写 `clock()` 循环取中位数 |
| `rlyeh doc <file>` | 从 `///` 注释生成 Markdown | ≈ Doxygen |
| `rlyeh publish [--registry] [--verbose]` | 发布到 dagon 注册表 | 重复版本拦截 |
| `rlyeh lsp` | 语言服务器（LSP over stdio） | 编辑器诊断推送 |
| `rlyeh profile <file.rl_profile>` | PGO 画像 → 区域大小预测 | `--out` 输出报告 |

---

## 13.2 目标平台（arch）

| 目标 triple | 状态 |
|-------------|------|
| `x86_64-unknown-linux-gnu` | ✅ 主平台 |
| `aarch64-apple-darwin` / `x86_64-apple-darwin` | ✅（Apple Silicon / Intel） |
| `x86_64-pc-windows-msvc` | ✅（WASI 实验性子目标） |
| `wasm32-wasip1` | ✅（Actor 交叉编译 L4） |

> **C 对照**：`--target <triple>` 复用 LLVM/Clang 的目标三元组约定，和你用 `clang --target=...` 交叉编译同套概念。不指定时默认编当前机器原生程序，产出自包含可执行文件，部署同 C 程序（`scp` + `chmod +x`）。

---

## 13.3 格式化与静态分析示例

```bash
rlyeh fmt src/main.rl --check     # 仅检查格式（CI 用）
rlyeh fmt src/main.rl -w          # 原地格式化（推荐保存前跑）
rlyeh check src/main.rl           # 静态分析
```

> Rlyeh **强制规范缩进**（类似 `gofmt`），`rlyeh fmt` 自动排版。强烈建议保存前跑一次，避免编译器因缩进风格报错。

---

## 更多示例

格式化 + 静态分析一键：

```bash
rlyeh fmt src/main.rl -w      # 原地格式化（建议保存前跑）
rlyeh check src/main.rl        # 静态分析（未用变量 / 不可达代码等）
```

命令清单详解见 [指南 §11 编译目标与工具链](../guide/11-targets-toolchain.md) 与 [`examples/by-chapter/`](../../examples/by-chapter/)。

---

[← 上一章：编译器与构建](./12-compiler-build.md) | [返回手册目录](./index.md) | [下一章：已知限制 →](./14-limits.md)
