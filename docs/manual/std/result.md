# Result\<T, E\>

表示「可能失败」的代数数据类型。变体：`Ok(T)`（成功值）/ `Err(E)`（错误值）。

> **C 程序员对照**：C 里"函数可能失败"靠返回 `-1` / `NULL` / 设置 `errno`，调用方**可能忘记检查**（于是用了一个无效返回值，后续崩溃）。`Result<T, E>` 把"可能失败"写进**返回类型**：你不 `match` 处理 `Err` 分支，编译器就**不让**你拿里面的成功值 `T`。配合 `?` 运算符（见下文）还能像异常一样"向上冒泡"错误，但完全基于返回值、没有隐藏的控制流。
>
> **`Option` vs `Result` 怎么选**：只想表达"有没有"用 `Option`（如查找命中与否）；想区分"成功值"和"失败原因"用 `Result`（如网络请求成功返回数据、失败返回错误码）。

### 与 C 习惯的等价映射

| C 写法 | Rlyeh 写法 |
|--------|------------|
| `int v = do(); if (v < 0) handle_err();` | `match do() { Ok(v) => use(v), Err(e) => handle(e) }` |
| 返回 `-1` 表示失败 | `Result::Err(-1)` |

## 构造

```rlyeh
let r: Result<i64, i64> = Result::Ok(10);
let e: Result<i64, i64> = Result::Err(-1);
```

- `Result::Ok(x: T) -> Result<T, E>`：成功包裹值。
- `Result::Err(e: E) -> Result<T, E>`：失败包裹错误。

## 方法

### `unwrap_or(default: T) -> T`
成功返回值，失败返回默认值。
```rlyeh
let r = Result::Err(-1);
println(r.unwrap_or(100));    // 100
```

### `unwrap() -> T`
成功返回值，失败运行时 panic（MVP 直接 abort）。
```rlyeh
let r = Result::Ok(5);
println(r.unwrap());          // 5
```

## 模式匹配

```rlyeh
match r {
    Result::Ok(x) => println(x),
    Result::Err(e) => println(e),
}
```

## `?` 运算符（K1）

在返回 `Result<T, E>` 的函数中，`expr?` 解包成功值，失败 `return Result::Err(e)`：
```rlyeh
fn safe_div(x: i64, y: i64) -> Result<i64, i64> {
    if y == 0 {
        return Result::Err(-1);
    }
    Ok(x / y)                  // 裸无参变体值表达式可用
}
```

## 完整示例

```rlyeh
fn safe_div(x: i64, y: i64) -> Result<i64, i64> {
    if y == 0 { return Result::Err(-1); }
    Result::Ok(x / y)
}

fn main() {
    let r = safe_div(10, 2);
    match r {
        Result::Ok(v) => println(v),
        Result::Err(e) => println(e),
    }
    println(r.unwrap_or(-999));
}
```

---

[← 返回标准库详述索引](./index.md)
