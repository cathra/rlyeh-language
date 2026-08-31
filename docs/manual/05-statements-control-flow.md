# 5. 语句与控制流

> 本章速查变量绑定、控制流、`match`、赋值与块表达式。讲解见 [指南 §3.3 控制流](../guide/03-basic-syntax.md)。

---

## 5.1 变量绑定

```rlyeh
let x = 6;                 // 不可变，推断 i64
let mut v = Vec::new();    // 可变绑定（但 Vec 内部可改需 mut 前缀）
let y: f64 = 3.14;         // 显式类型注解
```

| 形式 | 含义 | C 对照 |
|------|------|--------|
| `let x = 6;` | 不可变绑定 | `const int64_t x = 6;`（仅只读语义，但 x 是局部值） |
| `let mut x = 6;` | 可变绑定 | `int64_t x = 6;` |
| `let x: T = ...;` | 显式注解 | 类型声明 |

> **C 对照**：Rlyeh 变量**默认不可变**——这是和 C 最大的不同。C 里变量默认可改，Rlyeh 反过来：要改必须写 `mut`。理由：大多数变量其实不需要改，默认只读让编译器更容易推理安全。

### 块级遮蔽（U1 ✅）

块内 `let x` 覆盖外层同名变量；块内引用指向新绑定；块结束后原变量恢复。函数/闭包体为隔离边界（看不到外层局部变量）；循环体 / `match` 臂可穿透所属函数层。

```rlyeh
let x = 10;
{
    let x = 20;
    println(x);            // 20（遮蔽）
};
println(x);                // 10（恢复）
for i in 0..<3 {
    println(i);            // 0 1 2（for 变量遮蔽外层 i）
}
```

---

## 5.2 控制流

```rlyeh
if cond { } else { }          // if 是表达式（尾值可用）
while cond { }
loop { break; continue; }     // 无限循环，靠 break 退出
for i in 0..<10 { }           // 数值区间（步长固定 1），半开 [0,10)
for j in 1...3 { }            // 双闭区间 [1,3]
for x in vec { }              // 容器迭代（Vec/数组/HashMap）
match value { ... }           // 模式匹配（枚举解构）
```

> **C 对照**：
> - `for i in 0..<10` ≈ `for (int i = 0; i < 10; i++)`。
> - `loop` ≈ `for (;;)`（Rlyeh 没有 `for(;;)`，用 `loop`）。
> - `if` 是**表达式**（有值），可以 `let x = if c { 1 } else { 2 };`。
> - `match` 取代 `switch`，且强制穷尽（见 [§5.2 模式](./03-types.md) 与 [指南 §5.2](../guide/05-aggregates-generics.md)）。

`match` 模式支持：枚举变体 `Enum::Variant(...)`、结构字段 `{ w, h }`、嵌套 `Option<Vec<T>>`。

---

## 5.3 赋值与序列

- 赋值 `=` 为**表达式**（有值，可链式）：`a = b = 5;` 合法（先 `b=5` 再 `a=5`）。
- 块 `{ ... }` 末表达式即块值：`let x = { 1; 2; };` 绑定 `2`（分号结尾的语句丢弃值，无分号的末表达式作为块值）。

```rlyeh
let x = {
    let t = 1;
    t + 1            // 无分号 → 块值 = 2
};
println(x);          // 2
```

> **C 对照**：这类似 C 的逗号表达式 `(t=1, t+1)`，但 Rlyeh 用"语句序列 + 末表达式"实现，可读性更好，且是函数返回的主要方式（见 [教程 §3.4](../tutorial/03-first-program.md)）。

---

## 更多示例

块表达式作为值（末表达式即返回值）：

```rlyeh
let x = {
    let t = 1;
    t + 1            // 无分号 → 块值 = 2
};
println(x);          // 2
```

可运行版本见 [`examples/by-chapter/03-basic-syntax.rl`](../../examples/by-chapter/03-basic-syntax.rl)（变量遮蔽 + 控制流）。

---

[← 上一章：表达式与运算符](./04-expressions-operators.md) | [返回手册目录](./index.md) | [下一章：函数与闭包 →](./06-functions-closures.md)
