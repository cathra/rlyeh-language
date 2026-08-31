# Option\<T\>

表示「可能有值」的代数数据类型。变体：`Some(T)`（有值）/ `None`（无值）。

> **C 程序员对照**：C 里"可能有值"通常用**指针（`NULL` 表示无）**或**特殊哨兵值**（如 `-1`、`0`）表示。问题是：调用方**容易忘记检查** `NULL`/哨兵，于是拿一个无效值继续用，后面才崩溃（或静默错误）。`Option<T>` 把这个"可能没有"**编码进类型**里——你不 `match` 处理 `None` 分支，编译器就不让你拿里面的 `T`。这是"把错误当返回值显式传递"的类型安全版。

### 与 C 习惯的等价映射

| C 写法 | Rlyeh 写法 |
|--------|------------|
| `int* p = NULL;` 表示无 | `Option<int>` 的 `None` |
| `if (p != NULL) use(*p);` | `match o { Some(x) => use(x), None => {} }` |
| 返回 `-1` 表示查找失败 | 返回 `None` |

## 构造

```rlyeh
let o: Option<i64> = Some(42);
let n: Option<i64> = None;
```

- `Some(x: T) -> Option<T>`：包裹一个值。
- `None`（无参变体）：表示缺失。

## 方法

### `unwrap_or(default: T) -> T`
有值返回值，否则返回默认值。
```rlyeh
let o = Some(42);
println(o.unwrap_or(0));     // 42
let n: Option<i64> = None;
println(n.unwrap_or(0));      // 0
```

### `unwrap() -> T`
有值返回值，否则运行时 panic（MVP 直接 abort）。
```rlyeh
let o = Some(7);
println(o.unwrap());          // 7
```

### `map(f: fn(T) -> U) -> Option<U>`
有值时对内部值变换，否则透传 `None`。
```rlyeh
let o = Some(21);
let r = o.map(|x| x * 2);     // Some(42)
```

### `and_then(f: fn(T) -> Option<U>) -> Option<U>`
有值时调用返回 `Option` 的函数（链式）；否则 `None`。
```rlyeh
let o = Some(21);
let r = o.and_then(|x| Some(x + 1));   // Some(22)
```

## 模式匹配

```rlyeh
match o {
    Some(x) => println(x),
    None => println(0),
}
```

## `?` 运算符（K1）

在返回 `Option<T>` 的函数中，`expr?` 解包成功值，失败 `return None`：
```rlyeh
fn chain(a: i64, b: i64) -> Option<i64> {
    let q = try_div(a, b)?;    // 失败则 return None
    Some(q + 1)
}
```

## 完整示例

```rlyeh
fn main() {
    let o = Some(42);
    println(o.unwrap_or(0));
    match o {
        Some(x) => println(x * 2),
        None => println(0),
    }
    let mapped = o.map(|x| x + 1);
    println(mapped.unwrap());
}
```

---

[← 返回标准库详述索引](./index.md)
