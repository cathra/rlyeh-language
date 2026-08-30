# X2 标准 TOML + 解析鲁棒性

> **所属阶段**：阶段 X
> **状态**：✅ 已完成（`key = value` 空格 + round-trip + 整行注释 + 引号感知 + `[section]` 行式（含多级 `[a.b]`）+ f64 + `[T; N]` 数组 + 多行字符串 `"""` 全部落地，2026-08-30）
> **依赖**：U3/U4、Q4
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`toml::to_string` 输出标准形式 + 解析鲁棒性。

## 背景

阶段 阶段 X 子任务，详见 阶段详情文档 [`stages/X.md`](../../stages/X.md)。

## 技术细节

`toml::to_string` 输出标准形式——`key = value` 空格、`[section]` 行式子表（替代内联表）、`#` 注释、多行字符串（`"""`）；`toml::from_str` 接受注释/空格/子表；修复 §9.4 已知限制——值含逗号/嵌套容器分段不可靠（替换 `split(',')` 为逐字符/引号感知解析）、`[T; N]` 数组反序列化、f64 支持。

## 实施情况（2026-08-27，部分完成）

- **`toml::to_string` 标准空格**：`toml_ser.rs` struct 字段 `{fname} = ` + HashMap ` = `（`=` 两侧空格）。
- **`toml::from_str` round-trip 空格解析**：`toml.rs` struct 解析 `substring` 后加 `trim()` 剥离键/值空格（`key = value` → `key`/`value`）。
- **整行注释支持**：`toml::from_str` 跳过纯注释/空行（`#` 开头的行 `find("=")` 为 -1 自动忽略，`x2_toml_comments.rl`）。
- **引号感知解析（§9.4 修复）**：core.rl 新增 `split_quoted(s, delim)`（跳过双引号字符串内的分隔符）；`toml.rs` 的 Vec/嵌套 struct 内联表/HashMap 解析改用 `split_quoted` 替代 `split(",")`——值含逗号的字符串在内联表/数组/嵌套容器中正确分段（`x2_toml_quoted.rl`）。
- **`CACHE_VERSION` 4→5**：IR 生成变化，递增使增量缓存失效。
- **`[section]` 行式子表（完整实现，含多级 `[a.b]` 路径）**：
  - **序列化**（`toml_ser.rs`）：`toml_serialize_ast_path`（带 `sec_path` 路径栈）——顶层嵌套 struct 字段输出 `\n[point]\nx = 7\ny = 9`；多级嵌套输出 `[inner]\n[inner.p]`（点号路径，`is_nested_struct_type` 判断嵌套 struct）。
  - **反序列化**（`toml.rs`）：for 循环内 `[section]` 头检测（`find("[") == 0` 设置 `__cur_sec`）+ `dispatch` 按 `__cur_sec` 分派——顶层（`__cur_sec.len() == 0`）走原字段 if-else，`build_section_branches` 递归生成所有嵌套路径分支（`else if __cur_sec == "inner.p"`），`build_inner_field_dispatch`（接受路径）递归 FieldAccess 归组。
  - round-trip 正常（`x2_toml_section.rl` 单级 3+4、`x2_toml_section_multi.rl` 多级 16 + 直接 `[inner.p]` 输入）。
- 测试：`x2_toml_standard.rl`（标准空格 round-trip）、`x2_toml_comments.rl`（整行注释）、`x2_toml_quoted.rl`（引号感知）、`x2_toml_section.rl`（单级 [section]）、`x2_toml_section_multi.rl`（多级 [a.b]）。

## 实施情况（2026-08-30，全部完成）

- **f64 序列化 / 反序列化**（`toml_ser.rs` / `toml.rs`）：
  - 序列化：`Type::F64` → `float_to_string(arg)`（裸浮点值，无引号，`3.5`）。
  - 反序列化：`Type::F64` → `string_to_float(s)`；`try_parse` 严格校验走 `parse_float_strict`（合法十进制浮点 Ok，否则 Err）。
  - round-trip 正常（`x2_toml_f64.rl`：裸 `3.5` 反序列化、`to_string` 后再反序列化均得 3.5）。
- **固定数组 `[T; N]` 反序列化**（`toml.rs`）：
  - `Type::Array(elem_ty, n)` → 剥 `[]` 后 `split_quoted` 引号感知分段（元素含逗号字符串安全），静态展开 `[parse(parts[0]), ..., parse(parts[N-1])]`（N 编译期已知）。
  - 嵌套 `[[i64; 2]; 2]` 经 elem 类型递归展开为多级数组字面量。
  - `try_parse` 闭合校验同 `Vec`：首尾 `[]`（91 / 93）。
  - 序列化（`toml_ser.rs`）：`Type::Array(elem, len)` 静态展开 `[e0, e1, ...]`（长度编译期已知）。
  - round-trip 正常（`x2_toml_array.rl`：扁平 `[i64; 3]` = 6、嵌套 `[[i64; 2]; 2]` = 10）。
- **多行字符串 `"""..."""` 反序列化**（`toml.rs` 的 `toml_string_value_ast`）：
  - 先 `trim` 去除首尾空白（struct 字段 `key = """...""` 值含前导空格），再判定多行字符串。
  - 首尾各 3 字节均为 `"`（ASCII 34）且长度 ≥ 6 时 `substring(3, len-3)` 剥离 `"""` 定界得原始内容；否则走 `json_unescape`（JSON 风格转义还原）。
  - 序列化（`toml_ser.rs`）：String 含换行（10）序列化为 TOML 多行字符串 `"""...""`（原始不转义），否则 `"` + `json_escape(s)` + `"`（复用 JSON 转义）。
  - round-trip 正常（`x2_toml_multiline.rl`：顶层 `"""hello world"""`、section 内 `desc = """multi line"""`）。
- 新增测试：`x2_toml_f64.rl`、`x2_toml_array.rl`、`x2_toml_multiline.rl`（均位于 `tests/run-pass/`）。

## 验证

- [x] 标准 `key = value` 输出 + round-trip（`x2_toml_standard.rl`）。
- [x] 整行注释跳过（`x2_toml_comments.rl`）。
- [x] 引号感知解析（值含逗号）（`x2_toml_quoted.rl`）。
- [x] `[section]` 行式子表 + round-trip（单级，`x2_toml_section.rl`）。
- [x] 多级 `[a.b]` section 路径 + round-trip（`x2_toml_section_multi.rl`）。
- [x] f64 序列化 / 反序列化 round-trip（`x2_toml_f64.rl`）。
- [x] `[T; N]` 固定数组反序列化（扁平 + 嵌套）+ round-trip（`x2_toml_array.rl`）。
- [x] 多行字符串 `"""` 反序列化（顶层 + section 内）（`x2_toml_multiline.rl`）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-27 | `key = value` 空格 + round-trip trim + 整行注释 + CACHE_VERSION 递增 |
| 2026-08-30 | f64 序列化/反序列化 + `[T; N]` 固定数组反序列化 + 多行字符串 `"""` 反序列化全部落地；状态置 ✅ |
