# X4 Formatter 完整化

> **所属阶段**：阶段 X
> **状态**：✅ 已完成（Formatter 升级 ✅；`Result<(), FmtError>` 签名 ✅；`Debug::fmt` 改名 ✅；FmtError ✅；对齐格式占位符引擎应用 ✅，2026-08-28）
> **依赖**：U3/U4、Q3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`Formatter` 升级为真实格式化器 + `fmt -> Result<(), FmtError>` + `Debug::fmt` 改名。

## 背景

阶段 阶段 X 子任务，详见 阶段详情文档 [`stages/X.md`](../../stages/X.md)。

## 技术细节

`Formatter` 从占位类型升级为真实格式化器（对齐/宽度/精度/填充状态字段）；`trait Display { fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }`（目标签名，依赖 U3/U4，替换 `-> String` 降级；`Debug::fmt_debug` 改名对齐 `Debug::fmt`）；`FmtError` 类型。

## 实施情况（2026-08-27，核心完成）

- **std `fmt/module.rl`**：`Formatter` 升级为 `{ buf, fill, width, align }` + `write_str`/`result`；新增 `enum FmtError { Invalid, Fmt(String) }`；`Display::fmt`/`Debug::fmt` 签名改 `-> Result<(), FmtError>`；`Debug::fmt_debug` 改名 `Debug::fmt`。
- **语言级：`()` 单元值支持**（X4 前提）：`ExprKind::Unit` 新节点，parser 空 `()` → Unit（`in ()` 集合仍走 `parse_set_elements`）；typecheck `ExprKind::Unit` → `Type::Unit`；codegen 对 `()` 作为 enum（Result）payload 写 0 占位（`Wrapper::Empty(())` / `Result::Ok(())` 可构造）。
- **语言级：impl 方法符号带 trait 区分**（`Debug::fmt` 同名前提）：`instantiate_impl_method`/`check_method_call` 的 `base_fn` 对 trait impl 加 trait 后缀（`Point::fmt`（Display）→ `Point::fmt__fmt::Display`），消除同名 trait 方法符号碰撞。
- **语言级：方法调用按 trait 分派**：`ExprKind::MethodCall` 新增 `trait_hint` 字段；`check_method_call` 支持 `trait_hint`（`fmt::Display`/`fmt::Debug`）经 `find_impl_for_trait_method` 精确分派。
- **Q3b 引擎**：`{}` → `fmt::Display::fmt`，`{:?}` → `fmt::Debug::fmt`；`fmt` 返回 `Result<(), FmtError>`（写缓冲），调用后取 `Formatter::result()`。
- **测试**：`display_fmt.rl` 迁移为 `Result<(), fmt::FmtError>` + `f.write_str` + `Debug::fmt`（`.out` 精确对比：`P(1,2)`/`Point{x:1,y:2}`/`dbg: Point{x:1,y:2}`）；example `08-format/display_fmt.rl` 同步。
- **对齐格式占位符引擎应用**（2026-08-28）：`parse_format_string` 解析格式说明符（`fill align width`，如 `{:>10}`/`{:<5}`/`{:^8}`/`{:*>10}`）；`FormatSeg` 增加 `align`/`width`/`fill` 字段；`check_format_macro` 对占位符值生成对齐逻辑（`>` 右→`pad_start`、`<` 左→`pad_end`、`^` 居中→`(width-len)/2` 双 pad）。测试 `x4_format_align.{rl,out}`：`         7`/`ab   `/` rlyeh  `/`********42`/`123`。

## 验证

- [x] `display_fmt.{rl,out}`（`{}`/`{:?}` 按 trait 区分 + `dbg!` + 内建类型兼容 + `.out` 精确对比）。
- [x] `x4_format_align.{rl,out}`（对齐占位符：右/左/居中/自定义填充 + 宽度不足原样 + `.out` 精确对比）。
- [x] 全量回归：158 用例通过（`()` 值、codegen payload、方法符号带 trait、MethodCall trait_hint 改动零破坏）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-27 | Formatter 升级 + FmtError + Display/Debug 签名改 Result<(),FmtError> + Debug 改名 fmt + () 单元值支持 + 方法符号带 trait 区分 + MethodCall trait_hint 分派 |
| 2026-08-28 | 对齐格式占位符引擎应用（fill align width 解析 + pad_start/pad_end/居中），158 用例全过 |
