# Y6 错误体系完整化

> **所属阶段**：阶段 Y
> **状态**：✅ 已完成（Y6a source 非 dyn 退化 + Y6b From/Into std 层 ✅，2026-08-28；`?` 运算符 From 自动转换 ✅（2026-08-29，P6c）+ `Into::into` blanket 语义 ✅（2026-08-29，P6c-1/2）；P7d-1 `source()` 升级为 `Option<&dyn Error>` 真实错误链 ✅，2026-08-29，Y6c）
> **依赖**：U3/U4
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`protocol Error` 补 `source` 链 + `Into::into()` 自动转换。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`protocol Error { fn message(&self) -> String; fn source(&self) -> Option<&dyn Error>; }`（补 `source` 链，M2a 现状仅 message）；`Into::into()` 自动转换 + `?` 运算符的 From 自动转换（依赖 U3/U4；MVP 无 where 约束时退化显式 `into()` 调用）。

## 风险探测（2026-08-28）

**语言级障碍：`&dyn Error` protocol 对象构造/上转型失败。**
- 实测：`fn source() -> Option<&dyn Error>` **签名层面可编译**；但构造值失败——
  - `let d: dyn Error = e;`（e: MyError）→ `expected dyn Error, found MyError`（无自动上转型）
  - `let d: &dyn Error = r;`（r: &MyError）→ `expected &dyn Error, found &MyError`（引用也不能转 dyn）
- **影响**：`source() -> Option<&dyn Error>` 无法构造返回非 `None` 的值，链式 source 语义当前不可实现。

## 拆分方案（高 → 中低风险粒度）

### Y6a（✅ 已完成，2026-08-28）
- `protocol Error` 补 `source()` 的**非 dyn 退化**：`fn source(&self) -> Option<String>`（返回源错误 message 字符串，规避 dyn 构造障碍）；链式 message 链（`err.message()` → `err.source()` 逐级）。
- **实施**：io/error.rl `protocol Error` 补 `fn source(&self) -> Option<String>`；`impl Error for IoError` + future.rl `impl Error for TimeoutError` 补 `source()` 返回 `Option::None`；测试 `error_source.{rl,out}`（无源错误 None / 包装错误 Some(源 message) / message 独立，`1`/`file not found`/`load failed`）。

### Y6b（✅ 已完成，2026-08-29；`?` From 自动转换 + `Into::into` blanket 语义均已落地）
- io/error.rl 定义 `protocol From<T>`/`protocol Into<T>` + `impl From<IoErrorKind> for IoError`（复用 `from_kind` 默认 message）。
- **探测**：裸名泛型实参 `impl From<IoErrorKind>` 可行；**路径实参** `From<io::error::IoErrorKind>` 触发 parser 错误（泛型实参不支持 `::` 路径）→ 用裸名。
- 测试 `error_conversion.{rl,out}`：`IoError::from(NotFound/PermissionDenied/TimedOut)` → `entity not found`/`permission denied`/`operation timed out`。
- **已实现（2026-08-29，P6c / P6c-1/2）**：`?` 运算符 From 自动转换（`E: Into<F>`）+ `Into::into` blanket 语义（`Into::<U>::into(x)` 改写 `From::from(x)`，约束求解确认 `From<A> for U` 存在），详见 P6 叶子。

### P7d-1（Y6c，✅ 已完成，2026-08-29）：`source()` 升级为 `Option<&dyn Error>` 真实错误链

- **背景**：Y6a 因 `&dyn Error` 构造障碍（LD#2，当时未解决）将 `source()` 退化为 `Option<String>`（返回源错误 message 字符串）。P4 已完成 `&dyn Error` 上转型（`let d: &dyn Error = &x`），链式 source 语义的语言级障碍消除，故将 `source()` 升级回目标签名 `Option<&dyn Error>`。
- **实施（io/error.rl + future.rl）**：
  - `protocol Error { fn message(&self) -> String; fn source(&self) -> Option<&dyn Error>; }`（恢复目标签名）。
  - `impl Error for IoError` + `impl Error for TimeoutError` 的 `source()` 返回 `Option::None`（底层错误无源）。
- **typechecker 配套修复（发现并修复两处语言级缺陷）**：
  1. **自引用 protocol 解析（resolve.rs + collect.cs + context.rs）**：自引用 protocol（如 `protocol Error { fn source(&self) -> Option<&dyn Error> }`）在收集自身方法签名时需引用尚未注册进 `protocol_defs` 的自身名字。`collect_protocol` 收集期间设置 `TypeContext::collecting_protocol`（结束恢复），`resolve.rs` 的 `dyn` 分支在 `resolve_protocol_key` 未命中且当前正收集同名 protocol 时回退到自身（裸名/模块前缀均可），使 `&dyn Error` 在 protocol 定义完成前可解析。
  2. **`collect_impl` protocol 类型实参解析顺序（collect.rs）**：`collect_impl` 此前在**恢复 `ctx.type_params` 之后**才解析 `protocol_type_args`，导致 `impl<T> Wrap<T> for Pair<T>` 的 `Wrap<T>` 类型实参 `T`（impl 级泛型）在 `type_params` 已清空时解析报 `undefined type T`（预存缺陷，此前被 std `Error` 自引用收集失败「抢先报错」掩盖，全局测试长期假性失败）。修复：将 `protocol_type_args` 的解析移到恢复 `type_params` 之前（此时 `ctx.type_params` 仍为 `imp.generics`）。
- **MVP 约束（见 lang-defects 新增条目）**：`&self.struct_field` 返回引用会触发 codegen 悬空（方法返回的字段地址指向已销毁的 by-value `self`）——故错误包装器须**持有底层错误的引用类型字段**（`source: &IoFailure`）并在 `source()` 中上转该**存储的引用值**（`let d: &dyn Error = self.source;`），而非 `&self.source`；MVP 暂不支持 `&dyn Error` 直接作 struct 字段，故示例退化到具体引用 `&IoFailure`（语义等价的真实错误链载体）。
- **测试**：`error_source.{rl,out}`（真实错误链：底层 `source()=None` → `1`；包装 `source()` 返回底层 `&dyn Error`、链式 `.message()` 取 `file not found`；`message()` 独立 `load failed`；链遍历到 `IoFailure` 终止 `3`）。

## 验证

`error_source.{rl,out}`（真实错误链，`1`/`file not found`/`load failed`/`3`）+ `error_conversion.{rl,out}`（161 用例全绿）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-28 | 风险探测（&dyn Error 构造障碍）+ 拆分 Y6a/Y6b |
| 2026-08-28 | Y6a source 非 dyn 退化完成（159 用例全绿） |
| 2026-08-28 | Y6b From/Into std 层完成（161 用例全绿；? From 自动转换待语言级） |
| 2026-08-29 | P7d-1 `source()` 升级为 `Option<&dyn Error>` 真实错误链（Y6c）：io/error.rl + future.rl 恢复目标签名；typechecker 修复自引用 protocol 解析（`collecting_protocol`）+ `collect_impl` `protocol_type_args` 解析顺序（暴露并修复预存 `undefined type T` 缺陷）；`error_source.{rl,out}` 验证真实错误链（131 run-pass + 12 compile-pass 全绿） |
