# U8 泛型结构体构造 + 泛型 protocol

> **所属阶段**：阶段 U
> **状态**：✅ 已完成
> **依赖**：U7
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`Foo<i64> { x: 1 }` 泛型构造 + `impl<T> X: Protocol<T>`。

## 背景

阶段 阶段 U 子任务，详见 阶段详情文档 [`stages/U.md`](../../stages/U.md)。

## 技术细节

① 泛型结构体构造：`ExprKind::StructCtor.type_args` 字段 + parser `looks_like_generic_struct_ctor` 前瞻 + typecheck `check_struct_construct` substitute 替换泛型字段 + `GenericArityMismatch`；② 泛型 protocol：parser `looks_like_generic_protocol_impl` 前瞻（`<...>for`）+ typecheck 方法解析经 `find_impl_for_method` + `unify` 绑定 impl 泛型。

## 验证

`generic_ctor_protocol.{rl,out}` 6 断言（泛型构造 + 泛型 impl + 方法泛型 + 泛型 protocol）+ 全量 121 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
