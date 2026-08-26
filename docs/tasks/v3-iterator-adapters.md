# V3 — Iterator 关联类型 + 适配器迁移

> **阶段**：V（集合与迭代器完整化）
> **状态**：🔧 部分完成（trait 默认方法机制 ✅；`Iterator` 默认方法 count/sum/any/all ✅；`Iterator::Item` 关联类型与适配器迁移 📋）
> **最后更新**：2026-08-26
> **权威来源**：阶段详情文档 [`stages/V.md`](../stages/V.md)
> **核心目标**：① 为 `Iterator` trait 引入 `type Item` 关联类型（替代固定 i64）；② 将适配器 `map`/`filter`/`fold`/`collect`/`take`/`skip` 从"typecheck 内建 desugar"迁移为 trait 默认方法 + 包装迭代器。
> **本层职责**：本索引文档只记录 V3 各子任务（叶子文档）的任务列表与实现进度；**具体实施情况见各叶子文档**。
>
> **风险分解原则**：高风险任务（V3-A 关联类型、V3-D 适配器迁移）已细化为中/低风险子任务（V3-A1~A4、V3-D1~D5），每次改动独立可测、可回退。

---

## 子任务列表与进度

| 子任务 | 叶子文档 | 依赖 | 风险 | 状态 |
|--------|---------|------|------|------|
| **V3-A1** | [`v3-a1-trait-type-parser.md`](./leaf/v3-a1-trait-type-parser.md) | — | 低 | 📋 规划 |
| **V3-A2** | [`v3-a2-assoc-type-resolve.md`](./leaf/v3-a2-assoc-type-resolve.md) | V3-A1 | 中 | 📋 规划 |
| **V3-A3** | [`v3-a3-iterator-item.md`](./leaf/v3-a3-iterator-item.md) | V3-A1、V3-A2 | 中 | 📋 规划 |
| **V3-A4** | [`v3-a4-item-projection.md`](./leaf/v3-a4-item-projection.md) | V3-A3 | 低 | 📋 规划 |
| **V3-B** | [`v3-b-default-methods.md`](./leaf/v3-b-default-methods.md) | V3-A（全）、V3-C | 中 | 📋 规划 |
| **V3-C** | [`v3-c-wrapper-iterators.md`](./leaf/v3-c-wrapper-iterators.md) | V3-A（全） | 中 | 📋 规划 |
| **V3-D1** | [`v3-d1-map-filter-methods.md`](./leaf/v3-d1-map-filter-methods.md) | V3-B、V3-C | 中 | 📋 规划 |
| **V3-D2** | [`v3-d2-take-skip-methods.md`](./leaf/v3-d2-take-skip-methods.md) | V3-B、V3-C | 中 | 📋 规划 |
| **V3-D3** | [`v3-d3-collect-fold-methods.md`](./leaf/v3-d3-collect-fold-methods.md) | V3-B、V3-C | 中 | 📋 规划 |
| **V3-D4** | [`v3-d4-adapter-dispatch.md`](./leaf/v3-d4-adapter-dispatch.md) | V3-D1、V3-D2、V3-D3 | 中 | 📋 规划 |
| **V3-D5** | [`v3-d5-builtin-cleanup.md`](./leaf/v3-d5-builtin-cleanup.md) | V3-D4 | 低 | 📋 规划 |
| **V3-E** | [`v3-e-builtin-cleanup.md`](./leaf/v3-e-builtin-cleanup.md) | V3-D（全） | 中 | 📋 规划 |

**进度小结**：V3 全部子任务待办；已具备基础（trait 默认方法机制 + count/sum/any/all，见 [CODEBUDDY.md](../../CODEBUDDY.md) §5.5）。高风险 V3-A/V3-D 已分解为中/低风险子任务（V3-A1~A4、V3-D1~D5）。

---

## 执行顺序与依赖

```
V3-A1 → V3-A2 → V3-A3 → V3-A4   （关联类型，低→中→中→低）
        │
        ├───────→ V3-C（包装迭代器，中）
        │                └────→ V3-D1 / V3-D2 / V3-D3（各适配器对，中）
        │                         └──→ V3-D4（分派收敛，中）→ V3-D5（清理，低）
        └──────→ V3-B（剩余默认方法，中）
                                       V3-D（全）→ V3-E（内建清理，中）
```

**建议顺序**：V3-A1 → V3-A2 → V3-A3 → V3-A4 → V3-C → V3-B → V3-D1 → V3-D2 → V3-D3 → V3-D4 → V3-D5 → V3-E。

---

## 风险分解说明

| 原任务 | 原风险 | 细化后子任务 | 细化后风险 |
|--------|:---:|-------------|-----------|
| **V3-A**（关联类型） | 高 | V3-A1 / V3-A4 | 低 |
| | | V3-A2 / V3-A3 | 中 |
| **V3-D**（适配器迁移） | 高 | V3-D5 | 低 |
| | | V3-D1 / V3-D2 / V3-D3 / V3-D4 | 中 |

> 分解依据：将「类型系统结构性改动」与「集中式分派重构」拆为**按序小步**，每步只改一个载体/一对适配器，独立可测、失败可回退，避免单点大面积改动。

---

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 初始拆分（V3-A ~ V3-E 五个叶子文档） |
| 2026-08-26 | 高风险分解：V3-A → V3-A1~A4、V3-D → V3-D1~D5（各细化为中/低风险） |
