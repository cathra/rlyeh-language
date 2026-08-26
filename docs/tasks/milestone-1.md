# Milestone 1 — 编译器 MVP（Month 0-4）

> **里程碑范围**：M1.1–M1.9
> **所属任务树**：[任务文档导航](./README.md) → 里程碑列表（[`CODEBUDDY.md`](../../CODEBUDDY.md) §6）
> **本层职责**：该里程碑细化后的**任务列表**（每个任务一个单任务文档，见 milestone-tasks/）。

> 「编译器 MVP」里程碑：完成 Rlyeh 编译器最小闭环（Lexer → Parser → HIR → 类型检查 → 区域系统 → MIR → 代码生成），能编译运行 hello-world.rl。

---

## 任务列表

| 任务 | 内容 | 状态 | 单任务文档 |
|------|------|------|-----------|
| M1.1 | Lexer 完成 | ✅ | [`m1-1.md`](milestone-tasks/m1-1.md) |
| M1.2 | Parser 完成 | ✅ | [`m1-2.md`](milestone-tasks/m1-2.md) |
| M1.3 | AST → HIR  lowering | ✅ | [`m1-3.md`](milestone-tasks/m1-3.md) |
| M1.4 | 类型检查器基础 | ✅ | [`m1-4.md`](milestone-tasks/m1-4.md) |
| M1.5 | 借用检查器 | ✅ | [`m1-5.md`](milestone-tasks/m1-5.md) |
| M1.6 | 区域系统 | ✅ | [`m1-6.md`](milestone-tasks/m1-6.md) |
| M1.7 | MIR + 基础优化 passes | ✅ | [`m1-7.md`](milestone-tasks/m1-7.md) |
| M1.8 | 代码生成 | ✅ | [`m1-8.md`](milestone-tasks/m1-8.md) |
| M1.9 | 能编译并运行 `hello-world.rl` | ✅ | [`m1-9.md`](milestone-tasks/m1-9.md) |

---

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 里程碑任务列表细化为单任务文档（milestone-tasks/），本文件改为任务列表 |
