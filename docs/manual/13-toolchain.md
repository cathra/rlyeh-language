# 13. 工具链命令速查

| 命令 | 功能 |
|------|------|
| `rlyeh new <name> [--lib]` | 创建项目脚手架（Rlyeh.toml + src/main.rl 或 lib.rl） |
| `rlyeh build <file> [-o <out>] [--target <triple>]` | 编译为可执行文件（`--target` 交叉编译 / `wasm32-wasip1` 生成 `.wasm`） |
| `rlyeh run <file>` | 编译并运行 |
| `rlyeh test` | 运行测试目录用例（compile-pass / compile-fail / run-pass） |
| `rlyeh fmt <file>` | 代码格式化（`--check` / `-w` / `--indent`） |
| `rlyeh check <file>` | 静态分析（未使用变量 / 恒常条件 / 冗余比较 / 不可达代码） |
| `rlyeh bench <file>` | 基准测试（`--runs` / `--warmup`） |
| `rlyeh doc <file>` | 从 `///` 注释生成 Markdown 文档 |
| `rlyeh publish [--registry] [--verbose]` | 打包发布到 dagon 注册表（重复版本拦截） |
| `rlyeh lsp` | 语言服务器（LSP over stdio，诊断推送） |
| `rlyeh profile <file.rl_profile>` | PGO 画像 → 区域大小预测报告（`--out`） |

## 13.1 目标平台

| 目标 | 状态 |
|------|------|
| `x86_64-unknown-linux-gnu` | ✅ 主平台 |
| `aarch64-apple-darwin` / `x86_64-apple-darwin` | ✅ |
| `x86_64-pc-windows-msvc` | ✅（WASI 实验性子目标） |
| `wasm32-wasip1` | ✅（Actor 交叉编译 L4） |

## 13.2 格式化与静态分析

```bash
rlyeh fmt src/main.rl --check     # 仅检查
rlyeh fmt src/main.rl -w          # 原地格式化
rlyeh check src/main.rl           # 未使用变量 / 恒常条件 / 冗余比较 / 不可达代码
```

---

[← 上一章：编译器与构建](./12-compiler-build.md) | [返回手册目录](./index.md) | [下一章：已知限制 →](./14-limits.md)
