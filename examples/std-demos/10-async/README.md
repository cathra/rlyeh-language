# 10 · 异步运行时

> 规范：docs/std-lib.md §10（异步运行时）/ docs/guide.md §9.3
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- `async fn`：desugar 为 Future 结构体 + poll 状态机 + 构造器
- `expr.await`：经状态机轮询子 future，支持 `Poll::Pending` 挂起 / 恢复，跨 await 变量提升
- `block_on`：阻塞轮询驱动 Future
- `join_all`：并发等待多个 Future
- `timeout`：超时控制
- 手写 poll 状态机：`Future` / `Poll` / `Waker` 底层协议

## 示例清单

| 文件 | 说明 |
|------|------|
| `async-fns.zeta` | S1c async fn / await：状态机 desugar、嵌套 await |
| `async_await.zeta` | async/await 基础用法 |
| `block_on.zeta` | block_on 驱动 Future 直至完成 |
| `join_all.zeta` | 多 Future 并发 join |
| `timeout.zeta` | Future 超时控制 |
| `manual_state_machine.zeta` | 手写 poll 状态机（底层协议演示） |

## 运行

```bash
zeta run examples/std-demos/10-async/async-fns.zeta
zeta run examples/std-demos/10-async/block_on.zeta
zeta run examples/std-demos/10-async/join_all.zeta
zeta run examples/std-demos/10-async/timeout.zeta
```
