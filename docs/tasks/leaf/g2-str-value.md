# G2 补全 `str` 值一等类型 + 实参自动升级

> **所属阶段**：阶段 G
> **状态**：✅ 已完成
> **依赖**：G2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`let s = "hi"` 绑定 Str 值自动升级为 String 对象；String 形参位置字面量实参自动升级。

## 背景

阶段 阶段 G 子任务，详见 阶段详情文档 [`stages/G.md`](../../stages/G.md)。

## 技术细节

① Str 值升级：`check_method_call` 对 `Type::Str` 值接收者升级（s.len()/substring/contains 可用）、`+` 拼接纳入 Str 值、比较纳入 Str 值；关键根因修复：`check_fn_body_with_self` 未隔离 `ctx.local_inits` 致同名变量 init 覆盖，补 `saved_inits` take/恢复。② 实参升级：提取 `upgrade_str_arg` helper，接入 check_call/check_indirect_call/check_static_method_call/check_method_call/check_protocol_object_call/check_generic_call 6 处。

## 验证

`str_value.{rlyeh,out}` 17 输出 + `str_value_arg.{rlyeh,out}` 17 输出 + 全量 122 套件 685 测试。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
