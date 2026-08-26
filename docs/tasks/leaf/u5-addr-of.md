# U5 MIR `AddrOf` 任意目标 + 字段/索引真实取址

> **所属阶段**：阶段 U
> **状态**：✅ 已完成
> **依赖**：U4
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`&`/`&mut` 目标放宽 + 字段/索引 GEP 真实取址。

## 背景

阶段 阶段 U 子任务，详见 阶段详情文档 [`stages/U.md`](../../stages/U.md)。

## 技术细节

typecheck `&`/`&mut` 目标放宽为变量/解引用 `&*p`/不可变表达式；MIR `Ref{Deref{..}}` 折叠直接透传指针（消除拷贝临时再取址语义错误）。**后置 V1 真实取址**：`&obj.field`/`&arr[i]` 生成真实槽地址——MIR `AddrOfField`/`PtrAdd` 指令 + codegen `FieldAddr`/`PtrAdd`（inttoptr + `%p + %index`）；`&mut arr[i] = v` 经 DerefWrite 写回。配套：MIR DCE/内联识别新指令、typecheck `substitute` 补 RawPtr、`StructDef.type_params` 字段访问替换。

## 验证

`addr_of_fold.{rlyeh,out}` + `addr_of_field_index.{rlyeh,out}` + `addr_of_test.rs` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
