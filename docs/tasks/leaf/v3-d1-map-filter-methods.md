# V3-D1：`map`/`filter` trait 默认方法 + 绑定包装迭代器

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-D 细分子任务）
> **状态**：🔧 部分完成（filter/take/skip/collect ✅；map/fold 见 V3-D2）
> **风险**：中（单一适配器对，独立可测）
> **依赖**：V3-B、V3-C、**V3-D 语言增强**（默认方法返回泛型包装时 `Self` 实例化）
> **权威来源**：`core.rl`（`Filter`/`Take`/`Skip` 结构体）、`rlyeh-typecheck`（`try_check_adapter`）

## 目标

`map`/`filter` 从 typecheck 内建 desugar 迁移为 `Iterator` trait 默认方法，返回 V3-C 的 `Map`/`Filter` 包装迭代器。

## 背景

迁移后 `v.map(|x| x*10)` 走 trait 默认方法 + `Map` 包装迭代器（V3-C 已定义），不再收集到 Vec。

## 改动范围

- **std**：`Iterator` trait 增 `fn map<B>(self, f: fn(Self::Item) -> B) -> Map<Self, B>` / `fn filter(self, p: fn(Self::Item) -> bool) -> Filter<Self, P>` 默认方法（绑定 V3-C 结构体 + `next()`）。
- **typecheck**：`try_check_adapter` 对 `map`/`filter` 优先走通用 trait 方法解析（`find_impl_for_method` / trait 默认方法回退）。
- **测试**：`v.map(...).collect()` / `v.iter().filter(pred).take(3)` 链式保持输出一致。

## 实施情况（部分完成，2026-08-27）

### V3-D 语言增强（前置，已完成）

- **`method.rs` `replace_type_self`**：方法返回类型 `Take2<Self>` 等含 `Self` 的签名，`Self` 替换为 impl 目标具体类型（否则 `Take2<Self>::next` 内 `Self::next` 无法解析）。
- **`field.rs`**：字段类型替换时，实例类型参数为 `Generic` 占位（默认方法返回泛型包装）时不覆盖全局 `generic_subst` 的已解析映射。

### filter/take/skip/collect 迁移（已完成）

- **std `Iterator` trait 默认方法**：`filter(self, pred) -> Filter<Self>`、`take(self, n) -> Take<Self>`、`skip(self, n) -> Skip<Self>`、`collect(self) -> Vec<i64>`——消耗 self 返回 V3-C 包装迭代器（惰性）。
- **typecheck `try_check_adapter`**：filter/take/skip/collect 对**自定义迭代器**（实现 Iterator/next）走通用 trait 方法解析（惰性）；数组/`Vec<T>`（不实现 Iterator）保留内建 eager desugar（返回 Vec 兼容）。
- **测试**：`adapters.rl` 自定义迭代器用例改 `.collect()`；新增 `v3d1_lazy_adapters.rl`（filter/take/skip/链式/collect）。
- map/fold 仍走内建（V3-D2 迁移）。

## 验证

- [x] `v.filter(pred)` 返回 `Filter`，跳过不满足项（`v3d1_lazy_adapters.rl` filter=6）。
- [x] `v.take(n)`/`v.skip(n)` 惰性取/跳（take=3/skip=9）。
- [x] 链式 `filter→take`、`filter→collect` 正确（2 / 6）。
- [ ] `v.map(...)` 返回 `Map` 包装迭代器（见 V3-D2）。
- [x] 全量回归通过（144 用例全过）。

## 为什么是中风险

每次仅迁移**一对适配器**，绑定 V3-C 结构体，风险点集中在本对包装迭代器 + 方法解析回退，独立可测、失败可回退。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-D（高风险）细化拆分而来 |
| 2026-08-27 | 语言增强（Self 实例化）+ filter/take/skip/collect 惰性化 + 测试更新 |
