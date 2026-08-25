# 07 · 时间：Duration / Instant / sleep

> 规范：docs/std-lib.md §7（时间模块）/ §10.2（sleep）
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- `Duration` / `Instant`：时间量表示与测量
- `std::time::sleep`：阻塞睡眠（毫秒）

## 示例清单

| 文件 | 说明 |
|------|------|
| `sleep_join.rl` | sleep 延时 + Instant 时间测量（含多任务 join） |

## 运行

```bash
rlyeh run examples/std-demos/07-time/sleep_join.rl
```
