# G3 裸指针

> **所属阶段**：阶段 G
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`*const T`/`*mut T` 类型 + `*p` 读写。

## 背景

阶段 阶段 G 子任务，详见 阶段详情文档 [`stages/G.md`](../../stages/G.md)。

## 技术细节

parser `ty.rs` `Token::Star` 分支 + AST `AstType::RawPtr` + typecheck `Type::RawPtr`；`*p` 读写（Deref 分支扩展，标量 load/store 与 `&T` 同构，codegen 同为 `i8*` 槽）；引用↔裸指针互视宽松规则（`compatible_with` 双向——参数检查是实参×形参方向）；`*mut` 降级 `*const`；聚合裸指针访问需显式 `(*p).field`。

## 验证

`raw_ptr.{rlyeh,out}` 7 输出。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
