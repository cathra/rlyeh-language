---
name: rlyeh-language
description: Rlyeh 系统级编程语言技能（v0.1.0，随工具链发布）。用于编写、阅读、审查、修复或迁移 Rlyeh 语言代码（.rl 文件）；解释 Rlyeh 的语法、语义、分层内存模型与 Actor 并发模型；使用 rlyeh 工具链（build/run/test/check/fmt/bench/doc/lsp/profile）编译验证；以及在 rlyeh-language 仓库内开发编译器（rlyeh-lexer/parser/typecheck/codegen 等 crates）。当用户提到 Rlyeh 语言、rlyeh 代码、.rl 文件、Rlyeh 编译错误、actor/region/比较链/in 表达式语法时触发。
---

# Rlyeh Language

## Overview

Rlyeh 是一门 Rust 风格的系统级编程语言：内存安全零 GC（分层所有权）、编译速度对标 Go、Actor 并发一等公民、数学式比较链语法（`0 < x < 10`）。当前为 MVP（v0.1.0），bootstrap 编译器用 Rust 实现、代码生成走 LLVM IR。

本技能使 LLM 能写出**可编译运行**的 Rlyeh 代码、准确审查既有代码、并利用工具链快速定位编译错误。MVP 有明确语法边界——不遵守会产出大量编译错误，务必先读 `references/pitfalls.md`。

> 最后更新：2026-08-31（与 `CODEBUDDY.md` / `docs/` 规范同步；MVP 特性以 `CODEBUDDY.md` 为准）

## 何时使用

- 用户要求编写、修改、审查、调试 `.rl` 文件或 Rlyeh 项目（`Rlyeh.toml` + `src/main.rl`）
- 需要解释 Rlyeh 语法、语义、内存模型、Actor 模型、标准库、工具链
- 处理 Rlyeh 编译器 Rust 代码（`crates/` 下各 crate）或 `rlyeh-std` 标准库 `.rl` 源码
- 将其他语言（Rust/Go/Python）代码迁移到 Rlyeh

## 核心工作流

1. **写代码前**：通读 `references/pitfalls.md`（MVP 限制）与 `references/language.md`（语法速查），确认语法在 MVP 支持范围内。
2. **写代码**：遵循 `references/language.md` 的语法与 `references/std-lib.md` 的 API 形态。标准语义细节见 `references/semantics.md`。
3. **验证**：用下方工具链命令编译运行；错误按阶段定位（lexer 分词 / parser 语法 / typecheck 类型 / codegen-LLVM 链接）。
4. **审查**：逐条对照 `references/pitfalls.md` 检查（宏、引用、闭包、位运算优先级、`fn main` 入口等）。
5. **涉及编译器本身**：修改 `crates/` 后跑 `cargo test --workspace` 全量回归（集成测试用例在 `tests/` 与各 crate `tests/`）。

## 验证命令

在仓库根或 Rlyeh 项目内使用 `rlyeh` 二进制（开发期可 `cargo run -p rlyeh-driver -- <args>`）：

| 命令 | 用途 |
|------|------|
| `rlyeh new <name> [--lib]` | 创建项目（`Rlyeh.toml` + `src/main.rl` / `src/lib.rl`） |
| `rlyeh run <file.rl>` | 编译并运行（快速验证首选） |
| `rlyeh build <file.rl> [-o out] [--target <triple>] [--profile <pgo>]` | 编译为可执行文件；`--target` 支持交叉编译 / `wasm32-wasip1` |
| `rlyeh check <file.rl>` | 静态分析（未使用变量 / 恒常条件 / 冗余比较 / 不可达代码） |
| `rlyeh fmt <file.rl> [--check]` | 格式化（AST 重建） |
| `rlyeh test` | 跑 `tests/compile-pass` `compile-fail` `run-pass` 用例 |
| `rlyeh bench <file.rl> [--runs N]` | 基准测试 |
| `rlyeh doc <file.rl> [--out dir]` | 从 `///` 注释生成 Markdown |
| `rlyeh lsp` | LSP 服务器（stdio） |
| `rlyeh profile <file.rl_profile> [--out r.md]` | PGO 画像 → 区域大小预测报告 |

调试提示：`rlyeh run` 的链接错误若提示 `_main` undefined，先确认源文件有 `fn main()`；LLVM 链接错误多为 extern 符号与 libc 不一致。

## References

| 文件 | 内容 | 何时加载 |
|------|------|----------|
| `references/language.md` | 完整语法速查（词法/类型/语句/表达式/比较链/in/region/actor/模块/FFI），含可运行示例 | 编写或审查代码时 |
| `references/std-lib.md` | 标准库 API 速查（String/Vec/HashMap/Option/Result/io/net/sync/time/内建打印） | 涉及集合、IO、并发原语、时间时 |
| `references/semantics.md` | 语义要点（所有权移动语义/比较链/in 语义/actor 崩溃协议/transfer/字符串别名陷阱） | 解释行为、调试运行时差异时 |
| `references/pitfalls.md` | MVP 已知限制 + 大模型最常见的编写错误清单 | **每次写代码前必读** |

## 与官方文档的关系

Skill references 是 `docs/` 权威规范的提炼速查。深度问题回源：
- 语法 EBNF：`docs/grammar.md`；语义：`docs/semantics.md`
- 内存模型：`docs/memory-model.md`；Actor：`docs/actor-model.md`
- 标准库规范：`docs/std-lib.md`；教程：`docs/guide.md`
- 项目总纲与执行记录：`CODEBUDDY.md`

## 随工具链发布

本技能随 Rlyeh 工具链一起构建、发布与归档：

- **源码位置**：仓库 `skills/rlyeh-language/`（SKILL.md + `references/`）
- **本地安装**：`toolchains/install.sh` 将整个目录复制到 `<prefix>/skills/rlyeh-language/`（默认 `~/.rl/skills/rlyeh-language/`）
- **归档发布**：`toolchains/build.sh` 归档 `rlyeh-toolchain-<ver>-<os>-<arch>.tar.gz` 内含 `skills/` 目录，解压后即可被 CodeBuddy 等 IDE 作为项目级技能加载（`<prefix>/skills/rlyeh-language/SKILL.md`）
- **版本同步**：Skill 版本与工具链发布节奏一致（当前 v0.1.0）

重建或发布工具链后，若 IDE 未自动发现新版本技能，可重新加载技能或重启 IDE 会话。
