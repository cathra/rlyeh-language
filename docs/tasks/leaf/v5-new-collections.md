# V5 新集合：VecDeque / HashSet / BTreeMap

> **所属阶段**：阶段 V
> **状态**：✅ 已完成
> **依赖**：V1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

三个新集合在 core.rl 根命名空间定义。

## 背景

阶段 阶段 V 子任务，详见 阶段详情文档 [`stages/V.md`](../../stages/V.md)。

## 技术细节

**VecDeque<T>**（3 槽 buf+front+len）：push_back/push_front/pop_front/pop_back/front/back/len/is_empty；**HashSet<T>**（5 槽开放寻址 + 线性探测 + 墓碑复用 + 负载 7/8 扩容）：insert/contains/remove/clear/elements/len/is_empty/cap；**BTreeMap<K,V>**（3 槽有序数组 + 二分）：find（未命中 -pos-1）/insert 右移保序/remove 左移/get/contains_key/first/last/keys/values（MVP 限 i64 键）。typecheck 特判构造器 `check_vecdeque/hashset/btreemap_construct`。调试修复 VecDeque push_front 覆盖（先 push 扩展物理长度再整体右移）。

## 验证

`v5_vecdeque.{rl,out}` 13 输出 + `v5_hashset_btreemap.{rl,out}` 26 输出 + 全量回归（历史遗留 v2_probe/trim 失败为既有问题）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
