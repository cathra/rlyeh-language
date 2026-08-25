# 06 · 同步原语：Mutex / Channel

> 规范：docs/std-lib.md §6（同步与并发原语）
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- `Mutex<T>`：`new` / `lock`（guard）/ `unlock`
- 并发通道：`channel()` → `(tx, rx)`，无界队列
  - `send` / `recv`（FIFO 阻塞）/ `try_send` / `try_recv` / `close` / `iter`
  - 多 Sender 共享同一 Receiver

## 示例清单

| 文件 | 说明 |
|------|------|
| `mutex_guard.rl` | Mutex 加锁 / 临界区 / 释放 |
| `channel.rl` | P1 并发通道：send/recv、try_*、close、iter、多 Sender 共享 |

## 运行

```bash
rlyeh run examples/std-demos/06-sync/mutex_guard.rl
rlyeh run examples/std-demos/06-sync/channel.rl
```

> `RwLock` / `Condvar` / `Barrier` 规划见 std-lib.md §6.1。
