# V3-A1：parser — protocol 内关联类型声明载体

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-A 细分子任务）
> **状态**：✅ 已完成（`type Item;` ✅ + `type Item = i64;` 默认具体化 ✅，2026-08-27）
> **风险**：低（纯 parse 层，不触碰类型系统）
> **依赖**：无
> **权威来源**：`rlyeh-parser`（`AstProtocolDecl` / protocol 成员解析）、`core.rl` 545

## 目标

parser 支持 protocol 块内解析 `type Item;` 关联类型声明（`AstProtocolDecl` 增 `types` 成员）。

## 背景

当前 parser 解析 protocol 成员时只识别方法声明；`type Item;` 关联类型无 AST 载体（S1a 已验证 parser 载体缺失）。本子任务仅在 parse 层补载体，不涉及类型检查。

## 改动范围（parse 层）

- `AstProtocolDecl` 增 `types: Vec<AstAssocType>`（`type Name;` 声明，可含 `type Name = 具体化;` 默认）。
- protocol 成员解析循环识别 `type` 关键字开头的声明。
- **不含** typecheck / std 改动（由 V3-A2/A3 承担）。

## 实施情况（已完成，2026-08-27）

- `AstProtocolDecl.types: Vec<String>` 与 `parse_protocol` 的 `type` 分支（U2 已有）确认支持 `type Item;`。
- 补全 `type Item = i64;` 带默认具体化：`parse_protocol` 在 `type` 关键字后识别 `=`，消费默认类型（`parse_type`）后记录名字。AST 仅记录关联类型名（`Vec<String>`），默认具体化由 typecheck 消费（V3-A3 范围）。
- 新增 parser 测试 `test_protocol_assoc_type_decl`（decl.rs）：`type Item;` 与 `type Item = i64;` 均解析并产出 `types` 载体。

## 验证

- [x] `protocol Iterator { type Item; fn next(&mut self) -> Option<Self::Item>; }` 可被 parser 接受并产出 `types` 载体。
- [x] `type Item = i64;`（带默认具体化）可解析。
- [x] 现有 protocol 声明（无 `type` 成员）解析不回归（56 parser 测试全过）。

## 为什么是低风险

仅新增 AST 字段与解析分支，编译器其余环节不动，失败影响面仅限 protocol 含 `type` 声明的解析，独立可测。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-A（高风险）细化拆分而来 |
| 2026-08-27 | 完成 `type Item = i64;` 默认具体化解析 + 测试 |
