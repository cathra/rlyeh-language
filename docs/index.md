# Rlyeh 文档中心

> 版本：0.1.0（MVP）　最后更新：2026-08-31
>
> Rlyeh 是一门面向未来十年基础设施的**系统级编程语言**：内存安全与零 GC 默认、并发一等公民、数学式语法直觉。

本目录是 Rlyeh 文档的总入口。三份核心文档按用途区分：

| 文档 | 定位 | 适合谁 |
|------|------|--------|
| **[编程语言指南（guide）](./guide/index.md)** | 渐进式**语言教程**：从安装到并发、标准库的完整教学，示例均可运行 | 想系统学会这门语言的人 |
| **[语言手册（manual）](./manual/index.md)** | 面向已会者的**快速参考**：词法、类型系统、运算符、控制流、标准库 API 与工具链命令速查；含 **[标准库详述](./manual/std/index.md)** | 写代码时查语法/查 API 的人 |
| **[教程指南（tutorial）](./tutorial/index.md)** | 新手**安装 → 第一个程序 → 发布项目**全流程 | 第一次接触 Rlyeh 的人 |

## 权威规范（实现细节与语法精确定义）

- [grammar.md](./grammar.md) — 完整语法规范（EBNF）
- [semantics.md](./semantics.md) — 语义规则
- [memory-model.md](./memory-model.md) — 分层内存管理规范
- [actor-model.md](./actor-model.md) — Actor 并发模型规范
- [module-system.md](./module-system.md) — 模块系统规范
- [std-lib.md](./std-lib.md) — 标准库 API 规范（含规划中模块，与 `manual/std/` 互为补充）

## 性能与基准

- [performance.md](./performance.md) — **与 C / C++ / Go / Swift / Rust 全方位性能对比**（13 项基准 × 6 语言：运行时 + 编译时 + region 策略，含复现方法）

## 项目总纲与进度

- [../CODEBUDDY.md](../CODEBUDDY.md) — 项目总纲（架构、工具链、执行记录）
- [development-plan.md](./development-plan.md) — 开发计划（阶段 A–Z 已全部完成）
- [CHANGELOG.md](../CHANGELOG.md) — 版本变更记录

> 所有示例均已对照 `examples/`、`tests/run-pass/` 中可编译运行的代码。标准库 API 以 `crates/rlyeh-std/rlyeh/` 源码为最终权威；本文档与 `std-lib.md` 描述 MVP 现状。
