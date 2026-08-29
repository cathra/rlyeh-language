# 内建解析器重构专项（Parser Rework）

> **所属阶段**：阶段 X（专项跟踪文档，非独立阶段）
> **状态**：📋 待办（集中登记所有涉及内建解析器重构的内容，最后统一修改）
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

集中跟踪所有涉及**编译器内建解析器**（`json::parse` / `toml::from_str` / 相关 desugar 分支）的重构项。按规则：凡涉及内建解析器重构的内容**先记录到本文档**，最后再统一修改。

## 背景

内建解析器（`check_json_parse` / `check_toml_parse` 及其 `json_parse_ast` / `toml_parse_ast` 递归 desugar 分支，`crates/rlyeh-typecheck/src/check_expr/{json.rs,toml.rs}`）当前以**字符串操作 desugar** 方式实现解析，返回目标类型 T（非 `Result`），**缺乏统一的运行时错误信号**：非法输入被静默吞掉或返回零值。改动会破坏现有 API 与测试，故集中登记、最后统一修改。

## 现状梳理（2026-08-28）

**入口与调用链**（`call.rs:45-60`）：
- `json::parse` / `json::from_str::<T>(s)` → `check_json_parse` → `json_parse_ast`（递归）
- `toml::parse` / `toml::from_str::<T>(s)` → `check_toml_parse` → `toml_parse_ast`（递归，带 inline 标志）
- 两者都经 turbofish 定型 `::<T>`，返回类型 = **T**（非 Result）。

**各类型当前 desugar 分支（均无错误信号）**：

| 目标类型 | json_parse_ast | toml_parse_ast |
|---------|---------------|----------------|
| `i64` | `string_to_int(s)` | 同 |
| `bool` | `if s == "true" { true } else { false }`（`"abc"` 静默→false） | 同 |
| `String` | `json_unescape(s)`（剥引号+转义，无引号闭合校验） | 同 |
| `Vec<T>` | —（json 不支持数组 MVP） | `substring(1,len-1)` 剥 `[]` + `split_quoted(",")` + for push |
| `HashMap<K,V>` | 剥 `{}` + `split_quoted(",")` + `find(":")` + insert | 剥 `{}` + `split_quoted(",")` + `find("=")` + insert |
| struct | 剥 `{}` + `split(",")` + 字段名 if-else 链 | 剥 `{}`/多行 + `split` + [section] 状态机 + 字段分派 |

**错误类型已存在**（`serde/module.rl`）：`JsonError { ParseError(String), InvalidType }`、`TomlError { ParseError(String), InvalidType }` ✅。

**已探测到的底层障碍**（2026-08-27）：`string_to_int("abc")`→0、`string_to_int("12x")`→12、`string_to_int("")`→0——**非法/部分合法输入无法区分**（`"abc"` 与 `"0"` 同返 0），i64 分支无法仅靠 string_to_int 判错，需新增严格校验 helper。

## 技术细节与已登记项

### 1. `json::parse` 非法输入改返回 `Result<T, JsonError>`（源自 X3）

- **目标**：`json::parse::<T>(s)` 返回 `Result<T, JsonError>`（当前返回 T）。`try_parse` 语义即此；`parse` 名义上仍可保留返回 T（unwrap 语义），故采用**新增 `json::try_parse::<T>(s) -> Result<T, JsonError>` + `parse` 退化为 `try_parse(...).unwrap()`** 的双入口方案，向后兼容破坏面最小。

- **核心改造**：`check_json_parse` 返回类型从 `T` 改为 `Result<T, JsonError>`，`json_parse_ast` 每个分支的 desugar AST 包裹为：
  ```
  { let __r = <严格校验 + 解析>;
    if __ok { Result::Ok(__r) } else { Result::Err(JsonError::ParseError(<消息>)) } }
  ```
  或直接短路：`if !<校验> { return Result::Err(...); } <解析>`（块表达式，校验失败提前返回 Err）。

- **各分支需注入的严格校验 + 错误路径**：
  - **i64**：新增 std helper `parse_int_strict(s) -> Result<i64, String>`（`core.rl`，逐字符校验全数字 + 可选负号；失败 `ParseError`）。替代 `string_to_int`。
  - **bool**：`if s == "true" { Ok(true) } else if s == "false" { Ok(false) } else { Err(ParseError) }`（当前 `"abc"` 静默→false，改造后报错）。
  - **String**：新增 `json_unescape_checked(s) -> Result<String, String>`——校验首尾闭合引号 + 转义合法性；失败 `ParseError`。
  - **HashMap<K,V>**：首尾 `{}` 闭合校验（`s.len() < 2 || s[0]!='{' || s[len-1]!='}'` → Err）+ 每段 `find(":")` 失败（非 `>=0`）→ Err（当前静默跳过）。
  - **struct**：首尾 `{}` 闭合校验 + 每段 `find(":")` 失败 → Err + 未知字段 `find(":")<0` 当前忽略、可保留宽松。
  - **嵌套/数组**：json 当前不支持嵌套 HashMap 值/数组（Unsupported），保持。

- **语言级依赖/障碍**：
  - `Result::Ok/Err` 构造函数可用（std Result ✅）；`JsonError::ParseError(String)` 变体构造可用。
  - **无语言级新障碍**（纯 desugar 注入校验 + Result 包裹），但**破坏面大**（见下）。
  - 需新增 std helper（`parse_int_strict` / `json_unescape_checked`）于 `core.rl`——**这本身是 std 新增，非内建解析器破坏**，可先行落地。

- **破坏面（需全量迁移）**：所有 `json::parse::<T>(s)` / `json::from_str::<T>(s)` 调用点从「当 T 用」改为「match 解包 `Result<T, JsonError>`」：
  - examples：`json_api.rl` / `json_serde.rl` / `json_derive.rl` / `toml_io.rl`（toml 部分见 #2）
  - tests：`json_serde.rl` / `json_derive.rl` 等 run-pass
  - std：`net/http.rl`（`json::parse::<T>(resp.text())` 需解包）
  - `json::from_reader::<T>`（`json.rs:730`）内部构造的 `json::parse` 调用链同步更新

- **推进建议（低风险拆分）**：
  1. 先行新增 `core.rl` 严格校验 helper（`parse_int_strict` / `json_unescape_checked`），不破坏现有
  2. 新增 `json::try_parse::<T>(s) -> Result<T, JsonError>` 内建（`check_json_try_parse`），`parse` 内部退化为 `try_parse(...).unwrap()`（向后兼容，现有调用点**无需迁移**）
  3. 逐步把测试/示例切到 `try_parse` + match 解包（可选，验证错误路径）
  - 此方案**破坏面最小**，`parse` 行为不变（非法输入仍 `unwrap` 死循环，与 `json::from_reader` 现状一致）。

- **风险**：中。**依赖**：`JsonError` 已存在 ✅。

### 2. `toml::from_str` 反序列化错误路径返回 `Result<T, TomlError>`

- **目标**：`toml::parse::<T>(s)` / `toml::from_str::<T>(s)` 返回 `Result<T, TomlError>`（当前返回 T）。与 #1 同构：新增 `toml::try_parse::<T>(s) -> Result<T, TomlError>`，`parse`/`from_str` 退化为 `try_parse(...).unwrap()`。

- **核心改造**：`check_toml_parse` + `toml_parse_ast` 各分支注入严格校验 + Result 包裹，**与 #1 共享同一套严格校验 helper**（i64/bool/String 分支同构）。

- **TOML 特有分支的校验注入**：
  - **Vec<T>**：首尾 `[]` 闭合校验 + 每元素 `parse_int_strict`/bool/String 校验（元素 parse 失败 → Err）。
  - **HashMap<K,V> 内联表**：首尾 `{}` 闭合 + `find("=")` 失败 → Err。
  - **struct 顶层多行**：**`=` 缺失行 → Err**（当前 `find("=")<0` 静默跳过）；`[section]` 头未闭合 `]` → Err；未知字段可保留忽略。
  - **inline 内联表**：首尾 `{}` 闭合 + `=` 缺失 → Err。

- **破坏面**：`toml_io.rl` / `x2_toml_section.rl` / `x2_toml_comments.rl` / 09-serialization 示例等所有 `toml::parse`/`toml::from_str` 调用点迁移为 match 解包。

- **语言级依赖**：`TomlError` 已存在 ✅；与 #1 共享 helper。

- **风险**：中。**依赖**：先完成 #1 的严格校验 helper 与 `try_parse` 内建框架，TOML 仅复用 + 新增 Vec/struct 特有校验。

### 3.（预留）内建解析器统一错误信息规范

- 若后续需要**结构化错误信息**（错误位置/期望 token/行号），统一登记于此。MVP 仅用 `ParseError(String)` 文本消息。

## 统一实施顺序（最后统一修改时）

1. `core.rl` 新增严格校验 helper（`parse_int_strict` / `json_unescape_checked`），本机 166 用例回归
2. `call.rs` + `json.rs` 新增 `check_json_try_parse`（`try_parse`），`parse` 退化为 unwrap（向后兼容）
3. `call.rs` + `toml.rs` 新增 `check_toml_try_parse`（`try_parse`），`parse` 退化为 unwrap
4. 新增测试：非法输入返回 Err（`parse_int_strict("abc")` → Err、bool `"abc"` → Err、String 未闭合引号 → Err、struct 缺冒号 → Err）
5. （可选）迁移示例到 `try_parse` + match，验证错误路径

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-27 | 建立专项文档；登记 X3 `json::parse` 返回 Err（含 string_to_int 非法输入探测结果） |
| 2026-08-28 | 细化：全量现状梳理（6 类型×json/toml 分支表）；#1 json 拆为 `try_parse` 双入口最小破坏方案 + 各分支严格校验注入点 + core.rl helper 先行；#2 toml 复用同构 + Vec/struct 特有校验；#3 预留错误信息规范；统一实施顺序 |
