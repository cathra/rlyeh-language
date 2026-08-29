# P2 json::parse 严格校验 + `try_parse`

> **所属专项**：[专项开发计划](../专项开发计划.md)（P2）
> **来源缺陷**：[`leaf/parser-rework.md`](parser-rework.md) #1（json::parse 非法输入改返回 Err，源自 X3）
> **状态**：✅ 已完成（2026-08-28，标量 i64/bool/String 严格校验；HashMap/struct 首尾 `{}` 校验）
> **风险**：中
> **前置能力**：无（P1 无关）；需先新增 `core.rl` 严格校验 helper

## 目标

新增 `json::try_parse::<T>(s) -> Result<T, JsonError>`（严格校验，非法输入返回 `Err`）；`parse`/`from_str` 退化为 `try_parse(...).unwrap()`（**向后兼容**，现有调用点无需迁移）。

## 现状（2026-08-28）

- 入口：`call.rs:45-60` → `check_json_parse` → `json_parse_ast`（递归 desugar）
- 当前 `json::parse::<T>(s)` 直接返回 **T**（非 Result），非法输入被静默吞掉或返回零值：
  - `i64` → `string_to_int(s)`：`"abc"`→0、`"12x"`→12、`""`→0（无法区分非法/部分合法）
  - `bool` → `if s == "true" { true } else { false }`：`"abc"` 静默→false
  - `String` → `json_unescape(s)`：无引号闭合校验
  - `HashMap`/struct → 剥 `{}` + `split_quoted(",")` + `find(":")`：`find` 失败静默跳过
- 错误类型 `JsonError { ParseError(String), InvalidType }` 已存在（`serde/module.rl`）✅

## 技术方案

`json_parse_ast` 各分支 desugar 包裹为 `Result<T, JsonError>`：
```
{ let __r = <严格校验 + 解析>;
  if __ok { Result::Ok(__r) } else { Result::Err(JsonError::ParseError(<消息>)) } }
```

各分支严格校验：

| 分支 | 校验注入 |
|------|---------|
| `i64` | 新增 `core.rl` `parse_int_strict(s) -> Result<i64, String>`（全数字 + 可选负号），替代 `string_to_int` |
| `bool` | 三态：`s=="true"`→Ok(true) / `s=="false"`→Ok(false) / 其余→Err |
| `String` | 新增 `core.rl` `json_unescape_checked(s) -> Result<String, String>`（引号闭合 + 转义校验） |
| `HashMap`/struct | 首尾 `{}` 闭合校验 + 每段 `find(":")` 失败（非 `>=0`）→ Err |

## 波及范围

- `core.rl`：新增 `parse_int_strict` / `json_unescape_checked` helper（std 新增，非破坏，可先行）
- `call.rs` + `json.rs`：新增 `check_json_try_parse` 入口
- `json::parse`/`from_str` 内部退化为 `try_parse(...).unwrap()`；`json::from_reader`（`json.rs:730`）调用链同步
- 现有调用点（`json_api.rl`/`json_serde.rl`/`json_derive.rl`/`net/http.rl`）：`parse` 行为不变，**无需迁移**

## 执行步骤

1. `core.rl` 新增 `parse_int_strict` 与 `json_unescape_checked`；166 用例回归
2. `call.rs` + `json.rs` 新增 `check_json_try_parse` 入口（返回 `Result<T, JsonError>`）
3. 各分支注入校验（i64/bool/String/HashMap/struct）
4. `json::parse`/`from_str` 退化为 `try_parse(...).unwrap()`；`json::from_reader` 同步
5. 新增测试：非法输入（`"abc"`/未闭合引号/缺冒号）返回 Err

## 验收标准

- `try_parse` 合法输入返回 `Ok`、非法返回 `Err(JsonError::ParseError)`
- 现有 `parse` 行为不变；回归全绿

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 由专项开发计划 P2 生成叶子文档 |
| 2026-08-28 | ✅ 完成：`core.rl` 新增 `parse_int_strict`/`json_unescape_checked` 严格 helper；`json.rs` 新增 `check_json_try_parse` + `json_try_parse_ast`（构造块 AST：`String::from(arg)` → 严格校验 → `Result::Ok(<json_parse_ast>)` / `Result::Err(serde::JsonError::ParseError(msg))`）；`call.rs` 新增 `json::try_parse` 分派；`mod.rs` re-export。验证：i64 非法 `"abc"`/部分合法 `"12x"`→Err（此前 string_to_int 宽松解析）、bool `"abc"`→Err、String 未闭合引号→Err；新增 run-pass `p2_json_try_parse.rl/.out`；cargo test 全绿。`json::parse` 保持宽松向后兼容（现有调用点不迁移）。后续：HashMap/struct 深层逐段校验 + 错误定位 |
