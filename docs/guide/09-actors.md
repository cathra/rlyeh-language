# 9. Actor 并发模型

Actor 是 Rlyeh 并发的一等公民，对标 Erlang/Akka。

## 9.1 定义与基础用法

```rlyeh
actor Counter {
    value: i64 = 0,

    // 方法返回 -1 会被 runtime 视为崩溃信号（Panic）
    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }
}

let counter = Counter::new();                 // 普通 spawn（无监督）
let result = counter.increment(10).await;     // 阻塞 ask：同步往返
println(result);                              // 10

send counter.increment(1);                    // fire-and-forget（异步）
```

## 9.2 受监督 actor

```rlyeh
// 0=OneForOne 1=AllForOne 2=RestartForOne，崩溃后 runtime 经 __state_new 重建初始状态并重启
let supervised = Counter::new_supervised(0);
```

## 9.3 异步运行时（async/.await）

普通函数 `async fn` / `.await` 已支持（S1c ✅）：

```rlyeh
async fn fetch_data() -> i64 {
    let data = async_read().await;           // 挂起点
    data + 1
}

fn main() {
    let f = fetch_data();                     // 返回 Future
    let result = block_on(f);                 // 轮询驱动（示例伪 API）
    println(result);
}
```

> **规划中**：await 位于控制流块 / 表达式中间、按引用捕获（std-lib §10.3）。`sync` 并发原语（Mutex/RwLock/Condvar/Barrier/Channel，P1–P3 ✅）已实现；`future::join_all` + `timeout` + `TimeoutError`（W4 ✅）与 `future::sleep` / `wait_fd` 事件驱动（W3 ✅）已可用；控制流块内 await 递归展开（W2 ✅）、Future 泛型化（W1 ✅）。

## 9.4 JSON 序列化

```rlyeh
let v: HashMap<i64, i64> = map![1 => 10, 2 => 20];
let s = json::stringify(v);                  // "{\"1\":10,\"2\":20}"
let back = json::parse::<HashMap<i64, i64>>(s);  // 反序列化（类型指引）
```

MVP 限制：`map![...]` 绑定后 K/V 为 `Infer`，须 `let m: HashMap<i64, i64>` 注解（与 `vec![...]` 一致）；`HashMap` parse 的键/值含逗号或冒号时经 `split(",")` / `find(":")` 分段不可靠；嵌套 `HashMap` 值 parse 报 Unsupported（值限标量）。自定义 `Serialize` / `Deserialize` trait 与 `#[derive]` 宏已实现（Q1 ✅，§9.5）；`json::to_writer` / `json::from_reader` 亦可用（Q2 ✅）。

### 9.5 TOML 序列化与 `Serialize` derive（Q1 / Q2 / Q4 ✅）

除 JSON 外，TOML 序列化与 `#[derive(Serialize, Deserialize)]` 自动编解码已支持：

```rlyeh
#[derive(Serialize, Deserialize)]
struct Config { name: String, port: i64 }

fn main() {
    let c = Config { name: String::from("svc"), port: 8080 };
    let s = toml::to_string(c);              // TOML 文本（顶层 key=value）
    let c2 = toml::from_str::<Config>(s);    // 反序列化（turbofish 指定目标类型）
    let j = json::to_writer(c);              // 序列化到写入器（std-lib §9）
}
```

- `toml::to_string(v)` / `toml::from_str::<T>(s)`（Q4 ✅）；`json::to_string`/`json::from_str`/`json::to_writer`/`json::from_reader`（Q2 ✅）为 `stringify`/`parse` 的泛型别名入口。
- `#[derive(Serialize, Deserialize)]`（Q1 ✅）：编译器自动生成 `Serialize`/`Deserialize` 实现，免去手写 trait。

## 9.6 Actor 交叉编译 / WASM 支持（L4 ✅）

`wasm32-wasip1` 目标下 driver 注入静态 `rlyeh_actor_resolve` 符号表替代 `dlsym` + WASI 单线程同步运行时，ask/send/FIFO/受监督崩溃重启协议与 native 一致。需先 `cargo build --target wasm32-wasip1 -p rlyeh-actor-runtime`。

---

[← 上一章：内存管理](./08-memory.md) | [返回指南目录](./index.md) | [下一章：标准库 →](./10-stdlib.md)
