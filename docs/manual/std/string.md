# String

UTF-8 字符串缓冲，堆上存储为 3 槽结构：`data`（指针）/ `len`（字节长度）/ `cap`（容量）。原地修改在容量不足时 O(n) 扩容。

> MVP 底层：`i8` 与 `u8` 共享 64 位整数槽（窄化 reinterpret）。`str` 字面量值与 `String` 自动升级互通：`String` 参数位置的字面量实参（`m.push_str("!")`、`m.contains("z")`、`map.insert("k", 1)`、`f("hi")`）自动升级为 `String`，覆盖实例/静态方法、普通函数、函数指针、泛型调用与 `dyn Trait` 方法。

## 构造

```rlyeh
let s = String::from("hello");          // 从字面量构造
let t = String::new();                   // 空字符串（cap = 0）
let u: String = "world";                 // 字面量赋值（自动升级为 String）
```

- `String::from(s: &str) -> String`：深拷贝字面量内容。
- `String::new() -> String`：空缓冲。

## 成员（字段）

| 成员 | 类型 | 说明 |
|------|------|------|
| `data` | `i8*` | 底层字节缓冲指针（一般不直接访问） |
| `len` | `i64` | 字节长度（字段或 `.len()` 方法均可读） |
| `cap` | `i64` | 容量（字段或 `.cap()` 方法均可读） |

```rlyeh
let s = String::from("hello");
println(s.len);            // 5
println(s.cap);            // >= 5
```

## 方法

### `push_str(sub: &str) -> ()`
原地追加子串（深拷贝），必要时扩容。
```rlyeh
let s = String::from("Hello");
s.push_str(" World");
println(s);                // Hello World
```

### `clone() -> String`
深拷贝，返回独立缓冲。
```rlyeh
let s = String::from("a");
let t = s.clone();
```

### `len() -> i64` / `cap() -> i64`
返回字节长度 / 容量。
```rlyeh
println(String::from("Rlyeh").len());   // 5
```

### 索引 `s[i] -> i8`
按字节（步长 1 字节，ASCII 假设）读取第 i 个字节。
```rlyeh
let s = String::from("abc");
println(s[0]);            // 97 ('a')
```

### 子串 `s[lo..<hi] -> String`
返回全新 `String`（值拷贝）。
```rlyeh
let s = String::from("hello");
let sub = s[1..<4];        // "ell"
```

### 拼接 `+`
`String + String` 深拷贝拼接，返回新 `String`。
```rlyeh
let s = String::from("Hello") + String::from(" World");
println(s);                // Hello World
```

### 比较 `==` / `<` / `>`
按字节内容比较（与 Rust 一致）。
```rlyeh
println(String::from("a") == String::from("a"));   // true
println(String::from("a") < String::from("b"));    // true
```

### `contains(pat: &str) -> bool`
是否包含子串。
```rlyeh
println(String::from("Rlyeh").contains("lye"));    // true
```

### `starts_with(pat: &str) -> bool`
```rlyeh
println(String::from("Hello").starts_with("He"));  // true
```

### `replace(from: &str, to: &str) -> String`
全量替换，返回新 `String`。
```rlyeh
let s = String::from("Hello").replace("Hello", "Hi");
println(s);                // Hi
```

### `to_upper() -> String` / `to_lower() -> String`
大小写转换，返回新 `String`。
```rlyeh
println(String::from("Hello").to_upper());   // HELLO
println(String::from("Hello").to_lower());   // hello
```

### `trim() -> String`
去除首尾空白，返回新 `String`。
```rlyeh
println(String::from("  hi  ").trim());      // hi
```

### `split(pat: &str) -> Vec<String>`
按分隔符切分，返回 `Vec<String>`。
```rlyeh
let parts = String::from("a,b,c").split(",");
println(parts.len());       // 3
```

### `repeat(n: i64) -> String`
重复 n 次。
```rlyeh
println(String::from("ab").repeat(3));       // ababab
```

### `strip_prefix(pat: &str) -> Option<String>` / `strip_suffix(pat: &str) -> Option<String>`
去除前缀/后缀，成功返回剩余部分（`Some`），否则 `None`。
```rlyeh
let s = String::from("HelloWorld");
match s.strip_prefix("Hello") {
    Some(rest) => println(rest),             // World
    None => println(0),
}
```

### `truncate(n: i64) -> ()`
截断到 n 字节（原地）。
```rlyeh
let s = String::from("hello");
s.truncate(3);
println(s);                // hel
```

### `as_str() -> &str`
返回只读借用视图（零拷贝；`data`+`len` 升级为 `&str` 2 槽胖指针）。
```rlyeh
let s = String::from("hi");
let v: &str = s.as_str();
println(v.len());          // 2
```

## 自由函数

### `int_to_string(x: i64) -> String`
整数转字符串。
```rlyeh
let s = int_to_string(42);
println(s);                // 42
```

### `string_to_int(s: String) -> i64`
字符串解析为整数。
```rlyeh
let n = string_to_int(String::from("42"));
println(n);                // 42
```

## 完整示例

```rlyeh
fn main() {
    let mut s = String::from("Hello");
    s.push_str(" Rlyeh");
    let upper = s.to_upper();
    let sub = s[0..<5];
    let parts = s.split(" ");
    println(s.len());
    println(upper);                       // HELLO RLYEH
    println(sub);                         // Hello
    println(parts.len());                 // 2
    println(s.contains("lye"));           // true
    let v: &str = s.as_str();
    println(v.len());
}
```

---

[← 返回标准库详述索引](./index.md)
