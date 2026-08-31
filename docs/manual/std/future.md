# Future / Poll / Context（异步运行时）

异步运行时（S1 ✅ 基础；W1–W5 ✅ 泛型化 / 控制流 / timeout / 事件驱动 / 网络异步）。

> **C 程序员对照**：C **没有 `async`/`await`**——你得手写回调函数（`libuv`/`libevent`）或多线程，回调地狱很难维护。Rlyeh 的 `async fn` + `.await` 相当于 **JS/C#/Rust 的 async-await**：用同步写法表达异步逻辑，**协程化**（在等待 IO 时让出 CPU，不阻塞 OS 线程）。`block_on(f)` 是"驱动这个 future 直到完成"的入口点（类似 Rust 的 `tokio::block_on` 或 C# 的 `.Result`）。

## 核心类型

| 类型 | 说明 |
|------|------|
| `Future` | 异步计算（由 `async fn` desugar 为状态机结构体） |
| `Poll<T>` | 轮询结果：`Pending`（挂起）/ `Ready(T)`（完成） |
| `Context` | 轮询上下文（`cx: &mut Context`），携带 `deadline` 槽（W3）供事件驱动唤醒 |
| `block_on` | 轮询驱动 future 直到完成（示例 API） |

## async fn 与 await

```rlyeh
async fn fetch_data() -> i64 {
    let data = async_read().await;           // 挂起点
    data + 1
}

fn main() {
    let f = fetch_data();                     // 返回 Future
    let result = block_on(f);                 // 轮询驱动
    println(result);
}
```

- `Future` 泛型化（W1 ✅）：`type Output` 关联类型 + `cx: &mut Context` 参数（`fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output>`）。
- 控制流内 await（W2 ✅）：`if`/`while`/`for`/`loop`/`match` 子块内 await 递归展开为扁平段；表达式中间嵌套 await 经临时变量 `__w_N` 提取。
- `block_on` 循环推进 poll，跨 await 变量类型扩展至 `f64`/`bool`/`char`/`String`。

## future 模块助手

### `future::join_all`
并发等待多个 future，返回各自输出组成的向量。
```rlyeh
let results = future::join_all(vec![f1, f2, f3]);   // Vec<F::Output>
```

### `future::timeout`
为 future 附加超时（W4 ✅）。超时触发返回 `TimeoutError`。
```rlyeh
let r = future::timeout(future::sleep(d), Duration::from_secs(1));
```

### `future::sleep`
异步休眠（事件驱动，W3 ✅）。
```rlyeh
future::sleep(Duration::from_millis(100)).await;
```

### `future::wait_fd`
在事件循环上等待文件描述符可读/可写（W3 ✅，事件驱动核心）。

## 网络异步（W5 ✅）

- `recv_async` / `get_async` / `post_async`：actor/HTTP 的异步变体。

## 完整示例

```rlyeh
async fn total() -> i64 {
    let a = async_read_a().await;
    let b = async_read_b().await;
    a + b
}

fn main() {
    let f = total();
    let r = block_on(f);
    // 超时 + 并发：
    let all = future::join_all(vec![total(), total()]);
    let t = future::timeout(total(), Duration::from_secs(2));
}
```

> 规划中：await 位于控制流块 / 表达式中间、按引用捕获。详见 [../10-concurrency.md](../10-concurrency.md)。

---

[← 返回标准库详述索引](./index.md)
