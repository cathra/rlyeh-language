# 12. 编译器与构建

> 速查编译流程、增量编译、构建命令、项目结构。动手安装见 [教程 · 工具链部署](../tutorial/02-install.md)。

---

## 12.1 编译流程

```
源文件 .rl → 词法 → 语法 → HIR → MIR → LIR → LLVM IR → 目标文件 → 链接 → 可执行
```

| 阶段 | 组件 crate |
|------|-----------|
| 词法 / 语法 / AST | `rlyeh-lexer` / `rlyeh-parser` / `rlyeh-ast` |
| 高层 IR | `rlyeh-hir` |
| 中层 IR | `rlyeh-mir` |
| 低层 IR | `rlyeh-lir` |
| 语义检查 | `rlyeh-typecheck` / `rlyeh-borrowck` / `rlyeh-regionck` |
| 代码生成 | `rlyeh-codegen`（LLVM / Cranelift） |
| 驱动入口 | `rlyeh-driver`（CLI） |

> **C 对照**：这整套 ≈ `clang` 内部的 `预处理 → 词法 → 语法 → AST → LLVM IR → 汇编 → 链接`。Rlyeh 在"语义检查"阶段多了**借用检查 / 区域检查**（C 完全不做），这正是它比 C 安全的原因。后端同样走 LLVM，所以生成的机器码质量和 clang 同档。

---

## 12.2 增量编译

- 已实现**模块级缓存**（改一个模块只重编受影响的，秒级）。
- **PGO 画像回灌**：`rlyeh profile` 生成的画像可注入区域分配器的初始容量，减少运行时扩容。
- 函数级并行（规划中）。

> **C 对照**：类似 `make` 的增量构建 / `ccache` 缓存，但 Rlyeh 在编译器内建、无需额外配置。编译速度对标 Go（远快于 C++ 模板元编程那种慢编译）。

---

## 12.3 构建命令

```bash
rlyeh build src/main.rl -o main            # 默认目标（当前平台）
rlyeh build src/main.rl --target wasm32-wasip1 -o main.wasm
rlyeh build src/main.rl --profile hot.rl_profile   # 注入 PGO 画像
```

---

## 12.4 项目结构

```
<project>/
├── Rlyeh.toml          # 项目清单（name / version / deps / bin/lib）
├── src/
│   ├── main.rl         # 可执行入口（--bin）
│   └── lib.rl          # 库（--lib）
└── tests/              # 测试目录（compile-pass / compile-fail / run-pass）
```

> **C 对照**：`Rlyeh.toml` ≈ `Makefile`/`CMakeLists.txt` + `package.json` 的合体；`src/main.rl`/`lib.rl` ≈ `main.c`/`lib.c`；`tests/` 是约定式测试根（见 [教程 §4.7](../tutorial/04-publish.md)）。

---

## 更多示例

基本构建与交叉编译：

```bash
rlyeh build src/main.rl -o main
rlyeh build src/main.rl --target wasm32-wasip1 -o main.wasm
```

每个语言特性的可运行示例见 [`examples/by-chapter/`](../../examples/by-chapter/)（每篇含对应 `rlyeh run` 命令）。

---

[← 上一章：标准库参考](./11-stdlib.md) | [返回手册目录](./index.md) | [下一章：工具链命令速查 →](./13-toolchain.md)
