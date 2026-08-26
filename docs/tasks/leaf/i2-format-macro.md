# I2 内置格式化宏

> **所属阶段**：阶段 I
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`println!`/`print!`/`format!`/`dbg!` 格式化引擎。

## 背景

阶段 阶段 I 子任务，详见 阶段详情文档 [`stages/I.md`](../../stages/I.md)。

## 技术细节

typecheck 格式化引擎——`parse_format_string`（`{}`/`{:?}` 同构/`{{` `}}` 转义/占位符数量检测）+ `value_to_string_for_ty`（i64→int_to_string、bool→If 三元、String/&str/Str→String::from）+ `check_format_macro`（`+` 拼接复用 A3 clone+push_str）+ `check_dbg_macro`（let __tmp + println + 返回 HirExpr::Block）。关键修复：matcher 预消费吞 `$` token、dbg! 先推断参数类型。

## 验证

`macro.{rlyeh,out}` 输出精确对比 + 全量 116 套件 + clippy 0 警告。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
