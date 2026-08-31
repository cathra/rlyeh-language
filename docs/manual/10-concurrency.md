# 10. 并发模型

> 速查 Actor、异步运行时、同步原语。深入讲解 + 示例见 [指南 §9 Actor 并发](../guide/09-actors.md)。

---

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
let result = counter.increment(10).await;     // ask：发消息同步等回复
send counter.increment(1);                    // send：fire-and-forget（异步）
let supervised = Counter::new_supervised(0);  // 0=OneForOne 1=AllForOne 2=RestartForOne
```

**语义**：

- actor 方法消息经「kind 槽 + 3 个 i64 消息槽」传递。
- 同一 actor 消息按**邮箱 FIFO 互斥处理**（内部串行，无数据竞争）。
- 方法返回 `-1` 触发**崩溃协议**：ask 立即返回 0（不卡死）；supervisor 经 `__state_new` 重建初始状态并重启；无监督则 actor 停止。

> **C 对照**：Actor ≈ "独立线程 + 状态机 + 消息队列"，但 Rlyeh 把"队列、互斥、单线程处理、崩溃重启"全做掉了。你只写 `actor` 业务逻辑，不用手写 `pthread` + `mutex` + watchdog。

---

## 10.2 交叉编译 / WASM（L4 ✅）

`wasm32-wasip1` 目标下 driver 注入静态 `rlyeh_actor_resolve` 符号表替代 `dlsym` + WASI 单线程同步运行时，ask/send/FIFO/受监督崩溃重启协议与 native 一致。

```bash
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1 -p rlyeh-actor-runtime
```

> WASI 下**网络禁用**（L4 ✅）。

---

## 10.3 异步运行时（S1 / W1–W5）

- `Future` / `Poll` / `block_on` / `async fn` 状态机（S1）。
- `Future::poll` 泛型化 `type Output` + `cx: &mut Context`（W1）。
- 控制流块内 await 递归展开（W2）。
- `future::join_all` / `timeout` / `sleep` / `wait_fd` 事件驱动（W4 / W3）。
- `recv_async` / `get_async` / `post_async`（W5）。

```rlyeh
async fn fetch_data() -> i64 {
    let data = async_read().await;     // 挂起点
    data + 1
}
fn main() {
    let f = fetch_data();
    let result = block_on(f);          // 轮询驱动
    println(result);
}
```

> **规划中**：await 位于控制流块 / 表达式中间、按引用捕获。详见 [std/future.md](./std/future.md)。

---

## 10.4 同步原语（sync，P1–P3 ✅ / S0/S2/Y8）

| 原语 | 说明 | C 对照 |
|------|------|--------|
| `Mutex` / `RwLock` | 互斥锁 / 读写锁 | pthread mutex / rwlock |
| `Condvar` | 条件变量 | pthread cond |
| `Barrier` | 栅栏（等一组线程到齐） | pthread barrier |
| `Channel` | 线程间通道（消息传递） | 无直接 C 对应（类似 Go channel） |
| `Thread` | `start`/`join`/`sleep`/`join_all`/`Builder` | pthread_create/join |

> 这些和 C 的 pthread 对应物语义类似，但用 Rlyeh 类型系统包装得更安全（离开作用域自动解锁）。详见 [std/sync.md](./std/sync.md)。

---

## 更多示例

Actor 的 ask 与 send：

```rlyeh
actor Counter {
    value: i64 = 0,
    pub fn increment(amount: i64) -> i64 { self.value += amount; self.value }
}
fn main() {
    let c = Counter::new_supervised(0);
    println(c.increment(5).await);   // 5：ask 同步往返
    send c.increment(1);             // fire-and-forget（发完即走）
}
```

可运行版本见 [`examples/by-chapter/09-actors.rl`](../../examples/by-chapter/09-actors.rl)。

---

[← 上一章：内存模型](./09-memory.md) | [返回手册目录](./index.md) | [下一章：标准库参考 →](./11-stdlib.md)
