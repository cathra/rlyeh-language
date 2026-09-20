# V3-B：剩余默认方法 `chain`/`enumerate`/`find`/`fold`

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)
> **状态**：✅ 已完成（find/fold/chain/enumerate 全部落地，2026-08-27）
> **风险**：中
> **依赖**：V3-A（A1~A4）、V3-C
> **权威来源**：`core.rl`（`Iterator` protocol）

## 目标

基于关联类型补充 `Iterator` protocol 剩余默认方法。

## 背景

protocol 默认方法机制已实现（`count`/`sum`/`any`/`all` ✅），本任务补充 `chain`/`enumerate`/`find`/`fold`。

## 实施情况（已完成，2026-08-27）

core.rl `Iterator` protocol 内追加默认方法（MVP 元素 i64）：
- **`find`** ✅：`fn find(&mut self, pred: fn(i64) -> bool) -> i64`——遍历返回首个满足谓词元素，未找到返回 -1（`v3b_find.rl`：find(is_even)=2，find(is_big)=-1）。
- **`fold`** ✅：`fn fold(&mut self, init: i64, f: fn(i64,i64)->i64) -> i64`——V3-D3 关闭内建 fold 对自定义迭代器的拦截后走 protocol 默认（`v3d3_fold_lazy.rl`）。
- **`chain`** ✅：`fn chain(self, other: Self) -> Chain<Self, Self>`——消耗 self 返回 Chain 包装；MVP `other` 与 `Self` 同类型（方法级泛型 `<U>` 接异类型受默认方法方法级泛型限制）。依赖 **参数 `Self` 替换**语言增强（`method.rs` 参数类型应用 `replace_type_self`，否则 `other: Self` 占位无法匹配实参）。
- **`enumerate`** ✅：`fn enumerate(self) -> Enumerate<Self>`——消耗 self 返回 Enumerate 包装（产出序号）。

## 改动范围

core.rl `Iterator` protocol 内追加默认方法：
- `chain(self, other: Self) -> Chain<Self, Self>`（MVP 同类型；异类型 `<U>` 受限）。
- `enumerate(self) -> Enumerate<Self>`。
- `find(&mut self, pred: fn(i64) -> bool) -> i64`（MVP i64 简化）。
- `fold(&mut self, init: i64, f: fn(i64,i64) -> i64) -> i64`。
- 默认方法内调用 `self.next()`（已验证可行）。

## 验证

- [x] 自定义迭代器调用 `find` 正确（`v3b_find.rl` 通过）。
- [x] `fold` protocol 默认（`v3d3_fold_lazy.rl` 通过）。
- [x] `chain`/`enumerate` 返回对应包装迭代器（`v3b_chain_enumerate.rl`：chain=10/enumerate=6/filter→enumerate）。
- [x] 全量回归通过（147 用例全过）。
