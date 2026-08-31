# 11. 编译目标与工具链

## 11.1 编译目标

`rlyeh build --target <triple>` 支持的 arch：

| 目标 | 状态 |
|------|------|
| `x86_64-unknown-linux-gnu` | 主要开发 / 测试平台（✅） |
| `aarch64-apple-darwin` | ✅（Apple Silicon） |
| `x86_64-apple-darwin` | ✅ |
| `x86_64-pc-windows-msvc` | ✅（WASI 实验性子目标） |
| `wasm32-wasip1` | ✅（Actor 交叉编译 / WASM 支持 L4） |

## 11.2 工具链命令

| 命令 | 功能 |
|------|------|
| `rlyeh new <name> [--lib]` | 创建项目脚手架（Rlyeh.toml + src/main.rl 或 lib.rl） |
| `rlyeh build <file> [-o <out>] [--target <triple>]` | 编译为可执行文件（`--target` 交叉编译） |
| `rlyeh run <file>` | 编译并运行 |
| `rlyeh test` | 运行测试目录用例 |
| `rlyeh fmt <file>` | 代码格式化（`--check` / `-w` / `--indent`） |
| `rlyeh check <file>` | 静态分析（未使用变量 / 恒常条件 / 冗余比较 / 不可达代码） |
| `rlyeh bench <file>` | 基准测试（`--runs` / `--warmup`） |
| `rlyeh doc <file>` | 从 `///` 注释生成 Markdown 文档 |
| `rlyeh publish [--registry] [--verbose]` | 打包发布到 dagon 注册表 |
| `rlyeh lsp` | 语言服务器（LSP over stdio） |
| `rlyeh profile <file.rl_profile>` | PGO 画像 → 区域大小预测报告（`--out`） |

详见 [语言手册 · 工具链命令速查](../manual/13-toolchain.md)。

## 11.3 WASI / WASM 交叉编译

```bash
rlyeh build src/main.rl --target wasm32-wasip1 -o main.wasm
```

Actor 运行时在 `wasm32-wasip1` 下由 driver 注入静态 `rlyeh_actor_resolve` 符号表替代 `dlsym`，WASI 单线程同步运行时保证 ask/send/FIFO/受监督崩溃重启协议与 native 一致（L4 ✅）。WASI 下网络禁用（L4 ✅）。

---

[← 上一章：标准库](./10-stdlib.md) | [返回指南目录](./index.md) | [下一章：外部函数接口 →](./12-ffi.md)
