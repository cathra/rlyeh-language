# V4 `get_mut` 引用语义

> **所属阶段**：阶段 V
> **状态**：✅ 已完成
> **依赖**：V1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`Vec::get_mut -> Option<&mut T>`/`HashMap::get_mut -> Option<&mut V>` 引用语义。

## 背景

阶段 阶段 V 子任务，详见 阶段详情文档 [`stages/V.md`](../../stages/V.md)。

## 技术细节

std `Vec::get_mut`（越界检查 `i >= 0 && i < self.len`，命中 `Some(&mut self.data[i])`——V1 GEP 真实取址复用）+ `HashMap::get_mut`（find 缺失 None，命中 `Some(&mut self.vals[idx])`）；调用方 `match { Some(r) => *r = x }` 经解引用写回真实槽（非拷贝）；编译器能力确认（零新增 IR，G1 + U 阶段自然组合）。

## 验证

`get_mut_ref.{rlyeh,out}` 11 断言（vec 写回/越界/负索引 None/连续写回/HashMap 写回 + get 读回 + len 不变 + 他键不受影响/引用剥层读）+ suite_test 全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
