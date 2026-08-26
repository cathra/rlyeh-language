# Q3b 格式化引擎接入

> **所属阶段**：阶段 Q
> **状态**：✅ 已完成
> **依赖**：Q3a
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`{}`/`{:?}` 占位优先查 `Display`/`Debug` impl。

## 背景

阶段 阶段 Q 子任务，详见 阶段详情文档 [`stages/Q.md`](../../stages/Q.md)。

## 技术细节

`parse_format_string` 区分 `{}`/`{:?}`（`FormatSeg.is_debug`）；`value_to_string_for_ty` 对自定义类型查 `fmt`/`fmt_debug` 方法，生成 `{ let mut __fmt_q3 = Formatter::new(); x.fmt(&mut __fmt_q3) }` 块表达式；`dbg!` 走 Debug 路径；内建类型行为不变；无 impl 报 Unsupported。

## 验证

`display_fmt.{rlyeh,out}` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
