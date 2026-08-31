# json 序列化

JSON 编解码（L2 ✅ 基础；Q2 ✅ 泛型入口）。序列化规则：标量（`i64`/`bool`/`String`/`&str`）按值输出；数组/struct/`Vec`/`HashMap` 递归；`HashMap` 键序确定性（便于可重现输出）。

> **C 程序员对照**：C 标准库**没有 JSON**——要么引入 cJSON/jansson，要么手写字符串拼装与解析（极易出错）。Rlyeh 的 `json::stringify`/`json::from_str::<T>` 让你**类型驱动**地序列化/反序列化：`from_str::<HashMap<String,i64>>` 直接拿到强类型结果，不用 `cJSON_GetObjectItem` 一步步取值。MVP 限制：非法 JSON 直接给默认值（不抛异常），生产环境需自行校验。

## 函数

### `json::stringify(v) -> String` / `json::to_string(v) -> String`
序列化为 JSON 文本（`to_string` 为泛型别名入口）。
```rlyeh
let v: HashMap<String, i64> = map![String::from("a") => 10];
let s = json::stringify(v);              // "{\"a\":10}"
let s2 = json::to_string(v);            // 同上
```

### `json::parse::<T>(s) -> T` / `json::from_str::<T>(s) -> T`
反序列化（类型由 turbofish 指引）。MVP 直接返回 T，非法输入给默认值。
```rlyeh
let back = json::parse::<HashMap<String, i64>>(s);
let back2 = json::from_str::<i64>(String::from("42"));   // 42
```

### `json::to_writer(w, v) -> ()` / `json::from_reader::<T>(r) -> T`
流式读写（Q2 ✅）。`w`/`r` 为 `File` 或类似写入器/读取器。
```rlyeh
let f = File::create(String::from("out.json"));
json::to_writer(f, v);
let f2 = File::open(String::from("out.json"));
let v2 = json::from_reader::<HashMap<String, i64>>(f2);
```

## 支持的类型

| 类型 | 序列化行为 |
|------|------------|
| `i64` / `bool` / `String` / `&str` | 标量值 |
| 数组 / `[T; N]` | JSON 数组 |
| `Vec<T>` | JSON 数组 |
| `HashMap<K, V>` | JSON 对象（键序确定性） |
| struct | JSON 对象（字段名 → 值） |

> MVP 限制：`HashMap` parse 的键/值含逗号或冒号经 `split(",")`/`find(":")` 分段不可靠；嵌套 `HashMap` 值 parse 报 Unsupported（值限标量）。

## `#[derive(Serialize, Deserialize)]`（Q1 ✅）

标注结构体后，编译器自动生成 `Serialize`/`Deserialize` 实现，免去手写 trait：
```rlyeh
#[derive(Serialize, Deserialize)]
struct Config { name: String, port: i64 }

fn main() {
    let c = Config { name: String::from("svc"), port: 8080 };
    let s = json::to_string(c);
    let c2 = json::from_str::<Config>(s);
}
```

## 完整示例

```rlyeh
#[derive(Serialize, Deserialize)]
struct Point { x: i64, y: i64 }

fn main() {
    let p = Point { x: 1, y: 2 };
    let s = json::stringify(p);              // {"x":1,"y":2}
    let p2 = json::from_str::<Point>(s);
    println(json::to_string(p2));
}
```

---

[← 返回标准库详述索引](./index.md)
