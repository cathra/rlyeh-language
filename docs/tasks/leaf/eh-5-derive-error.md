# EH-5 `#[derive(Error)]`（thiserror 式）

> **级别**：P2 · **风险**：🟠 中 · **状态**：✅ 完成（2026-09-21） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.5 · **关联**：`sh-p1-2-derive.md`、`q1b-derive-macro.md`

## 目标
为错误枚举/结构体提供 derive，自动生成 `Error`/`Display`/`From` 实现，消除手写样板：
- 枚举变体 `#[error("msg {0}")]` → `message()`；
- `#[from]` 字段 → `impl E: From<T>`；
- `#[source]` 字段 → `source()`。

## 现状（2026-09-21 落地）
- 既有 derive 框架（`check_item/derive.rs`）支持 **struct** 的 `Clone`/`PartialEq`/`Copy`/`Debug`，复用 `collect_impl` 合成 impl 块（方法体在调用点实例化，零新增 IR）。**enum 的 derive 此前完全缺失**（`AstEnumDecl` 无 `derive` 字段，parser 拿到 `#[derive]` 后**丢弃**）。
- 属性此前仅在**项级**解析（`derive` / `repr(C)` / `memory(gc)`，其它名报错）；**字段 / 变体级属性无载体**。

## 风险分解与落地
- **M1（中）derive 解析属性（`#[error]`/`#[from]`/`#[source]`）** ✅
  - AST：新增 `AstAttr { name, value: Option<String>, span }`（元素级属性的紧凑形态）；`AstStructField.attrs`、`AstEnumVariant.attrs`、`AstStructDecl.attrs`、`AstEnumDecl.attrs`；`AstEnumDecl.derive`。
  - parser：新增 `parse_member_attrs()`（`#[error("模板")]` / `#[from]` / `#[source]`，未知名报错）；`parse_attributes()` 增加项级 `#[error("..")]` 收集；**枚举分支回填 `e.derive`**（此前丢弃）。
- **M2（中）生成 `message()`/`source()`/`impl From`** ✅
  - `build_error_impl_struct`：`message()` 取项级模板；`source()` 取首个 `#[source]` 字段（`self.<f>.source()`），无则 `Option::None`。
  - `build_error_impl_enum`：`message()` 逐变体 `match`（变体级模板，缺失回退 enum 项级模板），各臂绑定名**逐变体唯一**（`__err<vi>_f<j>` / `__err<vi>_<name>`）。
  - `build_from_impl_struct`（要求唯一字段）/ `build_from_impl_enum`（要求单元素组字段变体）→ `impl X: From<T>`。
  - 模板展开：`{0}`/`{name}` → `String::from("lit") + <占位> + …`；占位支持整型 / `f64` / `bool` / `char` / `String`。
- **M3（低）与既有 derive 框架整合** ✅
  - `canonical_protocol` 增加 `"Error"`；`expand_derives_for_struct` 分支化（`Error` 失败可传播）；新增 `expand_derives_for_enum`，在 `collect_item_decls` 的枚举分支调用。既有 `Clone`/`PartialEq`/`Copy`/`Debug` 行为不变（未知 derive 名仍宽松忽略）。

## 连带修复（4 处）
1. **`From` / `Into` 未导出到根命名空间**（`rlyeh-std/rlyeh/module.rl`）：用户侧 `impl X: From<T>` 的协议名无法解析为规范名 `io::error::From`，致 `?` 运算符的 `From` 自动转换**找不到 impl**（报「找不到 `io::error::From::from` 的可用 protocol impl」）。补 `pub import io::error::From; / Into;` 后 `#[from]` + `?` 打通。
2. **`rlyeh-fmt` 丢弃 `#[derive(..)]`**（AST 重建未回写）：本轮补齐 derive / `#[error]` / `#[from]` / `#[source]` / `#[repr(C)]` 回写，并保证幂等（`#[error]` 模板按 Rust 字符串字面量重新转义）。
3. **枚举 derive 丢弃**：`parse_item` 的 enum 两个分支未回填 `derive`（仅 struct 回填）。
4. **枚举臂绑定名冲突**：初版各臂统一用 `f0`，同一 `match` 内 String 臂与 i64 臂复用同名局部 → LIR 报「变量 `f0` 类型冲突：期望 ptr，实际 i64」（`docs/std-lib.md` §12 已知限制）；改为逐变体唯一绑定名。

## 已知限制
- **`Enum::from(x)` 直接静态调用不可用**：**枚举静态 / 固有方法调用**为既有缺口（实测 `impl E { fn make() }` + `E::make()` 与 `impl E: From<T>` + `E::from(x)` 均报 `TC015`；enum 的**实例**方法与 protocol 方法（含 derive 生成的 `message`）正常）。与 derive 无关；`#[from]` 经 `?` 运算符与 `impl From` 注册**生效**。
- **enum 变体级 `#[source]` 待办**：枚举的 `source()` 恒返回 `Option::None`（错误链请用 struct 包装表达）。
- 模板占位不支持嵌套错误类型（请用 `#[source]`）；格式修饰（`{0:?}`）未支持。
- 泛型 struct 的字段类型须自身实现对应 protocol（沿用既有 derive 约定，MVP 不强制 bound）。

## 受影响组件
`rlyeh-ast`（`AstAttr` + 4 处字段）、`rlyeh-parser`（`parser.rs` / `item.rs`）、`rlyeh-desugar`（合成 struct 补 `attrs`）、`rlyeh-typecheck`（`check_item/derive.rs`、`check_item/mod.rs`）、`rlyeh-std`（根导出 `From`/`Into`）、`tools/rlyeh-fmt`（属性回写）。

## 验证
- `tests/run-pass/eh_derive_error.rl`（+ `.out`，12 行）：
  - struct `ConfigError`（项级模板 `{key}`/`{value}` 插值）→ `config: port = 8080`；
  - struct `WrappedIo`（`#[from]` 单字段 + `#[source]`）→ `IoError` 经 `From`/`?` 转换、`message()` = `io failure`、`source()` = `None`；
  - enum `LoadError`（逐变体模板）：`NotFound`/`Timeout`(f64)/`Flag`(bool)/`Bare`(单元) 与 `{0}` 插值；
  - enum `#[from]` 变体 + `?` 转换（`String` → `LoadError::Io`，`io: boom`）；
  - `Error` protocol 上转 `&dyn Error` 后虚调用。
- `rlyeh fmt`：属性回写 + 幂等（连续两次格式化输出一致），格式化产物可再编译运行。
- 全量 `rlyeh test tests`：**342/342 通过**（本轮新增 1 例）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
| 2026-09-21 | **M1+M2+M3 落地**：`AstAttr` + 字段/变体/项级属性载体；`parse_member_attrs` + 项级 `#[error]`；`AstEnumDecl.derive` 回填；`expand_derives_for_enum` / `build_error_impl_{struct,enum}` / `build_from_impl_{struct,enum}`。连带修复 4 处（`From`/`Into` 根导出、fmt 属性回写、枚举 derive 回填、分支绑定名唯一）。新增 `tests/run-pass/eh_derive_error.{rl,out}`；全量 342/342。登记限制：枚举静态 / 固有方法调用不可用（既有缺口）、enum 变体级 `#[source]` 待办 |
