# Option\<T\>

表示「可能有值」的代数数据类型。变体：`Some(T)`（有值）/ `None`（无值）。

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
