# B5 Vec/HashMap 方法补齐

> **所属阶段**：阶段 B
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`Vec` 补充 `first/last/reverse/swap/binary_search` + `HashMap len/is_empty` 固化。

## 背景

阶段 阶段 B 子任务，详见 阶段详情文档 [`stages/B.md`](../../stages/B.md)。

## 技术细节

`first/last` 空返回 `Option::None`、否则值拷贝；`reverse` 双指针原地交换；`swap` 越界 `loop{}` 崩溃；`binary_search` 三态二分（双比较区分 `<`，i64 数值/String 字典序）。

## 验证

`vec_more_ops_test.rs` 5 用例 + `hashmap_len_test.rs` 4 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
