# V5d 运算符重载（语言机制）+ 集合运算符糖（2026-09-02）

> **归属**：阶段 V（集合与迭代器完整化）· V5 新集合收尾项
> **状态**：✅ 已完成
> **关联**：[`v5-new-collections.md`](./v5-new-collections.md)（V5 ✅）· [`v5b-set-operations.md`](./v5b-set-operations.md)（V5b ✅）· [`v5c-hashset-iter.md`](./v5c-hashset-iter.md)（V5c ✅）· std-lib §3.4

## 背景

V5b 落地的 `HashSet` 集合代数仅提供命名方法（`union`/`intersection`/`difference`/`symmetric_difference`），其 `std-lib.md §3.4` 将「运算符糖 `|`/`&`/`-`/`^`/`<`/`>`」列为需语言层运算符重载支持的可选增强。P009 设计稿（`docs/design/prompts/P009_标准库核心模块.md:1135`）已给出权威设计：`trait Add { type Output; fn add(self, other) -> Self::Output; }` 形式的运算符 trait。

本项交付**通用运算符重载语言机制**（不止服务集合），并以 `HashSet` 为首个应用给出集合运算符糖。

## 方案

### 1. 运算符 trait（core.rl，对标 Rust std::ops + P009）

```rlyeh
trait Add  { type Output; fn add(self, other: Self) -> Self::Output; }
trait Sub  { type Output; fn sub(self, other: Self) -> Self::Output; }
trait Mul  { type Output; fn mul(self, other: Self) -> Self::Output; }
trait Div  { type Output; fn div(self, other: Self) -> Self::Output; }
trait Rem  { type Output; fn rem(self, other: Self) -> Self::Output; }
trait BitAnd { type Output; fn bitand(self, other: Self) -> Self::Output; }
trait BitOr  { type Output; fn bitor(self, other: Self) -> Self::Output; }
trait BitXor { type Output; fn bitxor(self, other: Self) -> Self::Output; }
trait Shl  { type Output; fn shl(self, other: Self) -> Self::Output; }
trait Shr  { type Output; fn shr(self, other: Self) -> Self::Output; }
```
关联类型 `type Output` 复用 `Future::Output`（W1 ✅）已落地的机制；`Self` 接收者/返回与 `Iterator::chain(other: Self)` 同构。

### 2. typecheck 回退（check_expr/mod.rs · Binary 派发尾部）

`check_binary` 仅处理内建语义（算术→数值、位运算→整型、逻辑→bool）。在内建路径 `Err` 时，若运算符可重载，降级为方法调用：

```rust
Err(builtin_err) => {
    if let Some(method) = overload_method(*op) {       // Add→"add" … BitOr→"bitor" … Mod→"rem"
        let args = [right.clone()];
        if let Ok((hir, ty)) = check_method_call(ctx, left, method, &args, None, span) {
            return Ok((hir, ty));                       // 复用既有 method-call 全链路
        }
    }
    Err(builtin_err)                                   // 重载失败维持内建错误，保留既有诊断
}
```

`overload_method(BinaryOp)` 映射（`BinaryOp::Mod` 取余变体→`"rem"`；`&&`/`||` 短路不可重载→`None`）。**codegen 零改动**——`a OP b` 在 HIR 层即成为 `a.<op>(b)` 方法调用，沿既有 method-call 通路生成 LLVM。

### 3. HashSet 运算符糖（core.rl）

```rlyeh
impl<T> BitOr for HashSet<T> {
    type Output = HashSet<T>;
    fn bitor(self, other: HashSet<T>) -> HashSet<T> { self.union(&other) }
}
// BitAnd→intersection / Sub→difference / BitXor→symmetric_difference 同理
```
`self` 按值消费（与 Rust std::ops 一致），内部集合方法经 `&self` 自动借用读原集、返回全新集合。

### 4. 范围与限制

- **仅 `BinaryOp` 可重载**：算术 `+ - * / %`、位运算 `& | ^ << >>`。
- **逻辑 `&&`/`||`**：短路语义，不可重载（不进入回退）。
- **比较链 `<`/`>`/`<=`/`>=`/`==`/`!=`**：走 `CompareOp` 独立路径（支持比较链 `0 < x < 10`）。其中 `==`/`!=` 早已经 `PartialEq`（`a.eq(&b)`）重载；`Lt`/`Le`/`Gt`/`Ge` 的集合子集/超集语义已由 **V5d+ 补遗（[`v5d1-comparison-overload`](./v5d1-comparison-overload.md)，2026-09-02）** 经 `PartialOrd` trait 重载交付（`A < B`=真子集 … `A >= B`=超集），其余非集合类型仍报 `MissingPartialOrd`。
- impl 方法签名建议用具体类型（如 `HashSet<T>`）而非 `Self` 作参数（见下「踩坑」）。

## 踩坑记录

1. **`union` 等集合方法的 `other` 为 `&HashSet<T>` 引用**——初版 `self.union(other)` 报 `expects &HashSet<i64>, found HashSet<i64>`；改为 `self.union(&other)` 修正。
2. **`Self` 作 impl 方法参数类型的解析路径未经充分验证**——`Iterator::chain(other: Self)` 虽存在但未见实际调用；为使运算符 impl 稳健，采用具体 `HashSet<T>`（与 core.rl 其它泛型 impl 风格一致）而非 `Self` 作参数/返回。trait 声明本身仍用 `Self`（与 `Future` 一致）。
3. **调试期回退曾透出 `check_method_call` 真实错误**便于定位；最终恢复为透出 `builtin_err`，以保留存量代码在「无对应 trait impl」时的既有诊断（如 `setA * setB` 仍报内建数值错误而非 `mul not found`）。

## 验收

- `tests/run-pass/hashset_ops_symbol.{rl,out}`：
  - `a1 | b1`→并集规模 4、`a2 & b2`→交集 1、`a3 - b3`→差集 2、`a4 ^ b4`→对称差 3；
  - 自定义 `struct Point { x; y }` + `impl Add for Point` 后 `Point{..} + Point{..}`→分量 11/22（验证通用算术重载，非集合专属）。
- 全量 `rlyeh test tests` 258/258 通过。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-02 | 实现运算符重载语言机制：`check_expr/binary.rs` 新增 `overload_method`；`check_expr/mod.rs` Binary 派发尾部加内建失败→方法调用回退（codegen 零改动）；`core.rl` 新增 10 个运算符 trait（`type Output`）+ `HashSet` 的 `BitOr`/`BitAnd`/`Sub`/`BitXor` 实现（降级到 V5b 集合方法）；新增 `tests/run-pass/hashset_ops_symbol.{rl,out}`（含自定义类型 `+` 算术重载）；全量 258/258 通过 |
