# E1 `--target` 交叉编译

> **所属阶段**：阶段 E
> **状态**：✅ 大部分完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`--target` 交叉编译（macOS 双架构基础）。

## 背景

阶段 阶段 E 子任务，详见 阶段详情文档 [`stages/E.md`](../../stages/E.md)。

## 技术细节

编译流水线目标无关（LLVM IR → clang），交叉编译 = 链接阶段注入 triple；`host_triple()`/`target_arch()`（arm64/aarch64 归一）/`is_cross_target()`；`assemble` 接受 target → clang `--target=<t>`；Actor 运行时跨架构跳过；API `build_executable_with_target`/`IncrementalDriver.with_target`；CLI `rlyeh build <file> --target <triple>`。

## 验证

`cross_compile_test.rs` 6 集成：arch/triple 归一化、本机编译运行、无效 target 报错、macOS 双架构产物、std 交叉编译 Rosetta。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
