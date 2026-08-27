# V3-C：包装迭代器类型

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)
> **状态**：✅ 已完成（Filter/Take/Skip/Chain/Enumerate 5 个包装迭代器，2026-08-27）
> **风险**：中
> **依赖**：V3-A（A1~A4）
> **权威来源**：`core.rl`（新结构体 + impl Iterator）

## 目标

引入适配器所需的包装迭代器结构体 + `next()` 实现。

## 背景

适配器 `map`/`filter`/`take`/`skip`/`chain`/`enumerate` 目标签名返回包装迭代器（`-> Map<Self, B>` 等）。这些结构体持有底层迭代器 + 闭包/参数槽，`next()` 实现变换逻辑。

## 实施情况（已完成，2026-08-27）

core.rl 新增 5 个泛型包装迭代器（持底层迭代器 `I` 按值 + 参数槽，`impl<I> Iterator`）：
- `Filter<I>`：持底层迭代器 + 谓词 `fn(i64)->bool`；`next()` 循环跳过不满足项。
- `Take<I>`：持底层迭代器 + `remaining` 计数；`next()` 计数归零返回 None。
- `Skip<I>`：持底层迭代器 + `to_skip`；`next()` 先跳够再产出。
- `Chain<A, B>`：持前后迭代器 + `on_a: bool`；`next()` 前者耗尽转后者。
- `Enumerate<I>`：持底层迭代器 + `idx`；`next()` 产出递增序号。
- **Map**（变换元素类型）MVP 走 eager desugar（适配器内建），未做成惰性包装。

**关键设计**：底层迭代器按值持有（迭代器为值对象）；MVP 元素固定 i64（`type Item = i64`，与 V3-A3 默认方法一致），泛型 `I::Item` 投影传播待 `where I: Iterator` 约束完善后泛化。泛型结构体构造用 U8 语法 `Filter<Range> { .. }`（带类型实参，字段推断不可用）。

## 验证

- [x] 每个包装迭代器 `next()` 变换逻辑正确（`v3c_wrapper_iterators.rl`：Filter=6/Take=3/Skip=9/Chain=10/Enumerate=6）。
- [ ] 包装迭代器可链式嵌套（`map` 后再 `filter`）——需适配器迁移（V3-D）后验证。
- [x] 元素类型经关联类型正确传播（`type Item = i64` 一致）。
- [x] 全量回归通过（142 用例全过）。
