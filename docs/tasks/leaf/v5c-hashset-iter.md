# V5c HashSet 只读引用迭代器（`iter`，2026-09-02）

> **归属**：阶段 V（集合与迭代器完整化）· V5 新集合收尾项
> **状态**：✅ 已完成
> **关联**：[`v5-new-collections.md`](./v5-new-collections.md)（V5 ✅）· [`v5b-set-operations.md`](./v5b-set-operations.md)（V5b ✅）· std-lib §3.4

## 背景

`std-lib.md §3.4` 将 HashSet 的「借用迭代器 `iter -> Iter<'_, T>`」列为 MVP 限制（V5b 完成集合代数后剩余的唯一限制项）。V 阶段 V1 已为 `Vec`（`IterRef<T>` 返回 `Option<&T>`）与 `HashMap`（`HashMapIter<K,V>` 返回 `Option<KVRef>`）交付借用迭代器，HashSet 跟进同一模式即可闭环。

## 方案

- 新增 `HashSetIter<T>` 结构体（裸指针视图，与 `HashMapIter` 同式）：`states: *const i64` / `items: *const T` / `idx: i64` / `cap: i64`。
- `HashSetIter::next(&mut self) -> Option<&T>`：线性扫描跳过 `states != 1` 的空/墓碑槽，命中返回 `&self.items[idx]`（指向原 items 真实槽，零拷贝），耗尽返回 `None`。
- `HashSet::iter(&self) -> HashSetIter<T>`：取 `&self.states[0]` / `&self.items[0]` 裸指针构造迭代器（复用 `HashMap::iter_pairs` 同式取址）。
- 接入 `for` 循环（inherent `next` 检测，无需 `Iterator` protocol——`type Item = &T` 引用类型对适配器框架不友好，与 `IterRef`/`HashMapIter` 一致）。
- 约束：迭代期间不得对集合做结构性修改（insert/remove/grow 重哈希使裸指针悬垂），与既有借用迭代器一致。

## 验收

- `tests/run-pass/hashset_iter.{rl,out}`（规模 `3` / 求和 `60`（顺序无关）/ 引用解引用命中 `true` / 空集合迭代 `0` 次）。
- 全量 `rlyeh test tests` 257/257 通过。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-02 | 由 V5b 收尾自然延伸：实现 `HashSetIter<T>` + `HashSet::iter()`（零新增语言特性，照搬 `HashMapIter`/`IterRef` 模式）；新增 `tests/run-pass/hashset_iter.{rl,out}`；全量 257/257 通过 |
