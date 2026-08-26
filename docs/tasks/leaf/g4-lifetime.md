# G4 生命周期标注

> **所属阶段**：阶段 G
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

生命周期标注 MVP 语法接受。

## 背景

阶段 阶段 G 子任务，详见 阶段详情文档 [`stages/G.md`](../../stages/G.md)。

## 技术细节

parser 两处：`parse_generics` 遇 `Token::Lifetime` 跳过（`'a` 及可选 `: 'b` bound）、`parse_type` 的 `&` 分支跳过 `&'a T` 生命周期（其后可选 mut）；typecheck 无改动（生命周期信息解析后丢弃，宽松检查）；borrowck 生命周期检查规划中。

## 验证

`lifetime.{rlyeh,out}` 4 输出。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
