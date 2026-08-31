# 4. 表达式与运算符

> 核心规则：**一切皆为表达式，块/函数体的末表达式即返回值**（隐式 return）。本章列出全部运算符及语义，配合 [指南 §3 基础语法](../guide/03-basic-syntax.md)、[§4 数学条件](../guide/04-math-conditions.md)。

---

## 4.1 运算符一览

| 类别 | 运算符 | C 对照 |
|------|--------|--------|
| 算术 | `+ - * / %` | 同 C（`/` 对整数取整，无 C 的"向零/向负无穷"平台差异，明确向零截断） |
| 比较 | `== != < <= > >=` | 同 C |
| 逻辑 | `&& || not` | `&&`/`||` 短路；`not` 等同 `!`（一元非） |
| 位运算 | `& | ^ << >> ~` | `&` 与、`|` 或、`^` 异或、`<<`/`>>` 移位、`~` 按位取反（同 C 的 `~`） |
| 类型转换 | `as`（U6 ✅） | 类似 C 的 `(T)x` 但语义固定为数值→数值 |
| 错误传播 | `?`（K1 ✅） | 无 C 对应（类似异常 `throw` 的早返回版） |
| 解引用/引用 | `* & &mut` | `*` 解引用、`&` 取地址，同 C 的 `*`/`&` |
| 成员/调用/索引 | `. () []` | 同 C |

> **C 对照要点**：
> - 整数除法 `/`：Rlyeh 向零截断（与 C 大多数平台一致）。`-7 / 2` = `-3`。
> - `not` 与 `!` 等价：`not x` == `!x`。
> - `&`/`|` 在**类型位置**（如 `&T`、`A | B`）和**表达式位置**（如 `a & b`）含义不同——见 §4.6 与 [§3.8 类型联合](./03-types.md)。

---

## 4.2 比较链（数学式）

```rlyeh
if 0 < x < 10 {}        // 展开为 0 < x && x < 10
if 0 > x > 10 {}        // 展开为 x < 0 || x > 10（反向外链 = 并集）
```

- **正向链**（`a < x < b`、`a <= x <= b`）：区间内（交集 `&&`）。
- **反向链**（`a > x > b`、`a >= x >= b`）：区间外（并集 `||`）。
- 中间表达式只求值一次（不会重复计算）。

> **C 对照（重点坑）**：C 里 `0 < x < 10` 是 ` (0 < x) < 10 ` 即 `(0或1) < 10` 永远为真。Rlyeh **重新定义了该语法**为数学连不等式，符合直觉。这是 Rlyeh 对 C 语义的刻意修正。

---

## 4.3 集合 / 区间判断（`in` / `not in`）

```rlyeh
if x in (1, 3, 5) {}          // x == 1 || x == 3 || x == 5
if x in 0..<10 {}             // 0 <= x < 10（裸范围 = 区间判断，连续）
if x in (0..<10) {}           // 离散集合成员（把 0..9 每个整数当成员展开）
if x not in (0..<10) {}       // !（x in (0..<10)）
if hour in (9am...6pm) {}     // 时间字面量区间
```

- **裸范围**（`x in 0..<10`）= 连续区间判断；**括号包范围**（`x in (0..<10)`）= 离散成员。对浮点只能用裸范围。
- 范围语法：`a..<b`（半开 `[a,b)`）、`a...b`（双闭）、`a<..b`（左开）。见 [指南 §4.5](../guide/04-math-conditions.md)。

---

## 4.4 `?` 错误传播（K1）

`expr?` 在 `Option<T>` / `Result<T, E>` 上下文解包成功值，失败则**从当前函数早返回失败值**：

- `Option<T>` → `match { Some(v) => v, None => return Option::None }`
- `Result<T, E>` → `match { Ok(v) => v, Err(e) => return Result::Err(e) }`

```rlyeh
fn chain(a: i64, b: i64, c: i64) -> Option<i64> {
    let q = try_div(a, b)?;        // 失败则 return None
    let r = try_div(q, c)?;
    Some(r + 1)
}
```

- 支持表达式中间嵌套 `?`（如 `Some(a? + b?)`）。
- `?` 用于非 `Option`/`Result` 类型报 `Unsupported`。

> **C 对照**：类似 C 里你手写的 `if ((q = try_div(a,b)) == NULL) return NULL;`，但 Rlyeh 用类型系统保证你处理了错误，且写法极简洁。

---

## 4.5 `as` 数值转换（U6）

| 转换 | 语义 | 示例 |
|------|------|------|
| f64 ↔ i64 | `fptosi`/`sitofp`（向零截断） | `3.7 as i64` = 3、`(-3.7) as i64` = -3 |
| 整数截断 / 扩展 | `trunc`/`sext`/`zext` | `300 as i8` = 44、`(-1) as u8` = 255 |
| 整 ↔ bool | `icmp ne 0` / `zext` | `5 as bool` = true、`0 as bool` = false |
| 整 ↔ char | `trunc`/`zext`（char 为 32 位码点） | `'a' as i64` = 97、`66 as char` = `'B'` |
| 同类型 | 零指令 | `42 as i64` = 42 |

- 范围：≤64 位整族 + 浮点 + bool + char（32 位码点）。
- `i128`/`u128` 与指针/引用/聚合转换保持擦除（不支持）。
- 数值语义与 Rust `as` 一致（向零截断 / 环绕截断）。

---

## 4.6 `dyn Trait`（H4）

`&T` 强制转换 → vtable + 2 槽胖指针（`{data, vtable}`），方法调用经 vtable 间接分派：

```rlyeh
let d: dyn Shape = &c;     // &Circle → dyn Shape 胖指针
println(d.area());         // 经 vtable 间接调用
```

- **去虚拟化（2026-08-24 ✅）**：`let d: dyn Trait = &obj;` 绑定变量时记录来源具体类型，后续 `d.method()` 静态分派（LLVM 可内联），性能与 Rust 持平；`d` 被重新赋值时保守回退 vtable 间接调用。
- MVP 限制：非泛型 trait/impl、含 `Self` 签名方法不可经 dyn 调用；vtable 的 drop/size/align 槽置 0（显式释放语义与 `Box`/`Rc` 一致）。

---

## 更多示例

比较链与 `?` 错误传播：

```rlyeh
fn try_div(x: i64, y: i64) -> Option<i64> {
    if y == 0 { return None; }
    Some(x / y)
}
fn chain(a: i64, b: i64, c: i64) -> Option<i64> {
    let q = try_div(a, b)?;          // 失败则早返回 None
    Some(q + try_div(c, 2)?)
}
```

可运行版本见 [`examples/by-chapter/04-math-conditions.rl`](../../examples/by-chapter/04-math-conditions.rl)（比较链 / 集合判断）与 [`03-basic-syntax.rl`](../../examples/by-chapter/03-basic-syntax.rl)（函数 / 闭包）。

---

[← 上一章：类型系统](./03-types.md) | [返回手册目录](./index.md) | [下一章：语句与控制流 →](./05-statements-control-flow.md)
