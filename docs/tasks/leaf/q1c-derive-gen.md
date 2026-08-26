# Q1c derive 生成接线

> **所属阶段**：阶段 Q
> **状态**：✅ 已完成
> **依赖**：Q1b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

struct 派生 impl 生成。

## 背景

阶段 阶段 Q 子任务，详见 阶段详情文档 [`stages/Q.md`](../../stages/Q.md)。

## 技术细节

序列化复用 L2 内建 stringify（字段序 = 定义序）；反序列化 `json_parse_ast` 新增 struct 分支（desugar 为块表达式：`String::from` → `substring` 剥离 `{}` → `split(『,』)` → `for` 遍历 → 字段名匹配 if-else 链逐字段 `Assign`，值递归；字段顺序任意、缺失零值、未知忽略、嵌套 struct 支持；泛型 struct 不支持）。

## 验证

`json_derive.{rlyeh,out}` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
