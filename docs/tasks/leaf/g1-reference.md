# G1 引用类型与表达式

> **所属阶段**：阶段 G
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`&T`/`&mut T` 类型 + `&x`/`&mut x`/`*` 表达式全链路。

## 背景

阶段 阶段 G 子任务，详见 阶段详情文档 [`stages/G.md`](../../stages/G.md)。

## 技术细节

标量引用存 `i8*` 值槽（`*p` 按标量种类 bitcast）、聚合引用拷贝对象指针；字段访问/方法调用自动 `peel_ref`（`p.x`/`v.len()` 免显式解引用）；`&mut T` 兼容 `&T` 参数（宽松规则）。关键修复：MIR inline 补 AddrOf/DerefRead/DerefWrite 映射、DCE 活跃性纳入 Deref* 读、codegen 指针拷贝 `%%` 转义、顶层同名遮蔽（用户 `fn read` vs std extern：`read@shadow<N>` mangle）。

## 验证

`reference_test.rs` 10 用例（标量读写/struct 读写/vec 方法/引用链/返回引用/`&mut`→`&`）+ 全量 112 套件。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
