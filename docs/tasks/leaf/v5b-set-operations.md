# V5b HashSet 集合运算（参考 Python `set`）

> **所属阶段**：阶段 V（V5 新集合的延期子任务）
> **状态**：⏳ 规划中
> **依赖**：V5（`HashSet<T>` ✅，见 [`v5-new-collections.md`](./v5-new-collections.md)）
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)
> **参考**：Python `set` 集合代数语义

## 目标

为 `HashSet<T>` 补齐集合代数运算（并 / 交 / 差 / 对称差 / 子集 / 超集 / 不相交），语义对齐 Python `set`，使 Rlyeh 集合类型具备与 Python 同级的代数表达能力，替代手写 `for` + `contains` 拼装。

## 背景

`std-lib.md §3.4` 明确 `HashSet<T>` 的 `union`/`intersection`/`difference` 集合运算与借用迭代器 `iter` 为 **MVP 限制（规划中）**。当前 `HashSet<T>` 仅支持 `insert`/`contains`/`remove`/`clear`/`elements`/`len`/`cap`/`is_empty`，无集合级运算。Python `set` 提供完备的集合代数（详见 §技术细节），是 Rlyeh 该能力的直接对标。

## 技术细节

### Python `set` 运算语义（对标参考）

| 运算 | Python 运算符 | Python 命名方法 | 返回 | 说明 |
|------|--------------|----------------|------|------|
| 并集 | `a \| b` | `a.union(b, ...)` | 新集合 | 两集合所有元素 |
| 交集 | `a & b` | `a.intersection(b, ...)` | 新集合 | 共有元素 |
| 差集 | `a - b` | `a.difference(b, ...)` | 新集合 | 属于 a 不属于 b |
| 对称差 | `a ^ b` | `a.symmetric_difference(b)` | 新集合 | 仅属于其一 |
| 子集 | `a <= b` | `a.issubset(b)` | bool | a 所有元素 ∈ b |
| 真子集 | `a < b` | — | bool | a⊆b 且 a≠b |
| 超集 | `a >= b` | `a.issuperset(b)` | bool | b 所有元素 ∈ a |
| 真超集 | `a > b` | — | bool | b⊆a 且 a≠b |
| 不相交 | — | `a.isdisjoint(b)` | bool | 交集为空 |

原地变体（修改自身）：`update()`/`|=`、`intersection_update()`/`&=`、`difference_update()`/`-=`、`symmetric_difference_update()`/`^=`。

### 建议 Rlyeh API（命名方法优先，运算符糖可选）

Rlyeh 现有容器 API 均为命名方法（如 `Vec::push`、`HashSet::insert`），运算符重载机制尚待确认；本期以**命名方法**为一级 API（与 Python 命名方法一致），运算符糖（`|`/`&`/`-`/`^`/`<`/`>` 作集合语义）列为可选增强：

```rlyeh
impl<T> HashSet<T> {
    // 返回新集合（不修改自身）
    fn union(&self, other: &HashSet<T>) -> HashSet<T>;
    fn intersection(&self, other: &HashSet<T>) -> HashSet<T>;
    fn difference(&self, other: &HashSet<T>) -> HashSet<T>;
    fn symmetric_difference(&self, other: &HashSet<T>) -> HashSet<T>;
    // 关系判断（bool）
    fn is_subset(&self, other: &HashSet<T>) -> bool;
    fn is_superset(&self, other: &HashSet<T>) -> bool;
    fn is_proper_subset(&self, other: &HashSet<T>) -> bool;
    fn is_proper_superset(&self, other: &HashSet<T>) -> bool;
    fn is_disjoint(&self, other: &HashSet<T>) -> bool;
    // 原地变体（修改自身，等价 Python update/intersection_update/...）
    fn union_with(&mut self, other: &HashSet<T>);
    fn intersect_with(&mut self, other: &HashSet<T>);
    fn difference_with(&mut self, other: &HashSet<T>);
    fn symmetric_with(&mut self, other: &HashSet<T>);
}
```

### 实现路径（基于现有 HashSet 能力，零新增语言特性）

- `union`：新建空集合，`self.elements()` 与 `other.elements()` 逐元素 `insert`。
- `intersection`：遍历较小集合元素，`other.contains(x)` 为真则 `insert`。
- `difference`：遍历 `self.elements()`，`other.contains(x)` 为假则 `insert`。
- `symmetric_difference`：`union` 减 `intersection`（或遍历两侧、仅 `contains` 另一侧为假者 `insert`）。
- `is_subset` / `is_superset` / `is_proper_*`：遍历 + `contains` + 长度比较（`proper` 需 `len` 不等）。
- `is_disjoint`：遍历一侧，`contains` 任一项命中即返回 false。
- 遍历依赖 `elements() -> Vec<T>`（已有）；借用迭代器 `iter -> Iter<'_, T>` 仍规划中，不阻塞本任务（见 §约束）。

## 约束 / 已知限制

- **元素类型**：受 `hash_value` 内建范围限制，仅 `i64` / `String` 可作 `T`（与现有 HashSet 一致）。
- **运算符糖**：`|`/`&`/`-`/`^`/`<`/`>` 作集合语义需语言层运算符重载支持，本期仅落地命名方法；多集合参数（Python `a.union(b, c)`）亦因可变参数未支持，改以链式 `a.union(b).union(c)` 表达。
- **借用迭代器**：`iter -> Iter<'_, T>` 仍规划中；本任务用 `elements()` 返回的 `Vec<T>` 遍历即可实现全部运算。

## 验证

- 单元：`tests/run-pass/hashset_setops.rl` —— 对拍 Python `set` 语义：
  - `a = {1,2,3}`、`b = {3,4,5}` → `union`=`{1,2,3,4,5}`、`intersection`=`{3}`、`difference`=`{1,2}`、`symmetric_difference`=`{1,2,4,5}`。
  - 关系：`{1}.is_subset({1,2})`=true、`{1,2}.is_proper_subset({1,2})`=false、`{1}.is_disjoint({2})`=true。
  - 原地：`a.union_with(b)` 后 `a`=并集。
- 集成：全量 `rlyeh test tests` 无回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-02 | 由用户需求「集合支持集合运算，参考 Python」拆出为 V5 延期子任务叶子；对标 Python `set` 集合代数，定义命名方法优先 API 与基于 `elements()`/`contains` 的实现路径 |
