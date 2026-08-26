# 阶段 V — 集合与迭代器完整化

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-*.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：T 阶段已实现 MVP 退化版（`get_mut` 值拷贝、`chars` 字节级、`Iterator` 元素固定 i64、适配器内建 desugar）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| V1 | **借用迭代器**：`Vec::iter(&self) -> Iter<'_, T>` / `iter_mut(&mut self) -> IterMut<'_, T>`、`HashMap::iter(&self) -> Iter<'_, K, V>`——迭代器结构体（持有容器瘦指针 + 游标槽）+ `next() -> Option<&T>` / `Option<(&K, &V)>`（J2 方法式接入，元素为引用值，U1 解锁返回引用）；`for x in v.iter()` 解引用迭代（自动剥层） | ✅ 部分完成 | [`v1-borrow-iter.md`](../tasks/leaf/v1-borrow-iter.md) |
| V2 | **String 码点迭代器**：`chars(&self) -> Chars`（UTF-8 码点解码——首字节定宽 + 连续字节校验，返回 `Option<char>` 迭代器）替代 MVP 字节级；`lines(&self) -> Lines`（按 `\n`/`\r\n` 分行迭代器，替代 `split("\n")` 的 Vec 拷贝）；`trim(&self) -> &str` 引用视图（目标签名，替代字符串拷贝） | 🔧 进行中 | [`v2-str-view.md`](../tasks/v2-str-view.md) |
| V3 | **Iterator 默认方法 + 适配器迁移**：`Iterator` trait 补默认方法 `count`/`sum`/`chain`/`enumerate`/`find`/`any`/`all`（依赖 U2 关联类型 + U3 约束）；内置适配器 `map`/`filter`/`fold`/`collect`/`take`/`skip` 从「typecheck 内建 desugar」迁移为 trait 默认方法（目标签名 `-> Map<Self, B>` 等包装迭代器类型；**风险**：迁移重构面大（J3 全部调用点），可保留内建 + 新增默认方法双轨，逐一迁移后删内建） | 🔧 部分完成 | [`v3-iterator-adapters.md`](../tasks/v3-iterator-adapters.md) |
| V4 | **`get_mut` 引用语义**：`Vec::get_mut(&mut self, i) -> Option<&mut T>`、`HashMap::get_mut(&mut self, k) -> Option<&mut V>`（U1 解锁返回引用；经 `*` 写回原容器，取代 MVP 值拷贝） | ✅ 已完成 | [`v4-get-mut.md`](../tasks/leaf/v4-get-mut.md) |
| V5 | **新集合**（std-lib.md §1 目标架构目录）：`HashSet<T>`（`collections/hashset.rl`，复用 HashMap 7 槽存储 + djb2/Knuth 散列，`insert`/`contains`/`remove`/`iter`/`union`/`intersection`/`difference`）；`BTreeMap<K, V>`（`btree.rl`，有序键——数组二分插入 + 移动，`first`/`last`/`range` 规划 MVP 限 `i64` 键）；`VecDeque<T>`（`deque.rl`，环形缓冲，`push_front`/`push_back`/`pop_front`/`pop_back`） | ✅ 已完成 | [`v5-new-collections.md`](../tasks/leaf/v5-new-collections.md) |

**验收**：见任务树 [`stage-u-z.md`](../tasks/stage-u-z.md) 各子任务叶子的「验证」字段；全量回归通过。
