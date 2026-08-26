# Q3a `Display`/`Debug` trait + `Formatter`

> **所属阶段**：阶段 Q
> **状态**：✅ 已完成
> **依赖**：G1、I2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

定义 `Display`/`Debug` trait + `Formatter` 类型。

## 背景

阶段 阶段 Q 子任务，详见 阶段详情文档 [`stages/Q.md`](../../stages/Q.md)。

## 技术细节

std `fmt/module.rl`：`struct Formatter { buf: String }` + `impl Formatter { pub fn new() }`；`trait Display { fn fmt(&self, f: &mut Formatter) -> String }`、`trait Debug { fn fmt_debug(&self, f: &mut Formatter) -> String }`；core.rl 注入 `mod fmt;` + `use fmt::{...}`。MVP 签名降级：fmt 直接返回显示字符串（`Result<(), FmtError>` 未支持）；`Debug` 方法名用 `fmt_debug` 避免同名冲突。配套修复：`resolve_full_name` 加当前模块前缀回退 + `module_prefix` 设置。

## 验证

`display_fmt.{rlyeh,out}`（Q3：手写 Display/Debug 的 `{}`/`{:?}` 占位 + format! + dbg! + 内建类型兼容）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
