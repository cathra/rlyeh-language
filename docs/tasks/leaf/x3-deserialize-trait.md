# X3 `Deserialize` trait + Serializer/Deserializer 框架

> **所属阶段**：阶段 X
> **状态**：📋 规划
> **依赖**：U4、U3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`trait Deserialize { fn from_json(s: String) -> Self; }` + Serializer/Deserializer 框架。

## 背景

阶段 阶段 X 子任务，详见 阶段详情文档 [`stages/X.md`](../../stages/X.md)。

## 技术细节

`trait Deserialize { fn from_json(s: String) -> Self; }`（依赖 U4 `-> Self`，替换「解析统一走内建」退化）；`Serializer`/`Deserializer` 访问器框架（目标签名 `serialize(&self, &mut Serializer) -> Result<(), SerError>`，U3 约束泛型化 `to_string<T: Serialize>` 落地）；`JsonError`/`TomlError` 类型；`json::parse` 非法输入改返回 `Err`。

## 验证

`deserialize_trait.{rlyeh,out}`（手写 `impl Deserialize` + `from_json` 调用 + `Result` 化错误）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
