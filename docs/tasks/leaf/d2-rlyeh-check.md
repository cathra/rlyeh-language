# D2 `rlyeh check` 静态分析器

> **所属阶段**：阶段 D
> **状态**：✅ 已完成
> **依赖**：D1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`rlyeh check` 静态分析器（tools/rlyeh-check）。

## 背景

阶段 阶段 D 子任务，详见 阶段详情文档 [`stages/D.md`](../../stages/D.md)。

## 技术细节

4 条 AST lint 规则 + parse-error——`unused-variable`（作用域栈 + 遮蔽查找，`_` 与 self 豁免）、`constant-condition`、`redundant-compare`、`unreachable-code`；诊断格式 `line:col: level[rule]: message`；CLI `rlyeh check <file>`（有诊断 exit 1）。driver 挂载 fmt/check 子命令。

## 验证

`rlyeh-check` 12 单测 + `fmt_check_test.rs` 5 集成。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
