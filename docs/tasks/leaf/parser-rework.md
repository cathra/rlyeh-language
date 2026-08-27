# 内建解析器重构专项（Parser Rework）

> **所属阶段**：阶段 X（专项跟踪文档，非独立阶段）
> **状态**：📋 待办（集中登记所有涉及内建解析器重构的内容，最后统一修改）
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

集中跟踪所有涉及**编译器内建解析器**（`json::parse` / `toml::from_str` / 相关 desugar 分支）的重构项。按规则：凡涉及内建解析器重构的内容**先记录到本文档**，最后再统一修改。

## 背景

内建解析器（`check_json_parse` / `check_json_parse` 的 desugar 分支等，`crates/rlyeh-typecheck/src/check_expr/json.rs`）当前以**字符串操作 desugar** 方式实现解析，缺乏统一的运行时错误信号，且改动会破坏现有 API 与测试。

## 技术细节与已登记项

### 1. `json::parse` 非法输入改返回 `Err`（源自 X3）

- **目标**：`json::parse::<T>(s)` 非法输入返回 `Err(JsonError)`（当前返回 T）。
- **现状**：`check_json_parse` 对 i64 生成 `string_to_int(s)`、bool 生成 `if s == "true"`、String 生成 `json_unescape(s)`、struct/HashMap 生成 `split(",")` 分段 desugar——**均无统一非法输入错误信号**。
- **已探测到的障碍**（2026-08-27）：
  - `string_to_int("abc")` 返回 `0`、`string_to_int("12x")` 返回 `12`、`string_to_int("")` 返回 `0`——**非法/部分合法输入无法区分**（`"abc"` 与 `"0"` 同返 0）。
  - 因此 i64 分支需新增**严格格式校验**（全数字正则），bool 需 `s == "true" || s == "false"` 且其余报 Err，String 需引号闭合校验，struct/HashMap 需结构完整性校验。
- **破坏面**：现有使用点（`json_api.rl` / `json_serde.rl` / `json_derive.rl` 及 examples）`json::parse::<T>(s)` 直接当 T 用，需全量迁移为 match 解包 `Result<T, JsonError>`。
- **依赖**：`JsonError` 类型已存在（std serde/module.rl ✅）；需内建解析器接入严格校验 + 错误路径。

### 2.（预留）TOML 反序列化错误路径

- 若 `toml::from_str` 后续引入 Result 错误返回，一并登记于此。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-27 | 建立专项文档；登记 X3 `json::parse` 返回 Err（含 string_to_int 非法输入探测结果） |
