# Rlyeh 语言手册（Language Manual）

> 最后更新：2026-08-31
>
> **定位**：面向已会使用 Rlyeh 的读者的**快速参考**：词法、类型系统、运算符、控制流、标准库 API 与工具链命令速查。
> **读者背景**：本文档旁注 `> **C 对照**` 专门帮**有 C 基础、Rust/Rlyeh 零基础**的读者建立直觉（例如"Rlyeh 的 `char` 是 32 位码点，不是 C 的 8 位 `char`"这类易错点）。
> **入门**：新手请先阅读 [tutorial](../tutorial/index.md)（安装 → 第一个程序 → 发布项目）。
> **完整教程**：渐进式语言教学（含示例讲解与已知限制）见 [guide](../guide/index.md)。
> **权威规范**：[grammar.md](../grammar.md)（EBNF）/ [semantics.md](../semantics.md) / [memory-model.md](../memory-model.md) / [actor-model.md](../actor-model.md) / [std-lib.md](../std-lib.md)。

---

## 章节导航

1. [语言概述](./01-overview.md)
2. [词法与字面量](./02-lexical.md)
3. [类型系统](./03-types.md)
4. [表达式与运算符](./04-expressions-operators.md)
5. [语句与控制流](./05-statements-control-flow.md)
6. [函数与闭包](./06-functions-closures.md)
7. [模块与可见性](./07-modules.md)
8. [泛型与单态化](./08-generics.md)
9. [内存模型](./09-memory.md)
10. [并发模型](./10-concurrency.md)
11. [标准库参考](./11-stdlib.md)（含 [标准库详述](./std/index.md)）
12. [编译器与构建](./12-compiler-build.md)
13. [工具链命令速查](./13-toolchain.md)
14. [已知限制](./14-limits.md)

---

## 标准库详述入口

[manual/std/index.md](./std/index.md) — 每个类型的成员、方法、签名与用例逐一说明。
