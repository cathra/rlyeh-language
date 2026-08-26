# U2 trait 关联类型

> **所属阶段**：阶段 U
> **状态**：✅ 已完成
> **依赖**：U1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

parser/typecheck 支持 `trait T { type Item; }` 关联类型。

## 背景

阶段 阶段 U 子任务，详见 阶段详情文档 [`stages/U.md`](../../stages/U.md)。

## 技术细节

AST `AstTraitDecl.types`/`AstImplBlock.types`；parser trait/impl body 支持 `type` 成员 + `Self::Item` 路径；typecheck `TraitDef.assoc_types`/`ImplDef.assoc_types` + `TypeContext.assoc_types` 保存/恢复式填充，方法签名中 `Self::Item` 经 `resolve_ast_type` 查表替换，trait 声明期退化为 `Generic("Self::Item")` 占位。附带修复 codegen bug：`fn_llvm_type` 对 `Unit` 返回未处理 → void 返回。

## 验证

`assoc_type.{rlyeh,out}`（i64/String 关联类型、返回/参数/多关联、dyn 去虚拟化）+ `assoc_type_test.rs` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
