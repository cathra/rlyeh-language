# E2 WASM 目标支持

> **所属阶段**：阶段 E
> **状态**：✅ 已完成
> **依赖**：E1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`--target wasm32-wasi` 编译 + wasmtime 运行验证。

## 背景

阶段 阶段 E 子任务，详见 阶段详情文档 [`stages/E.md`](../../stages/E.md)。

## 技术细节

工具链安装（lld 含 wasm-ld、wasmtime、wasi-libc sysroot）；入口重命名 `main`→`__main_argc_argv(i32, i8**)`（crt1 链）+ `-nostdlib` 手动链接 crt1.o + libc.a + `adapt_wide_int_args`（i64→i32，寄存器实参前插 trunc 指令——LLVM 禁止 call 实参内嵌 trunc）。

## 验证

`wasm_target_test.rs` 3 用例：triple 判定 + hello world + std 特性数组/String/位运算。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
