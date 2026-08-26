# T2 `Iterator` trait 定义

> **所属阶段**：阶段 T
> **状态**：✅ 已完成
> **依赖**：J
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

定义 `Iterator` trait + 自定义迭代器接入。

## 背景

阶段 阶段 T 子任务，详见 阶段详情文档 [`stages/T.md`](../../stages/T.md)。

## 技术细节

MVP 退化：关联类型 `type Item` 验证不可行（parser/typecheck 无 trait `type` 成员载体），`trait Iterator { fn next(&mut self) -> Option<i64>; }` 元素固定 i64（core.rl 顶部定义）。自定义迭代器 `impl Iterator for T` 后经 for 接入 ✓（check_for_iterator 检测 next() 方法，inherent 或 trait impl 均可）；适配器保持内建 desugar 不迁移。

## 验证

`iterator_trait.{rlyeh,out}`（Counter/Step 自定义迭代器 `impl Iterator` 经 for 接入，输出 `10/0/12` + 直接 next `1/2/3/-1`）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
