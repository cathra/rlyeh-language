# V3-B：剩余默认方法 `chain`/`enumerate`/`find`/`fold`

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)
> **状态**：📋 规划
> **风险**：中
> **依赖**：V3-A（A1~A4）、V3-C
> **权威来源**：`core.rl`（`Iterator` trait）

## 目标

基于关联类型补充 `Iterator` trait 剩余默认方法。

## 背景

trait 默认方法机制已实现（`count`/`sum`/`any`/`all` ✅），本任务补充 `chain`/`enumerate`/`find`/`fold`。

## 实施情况

（未开始实施）

## 改动范围

core.rl `Iterator` trait 内追加默认方法：
- `chain<U: Iterator>(self, other: U) -> Chain<Self, U>`（依赖 V3-C 的 `Chain`）。
- `enumerate(self) -> Enumerate<Self>`（依赖 V3-C 的 `Enumerate`）。
- `find(self, pred: fn(&Item) -> bool) -> Option<Item>`（遍历，满足谓词返回）。
- `fold<B>(self, init: B, f: fn(B, Item) -> B) -> B`（归约累加）。
- 默认方法内调用 `self.next()`（已验证 trait 默认方法体内调用抽象方法可行）。

## 验证

- [ ] 自定义迭代器调用 `find`/`fold`（无需包装迭代器的场景）正确。
- [ ] `chain`/`enumerate` 返回对应包装迭代器（依赖 V3-C）。
- [ ] 元素类型经 `Item` 关联类型正确（非 i64 元素也可）。
