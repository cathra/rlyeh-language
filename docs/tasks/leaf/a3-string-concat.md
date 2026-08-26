# A3 String 拼接拷贝语义

> **所属阶段**：阶段 A
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

消除 `a + b` 共享缓冲别名隐患，改为深拷贝拼接。

## 背景

阶段 阶段 A 子任务，详见 阶段详情文档 [`stages/A.md`](../../stages/A.md)。

## 技术细节

原 `a + b` desugar 为 `let __s = a; __s.push_str(b)`——3 槽值拷贝共享 data 缓冲，拼接结果与左操作数互相污染。修复为 `let __s = a.clone(); __s.push_str(b)`：std `String::clone()` 深拷贝（`String::new()` + 逐字节 `push_byte`）+ typecheck desugar 改调用 clone。

## 验证

`string_concat_test.rs` 9 用例（result_isolation/lhs_isolation/clone_direct）；`concat_grow` cap 预期 24→16。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
