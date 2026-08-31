# Result\<T, E\>

表示「可能失败」的代数数据类型。变体：`Ok(T)`（成功值）/ `Err(E)`（错误值）。

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
