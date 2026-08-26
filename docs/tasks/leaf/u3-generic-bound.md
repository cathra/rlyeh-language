# U3 泛型 trait 约束（bound / where）

> **所属阶段**：阶段 U
> **状态**：✅ 已完成
> **依赖**：U2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`fn f<T: Bound>` 内联 bound + `impl where K: B1 + B2` 子句。

## 背景

阶段 阶段 U 子任务，详见 阶段详情文档 [`stages/U.md`](../../stages/U.md)。

## 技术细节

AST 新增 `AstTypeParam { name, bounds }`（fn/struct/enum/trait/impl 五处泛型升级）+ parser `parse_generics`/`parse_where_clause`；typecheck `FnTemplate.bounds`/`ImplDef.bounds` 记录 + `check_generic_bounds` 调用点宽松校验（单态化实参确定后查 `impl Trait for Concrete`）+ `GenericBoundMismatch` 错误；泛型函数体内 `t.area()` 经单态化替换自然解析；fmt/doc `fmt_generics`。

## 验证

`bound_test.{rlyeh,out}`（单/多 bound、混合参数、多次实例化）+ `bound-mismatch.rl` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
