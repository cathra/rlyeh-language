# toml 序列化

TOML 编解码（Q4 ✅）。与 `json` 模块对称：`toml::to_string` / `toml::from_str`，并复用 `#[derive(Serialize, Deserialize)]`（Q1 ✅）。

> **C 程序员对照**：TOML 是"对人友好的配置文件格式"（比 ini 强、比 JSON 更适合手写配置）。C 里解析 TOML 通常要引入 `tomlc99` 之类的库并手写取值。Rlyeh 用 `toml::from_str::<Config>(s)` **一步反序列化成你的 `struct`**——配合 `#[derive(Serialize, Deserialize)]`，配置文件 ↔ 结构体之间零样板代码。和 `json` 模块共用同一套 derive 标注。

## 函数

### `toml::to_string(v) -> String`
序列化为 TOML 文本（顶层为 `key = value` 表）。
```rlyeh
#[derive(Serialize, Deserialize)]
struct Config { name: String, port: i64 }

fn main() {
    let c = Config { name: String::from("svc"), port: 8080 };
    let s = toml::to_string(c);
    // name = "svc"
    // port = 8080
}
```

### `toml::from_str::<T>(s) -> T`
反序列化（类型由 turbofish 指引）。
```rlyeh
let c2 = toml::from_str::<Config>(s);
println(c2.port);            // 8080
```

## 与 json 共用 derive

`#[derive(Serialize, Deserialize)]` 同时生成 TOML 与 JSON 的编解码实现：
```rlyeh
#[derive(Serialize, Deserialize)]
struct Config { name: String, port: i64 }

fn main() {
    let c = Config { name: String::from("svc"), port: 8080 };
    let js = json::to_string(c);          // JSON
    let tm = toml::to_string(c);          // TOML
    let from_toml = toml::from_str::<Config>(tm);
    let from_json = json::from_str::<Config>(js);
}
```

## 完整示例

```rlyeh
#[derive(Serialize, Deserialize)]
struct Server { host: String, port: i64 }

fn main() {
    let srv = Server { host: String::from("127.0.0.1"), port: 5432 };
    let text = toml::to_string(srv);
    let back = toml::from_str::<Server>(text);
    println(back.host);
    println(back.port);
}
```

---

[← 返回标准库详述索引](./index.md)
