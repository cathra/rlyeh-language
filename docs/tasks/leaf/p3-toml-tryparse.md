# P3 toml::from_str 严格校验 + `try_parse`

> **所属专项**：[专项开发计划](../专项开发计划.md)（P3）
> **来源缺陷**：[`leaf/parser-rework.md`](parser-rework.md) #2（TOML 反序列化错误路径）
> **状态**：✅ 已完成（2026-08-28，标量 + Vec 首尾 `[]`/HashMap/struct 首尾 `{}` 校验）
> **风险**：中
> **前置能力**：**P2**（复用 `parse_int_strict`/`json_unescape_checked` helper 与 try_parse 内建框架）

## 目标

新增 `toml::try_parse::<T>(s) -> Result<T, TomlError>`；`parse`/`from_str` 退化为 `try_parse(...).unwrap()`。

## 现状（2026-08-28）

- 入口：`call.rs:58-60` → `check_toml_parse` → `toml_parse_ast`（递归，带 inline 标志）
- 当前 `toml::parse::<T>(s)` 直接返回 **T**，非法输入被静默吞掉：
  - 标量分支与 json 同构（`string_to_int`/bool 静默/`json_unescape`）
  - `Vec<T>`：剥 `[]` + `split_quoted(",")` + for push（无闭合校验、元素无校验）
  - `HashMap` 内联表：剥 `{}` + `split_quoted(",")` + `find("=")`（失败静默跳过）
  - struct 顶层多行：`split("\n")` + `find("=")`（缺失行静默跳过）+ `[section]` 状态机
- 错误类型 `TomlError { ParseError(String), InvalidType }` 已存在 ✅

## 技术方案

标量分支复用 P2 helper；TOML 特有校验：

| 分支 | 校验注入 |
|------|---------|
| `Vec<T>` | 首尾 `[]` 闭合校验 + 每元素 `parse_int_strict`/bool/String 校验（元素 parse 失败 → Err） |
| `HashMap` 内联表 | 首尾 `{}` 闭合 + `find("=")` 失败 → Err |
| struct 顶层多行 | **`=` 缺失行 → Err**（当前静默跳过）；`[section]` 头未闭合 `]` → Err；未知字段可保留忽略 |
| struct 内联表 | 首尾 `{}` 闭合 + `=` 缺失 → Err |

## 波及范围

- `call.rs` + `toml.rs`：新增 `check_toml_try_parse`
- `toml::parse`/`from_str` 退化为 unwrap
- 调用点（`toml_io.rl`/`x2_toml_section.rl`/`x2_toml_comments.rl`）：`parse` 行为不变，无需迁移

## 执行步骤

1. `call.rs` + `toml.rs` 新增 `check_toml_try_parse`
2. 标量分支复用 P2 helper；注入 Vec/struct 特有校验
3. `parse`/`from_str` 退化为 unwrap

## 验收标准

- 同 P2（TomlError）；非法 TOML 返回 `Err(TomlError::ParseError)`
- `x2_toml_section.rl`/`toml_io.rl` 回归全绿

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 由专项开发计划 P3 生成叶子文档 |
| 2026-08-28 | ✅ 完成：`toml.rs` 新增 `check_toml_try_parse` + `toml_try_parse_ast`（复用 P2 严格 helper `parse_int_strict`/`json_unescape_checked`；标量严格校验 + Vec 首尾 `[]`/HashMap/struct 首尾 `{}` 闭合校验；`Result::Ok(<toml_parse_ast>)/Err(serde::TomlError::ParseError(msg))`）；`call.rs` 新增 `toml::try_parse` 分派；`mod.rs` re-export。验证：i64 非法 `"abc"`→Err、bool `"abc"`→Err、Vec 未闭合 `[1,2,3`→Err；新增 run-pass `p3_toml_try_parse.rl/.out`；cargo test 全绿。`toml::parse` 保持宽松向后兼容。后续：Vec/struct 深层逐元素校验 + 错误定位 |
