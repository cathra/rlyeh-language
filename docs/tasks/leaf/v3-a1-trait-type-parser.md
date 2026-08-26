# V3-A1：parser — trait 内关联类型声明载体

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-A 细分子任务）
> **状态**：📋 规划
> **风险**：低（纯 parse 层，不触碰类型系统）
> **依赖**：无
> **权威来源**：`rlyeh-parser`（`AstTraitDecl` / trait 成员解析）、`core.rl` 545

## 目标

parser 支持 trait 块内解析 `type Item;` 关联类型声明（`AstTraitDecl` 增 `types` 成员）。

## 背景

当前 parser 解析 trait 成员时只识别方法声明；`type Item;` 关联类型无 AST 载体（S1a 已验证 parser 载体缺失）。本子任务仅在 parse 层补载体，不涉及类型检查。

## 改动范围（parse 层）

- `AstTraitDecl` 增 `types: Vec<AstAssocType>`（`type Name;` 声明，可含 `type Name = 具体化;` 默认）。
- trait 成员解析循环识别 `type` 关键字开头的声明。
- **不含** typecheck / std 改动（由 V3-A2/A3 承担）。

## 验证

- [ ] `trait Iterator { type Item; fn next(&mut self) -> Option<Self::Item>; }` 可被 parser 接受并产出 `types` 载体。
- [ ] `type Item = i64;`（带默认具体化）可解析。
- [ ] 现有 trait 声明（无 `type` 成员）解析不回归。

## 为什么是低风险

仅新增 AST 字段与解析分支，编译器其余环节不动，失败影响面仅限 trait 含 `type` 声明的解析，独立可测。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-A（高风险）细化拆分而来 |
