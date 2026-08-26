# V3-C：包装迭代器类型

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)
> **状态**：📋 规划
> **风险**：中
> **依赖**：V3-A（A1~A4）
> **权威来源**：`core.rl`（新结构体 + impl Iterator）

## 目标

引入适配器所需的包装迭代器结构体 + `next()` 实现。

## 背景

适配器 `map`/`filter`/`take`/`skip`/`chain`/`enumerate` 目标签名返回包装迭代器（`-> Map<Self, B>` 等）。这些结构体持有底层迭代器 + 闭包/参数槽，`next()` 实现变换逻辑。

## 实施情况

（未开始实施）

## 改动范围

core.rl 新增结构体 + impl Iterator：
- `Map<Self, B>`：槽 = 底层迭代器 + 闭包函数指针；`next()` = `map_f(self.next())`。
- `Filter<Self, P>`：槽 = 底层迭代器 + 谓词；`next()` = 跳过不满足项。
- `Take<Self>`：槽 = 底层迭代器 + 剩余计数；`next()` = 计数归零返回 None。
- `Skip<Self>`：槽 = 底层迭代器 + 待跳计数；`next()` = 先跳够再产出。
- `Chain<Self, U>`：槽 = 前迭代器 + 后迭代器；`next()` = 前者耗尽转后者。
- `Enumerate<Self>`：槽 = 底层迭代器 + 序号；`next()` = 产出 `(i, item)`。

**关键设计**：底层迭代器槽的存储（按值持有迭代器 vs 引用）——MVP 建议按值（迭代器为值对象），包装迭代器 `impl Iterator` 的 `type Item` 各异。

## 验证

- [ ] 每个包装迭代器 `next()` 变换逻辑正确（独立测试）。
- [ ] 包装迭代器可链式嵌套（`map` 后再 `filter`）。
- [ ] 元素类型经关联类型正确传播。
