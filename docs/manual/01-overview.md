# 1. 语言概述

Rlyeh 是一门面向未来十年基础设施的**系统级编程语言**，设计目标：

| 目标 | 对标 |
|------|------|
| 内存安全、零 GC | Rust |
| 编译速度极快 | Go |
| 并发模型一等公民 | Erlang / Akka |
| 数学式语法直觉 | Python / MATLAB |

**一句话定位**：Rlyeh = Rust 的安全性 + Go 的编译速度 + Erlang 的并发模型 + 数学的自然表达。

当前编译器为 Rust 实现的 bootstrap 阶段（自举目标），代码生成走 LLVM IR。

## MVP 已落地核心能力

- 标量 / 聚合类型、引用与借用（严格借用检查）、`ref`/`ref mut` 模式
- `struct` / `enum`（具名变体 + 显式判别式）/ `trait` + `impl` + 泛型单态化
- 类型联合 `A | B`、枚举显式判别式（`U1`–`U3`）
- 切片引用 `&[T]` / `&mut [T]`（`S1`–`S3`）、动态切片、`as` 数值转换（`U6`）
- 闭包（H1 函数指针 / H2 无捕获 / H3 捕获 IIFE / H5 闭包值对象）、`dyn Trait`（H4）
- 分层内存（L0 所有权 → L1 区域 → L2 `Rc`/`Arc` → L3 `Gc`）
- Actor 并发（监督 / 交叉编译 WASM，`L4`）、`async`/`.await`（S1/W1–W5）
- 标准库：String / Vec / HashMap / Option / Result / 时间 / IO / 网络 / 同步 / 序列化（JSON+TOML）/ 迭代器

---

[← 返回手册目录](./index.md) | [下一章：词法与字面量 →](./02-lexical.md)
