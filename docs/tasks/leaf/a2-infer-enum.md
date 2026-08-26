# A2 Infer 枚举自动定型

> **所属阶段**：阶段 A
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

修复裸 `Result::Err(7).unwrap_or(100)` 返回 `_`，Infer 枚举自动定型。

## 背景

阶段 阶段 A 子任务，详见 阶段详情文档 [`stages/A.md`](../../stages/A.md)。

## 技术细节

`check_method_call` 参数检查时对含 `_` 的期望类型用实参 unify 回填 subst（`unify` 新增 `Type::Infer` 分支：替换 subst 中所有 Infer 条目），回填后重算签名再实例化方法。

## 验证

`option_result_test.rs::bare_enum_infer` 5 断言覆盖裸 Ok/Err/Some + 聚合载荷 + 算术链。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
