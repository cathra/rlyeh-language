# Y6 错误体系完整化

> **所属阶段**：阶段 Y
> **状态**：✅ 已完成（Y6a source 非 dyn 退化 + Y6b From/Into std 层 ✅，2026-08-28；`?` 运算符 From 自动转换待语言级）
> **依赖**：U3/U4
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`trait Error` 补 `source` 链 + `Into::into()` 自动转换。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`trait Error { fn message(&self) -> String; fn source(&self) -> Option<&dyn Error>; }`（补 `source` 链，M2a 现状仅 message）；`Into::into()` 自动转换 + `?` 运算符的 From 自动转换（依赖 U3/U4；MVP 无 where 约束时退化显式 `into()` 调用）。

## 风险探测（2026-08-28）

**语言级障碍：`&dyn Error` trait 对象构造/上转型失败。**
- 实测：`fn source() -> Option<&dyn Error>` **签名层面可编译**；但构造值失败——
  - `let d: dyn Error = e;`（e: MyError）→ `expected dyn Error, found MyError`（无自动上转型）
  - `let d: &dyn Error = r;`（r: &MyError）→ `expected &dyn Error, found &MyError`（引用也不能转 dyn）
- **影响**：`source() -> Option<&dyn Error>` 无法构造返回非 `None` 的值，链式 source 语义当前不可实现。

## 拆分方案（高 → 中低风险粒度）

### Y6a（✅ 已完成，2026-08-28）
- `trait Error` 补 `source()` 的**非 dyn 退化**：`fn source(&self) -> Option<String>`（返回源错误 message 字符串，规避 dyn 构造障碍）；链式 message 链（`err.message()` → `err.source()` 逐级）。
- **实施**：io/error.rl `trait Error` 补 `fn source(&self) -> Option<String>`；`impl Error for IoError` + future.rl `impl Error for TimeoutError` 补 `source()` 返回 `Option::None`；测试 `error_source.{rl,out}`（无源错误 None / 包装错误 Some(源 message) / message 独立，`1`/`file not found`/`load failed`）。

### Y6b（✅ std 层完成，2026-08-28；`?` From 自动转换待语言级）
- io/error.rl 定义 `trait From<T>`/`trait Into<T>` + `impl From<IoErrorKind> for IoError`（复用 `from_kind` 默认 message）。
- **探测**：裸名泛型实参 `impl From<IoErrorKind>` 可行；**路径实参** `From<io::error::IoErrorKind>` 触发 parser 错误（泛型实参不支持 `::` 路径）→ 用裸名。
- 测试 `error_conversion.{rl,out}`：`IoError::from(NotFound/PermissionDenied/TimedOut)` → `entity not found`/`permission denied`/`operation timed out`。
- **待语言级**：`?` 运算符 From 自动转换（`E: Into<F>`）+ parser where 子句（blanket impl `impl<T,U> Into<U> for T where U: From<T>`）→ 登记 `lang-defects.md`。

## 验证

`error_source.{rlyeh,out}` + `error_conversion.{rlyeh,out}`（161 用例全绿）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-28 | 风险探测（&dyn Error 构造障碍）+ 拆分 Y6a/Y6b |
| 2026-08-28 | Y6a source 非 dyn 退化完成（159 用例全绿） |
| 2026-08-28 | Y6b From/Into std 层完成（161 用例全绿；? From 自动转换待语言级） |
