# P9 遗留低优项收口

> **所属专项**：[专项开发计划](../专项开发计划.md)（P9）
> **来源缺陷**：[`leaf/legacy-misc.md`](legacy-misc.md)（#1/#3/#4/#5 遗留限制）
> **状态**：📋 待办
> **风险**：低（各子项独立）
> **前置能力**：依各子任务

## 目标

收口 legacy-misc 中登记的低优先级遗留项：`rlyeh new` 增强、`String::from` 追踪扩展、dyn Trait 参数/返回值、空数组字面量/模式。

## 子任务

### P9a `rlyeh new` 增强（LM#1，低风险）

- **现状**：`main.rs` `run_new` 仅识别 `--lib`；任何 `-` 开头非 `--lib` 参数报错（`main.rs:672`）。lib 模板空壳（无导出函数）；`version`/`edition` 硬编码；`verbose=false` 固定。
- **方案**：`--target`/`--toolchain` 选项 + lib 模板补导出函数（`pub fn hello()`）。
- **涉及**：`crates/rlyeh-driver/src/main.rs` + `dagon/src/commands.rs`
- **验收**：`rlyeh new` 支持新选项；lib 模板含导出函数。

### P9b `String::from` 追踪扩展（LM#3，低风险）

- **现状**：`local_inits`（`context.rs:229-253`）只追踪 `let` 绑定、只认直链字面量（一层 `construct.rs:918-929`）、fn 边界不穿透。
- **方案**：多层直链（变量=变量=字面量）。
- **涉及**：`check_expr/construct.rs` + `context.rs`
- **验收**：`let a="x"; let b=a; String::from(b)` 可解析。

### P9c dyn Trait 参数/返回值（LM#4，连 P4）

- **现状**：`fn_sig.rs:74-94` H4 MVP 限制——`dyn Trait` 为 2 槽胖指针，仅支持 `let d: dyn Trait = &obj;` 局部变量主路径，**暂不支持作函数/方法参数与返回值**。
- **方案**：连 P4（dyn 上转型）一并解决，胖指针作参数/返回值。
- **涉及**：`check_item/fn_sig.rs`
- **验收**：`fn f(e: &dyn Error) -> &dyn Error` 可写。

### P9d 空数组字面量/模式（LM#5，低风险）

- **现状**：`index_enum.rs` 空数组字面量 `[]` MVP 不支持；元组/结构体模式、范围模式 MVP 不支持。
- **方案**：`[]` 字面量支持（可 `Vec::new()` 替代，低优）。
- **涉及**：`check_expr/index_enum.rs`
- **验收**：`let v: Vec<i64> = [];` 可用。

## 执行步骤

1. P9a：`rlyeh new` 选项 + lib 模板增强
2. P9b：`String::from` 多层直链追踪
3. P9c：dyn Trait 参数/返回值（连 P4）
4. P9d：空数组字面量支持

## 验收标准

- 各子项独立验证
- 回归全绿

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 由专项开发计划 P9 生成叶子文档（拆分 P9a/b/c/d） |
