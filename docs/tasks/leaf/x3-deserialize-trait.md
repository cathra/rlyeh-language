# X3 `Deserialize` trait + Serializer/Deserializer 框架

> **所属阶段**：阶段 X
> **状态**：✅ 已完成（`Deserialize` trait ✅；`JsonError`/`TomlError` ✅；Serializer/Deserializer 访问器框架 ✅；`json::try_parse::<T> -> Result<T, JsonError>` Err 路径 ✅，2026-08-30）
> **依赖**：U4、U3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`trait Deserialize { fn from_json(s: String) -> Self; }` + Serializer/Deserializer 框架。

## 背景

阶段 阶段 X 子任务，详见 阶段详情文档 [`stages/X.md`](../../stages/X.md)。

## 技术细节

`trait Deserialize { fn from_json(s: String) -> Self; }`（依赖 U4 `-> Self`，替换「解析统一走内建」退化）；`Serializer`/`Deserializer` 访问器框架（目标签名 `serialize(&self, &mut Serializer) -> Result<(), SerError>`，U3 约束泛型化 `to_string<T: Serialize>` 落地）；`JsonError`/`TomlError` 类型；`json::parse` 非法输入改返回 `Err`。

## 实施情况（2026-08-27，核心完成）

- **`Deserialize` trait + 手写 impl**：`trait Deserialize { fn from_json(s: String) -> Self; }`，`-> Self` 返回可行（U4）；`impl Deserialize for Point` 手写 + `Point::from_json(...)` 调用（`x3_deserialize_trait.rl`）。泛型 trait 约束 `fn f<T: Foo>(x: T)` 验证可行（`to_string<T: Serialize>` 前置）。
- **std serde 模块**：serde/module.rl 定义 `Deserialize` trait + `enum JsonError { ParseError(String), InvalidType }` + `enum TomlError`；用户经 `serde::JsonError`/`serde::TomlError` 路径访问（`x3_error_types.rl`：`Result<i64, JsonError>` + match 解包）。
- **Serializer 访问器框架**：serde/module.rl 新增 `struct Serializer { buf: String }`（`serialize_i64`/`serialize_str`/`result`）；手写 serialize + 泛型 `to_string<T: Serialize>` 验证可行；用户经 `serde::Serializer` 字面量构造（`serde::Serializer::new` 静态方法在模块中不解析，Rlyeh 模块静态方法限制）（`x3_serializer_framework.rl`）。
- **Deserializer 访问器框架**：serde/module.rl 新增 `struct Deserializer { buf: String, pos: i64 }`（`deserialize_i64`/`deserialize_str`/`next_token`），与 Serializer 对称（反向读取）；`deserialize_i64`/`deserialize_str` 截取从 pos 到分隔符（`:`/`,`/`}`/`]`）子串并推进 pos，`next_token` 跳过单个分隔符；用户经 `serde::Deserializer` 字面量构造（`Deserializer::new` 静态方法同样模块限制）（`x3_deserializer_framework.rl`，`.out` 验证 `16/30/name/rlyeh`）。
- **专项转移**：`json::parse` 返回 `Err` 涉及内建解析器重构，已登记至专项文档 [`parser-rework.md`](./parser-rework.md)（待统一修改）。
- **待办（非阻塞）**：`Serializer::new`/`Deserializer::new` 静态方法模块解析（Rlyeh 模块静态方法限制，用户可用字面量构造替代）。

## 实施情况（2026-08-28/30，Err 路径落地）

- **`json::try_parse::<T>(s) -> Result<T, serde::JsonError>`**（P2，2026-08-28）：`check_json_try_parse` → `json_try_parse_ast`（`crates/rlyeh-typecheck/src/check_expr/json.rs`）。各标量分支注入严格校验 + `Result` 包装：i64 → `parse_int_strict(s).is_ok()`、`bool` → `s == "true" || s == "false"`、`String` → `json_unescape_checked(s).is_ok()`、HashMap/struct → 首尾 `{}`（123）闭合校验；校验失败返回 `Result::Err(serde::JsonError::ParseError(msg))`。`json::parse::<T>(s)` 仍走原 `json_parse_ast`（返回 T，向后兼容，非法输入行为不变）。
- **`toml::try_parse::<T>(s) -> Result<T, serde::TomlError>`**（P3，2026-08-28）：`check_toml_try_parse` → `toml_try_parse_ast`（`toml.rs`），与 JSON 同构，复用同一套严格校验 helper + TOML 特有的 `[]`/`{}` 闭合与 `=` 缺失校验。
- **std 严格校验 helper**（`core.rl`，P2/P3）：`parse_int_strict(s) -> Result<i64, String>`（全数字 + 可选负号，替代 `string_to_int` 宽松停止）、`json_unescape_checked(s) -> Result<String, String>`（首尾引号闭合 + 转义合法性）、`parse_float_strict`（`toml.rs` f64 严格校验）。
- 测试：`p2_json_try_parse.rl`（i64/bool/String 合法 + 非法输入 Err 路径）、`p3_toml_try_parse.rl`、`x3_error_types.rl`（`Result<T, JsonError>` 错误处理）。

## 验证

- [x] 手写 `impl Deserialize` + `from_json` 调用（`x3_deserialize_trait.rl`）。
- [x] `JsonError`/`TomlError` 类型 + `Result<T, JsonError>` 错误处理（`x3_error_types.rl`）。
- [x] Serializer 访问器框架 + 手写 serialize + 泛型 to_string（`x3_serializer_framework.rl`）。
- [x] Deserializer 访问器框架 + 手写 deserialize（`x3_deserializer_framework.rl`，`.out` 验证）。
- [x] `json::try_parse::<T>(s)` 返回 `Result<T, JsonError>`——合法输入 `Ok`、非法输入 `Err`（i64 `"abc"`/`"12x"`、bool `"abc"`、未闭合 String，覆盖 `p2_json_try_parse.rl`；`toml::try_parse` 见 `p3_toml_try_parse.rl`）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-27 | Deserialize trait + JsonError/TomlError 错误类型 + Result 错误处理 |
| 2026-08-27 | Deserializer 访问器框架（与 Serializer 对称），157 用例全过 |
| 2026-08-30 | `json::try_parse`/`toml::try_parse` Err 路径 + `parse_int_strict`/`json_unescape_checked`/`parse_float_strict` helper 落地；状态置 ✅；全量套件通过 |
