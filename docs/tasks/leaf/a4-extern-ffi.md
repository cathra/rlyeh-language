# A4 通用 FFI `extern fn` 声明

> **所属阶段**：阶段 A
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

打通 `extern fn` 声明全链路（parser→typecheck→HIR→MIR→LIR→LLVM→链接）。

## 背景

阶段 阶段 A 子任务，详见 阶段详情文档 [`stages/A.md`](../../stages/A.md)。

## 技术细节

AST `AstFnDecl.is_extern` + parser 识别 `extern` 前缀；typecheck 允许无 body 并序列化签名（`type_to_extern_name`）；HIR `HirFnDecl.is_extern` + `extern_sig`；MIR `MirFunction.is_extern`（空 CFG，DCE 跳过）；LIR `parse_extern_type`（未知名→Ptr）+ extern 跳过类型推断；codegen 对 extern 生成 `declare` 而非 `define`。

## 验证

`ffi_extern_test.rs` 3 用例：i64（labs）/f64+void（fabs、srand）/嵌套+循环+运算。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
