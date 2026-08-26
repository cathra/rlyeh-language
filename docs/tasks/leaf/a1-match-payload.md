# A1 用户级 match 解构具体实例化枚举聚合载荷

> **所属阶段**：阶段 A
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

用户级 match 解构具体实例化枚举聚合载荷，修复 `expected String, found T`。

## 背景

阶段 阶段 A 子任务，详见 阶段详情文档 [`stages/A.md`](../../stages/A.md)。

## 技术细节

`check_pattern` Enum 分支原仅用 `ctx.generic_subst` 替换字段类型，用户级 match（非泛型方法体）subst 为空导致 `T` 未定型。修复为合并「当前 generic_subst + 由 `pat_ty` 类型参数与 `enum_def.type_params` 建立的新映射」，嵌套泛型 `Option<Vec<T>>`/`Result<Option<String>, i64>` 递归生效。

## 验证

`option_result_test.rs::user_level_match_string_payload` 8 断言覆盖 Some/None/Ok/Err/嵌套/方法链。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
