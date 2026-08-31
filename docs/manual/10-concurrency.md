# 10. 并发模型

## 10.1 Actor

```rlyeh
actor Counter {
    value: i64 = 0,
    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }
}

let counter = Counter::new();                 // 普通 spawn（无监督）
let result = counter.increment(10).await;     // ask 同步往返
send counter.increment(1);                    // fire-and-forget
let supervised = Counter::new_supervised(0);  // 0=OneForOne 1=AllForOne 2=RestartForOne
```

语义：actor 方法消息经「kind 槽 + 3 个 i64 消息槽」传递，同一 actor 消息按邮箱 FIFO 互斥处理；返回 -1 触发崩溃协议（ask 立即返回 0，supervisor 重启；无监督则 actor 停止）。

## 10.2 交叉编译 / WASM（L4 ✅）

`wasm32-wasip1` 目标下 driver 注入静态 `rlyeh_actor_resolve` 符号表替代 `dlsym` + WASI 单线程同步运行时，ask/send/FIFO/受监督崩溃重启协议与 native 一致。需先 `cargo build --target wasm32-wasip1 -p rlyeh-actor-runtime`。WASI 下网络禁用（L4 ✅）。

## 10.3 异步运行时（S1 / W1–W5）

`Future` / `Poll` / `block_on` / `async fn` 状态机（S1）；`Future::poll` 泛型化 `type Output` + `cx`（W1）；控制流内 await 展开（W2）；`future::join_all` / `timeout` / `sleep`（W4/W3）；`recv_async`/`get_async`/`post_async`（W5）。

> 规划中：await 位于控制流块 / 表达式中间、按引用捕获。

详见 [manual/std/future.md](./std/future.md)。

## 10.4 同步原语（sync）

`Mutex` / `RwLock`（pthread 锁）、`Condvar` / `Barrier` / `Channel`（P1–P3 ✅）、`Thread::start`/`join`/`sleep`/`join_all`/`Builder`（S0/S2/Y8）。详见 [manual/std/sync.md](./std/sync.md)。

---

[← 上一章：内存模型](./09-memory.md) | [返回手册目录](./index.md) | [下一章：标准库参考 →](./11-stdlib.md)
