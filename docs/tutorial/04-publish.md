# 4. 实战：创建并发布一个项目

## 4.1 创建项目脚手架

```bash
rlyeh new myapp          # 生成 Rlyeh.toml + src/main.rl
cd myapp
rlyeh run src/main.rl  # 编译并运行
```

## 4.2 格式化、检查与文档

```bash
rlyeh fmt src/main.rl -w          # 格式化并写回
rlyeh fmt src/main.rl --check     # 仅检查（CI 用）
rlyeh check src/main.rl           # 静态分析
rlyeh doc lib.rl --out docs/api.md   # 从 /// 注释生成文档
```

## 4.3 基准测试

```bash
rlyeh bench fib.rl --runs 10 --warmup 2   # 编译并基准计时
# 或独立工具
rlyeh-bench fib.rl --runs 5
```

## 4.4 依赖管理（dagon）

```bash
dagon add foo@^1.0       # 添加依赖（解析到 Rlyeh.lock）
dagon update             # 重新解析
dagon build && dagon run
```

## 4.5 发布到本地注册表

```bash
rlyeh publish           # 默认发布到 ~/.rl/registry
dagon search myapp       # 注册表中搜索
```

## 4.6 测试用例目录

`rlyeh test [<tests-dir>]` 运行目录下 `compile-pass/`、`compile-fail/`、`run-pass/` 三类用例（仓库 `tests/` 已内置 194 个用例：run-pass 149 + compile-pass 12 + compile-fail 33）。

---

[← 上一章：第一个程序](./03-first-program.md) | [返回教程目录](./index.md)

## 下一步

- 完整学习语言 → [编程语言指南](../guide/index.md)
- 语言速查 / 词法细节 / 命令参考 → [语言手册](../manual/index.md)
- 工具链构建与故障排查 → [../../toolchains/README.md](../../toolchains/README.md)、[../../toolchains/docs/REBUILD.md](../../toolchains/docs/REBUILD.md)
- 跨语言性能对比 → [../../examples/projects/benchmarks/README.md](../../examples/projects/benchmarks/README.md)
