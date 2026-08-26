# U7 方法级泛型参数

> **所属阶段**：阶段 U
> **状态**：✅ 已完成
> **依赖**：U6
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

方法自带 `<U>` 泛型参数（`fn map<U>(x: U) -> U`）。

## 背景

阶段 阶段 U 子任务，详见 阶段详情文档 [`stages/U.md`](../../stages/U.md)。

## 技术细节

修复 `collect_impl`（方法签名收集时把方法泛型并入 type_params，否则 `U` 报 undefined type）+ `instantiate_impl_method`（mono 键/后缀扩展为 impl 泛型 + 方法泛型，不同 U 调用不因键相同复用首次实例）。

## 验证

`method_generic.{rl,out}`（同一 map<U> 以 U=i64/f64/String 调用输出 10/3.5/hi + 嵌套）+ 全量 120 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
