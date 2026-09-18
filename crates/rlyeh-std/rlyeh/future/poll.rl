// future/poll.rl：轮询基础类型（`Poll` / `Context`）——2026-09-18 由 future/module.rl 拆出。
//
// 归属子模块 `future::poll`。对外 `future::Poll` / `future::Context` 由
// future/module.rl 的 `pub import` 保持；std 内 net/http、sync 与 async desugar
// 经标准库根的 `pub import` 以裸名 `Poll` / `Context` 引用。

// S1a：轮询结果（泛型枚举，与 Option 同构）。
enum Poll<T> {
    Ready(T),
    Pending,
}

// W1/W3：Context（poll 上下文）。W1 ✅ 保留签名参数位；W3（2026-08-25）升级携带
// 唤醒请求槽——future 在 `Pending` 时可写入「何时/何事件应被重新 poll」，供事件驱动
// executor（`block_on` / `timeout`）据此休眠或等事件再轮询（替代忙等）：
// - `deadline`：定时器唤醒截止（单调时钟微秒，`__rlyeh_clock_monotonic`）；
// - `fd` / `interest`：fd 事件唤醒（W3 第二步）——`fd` 为要监听的 fd（0 = 无），
//   `interest` 为关注方向掩码（1 = POLLIN 读 / 4 = POLLOUT 写）。
// 全 0 表示未请求（executor 退回忙等，向后兼容）。规划 `Context<'a>`（Waker 引用 +
// 唤醒器 API）语义 MVP 退化；结构体须非空（`_unit` 哨兵字段保留）。
struct Context {
    _unit: i64,
    deadline: i64,
    fd: i64,
    interest: i64,
}
