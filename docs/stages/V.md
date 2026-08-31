# 阶段 V — 集合与迭代器完整化

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-u-z.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：T 阶段已实现 MVP 退化版（`get_mut` 值拷贝、`chars` 字节级、`Iterator` 元素固定 i64、适配器内建 desugar）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| V1 | **借用迭代器**：`Vec::iter(&self) -> Iter<T>` / `iter_mut(&mut self) -> IterMut<T>` 零分配零拷贝值视图、`Vec::iter_ref(&self) -> IterRef<T>` 返回 `Option<&T>` 元素引用（零拷贝、可 `*r = x` 原地写回）、`HashMap::iter_pairs(&self) -> HashMapIter<K,V>` 经 `KVRef<K,V>`（`key`/`val` 两裸指针）零拷贝 KV 引用迭代（等价 `(&K,&V)`；元组运行时未就绪故以结构体承载）；迭代器结构体（持有容器瘦指针 + 游标槽）+ `next()`（J2 方法式接入，元素为引用值，U1 解锁返回引用）；`for x in v.iter()` 解引用迭代（自动剥层） | ✅ 已完成（2026-08-29：`iter_ref` 引用元素 `Option<&T>` 与 `iter_pairs`/`KVRef` KV 引用迭代均已落地，全量 177 用例通过） | [`v1-borrow-iter.md`](../tasks/leaf/v1-borrow-iter.md) |
| V2 | **String 码点迭代器**：`chars(&self) -> Chars`（UTF-8 码点解码——首字节定宽 + 连续字节校验，返回 `Option<char>` 迭代器）替代 MVP 字节级；`lines(&self) -> Lines`（按 `\n`/`\r\n` 分行、剥 `\r`，替代 `split("\n")` 的 Vec 拷贝）；`trim`/`trim_start`/`trim_end` `&str` 引用视图（StrFat 双槽，替代字符串拷贝）；`char` 类型拓宽至 32 位 Unicode 码点（codegen i32） | ✅ 已完成（2026-08-29：`chars`/`lines` 目标签名升级为返回 `Chars`/`Lines` 迭代器、`Chars::next() -> Option<char>`，`char` 拓宽 32 位；V2-A~E 全部完成，全量 177 用例通过） | [`v2-str-view.md`](../tasks/v2-str-view.md) |
| V3 | **Iterator 默认方法 + 适配器迁移**：`Iterator` trait 引入 `type Item` 关联类型（替代固定 i64，依赖 U2 关联类型 + U3 约束）；补默认方法 `count`/`sum`/`chain`/`enumerate`/`find`/`any`/`all`/`fold`；内置适配器 `map`/`filter`/`collect`/`take`/`skip` 对**自定义迭代器**已迁移为 trait 默认方法 + 包装迭代器（`Filter`/`Take`/`Skip`/`Chain`/`Enumerate`，惰性化）；数组/`Vec` 适配器因 Rlyeh 数组非命名类型、无法 `impl Iterator`，仍走内建 desugar（返回 `Vec`）——属**语言限制**，记为已知限制，非 V 阶段阻塞项（见下注） | ✅ 已完成（2026-08-27：A1~A4/B/C/D1~D5/E 子任务全部落地，自定义迭代器适配器迁移 + 关联类型；数组/Vec 适配器内建保留为语言限制） | [`v3-iterator-adapters.md`](../tasks/v3-iterator-adapters.md) |
| V4 | **`get_mut` 引用语义**：`Vec::get_mut(&mut self, i) -> Option<&mut T>`、`HashMap::get_mut(&mut self, k) -> Option<&mut V>`（U1 解锁返回引用；经 `*` 写回原容器，取代 MVP 值拷贝） | ✅ 已完成 | [`v4-get-mut.md`](../tasks/leaf/v4-get-mut.md) |
| V5 | **新集合**（std-lib.md §1 目标架构目录）：`HashSet<T>`（`collections/hashset.rl`，复用 HashMap 7 槽存储 + djb2/Knuth 散列，`insert`/`contains`/`remove`/`iter`/`union`/`intersection`/`difference`）；`BTreeMap<K, V>`（`btree.rl`，有序键——数组二分插入 + 移动，`first`/`last`/`range` 规划 MVP 限 `i64` 键）；`VecDeque<T>`（`deque.rl`，环形缓冲，`push_front`/`push_back`/`pop_front`/`pop_back`） | ✅ 已完成 | [`v5-new-collections.md`](../tasks/leaf/v5-new-collections.md) |

> **V 阶段收口说明（2026-08-29）**：V1/V2/V3/V4/V5 全部完成。V3 唯一未完全迁移项为「数组/`Vec` 适配器走内建 desugar（返回 `Vec`）」——根因是 Rlyeh 数组 `[T; N]` 非命名类型、无法 `impl Iterator`，需语言增强（如数组内建迭代器类型 / `Vec` 实现 `Iterator`）后方可迁移；当前该路径功能完整（适配结果正确、全量测试通过），故记为**已知语言限制**而非阻塞项，不在 V 阶段范围内强行实施（避免引入编译器结构性改动风险；其闭环方案见 [`v3-f-array-vec-iterator.md`](../tasks/leaf/v3-f-array-vec-iterator.md)）。
> **V 阶段收口复核（2026-08-30）**：闭环方案 `v3-f` 已实施复核——尝试泛化 `Iterator` trait（含补 `collect_trait` 的 trait 方法级泛型支持）后确认，数组/`Vec` 适配器经 trait 默认方法迁移在现有类型系统下**不可行**：`map`/`filter` 的泛型输出 `U` 令接收闭包处于 `fn(Self::Item) -> U`（返回位含未定 `U`）上下文，而 **H2 闭包推断要求具体 fn 类型上下文**才能定型，导致 `vec.map(|x| ..)` 直接报「闭包缺少 fn 类型上下文」；内建 desugar 之所以对任意元素类型可用，正是因为它手工特判闭包推断（`closure_return_ty` / `check_closure_expected`），trait 默认方法无法复制该逻辑。→ 该闭环须先攻克「trait 默认方法 + 闭包泛型输出推断」（重大类型系统增强），远超增量范畴。据此**正式关闭 `v3-f` 跟踪项**：数组/`Vec` 适配器走内建 desugar 确认为**正确的永久设计**（非缺陷），V3 收口结论不变，不再纳入后续迭代目标。

**验收**：见任务树 [`stage-u-z.md`](../tasks/stage-u-z.md) 各子任务叶子的「验证」字段；全量回归通过（177 用例）。
