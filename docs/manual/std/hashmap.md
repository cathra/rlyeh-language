# HashMap\<K, V\>

哈希表（键值映射）。MVP 中键/值类型须为可哈希标量/`String`；`String` 键按内容哈希。构造后裸名即可调用方法（`import` 已在根模块重导出）。

## 构造

```rlyeh
let m: HashMap<String, i64> = HashMap::new();
let m2: HashMap<String, i64> = HashMap::with_capacity(8);
let m3: HashMap<i64, i64> = map![1 => 10, 2 => 20];   // 宏构造（I3 ✅）
```

- `HashMap::new() -> HashMap<K, V>`：空表。
- `HashMap::with_capacity(n: i64) -> HashMap<K, V>`：预分配桶。
- `map![k => v, ...]`：块表达式 `with_capacity` + 逐元素 `insert`，空集合 → `new()`。

> MVP 限制：`map![...]` 绑定后 K/V 为 `Infer`，须显式类型注解（如 `let m: HashMap<i64, i64> = map![...]`）；键/值含逗号或冒号经 `split(",")`/`find(":")` 分段不可靠；嵌套 `HashMap` 值 parse 报 Unsupported（值限标量）。

## 方法

### `insert(key: K, value: V) -> ()`
插入/覆盖键值对。
```rlyeh
let m: HashMap<String, i64> = HashMap::new();
m.insert(String::from("a"), 1);
```

### `get(key: K) -> Option<V>`
查询键，存在返回 `Some(v)`，否则 `None`。
```rlyeh
let v = m.get(String::from("a"));     // Option<i64>
```

### `remove(key: K) -> ()`
删除键值对（存在与否均安全）。
```rlyeh
m.remove(String::from("a"));
```

### `contains_key(key: K) -> bool`
是否存在该键。
```rlyeh
println(m.contains_key(String::from("a")));
```

### `iter_pairs() -> Vec<(K, V)>`
返回键值对数组（便于遍历）。
```rlyeh
let pairs = m.iter_pairs();
for p in pairs {
    // p.0 = key, p.1 = value（元组访问）
}
```

### `with_capacity(n: i64) -> HashMap<K, V>`
（构造器，见上）预分配。

## 完整示例

```rlyeh
fn main() {
    let m: HashMap<String, i64> = HashMap::new();
    m.insert(String::from("a"), 1);
    m.insert(String::from("b"), 2);
    println(m.contains_key(String::from("a")));    // true
    match m.get(String::from("a")) {
        Some(v) => println(v),                      // 1
        None => println(0),
    }
    let pairs = m.iter_pairs();
    println(pairs.len());                           // 2
    m.remove(String::from("a"));
    println(m.contains_key(String::from("a")));    // false
}
```

> 对 `HashMap` 的 JSON 反序列化：`json::parse::<HashMap<String, i64>>(s)`（键/值限标量，嵌套值报 Unsupported）。

---

[← 返回标准库详述索引](./index.md)
