# Vec\<T\>

动态数组（可增长缓冲）。底层与 `String` 类似的 3 槽结构，元素类型为 `T`（`i8`/`u8` 共享 64 位整数槽）。`Vec<u8>` 元素紧凑（步长 1 字节），`Vec<i64>` 等标量步长 8 字节。支持迭代器（`iter`/`iter_mut`，V1）与高阶函数适配器（map/filter/fold/collect，数组+Vec 通用）。

## 构造

```rlyeh
let v: Vec<i64> = Vec::new();              // 空向量
let v2: Vec<i64> = vec![1, 2, 3];          // 宏构造（I3 ✅）
let v3: Vec<i64> = Vec::with_capacity(8);  // 预分配容量
```

- `Vec::new() -> Vec<T>`：空向量（cap = 0）。
- `vec![...]`：块表达式 `Vec::with_capacity(n)` + 逐元素 `push`，空集合 → `new()`，元素支持完整表达式与嵌套宏调用。
- `Vec::with_capacity(n: i64) -> Vec<T>`：预分配 n 容量。

## 方法

### `push(x: T) -> ()`
追加元素到末尾（必要时扩容）。
```rlyeh
let v: Vec<i64> = Vec::new();
v.push(10);
v.push(20);
```

### `len() -> i64`
元素数量。
```rlyeh
println(v.len());          // 2
```

### 索引 `v[i] -> T`
读取第 i 个元素（越界编译期可查）；别名共享，互相可见。
```rlyeh
let x = v[0];              // 10
```

### 动态切片 `v[lo..<hi] -> Vec<T>`
返回全新缓冲（按值拷贝）；越界 clamp 到 `[0, len]`，`start>=end` 返回空。
```rlyeh
let part = v[1..<3];       // [20, ...]（值拷贝）
```

### `first() -> Option<T>` / `last() -> Option<T>`
首/尾元素（空时 `None`）。
```rlyeh
let f = v.first();         // Some(10)
let l = v.last();          // Some(20)
```

### `pop() -> Option<T>`
移除并返回末尾元素。
```rlyeh
let x = v.pop();           // Some(20)
```

### `get(i: i64) -> Option<T>`
安全索引（越界返回 `None`）。
```rlyeh
let x = v.get(2);          // None（越界）
```

### `iter() -> Iter<T>` / `iter_mut() -> IterMut<T>`
瘦指针迭代器（V1）；`for x in v.iter()` 或 `for x in v` 遍历。
```rlyeh
for x in v.iter() {
    println(x);
}
```

### `reverse() -> ()` / `swap(i: i64, j: i64) -> ()`
原地反转 / 交换两元素。
```rlyeh
v.reverse();
v.swap(0, 1);
```

### `sort() -> ()`
原地升序排序（标量元素）。
```rlyeh
v.sort();
```

### `is_empty() -> bool`
是否为空。
```rlyeh
println(v.is_empty());
```

### `binary_search(x: &T) -> Option<i64>`
二分查找（返回命中索引，`None` 未命中）。仅对有序向量有意义。
```rlyeh
let idx = v.binary_search(&30);
```

### `as_slice() -> &[T]` / `as_mut_slice() -> &mut [T]`
零拷贝切片引用视图（S1/S2/S3）。
```rlyeh
let bp: &[i64] = v.as_slice();
let bm: &mut [i64] = v.as_mut_slice();
println(bp.len());
```

## 高阶函数（适配器，数组+Vec 通用）

返回新 `Vec<T>`，参数须为**无捕获闭包**（H2）：
```rlyeh
let d = v.map(|x| x * 2);              // Vec: [20, 40]
let e = v.filter(|x| x > 10);          // Vec，保留满足条件者
let t = v.fold(0, |acc, x| acc + x);   // 归约返回 acc（i64）
let c = v.collect();                   // 原样收集为 Vec
let tk = v.take(3);                    // 取前 3 个
let sk = v.skip(2);                    // 跳过前 2 个
```

## 完整示例

```rlyeh
fn main() {
    let v: Vec<i64> = vec![3, 1, 2];
    v.push(4);
    println(v.len());                  // 4
    println(v.first());                // Some(3)
    v.sort();
    let doubled = v.map(|x| x * 2);
    let sum = v.fold(0, |acc, x| acc + x);
    let slice: &[i64] = v.as_slice();
    println(slice.len());              // 4
    println(sum);
    println(doubled);
}
```

---

[← 返回标准库详述索引](./index.md)
