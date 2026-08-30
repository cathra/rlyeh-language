# V3-F：数组 / `Vec` 适配器迁移到 trait 方法（V3 已知限制的闭环方案）

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)
> **状态**：❌ 经实施复核不可行，已于 2026-08-30 正式关闭跟踪项（受 H2 闭包推断 + trait 方法级泛型限制）；结论并入 V 阶段收口说明，数组/`Vec` 内建 desugar 为永久设计（非缺陷）
> **风险**：中（回归面大——全量测试大量使用数组/`Vec` 迭代与适配器；`for` 循环语义需保持不变）
> **依赖**：V3-A~E 已全部落地；本任务为收尾增强
> **权威来源（现状代码）**：`rlyeh-typecheck/src/check_expr/iterator.rs:164`（`try_check_adapter`）、`:263`（`check_iterator_adapter`）、`rlyeh-std/rlyeh/core.rl`（`Vec::iter`/`Iter<T>` 已 `impl Iterator`）、`check_for`/`check_for_vec`（数组/`Vec` 的 `for` 循环 borrow 分派）

## 背景与现状

V3 把 `map`/`filter`/`collect`/`take`/`skip`/`fold`/`chain`/`enumerate` 对**自定义迭代器**迁移为 `Iterator` trait 默认方法 + 包装迭代器（惰性化）。但 `try_check_adapter`（`iterator.rs:176-192`）仍保留一个 `is_array_vec` 分支：当接收者为数组 `[T; N]` 或 `Vec<T>` 时，走内建 **eager desugar**（`check_iterator_adapter`，在 HIR 层生成「循环 + `Vec::push`」块，返回 `Vec`），而非 trait 默认方法。根因是：

1. **`Vec<T>` 未实现 `Iterator`** → 直接 `vec.map(f)` 时 trait 默认方法不可见，只能走内建。
2. **数组 `[T; N]` 非命名类型** → 当前类型系统无法 `impl Iterator for [T; N]`，永远只能走内建。

> 注：间接路径已可用——`vec.iter()` 返回 `Iter<T>`，而 `Iter<T>` **已实现 `Iterator`**，故 `vec.iter().map(f)` 已走 trait 惰性路径。缺口仅在「直接对容器调用适配器」。`for x in vec` / `for x in arr` 由 `check_for` / `check_for_vec` 经 borrow 分派（`iter()`），不消耗容器。

## ⚠️ 关键阻塞（实施前复核，2026-08-29）

`core.rl` 中 `Iterator` trait 的适配器默认方法**目前仅支持 `i64` 元素**：
- `map(self, f: fn(i64) -> i64) -> Map<Self, i64>`、`filter` 的 `pred: fn(i64) -> bool`、`Filter::Item = i64`
- `collect(self) -> Vec<i64>`、`fold -> i64`、`take`/`skip`/`enumerate`/`chain` 的 `Item = i64`
（`core.rl:707/712/717/722/737/743/750`、`:761-885`；与 `v3-d2`/`v3-b` 叶子标注的「MVP 元素 i64 / 方法级泛型受限」一致）。

而内建 eager desugar（`check_iterator_adapter`，`iterator.rs:263`）按 `elem_ty` **泛型**生成 `Vec<u_ty>`，即 **`vec.map` 当前对任意元素类型（`Vec<String>` 等）均可用**。

**推论**：若仅「给 `Vec` 加 `impl Iterator` + 删 `try_check_adapter` 的 Vec 分支」，则 `vec.map` 会改走 i64-only 的 trait 默认方法 → **对 `Vec<非i64>` 的适配器直接回归**。因此本任务的真正前置是 **先把 `Iterator` trait 适配器默认方法 + 包装类型泛化**（元素类型用 `Self::Item`、方法级泛型 `U`/`Acc`），该能力正是 V3 文档已标注为「待类型系统支持」的延期项。

## 目标

在 `Iterator` trait 适配器**完成泛化**的前提下，让数组 / `Vec` 的直接适配器调用（`vec.map(f)` / `[1,2,3].filter(...)` 等）解析到 trait 默认方法（惰性 + 与自定义迭代器统一），消除 `try_check_adapter` 中的双轨内建分支；且对任意元素类型行为与现内建一致。

## 方案选型

### 方案 A — `Vec<T>` 实现 `Iterator`（消费式）
- 新增 `impl<T> Iterator for Vec<T> { type Item = T; fn next(&mut self) -> Option<T> }`，`next()` 按索引推进并返回元素（值拷贝 / 移动）。
- 收益：`vec.map(f)` 等直接解析到 trait 默认 `map`，返回 `Map<Vec<T>, F>` 惰性包装，经 `.collect()` 成 `Vec`，与自定义迭代器完全统一。
- **风险 1（`for` 循环语义）**：若 `Vec: Iterator`，`check_for` 会经消费式 `next()` 把 `Vec` 移出/耗尽，破坏既有 `for x in vec { ... }` 后容器仍可用的约定。须让 `check_for` 对 `Vec`/`[T;N]` **优先走 borrow 分派**（`iter()`/`check_for_vec`），仅适配器直接调用走消费式 `Iterator`。
- **风险 2（仍留数组缺口）**：数组非命名类型，方案 A 不动数组，故 `try_check_adapter` 的数组分支仍需保留 → 双轨未彻底消除。

### 方案 B — 新增内建 `ArrayIter<T, N>` 类型
- 编译器引入 `ArrayIter<T, N>`（持有数组指针 + 游标），并让数组的迭代/适配器经此类型（`impl Iterator for ArrayIter`）。
- 收益：数组零拷贝惰性迭代，与 `Vec` 对称，彻底消除内建 desugar。
- 代价：新增内建类型 + 类型系统对 `[T;N]` 迭代的支持（属编译器结构性改动），风险最高，建议独立成任务。

### 方案 C — 数组迭代先提升为 `Vec`（拷贝）
- `[1,2,3].map(f)` 先经内建把数组拷贝进 `Vec`，再走方案 A 的 `Vec: Iterator` 路径。
- 收益：实现最简、零语义风险（数组只读拷贝一次，迭代语义不变）。
- 代价：数组迭代有一次拷贝（非零成本），但对小数组可接受；若后续追求零拷贝再上方案 B。

### 方案 D（推荐折中）
分阶段、可单独回退：
- **P1（低风险，数组）**：数组保留内建 desugar，或改为方案 C 的「数组→Vec 提升」；两者功能等价（返回 `Vec`），仅拷贝差异。本步只做清理与等价验证，不动 `Vec`。
- **P2（中风险，Vec）**：给 `Vec<T>` 加消费式 `impl Iterator`；修正 `check_for` 对 `Vec`/`[T;N]` 优先 borrow 分派；删除 `try_check_adapter` 中 `Vec` 分支（`is_array_vec` 仅保留数组，或数组也走 P1）。全量回归。
- **P3（可选，数组零拷贝）**：方案 B 的 `ArrayIter<T, N>` 内建类型，消除 P1 的数组拷贝；独立任务，不在 V3-F 必需范围内。

## 风险评估

- **回归面**：全量 177 用例中大量使用数组/`Vec` 迭代与适配器链（如 `vec_api.rl`、`adapters.rl`、`tokenize`/`split` 等）。任何 `try_check_adapter` / `check_for` 改动都需整轮回归。
- **`for` 循环语义变更**：P2 引入 `Vec: Iterator` 后，`check_for` 必须显式优先 borrow 分派，否则既有 `for x in vec` 行为破坏。
- **类型系统**：需确认 `Vec<T>` 新增 `impl Iterator` 不与既有 `Iter<T>` 的 `impl Iterator` 冲突（`Vec` 与 `Iter<T>` 是不同类型，互不冲突）；泛型 `Item = T` 的关联类型投影须走通。

## 实施步骤（分阶段，每步独立可测）

0. **P0（真正前置，语言增强）— 泛化 `Iterator` trait 适配器**：
   - `core.rl` 改写：`map<U>(self, f: fn(Self::Item) -> U) -> Map<Self, U>`、`filter(self, pred: fn(Self::Item) -> bool) -> Filter<Self>`、`collect(self) -> Vec<Self::Item>`、`fold<Acc>(self, init: Acc, f: fn(Acc, Self::Item) -> Acc) -> Acc`、`chain<U: Iterator<Item = Self::Item>>(self, other: U) -> Chain<Self, U>`；包装类型 `Map<I,B>`/`Filter<I>`/`Chain<A,B>` 的 `pred`/`f`/`Item` 改用 `I::Item`/`Self::Item`。
   - **类型系统依赖**：需 `trait_default_method`（`generic.rs:238`）支持方法级泛型 `<U>`/`<Acc>` 的调用点实例化，且 `Self::Item`（`resolve.rs:64`、`context.rs:114`）在默认方法签名中正确投影替换。当前 V3 文档标注这两点「受限于类型系统」——若不支持，P0 本身即一项独立的编译器增强（重开 V3-D2/B 延期项）。
   - 验收：现有 `i64` 自定义迭代器（`Counter` 等）适配器用例行为不变；新增 `Vec<String>`/`Vec<struct>` 经 `vec.iter().map(...)` 的泛型用例通过。
1. **P1**：数组适配器路径定稿（保留内建或方案 C 提升）；补充数组惰性/急切等价断言用例；全量回归确认无行为变化。
2. **P2**（依赖 P0）：
   - `core.rl` 加 `impl<T> Iterator for Vec<T>`（`next()` 按索引消费/拷贝，`type Item = T`）。
   - `check_for` 对 `Vec`/`[T;N]` 显式经 `iter()` / `check_for_vec` borrow 分派（不变更既有 `for` 行为）。
   - `iterator.rs:178-184` 的 `is_array_vec` 收敛：删除 `Vec` 分支，`Vec` 直接 `return Ok(None)` 走通用 trait 方法解析；数组视 P1 决定保留或移除。
   - 全量回归（177 用例）+ `vec.iter().map` 与 `vec.map` 行为一致性专项（含非 i64 元素）。
3. **P3（可选）**：方案 B `ArrayIter<T, N>` 内建类型，去除数组拷贝；独立回归。

## 验收标准

- [ ] `vec.map(f)` / `vec.filter(...)` / `vec.take(n)` / `vec.skip(n)` / `vec.fold(...)` / `vec.collect()` 解析到 `Iterator` trait 默认方法（惰性），结果与原内建 desugar 完全一致。
- [ ] `for x in vec` / `for x in arr` 行为（容器不被消耗、循环后仍可用）保持不变。
- [ ] `try_check_adapter` 中针对 `Vec` 的内建分支已删除；数组分支按 P1 决策收敛（消除或保留并标注）。
- [ ] 全量回归通过（177 用例），无新增 warning。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-29 | 由 V3-D4/D5「数组/Vec 适配器走内建为语言限制」衍生，作为该已知限制的闭环方案文档（可行性分析 + 分阶段实施计划）；V 阶段验收不阻塞，独立跟踪 |
| 2026-08-29 | **实施复核结论**：尝试 P0（泛化 `Iterator` trait）后确认闭环不可行——(1) `collect_trait` 已补 trait 方法级泛型支持（`fold<Acc>`/`map<U>` 声明期可编译）；(2) 但 `map`/`filter` 的通用输出 `U` 令接收闭包处于 `fn(Self::Item) -> U` 这种「返回位含未定 `U`」的 fn 上下文，H2 闭包推断要求具体 fn 类型上下文而无法定型 → `vec.map(\|x\| ..)` 直接报「闭包缺少 fn 类型上下文」。内建 desugar 之所以对任意元素类型可用，正是因为它手工特判闭包推断（`closure_return_ty`/`check_closure_expected`）。→ 闭环须先攻克「trait 默认方法 + 闭包泛型输出推断」，属重大类型系统增强，远超增量范畴。建议维持 V 阶段「已知限制」：数组/`Vec` 走内建 desugar 为正确且永久的设计，本任务关闭 |

## 闭环状态（2026-08-30，正式关闭）

经 2026-08-29 实施复核，本任务在现有类型系统下确认不可行（详见「⚠️ 关键阻塞」与变更记录 2026-08-29 行）。结论已并入 [`stages/V.md`](../../stages/V.md) 的「V 阶段收口复核（2026-08-30）」与 [`v3-iterator-adapters.md`](../../v3-iterator-adapters.md) 进度小结。据此：

- **正式关闭本跟踪项**：数组/`Vec` 适配器走内建 desugar 确认为**正确的永久设计**（非缺陷），不再纳入后续迭代目标。
- **V3 收口结论不变**：数组/`Vec` 适配器内建保留为已知语言限制，但功能完整、全量测试通过，不影响 V 阶段验收。
- **后续若重开**：须先攻克「trait 默认方法 + 闭包泛型输出推断」（重大类型系统增强），属独立编译器攻坚任务，非 V 阶段范畴。
