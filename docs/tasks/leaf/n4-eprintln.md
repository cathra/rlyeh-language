# N4 `eprintln!`/`eprint!` 宏

> **所属阶段**：阶段 N
> **状态**：✅ 已完成
> **依赖**：M、I2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

内置格式化宏扩展 stderr 输出。

## 背景

阶段 阶段 N 子任务，详见 阶段详情文档 [`stages/N.md`](../../stages/N.md)。

## 技术细节

parser `is_builtin_macro` + typecheck `builtin_signature`/`check_call` String 分支/宏分发 + LIR/IR 内建注册 + codegen 内建 `eprint`/`eprintln`/`eprint_string`/`eprintln_string`；POSIX `dprintf(2, ...)` 直写 stderr（不依赖 `stderr` 符号，跨平台）。

## 验证

`eprintln.rl` 验收：占位符/String 变量/非 String 参数/无参空行。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
