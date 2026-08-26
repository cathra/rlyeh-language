# Q1a `Serialize`/`Deserialize` trait 定义

> **所属阶段**：阶段 Q
> **状态**：✅ 已完成
> **依赖**：L2、I
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

定义序列化 trait + 内建类型默认 impl。

## 背景

阶段 阶段 Q 子任务，详见 阶段详情文档 [`stages/Q.md`](../../stages/Q.md)。

## 技术细节

`trait Serialize { fn to_json(&self) -> String; }` + `trait Deserialize { fn from_json(s: String) -> Self; }`。`-> Self` 返回自身类型未支持（typecheck `undefined type Self`，见 M2b）——`Deserialize` 按预案退化：`from_json` 未定义，解析统一走编译器内建 `json::parse::<T>`；内建类型默认 impl 为声明性文档；自定义类型可手写 `impl Serialize for T`。

## 验证

`json_derive.{rlyeh,out}`（Q1）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
