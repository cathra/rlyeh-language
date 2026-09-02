# V5d+ HashMap 按键集合运算符糖（`|`/`&`/`-`/`^`，2026-09-02）

> **归属**：阶段 V（集合与迭代器完整化）· V5d 运算符重载的延续项
> **状态**：✅ 已完成
> **关联**：[`v5d-operator-overload.md`](./v5d-operator-overload.md)（V5d ✅，BinaryOp 重载机制）· [`v5d1-comparison-overload.md`](./v5d1-comparison-overload.md)（V5d+ 比较链重载）· std-lib §3.2

## 背景

V5d 交付通用 `BinaryOp` 运算符重载（`BitOr`/`BitAnd`/`Sub`/`BitXor` 等 trait + `core.rl` 实现），并落地 `HashSet<T>` 的 `| & - ^` 集合代数糖（降级到 V5b 关系方法）。V5d+ 进一步补齐比较链 `<`/`<=`/`>`/`>=`（子集/超集）。

本项将同样的运算符糖扩展到 **`HashMap<K, V>` 的按键集合**——`|`/`&`/`-`/`^` 作用于键集合（value 视为键的附带数据），对标 Python `dict` 合并语义。

## 方案

### 1. 命名方法（core.rl，`impl HashMap<K, V>`）

```rlyeh
fn union(&self, other: &HashMap<K, V>) -> HashMap<K, V> {
    // 先放 a 的全部键值，再放 b 的全部键值（b 后插入自然覆盖冲突键 → b 胜）
}
fn intersection(&self, other: &HashMap<K, V>) -> HashMap<K, V> {
    // 仅保留同时存在于 a、b 的键，值取 a（结果 ⊆ a）
}
fn difference(&self, other: &HashMap<K, V>) -> HashMap<K, V> {
    // 仅保留存在于 a 但不存在于 b 的键，值取 a（结果 ⊆ a）
}
fn symmetric_difference(&self, other: &HashMap<K, V>) -> HashMap<K, V> {
    // 仅保留存在于一方（不共享）的键；a 独有取 a 值、b 独有取 b 值
}
```

实现复用既有 `keys()`（稀疏收集存活槽键为 `Vec<K>`）+ `get(ka[i]).unwrap()`（键来自 `keys()` 必命中）+ `contains_key` + `insert`（命中即覆盖值）。

### 2. 运算符糖（core.rl，`BitOr`/`BitAnd`/`Sub`/`BitXor` 实现）

```rlyeh
impl<K, V> BitOr  for HashMap<K, V> { type Output = HashMap<K, V>; fn bitor(self, other) -> HashMap<K, V>  { self.union(&other) } }
impl<K, V> BitAnd for HashMap<K, V> { type Output = HashMap<K, V>; fn bitand(self, other) -> HashMap<K, V> { self.intersection(&other) } }
impl<K, V> Sub    for HashMap<K, V> { type Output = HashMap<K, V>; fn sub(self, other) -> HashMap<K, V>    { self.difference(&other) } }
impl<K, V> BitXor for HashMap<K, V> { type Output = HashMap<K, V>; fn bitxor(self, other) -> HashMap<K, V> { self.symmetric_difference(&other) } }
```

完全复用 V5d 的 `BinaryOp` 重载回退（typecheck 在 `check_binary` 失败处 desugar 为 `left.<bitor|bitand|sub|bitxor>(right)`，经 method-call 全链路解析，**codegen 零改动**）。

### 3. 语义约定（与 Python `dict` 对齐）

| 运算符 | 键集合 | 值取舍 |
|--------|--------|--------|
| `a | b` | `A ∪ B`（两表所有键） | 冲突键 **右操作数 b 胜**（b 在 a 之后插入自然覆盖） |
| `a & b` | `A ∩ B`（共有键） | **左操作数 a 的值**（结果 ⊆ a，自然保留 a 的值） |
| `a - b` | `A - B`（仅 a 独有键） | **左操作数 a 的值**（结果 ⊆ a） |
| `a ^ b` | `(A - B) ∪ (B - A)`（仅一方独有键） | a 独有取 a 值、b 独有取 b 值 |

> 设计取舍：`|` 取 b 胜以对齐现代语言 `dict | dict` 合并惯例（Python 3.9+ / JS 展开）；`&`/`-` 取 a 值因结果天然是 a 的子集，左操作数的值最直观；`^` 各方独有键自然取各自原值。集合运算中「值」是从属信息（操作本质是按键集合），上述均为确定性约定。

### 4. 范围与限制

- `|`/`&`/`-`/`^`（并集/交集/差集/对称差）四则按键集合运算全部交付，与 `HashSet` 运算符糖同构（`HashSet` 为纯键集合，无值取舍问题；`HashMap` 按上表处理值）。
- `bitor`/`bitand`/`sub`/`bitxor` 均按值消费两个操作数（`fn bitor(self, other: Self)`），故 `let u = a | b;` 后 `a`、`b` 均被移动，不可再用（与 `HashSet` 运算符糖同构）。
- 键哈希范围继承 `HashMap` 既有约束（i64 / String）。

## 验收

- `tests/run-pass/hashmap_ops_symbol.{rl,out}`：覆盖 `|`（冲突键取 b 值 300）/ `&`（取 a 值 30）/ 不相交交集为 0 / 并集大小 4 / `-`（a 独有键 2 个、取 a 值 10）/ `^`（独有键 3 个、a 独有取 10、b 独有取 40），期望 `4 300 10 40 1 30 0 4 2 10 3 10 40`。
- 全量 `rlyeh test tests` 260/260 通过。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-02 | `core.rl` 为 `HashMap<K,V>` 新增 `union`/`intersection` 命名方法 + `BitOr`/`BitAnd` 实现（降级到上述方法，b 胜 / a 值语义）；新增 `tests/run-pass/hashmap_ops_symbol.{rl,out}`（并入 `.gitignore` 白名单）；全量 260/260 通过 |
| 2026-09-02 | 补齐 `HashMap<K,V>` 的 `-`/`^` 运算符糖——新增 `difference`/`symmetric_difference` 命名方法 + `Sub`/`BitXor` 实现（a 独有取 a 值 / b 独有取 b 值语义）；扩展 `tests/run-pass/hashmap_ops_symbol.{rl,out}` 覆盖 `-`/`^`；全量 260/260 通过 |
