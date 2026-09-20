# 阶段 J — 迭代器与集合协议

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-g-l.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：数组迭代、自定义迭代器接入 for、适配器均已完成（J1–J3 ✅）。实现细节见任务树 [`stage-g-l.md`](../tasks/stage-g-l.md) 对应叶子文档（`j1`–`j3`）。`Iterator` protocol（关联类型 + 适配器迁移）属 V3 阶段任务。std-lib.md §2.3 `Iterator` protocol 仍为规划 API。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| J1 | **数组迭代**：`for x in arr`（desugar 为数组下标循环，数组切片模式可复用） | ✅ | [`j1-array-iter.md`](../tasks/leaf/j1-array-iter.md) |
| J2 | **`Iterator` protocol**：`next() -> Option<Item>` 核心方法；自定义迭代器接入 `for`（经 protocol 方法调用） | ✅ | [`j2-custom-iter.md`](../tasks/leaf/j2-custom-iter.md) |
| J3 | **适配器**：`map`/`filter`/`fold`/`collect`/`take`/`skip`（依赖 H 闭包） | ✅ | [`j3-adapters.md`](../tasks/leaf/j3-adapters.md) |
