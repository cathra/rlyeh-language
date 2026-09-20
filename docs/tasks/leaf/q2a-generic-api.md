# Q2a 泛型 API 入口

> **所属阶段**：阶段 Q
> **状态**：✅ 已完成
> **依赖**：Q1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`json::to_string<T>`/`from_str<T>` 泛型入口。

## 背景

阶段 阶段 Q 子任务，详见 阶段详情文档 [`stages/Q.md`](../../stages/Q.md)。

## 技术细节

`json::to_string(v)` ≡ `json::stringify(v)`、`json::from_str::<T>(s)` ≡ `json::parse::<T>(s)`；`T: Serialize`/`T: Deserialize` protocol bound 未支持——MVP 无泛型 protocol 约束，签名退化为无 bound turbofish 形式。

## 验证

`json_api.{rlyeh,out}`（Q2：to_string/from_str 别名兼容）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
