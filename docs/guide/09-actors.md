# 9. Actor 并发模型

> 本章目标：理解 Rlyeh 的**并发一等公民——Actor**。和 C 的"线程 + 锁 + 共享内存"路线不同，Rlyeh 的 Actor 之间**只通过消息通信、不共享内存**，从语言层面消灭数据竞争。读完你能定义 actor、发消息、处理崩溃重启。

---

## 9.1 为什么是 Actor，而不是锁

C 写并发：多个线程共享同一块内存，靠 `mutex`/`pthread` 保护：

```c
pthread_mutex_t m;
int counter = 0;
// 线程 A 和线程 B 都要 counter++，必须都先 lock(m)
// 忘了 lock → 数据竞争（未定义行为）；lock 顺序错 → 死锁
```

问题：**共享内存 + 锁** 极易出错，且错误（死锁、竞态）往往运行时才暴露、难以复现。

Rlyeh 的 Actor 模型换了一条路：

> **Actor = 一个独立的状态机 + 一个私有的邮箱（消息队列）**。每个 actor 内部**串行**处理自己的消息（同一时刻只看一封信），状态完全私有、**不和其他 actor 共享内存**。actor 之间唯一的交互就是"发消息"。

> **C 程序员的视角**：Actor ≈ "一个独立线程跑一个状态机，外面通过消息队列和它通信"。但 Rlyeh 把"队列、互斥、单线程处理、崩溃重启"全做掉了，你只写业务逻辑。

---

## 9.2 定义与基础用法

```rlyeh
actor Counter {
    value: i64 = 0,             // actor 的私有状态（字段）

    // 方法返回 -1 会被 runtime 视为崩溃信号（Panic）
    pub fn increment(amount: i64) -> i64 {
        self.value += amount;   // self 指向 actor 自身状态
        self.value
    }
}

let counter = Counter::new();                 // 普通 spawn（无监督）
let result = counter.increment(10).await;     // ask：发消息并同步等回复
println(result);                              // 10

send counter.increment(1);                    // send：fire-and-forget（发完就走，不等）
```

要点：

- `actor Name { 字段, 方法 }`：声明一个 actor，字段是它的私有状态。
- `self`：指向 actor 自己的状态（类似 C++ 的 `this`、Python 的 `self`）。
- `Counter::new()`：创建并启动一个 actor 实例（spawn）。
- `counter.method(args).await`：**ask 模式**——发消息、阻塞等待回复（同步往返）。
- `send counter.method(args)`：**send 模式**（fire-and-forget）——发完即返回，不等结果。

> **消息如何传递**：`increment(10)` 的调用参数被打包进"消息"（经「kind 槽 + 3 个 i64 消息槽」传递），投递到 actor 的邮箱。actor 按 **FIFO** 顺序、互斥地处理每封消息——所以即使多个地方同时 `increment`，`value` 也不会被并发改坏。

---

## 9.3 受监督 actor（崩溃自动重启）

真实系统里 actor 可能崩溃（方法返回 `-1` 即被视为崩溃信号）。Rlyeh 提供**监督（supervision）**：崩溃后 runtime 自动重建初始状态并重启，无需你手写重启逻辑。

```rlyeh
// 参数 0/1/2 选择监督策略：
//   0 = OneForOne   只重启出事的 actor
//   1 = AllForOne   兄弟全重启
//   2 = RestartForOne
let supervised = Counter::new_supervised(0);
```

- **崩溃信号**：actor 方法返回 `-1`，runtime 视为 Panic。此时：
  - 若被监督：runtime 经 `__state_new` 用初始状态重建并重启（ask 调用立即返回 0）。
  - 若无监督：actor 停止（不再处理消息）。
- **ask 崩溃**：调用方 `.await` 立即拿到 `0`（而不是永远卡住）。
- **send 崩溃**：消息丢弃，监督树负责重启。

> **C 程序员的视角**：这等价于 C 里"watchdog 进程/线程监控 worker，worker 挂了就重启"。Rlyeh 把它做成语言内建的一等机制——你写 `new_supervised(strategy)` 即可，不用自己写监控循环。

---

## 9.4 异步运行时 `async` / `.await`（S1c ✅）

除了 actor，普通函数也支持异步（适合 I/O 等待场景）：

```rlyeh
async fn fetch_data() -> i64 {
    let data = async_read().await;           // 挂起点：等待时让出，不阻塞线程
    data + 1
}

fn main() {
    let f = fetch_data();                     // 返回 Future（尚未真正执行完）
    let result = block_on(f);                 // 轮询驱动 Future 直到完成
    println(result);
}
```

- `async fn`：声明异步函数，调用返回 `Future`（一个"将来会有结果"的占位）。
- `.await`：暂停当前异步任务、让出执行权，等结果就绪再继续。
- `block_on(f)`：在普通函数里"驱动"一个 Future 跑完（拿到最终结果）。

> **MVP 限制（规划中）**：参数限 `i64`、返回限 `i64`/`()`；`await` 位于控制流块内/表达式中间、按引用捕获等进阶用法尚在完善。但 `Future` 已泛型化（W1）、控制流块内 await 已递归展开（W2）、`future::join_all` / `timeout` / `sleep` / `wait_fd` 事件驱动已可用（W3/W4）。

---

## 9.5 同步并发原语（P1–P3 ✅）

如果确实需要在普通代码里做线程同步，标准库已提供：

- `Mutex` / `RwLock`：互斥锁 / 读写锁
- `Condvar`：条件变量
- `Barrier`：栅栏（等待一组线程到齐）
- `Channel`：线程间通道（也是"消息传递"，比共享内存安全）

> 这些和 C 的 pthread 对应物语义类似，但用 Rlyeh 的类型系统包装得更安全（离开作用域自动解锁）。

---

## 9.6 JSON / TOML 序列化

actor 常需要在网络上收发结构化数据，标准库提供序列化支持：

```rlyeh
let v: HashMap<i64, i64> = map![1 => 10, 2 => 20];
let s = json::stringify(v);                  // 序列化为 JSON 字符串
let back = json::parse::<HashMap<i64, i64>>(s);  // 反序列化（类型由 turbofish 指引）
```

`#[derive(Serialize, Deserialize)]` 自动为你的 struct 生成编解码（Q1 ✅）：

```rlyeh
#[derive(Serialize, Deserialize)]
struct Config { name: String, port: i64 }

fn main() {
    let c = Config { name: String::from("svc"), port: 8080 };
    let s = toml::to_string(c);              // TOML 文本
    let c2 = toml::from_str::<Config>(s);    // 反序列化回来
}
```

> **MVP 限制**：`map![...]` / `vec![...]` 绑定后 K/V 类型为 `Infer`，需显式注解（如 `let m: HashMap<i64, i64>`）；嵌套 `HashMap` 值的 parse 暂不支持（值限标量）；自定义 `Serialize`/`Deserialize` protocol 与 `#[derive]` 已实现。

---

## 9.7 Actor 交叉编译 / WASM 支持（L4 ✅）

actor 程序可以编译到 WebAssembly（`wasm32-wasip1`）目标，ask/send/FIFO/监督重启协议与 native 完全一致：

```bash
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1 -p rlyeh-actor-runtime
```

> WASM 下单线程同步运行时 + 静态 `rlyeh_actor_resolve` 符号表替代 `dlsym`，语义与 native 一致。

---

## 9.8 综合示例：受监督计数器服务

```rlyeh
actor Counter {
    value: i64 = 0,
    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }
    pub fn crash_probe() -> i64 {
        -1     // 返回 -1 → 触发崩溃，监督树会重启本 actor
    }
}

fn main() {
    let c = Counter::new_supervised(0);   // OneForOne 监督
    println(c.increment(5).await);        // 5
    // 若调用 c.crash_probe().await，返回 0（崩溃被吞，actor 重启）
    println(c.increment(3).await);        // 3（重启后 value 归零再 +3）
}
```

---

## 练习

1. 运行 [`examples/by-chapter/09-actors.rl`](../../examples/by-chapter/09-actors.rl)，观察 `increment(10).await` 的同步往返结果（ask 模式）。
2. 给 `Counter` 加一个返回 `-1` 的方法，调用它看受监督 actor 如何重建初始状态并重启（`value` 归零）。
3. 用 `send` 替代 `.await` 做 fire-and-forget 调用，理解 ask（等回复）与 send（不等）的语义差别。

---

[← 上一章：内存管理](./08-memory.md) | [返回指南目录](./index.md) | [下一章：标准库 →](./10-stdlib.md)
