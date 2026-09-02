# V5d+ 比较链运算符重载（`<`/`<=`/`>`/`>=` 子集/超集糖）（2026-09-02）

> **归属**：阶段 V（集合与迭代器完整化）· V5d 运算符重载的补遗项
> **状态**：✅ 已完成
> **关联**：[`v5d-operator-overload.md`](./v5d-operator-overload.md)（V5d ✅，BinaryOp 重载）· std-lib §3.4

## 背景

V5d 交付了 `BinaryOp`（`+ - * / % & | ^ << >>`）的通用运算符重载，并落地 `HashSet` 的 `| & - ^` 集合代数糖。但其「范围与限制」明确：比较链 `CompareOp`（`0 < x < 10`）走独立路径（`comparison.rs`），`<`/`>`（子集/超集）未重载，集合关系仍以命名方法 `is_subset`/`is_superset` 表达。

本项补齐该缺口：集合关系运算符 `<`/`<=`/`>`/`>=` 经**比较链运算符重载**降级为 `PartialOrd` trait 方法，与 Python `set` 语义一致。

## 方案

### 1. `PartialOrd` trait（core.rl，对齐既有 `PartialEq`）

```rlyeh
trait PartialOrd {
    fn lt(&self, other: &Self) -> bool;
    fn le(&self, other: &Self) -> bool;
    fn gt(&self, other: &Self) -> bool;
    fn ge(&self, other: &Self) -> bool;
}
```

方法**按引用**（`&self`/`&other`），与 `PartialEq::eq(&self, other: &Self)` 及 `HashSet` 关系方法（`is_subset(&self, other: &HashSet<T>) -> bool`）一致。按引用是为避免比较链 `a < b < c` 复用操作数 `b`（既作实参又作接收者）时的二次 move——`a.lt(&b) && b.lt(&c)` 全程借用，无所有权转移。

### 2. `comparison.rs` 重载回退

`check_comparison_chain` 两类分支均注入回退：

- **单比较**（`operators.len() == 1`）：在既有字符串字典序 / `PartialEq` 处理之后，对 `Lt`/`Le`/`Gt`/`Ge` 且非数值/字符/字符串、且 `has_partial_ord` 成立的类型，调用 `try_ordering_overload` 经 `infer_expr` 构造 `left.<lt|le|gt|ge>(&right)` 方法调用并返回 `bool` HIR；否则退回内建 `check_comparison` + `compare_hir`。
- **比较链**：新增 `check_build_pair` 逐对生成比较 HIR（先尝试 `try_ordering_overload`，失败则 `check_comparison` + `compare_hir`），`expand_forward`/`expand_backward` 复用预生成的逐对 HIR。反向链 `a > b > c` 展开为 `(a > b) || (b > c)`（与 `(b < a) || (b > c)` 等价，因 `a > b` ≡ `b < a`），既覆盖内建也覆盖重载。

`try_ordering_overload` 复用既有 method-call 全链路（`infer_expr` + `MethodCall` AST，与 `PartialEq` 的 `a.eq(&b)` desugar 同构），**codegen 零改动**。

### 3. HashSet 子集/超集糖（core.rl）

```rlyeh
impl<T> PartialOrd for HashSet<T> {
    fn lt(&self, other: &HashSet<T>) -> bool { self.is_proper_subset(other) }
    fn le(&self, other: &HashSet<T>) -> bool { self.is_subset(other) }
    fn gt(&self, other: &HashSet<T>) -> bool { self.is_proper_superset(other) }
    fn ge(&self, other: &HashSet<T>) -> bool { self.is_superset(other) }
}
```

| 运算符 | 语义 | 命名方法 |
|--------|------|----------|
| `A < B` | 真子集 `A ⊂ B` | `is_proper_subset` |
| `A <= B` | 子集 `A ⊆ B` | `is_subset` |
| `A > B` | 真超集 `A ⊃ B` | `is_proper_superset` |
| `A >= B` | 超集 `A ⊇ B` | `is_superset` |

### 4. 门控与诊断

`has_partial_ord(ty)` 经 `ctx.find_impl_candidates(ty, "lt")` 判定（内含 `type_matches`，可处理泛型 impl 实参反推），仅对**真正实现 `PartialOrd`** 的类型尝试重载。因此对无该 trait 的类型（如自定义结构体）`<` 仍精确报 `MissingPartialOrd`（"type `S` does not support ordering"），不产生 "lt not found" 误报。

### 5. 范围与限制

- 仅 `Lt`/`Le`/`Gt`/`Ge` 参与重载；`Eq`/`Ne` 仍走既有 `PartialEq`（`a.eq(&b)`）路径，未改动。
- 比较链方向约束不变：正向全 `<`/`<=`、反向全 `>`/`>=`；混合方向仍报错。反向链仍仅支持 3 元素。
- `==`/`!=` 的集合相等性未新增运算符糖（沿用 `PartialEq` 既有机制）。

## 验收

- `tests/run-pass/hashset_cmp_symbol.{rl,out}`：覆盖 `A < B`/`A <= B`/`A < A`/`A <= A`/`B > A`/`B >= A`/`A < C`/`C > A`（不相交）/ 正向链 `a < b < d` / 反向链 `d > b > a`，期望 `true true false true true true false false true true`）。
- 全量 `rlyeh test tests` 259/259 通过。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-02 | 实现比较链运算符重载：`comparison.rs` 新增 `has_partial_ord`/`try_ordering_overload`/`check_build_pair`，单比较与比较链分支注入 `lt`/`le`/`gt`/`ge` 重载回退（`code_build_pair` 复用预生成 HIR，重写 `expand_forward`/`expand_backward` 移除不再使用的 `reverse_op`）；`core.rl` 新增 `PartialOrd` trait + `HashSet<T>` 的 `PartialOrd` 实现（降级到 V5b 关系方法）；新增 `tests/run-pass/hashset_cmp_symbol.{rl,out}`（含正向/反向比较链）；全量 259/259 通过 |
