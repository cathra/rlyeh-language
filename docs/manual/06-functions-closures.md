# 6. 函数与闭包

> 速查函数定义、闭包四种形态、函数指针、trait 对象。讲解见 [指南 §3.2 函数与闭包](../guide/03-basic-syntax.md)、[§5.3 trait](../guide/05-aggregates-generics.md)。

---

## 6.1 函数定义

```rlyeh
fn add(a: i64, b: i64) -> i64 {
    a + b                        // 末表达式为返回值（隐式 return）
}

fn main() {
    let sum = add(6, 7);         // 13
}
```

- 参数 / 返回须**显式类型注解**（无推断）。
- **末表达式即返回值**（不加 `return`、行尾无分号）；也可用显式 `return expr;`。
- 函数可作值绑定、传递、返回（H1 函数指针）。

> **C 对照**：`fn add(a: i64, b: i64) -> i64` ≈ `int64_t add(int64_t a, int64_t b)`。区别：返回类型在**参数列表之后**用 `->` 标注；函数体末表达式自动返回。

---

## 6.2 闭包

| 形态 | 示例 | 特性 | 状态 |
|------|------|------|------|
| 无捕获 | `\|x, y\| x + y` | desugar 为匿名函数 + fn 指针（零开销） | H2 ✅ |
| 捕获 IIFE | `( \|x\| body )(args)` | 按值捕获外层变量，立即调用 | H3 ✅ |
| 闭包值对象 | `let f = \|x: i64\| ..; f(..)` | 绑定为值，反复调用，按值捕获 | H5 ✅ |
| 返回闭包 | `fn make() -> fn(..) { \|x\| .. }` | 尾闭包按 H2 签名检查 | H5 补全 ✅ |
| 闭包值作 fn 实参 | `apply(f, 41)` | 无捕获闭包值降级为 fn 指针 | H5 补全 ✅ |

### 无捕获闭包（H2）

需 fn 类型上下文（fn 形参实参 / `let f: fn(..) = ...` 注解）驱动参数推断：

```rlyeh
fn apply(f: fn(i64, i64) -> i64, x: i64, y: i64) -> i64 { f(x, y) }
fn main() {
    let r = apply(|a, b| a + b, 10, 20);   // 30
    let inc: fn(i64) -> i64 = |x| x + 1;
    println(inc(41));                       // 42
}
```

### 捕获闭包 IIFE（H3）

```rlyeh
let factor = 3;
let r = (|x| x * factor)(14);     // 42：factor 按值捕获
```

### 闭包值对象（H5）

```rlyeh
let inc = |x: i64| x + 1;
println(inc(41));                  // 42
let base = 40;
let add_base = |x: i64| x + base; // 捕获 base
println(add_base(2));              // 42
```

**参数类型确定规则**：有注解用注解；无注解由**首次调用点实参推断**（延迟固化，半注解 `|x: i64, y|` 亦可用）；从未调用则惰性不检查。

**MVP 约束**：`move` 忽略、按引用捕获规划中；**捕获闭包值不跨函数边界**（作 fn 实参 / 返回值报 Unsupported）；无捕获闭包值可作 fn 实参/返回值（降级为 fn 指针）。

> **C 对照**：闭包 ≈ "函数 + 捕获变量包"——C 里你要手写一个 `struct` 装着数据 + 一个函数指针。Rlyeh 的 H2 无捕获闭包**零运行时开销**（直接是函数指针），H3/H5 按值捕获自动打包，不用你手动管理那个 struct。

---

## 6.3 函数指针（H1）

`fn(T) -> R` 是类型；函数值可绑定、传递、返回、重新绑定、注解：

```rlyeh
fn add(a: i64, b: i64) -> i64 { a + b }
fn main() {
    let f: fn(i64, i64) -> i64 = add;   // 注解绑定
    let g = add;                         // 推断绑定
    println(f(1, 2));                    // 3
    println(g(3, 4));                    // 7
}
```

> 底层：`CallIndirect` → LLVM `i8*` 槽 + 按签名 `bitcast` + 间接 `call`。类似 C 的 `int64_t (*fp)(int64_t, int64_t) = add;`。

---

## 6.4 trait 对象（H4）

`let d: dyn Trait = &obj;` 经 vtable 胖指针间接分派。去虚拟化（2026-08-24 ✅）：绑定变量时记录来源具体类型，后续 `d.method()` 静态分派；`d` 被重新赋值时保守回退 vtable 间接调用。

**MVP 限制**：非泛型 trait/impl、含 `Self` 签名方法不可经 dyn 调用；vtable drop/size/align 槽置 0。

---

## 更多示例

无捕获闭包作函数实参（H2，零开销）：

```rlyeh
fn apply(f: fn(i64, i64) -> i64, x: i64, y: i64) -> i64 { f(x, y) }
fn main() {
    let r = apply(|a, b| a * b, 6, 7);   // 42
    println(r);
}
```

可运行版本见 [`examples/by-chapter/03-basic-syntax.rl`](../../examples/by-chapter/03-basic-syntax.rl)。

---

[← 上一章：语句与控制流](./05-statements-control-flow.md) | [返回手册目录](./index.md) | [下一章：模块与可见性 →](./07-modules.md)
