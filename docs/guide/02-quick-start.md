# 2. 快速上手

## 2.1 第一个程序

```rlyeh
// hello-world.rl
fn main() {
    println("Hello, Rlyeh!");
}
```

## 2.2 编译与运行

```bash
rlyeh build hello-world.rl     # 生成可执行文件 hello-world
rlyeh run hello-world.rl       # 编译并运行
rlyeh test                       # 运行 tests/ 目录 compile-pass/compile-fail/run-pass 用例
```

## 2.3 工具链速览

| 命令 | 功能 |
|------|------|
| `rlyeh new <name> [--lib]` | 创建新项目脚手架（Rlyeh.toml + src/main.rl 或 lib.rl） |
| `rlyeh build <file> [-o <out>] [--target <triple>]` | 编译为可执行文件（`--target` 交叉编译 / WASM / `--profile` 注入 PGO） |
| `rlyeh run <file>` | 编译并运行 |
| `rlyeh test` | 运行测试目录用例 |
| `rlyeh fmt <file>` | 代码格式化（`--check` / `-w` / `--indent`） |
| `rlyeh check <file>` | 静态分析（未使用变量 / 恒常条件 / 冗余比较 / 不可达代码） |
| `rlyeh bench <file>` | 基准测试（`--runs` / `--warmup`） |
| `rlyeh doc <file>` | 从 `///` 注释生成 Markdown 文档 |
| `rlyeh publish [--registry] [--verbose]` | 打包发布到 dagon 注册表（重复版本拦截） |
| `rlyeh lsp` | 语言服务器（LSP over stdio，诊断推送） |
| `rlyeh profile <file.rl_profile>` | PGO 画像 → 区域大小预测报告（`--out`） |

---

## 练习

1. 用 `rlyeh new demo` 创建项目，把 [`examples/by-chapter/02-quick-start.rl`](../../examples/by-chapter/02-quick-start.rl) 的内容放进 `src/main.rl`，用 `rlyeh run` 跑起来。
2. 用 `rlyeh check examples/by-chapter/02-quick-start.rl` 看静态分析输出（故意写一个未使用变量，观察告警）。
3. 故意把缩进写乱，用 `rlyeh fmt --check` 检查，再用 `rlyeh fmt -w` 格式化，对比前后差异。

---

[← 上一章：认识 Rlyeh](./01-what-is-rlyeh.md) | [返回指南目录](./index.md) | [下一章：基础语法 →](./03-basic-syntax.md)
