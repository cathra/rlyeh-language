# 8. 泛型与单态化

> 速查泛型语法、protocol、类型联合、显式判别式。讲解见 [指南 §5.3 protocol 与泛型](../guide/05-aggregates-generics.md)。

---

## 8.1 泛型基础

泛型通过**单态化（monomorphization）**实现：每个具体类型实例化生成独立代码（和 C++ 模板同理，零运行时开销，但编译产物更大）。

```rlyeh
struct Wrapper<T> {
    value: T,
}
impl<T> Wrapper<T> {
    fn new(v: T) -> Wrapper<T> { Wrapper { value: v } }
}

let w = Wrapper::new(5);        // T 推断为 i64 → Wrapper<i64>
```

> **C 对照**：C 没有泛型，通常用 `void*`（丢失类型安全）或**宏**（`#define` 生成重复代码）模拟。Rlyeh 泛型 = 类型安全的"模板"，且编译期展开，没有 `void*` 的隐患。

---

## 8.2 protocol 与 impl

```rlyeh
protocol Area { fn area(&self) -> f64; }
impl Shape: Area { fn area(&self) -> f64 { /* ... */ } }
```

- `protocol`：定义一组方法签名（类似 C++ 纯虚类 / Java interface）。
- `impl Type: Protocol`：为某个类型实现该 protocol（早期写法 `impl Protocol for Type` 已从语法中移除）。
- 泛型 impl 单态化后 `Self` 替换为具体类型（`Wrapper<i64>::new(v) -> Self` → `Wrapper<i64>`）。

### 泛型 protocol 约束（bound）

```rlyeh
protocol Convert<T> {
    fn convert(&self) -> T;
}
impl f64: Convert<i64> {
    fn convert(&self) -> i64 { *self as i64 }
}
let n = 3.7.convert();          // n: i64 = 3
```

> **MVP 限制**：
> - `Self` 仅支持**返回位置**——参数位置（关联返回）保持禁止。
> - 含 `Self` 签名的方法不可经 `dyn` 协议调用（见 [§4.6](./04-expressions-operators.md) H4 限制）。
> - `From::from(v) -> Self` / `Into::into() -> Self` / `Deserialize::from_json(s) -> Self` 落地待 std protocol 声明。

---

## 8.3 类型联合 `A | B`（U1 / U2 ✅）

```rlyeh
let x: i64 | String = 5;
match x {
    i64 => println(i64),
    String => println(String.len()),
}
```

- 成员须**两两互不相交**（`i64 | isize`、`&i64 | &mut i64` 报 `UnionMembersNotDisjoint`）。
- `match` 用**类型臂**（`i64 => ...`）解构，payload 绑定到同名变量。
- 未收窄的联合禁止直接运算 / 方法调用（`compatible_with` 单向：成员 → 联合）。
- 优先级：`&T | &mut U` = `(&T) | (&mut U)`。

> **C 对照**：`union` + 手工 tag 字段的自动版。编译器维护 tag 与 payload 一致性，并在 `match` 时强制处理每种类型。

---

## 8.4 枚举显式判别式（U3 ✅）

```rlyeh
enum Code { Ok = 200, NotFound = 404, Error = 500 }
let c = Code::Ok;        // tag = 200
```

判别值即该变体的 tag，构造与 `match` 均复用。等价于 C 的 `enum Code { Ok = 200, ... };`。

---

## 更多示例

泛型 struct + 方法（单态化，零开销）：

```rlyeh
struct Wrapper<T> { value: T }
impl<T> Wrapper<T> {
    fn new(v: T) -> Wrapper<T> { Wrapper { value: v } }
}
let w = Wrapper::new(5);        // 推断为 Wrapper<i64>
```

可运行版本见 [`examples/by-chapter/05-aggregates-generics.rl`](../../examples/by-chapter/05-aggregates-generics.rl)（含 enum / match / protocol）。

---

[← 上一章：模块与可见性](./07-modules.md) | [返回手册目录](./index.md) | [下一章：内存模型 →](./09-memory.md)
