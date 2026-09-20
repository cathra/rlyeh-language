# V3 — Iterator 关联类型 + 适配器迁移

> **阶段**：V（集合与迭代器完整化）
> **状态**：✅ 已完成（V3-A1~A4/B/C/D1~D5/E 已落地——关联类型 `type Item` + 自定义迭代器适配器迁移为 protocol 默认方法 + 包装迭代器（惰性化）；**数组/`Vec` 适配器仍走内建 desugar**——Rlyeh 数组非命名类型无法 `impl Iterator`，记为已知语言限制，非 V 阶段阻塞项）。**闭环复核（2026-08-30）**：已知限制的闭环方案 [`leaf/v3-f-array-vec-iterator.md`](./leaf/v3-f-array-vec-iterator.md) 已实施复核并确认**不可行**、正式关闭——该内建 desugar 为正确的永久设计，非缺陷。
> **最后更新**：2026-08-29
> **权威来源**：阶段详情文档 [`stages/V.md`](../stages/V.md)
> **核心目标**：① 为 `Iterator` protocol 引入 `type Item` 关联类型（替代固定 i64）；② 将适配器 `map`/`filter`/`fold`/`collect`/`take`/`skip` 从"typecheck 内建 desugar"迁移为 protocol 默认方法 + 包装迭代器。
> **本层职责**：本索引文档只记录 V3 各子任务（叶子文档）的任务列表与实现进度；**具体实施情况见各叶子文档**。
>
> **风险分解原则**：高风险任务（V3-A 关联类型、V3-D 适配器迁移）已细化为中/低风险子任务（V3-A1~A4、V3-D1~D5），每次改动独立可测、可回退。

---

## 子任务列表与进度

| 子任务 | 叶子文档 | 依赖 | 风险 | 状态 |
|--------|---------|------|------|------|
| **V3-A1** | [`v3-a1-protocol-type-parser.md`](./leaf/v3-a1-protocol-type-parser.md) | — | 低 | ✅ 已完成（type Item = i64 默认具体化，2026-08-27） |
| **V3-A2** | [`v3-a2-assoc-type-resolve.md`](./leaf/v3-a2-assoc-type-resolve.md) | V3-A1 | 中 | ✅ 已完成（U2 核实 + 关联类型投影，2026-08-27） |
| **V3-A3** | [`v3-a3-iterator-item.md`](./leaf/v3-a3-iterator-item.md) | V3-A1、V3-A2 | 中 | ✅ 已完成（Iterator::Item + 各 impl 补 type Item，2026-08-27） |
| **V3-A4** | [`v3-a4-item-projection.md`](./leaf/v3-a4-item-projection.md) | V3-A3 | 低 | ✅ 已完成（Range::Item 命名投影，2026-08-27） |
| **V3-B** | [`v3-b-default-methods.md`](./leaf/v3-b-default-methods.md) | V3-A（全）、V3-C | 中 | ✅ 已完成（find/fold/chain/enumerate，2026-08-27） |
| **V3-C** | [`v3-c-wrapper-iterators.md`](./leaf/v3-c-wrapper-iterators.md) | V3-A（全） | 中 | ✅ 已完成（Filter/Take/Skip/Chain/Enumerate，2026-08-27） |
| **V3-D1** | [`v3-d1-map-filter-methods.md`](./leaf/v3-d1-map-filter-methods.md) | V3-B、V3-C | 中 | ✅ 已完成（filter/take/skip/collect 惰性化 + 语言增强，2026-08-27） |
| **V3-D2** | [`v3-d2-take-skip-methods.md`](./leaf/v3-d2-take-skip-methods.md) | V3-B、V3-C | 中 | ✅ 已完成（map 惰性化 + Iter/IterMut 实现 Iterator，2026-08-27） |
| **V3-D3** | [`v3-d3-collect-fold-methods.md`](./leaf/v3-d3-collect-fold-methods.md) | V3-B、V3-C | 中 | ✅ 已完成（fold 惰性化，2026-08-27） |
| **V3-D4** | [`v3-d4-adapter-dispatch.md`](./leaf/v3-d4-adapter-dispatch.md) | V3-D1、V3-D2、V3-D3 | 中 | ✅ 已完成（双轨收敛：自定义迭代器走 protocol 方法，数组/`Vec` 走内建——语言限制，2026-08-27） |
| **V3-D5** | [`v3-d5-builtin-cleanup.md`](./leaf/v3-d5-builtin-cleanup.md) | V3-D4 | 低 | ✅ 已完成（数组/`Vec` 内建保留因数组非命名类型，非死代码，2026-08-27） |
| **V3-E** | [`v3-e-builtin-cleanup.md`](./leaf/v3-e-builtin-cleanup.md) | V3-D（全） | 中 | ✅ 已完成（双轨定案，2026-08-27） |

**进度小结**：V3-A1~A4 / V3-B / V3-C / V3-D1~D5 / V3-E 子任务已落地。适配器 map/filter/take/skip/collect/fold/chain/enumerate 对**自定义迭代器**已迁移为 protocol 默认方法 + 包装迭代器（惰性化）。**已知语言限制（非阻塞）**：数组/`Vec` 适配器仍走内建 desugar（返回 Vec）——Rlyeh 数组 `[T;N]` 非命名类型、无法 `impl Iterator`，需语言增强（如数组内建迭代器类型 / `Vec` 实现 `Iterator`）后迁移；当前该路径功能完整、全量测试通过，故 V3 判定为已完成，此项记为已知限制（闭环方案见 `leaf/v3-f-array-vec-iterator.md`）。全量 147 用例通过（含 V1/V2 收口后全量 177 用例通过）。**闭环复核（2026-08-30）**：上述已知限制的闭环方案 `v3-f` 已实施复核并确认**不可行**——`map`/`filter` 的泛型输出 `U` 触发 H2 闭包推断失败（`vec.map(|x| ..)` 报「闭包缺少 fn 类型上下文」），须先攻克「protocol 默认方法 + 闭包泛型输出推断」重大类型系统增强；据此**正式关闭 `v3-f`**，数组/`Vec` 内建 desugar 确认为永久设计（非缺陷），V3 收口结论不变。

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
