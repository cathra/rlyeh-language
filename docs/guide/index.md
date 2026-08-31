# Rlyeh 编程语言指南

> 版本：0.1.0（MVP）　最后更新：2026-08-31
>
> 本文为面向读者的**语言教程**（主文档）。所有示例均为 `examples/`、`tests/run-pass/` 中可编译运行的已验证代码（或其简化）。
> 新手入门（安装 → 第一个程序 → 实战）见 [tutorial](../tutorial/index.md)；语言速查 / 命令参考见 [manual](../manual/index.md)。
> 权威规范见：[grammar.md](../grammar.md)、[semantics.md](../semantics.md)、[memory-model.md](../memory-model.md)、[actor-model.md](../actor-model.md)、[std-lib.md](../std-lib.md)。

---

## 章节导航

1. [认识 Rlyeh](./01-what-is-rlyeh.md)
2. [快速上手](./02-quick-start.md)
3. [基础语法](./03-basic-syntax.md)
4. [数学式条件判断](./04-math-conditions.md)
5. [聚合类型与泛型](./05-aggregates-generics.md)
6. [数组与索引](./06-arrays-slices.md)
7. [模块系统](./07-modules.md)
8. [内存管理：分层所有权模型](./08-memory.md)
9. [Actor 并发模型](./09-actors.md)
10. [标准库](./10-stdlib.md)
11. [编译目标与工具链](./11-targets-toolchain.md)
12. [外部函数接口（FFI）](./12-ffi.md)
13. [参考与已知限制](./13-references-limits.md)

---

## 一句话定位

Rlyeh = Rust 的安全性 + Go 的编译速度 + Erlang 的并发模型 + 数学的自然表达。

当前编译器为 Rust 实现的 bootstrap 阶段（自举目标），代码生成走 LLVM IR。
