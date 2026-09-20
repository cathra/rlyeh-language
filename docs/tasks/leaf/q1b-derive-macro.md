# Q1b derive 宏语法

> **所属阶段**：阶段 Q
> **状态**：✅ 已完成
> **依赖**：Q1a
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`#[derive(Serialize, Deserialize)]` 解析。

## 背景

阶段 阶段 Q 子任务，详见 阶段详情文档 [`stages/Q.md`](../../stages/Q.md)。

## 技术细节

lexer 新增 `Pound` token + parser `parse_attributes` 特判，`AstStructDecl.derive` 存储 protocol 名列表；其它 attribute 名报错、非 struct 项宽松忽略。

## 验证

`json_derive.{rlyeh,out}`（derive 标记 struct 序列化/反序列化 round-trip）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
