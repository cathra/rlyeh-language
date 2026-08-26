# Q4 TOML 模块

> **所属阶段**：阶段 Q
> **状态**：✅ 已完成
> **依赖**：L2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

轻量 TOML 标量/嵌套表/数组 stringify/parse。

## 背景

阶段 阶段 Q 子任务，详见 阶段详情文档 [`stages/Q.md`](../../stages/Q.md)。

## 技术细节

`toml::to_string(v)` ≡ `toml::stringify(v)`（标量/数组/Vec/嵌套表/HashMap 内联表）；`toml::from_str::<T>(s)` ≡ `toml::parse::<T>(s)`（round-trip 对齐紧凑输出；复用 `json_escape`/`json_unescape`）。MVP 签名降级：无泛型 trait 约束、数组 `[T; N]` 反序列化不支持（用 Vec）、f64 不支持、紧凑输出（标准 TOML 空格形式/`[section]`/注释/多行字符串规划中）。

## 验证

`toml_io.{rlyeh,out}`（Q4：标量/数组/Vec/嵌套表/HashMap stringify/parse + round-trip）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
