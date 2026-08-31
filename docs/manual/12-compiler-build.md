# 12. 编译器与构建

## 12.1 编译流程

```
源文件 .rl → 词法 → 语法 → HIR → MIR → LIR → LLVM IR → 目标文件 → 链接 → 可执行
```

- `rlyeh-lexer` / `rlyeh-parser` / `rlyeh-ast` / `rlyeh-hir` / `rlyeh-mir` / `rlyeh-lir`
- `rlyeh-typecheck` / `rlyeh-borrowck` / `rlyeh-regionck`
- `rlyeh-codegen`（LLVM / Cranelift）/ `rlyeh-driver`（CLI 入口）

## 12.2 增量编译

已实现模块级缓存（PGO 画像回灌初始区域容量）；函数级并行（规划中）。

## 12.3 构建命令

```bash
rlyeh build src/main.rl -o main            # 默认目标（当前平台）
rlyeh build src/main.rl --target wasm32-wasip1 -o main.wasm
rlyeh build src/main.rl --profile hot.rl_profile   # 注入 PGO 画像
```

## 12.4 项目结构

```
<project>/
├── Rlyeh.toml          # 项目清单（name / version / deps / bin/lib）
├── src/
│   ├── main.rl         # 可执行入口（--bin）
│   └── lib.rl          # 库（--lib）
└── tests/              # 测试目录（compile-pass / compile-fail / run-pass）
```

---

[← 上一章：标准库参考](./11-stdlib.md) | [返回手册目录](./index.md) | [下一章：工具链命令速查 →](./13-toolchain.md)
