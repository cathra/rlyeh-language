# Rlyeh 性能对比：与 C / C++ / Go / Swift / Rust

> 版本：0.2.0　数据日期：**2026-11-15**　环境：Apple M5 Pro / 18 核 / macOS 27.2.1
>
> 本文是 0.2.0 的六语言全量性能对比摘要。完整原始数据、编译耗时与相对速度表见
> [`examples/projects/benchmarks/results/benchmark_report.md`](../examples/projects/benchmarks/results/benchmark_report.md)
> 与 [`examples/projects/benchmarks/README.md`](../examples/projects/benchmarks/README.md)。

## 0. 方法论

- 13 个基准覆盖：递归/循环/浮点矩阵/字符串/哈希表/排序/Actor/动态分发/region/回溯。
- 每种语言独立二进制、统一 warmup 3 次 + 正式 30 次取均值。
- Rlyeh 使用 `rlyeh build`（默认 `-O3`，clang 汇编链接）；C/C++ `-O3`，Go `go build`，Rust `rustc -O`，Swift `swiftc -O`。
- Go 在本机 Homebrew std 模块元数据异常，runner 使用 `GO111MODULE=off` 构建单文件 main 包；不影响优化级别与计时。

## 1. 运行耗时（ms，越低越好）

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift | Rlyeh/最优 |
|------|-----:|---:|---:|---:|-----:|------:|:---:|
| fib | 3.912 | 3.446 | 3.471 | 4.397 | 3.739 | 4.504 | 1.14x |
| loop_sum | 2.245 | 1.970 | 2.014 | 26.654 | 2.393 | 25.759 | 1.14x |
| matmul | 12.639 | 12.371 | 12.330 | 14.235 | 12.373 | 14.706 | 1.03x |
| strcat | 2.432 | 2.244 | 2.711 | 2.646 | 2.462 | 3.696 | 1.08x |
| hashmap | 5.897 | 4.862 | 8.516 | 19.526 | 5.232 | 8.843 | 1.21x |
| sort | 2.412 | 2.458 | 2.656 | 2.755 | 2.637 | 2.942 | **最快** |
| actor_pingpong | 5.371 | 173.502 | 173.809 | 11.694 | 164.549 | 165.746 | **最快** |
| btree | 2.175 | 2.288 | 2.157 | 2.665 | 2.452 | 2.755 | 1.01x |
| hashmap_str | 3.426 | 3.101 | 2.684 | 3.355 | 2.920 | 3.143 | 1.28x |
| dyn_dispatch | 2.092 | 11.907 | 12.048 | 4.882 | 2.215 | 25.566 | **最快** |
| region_alloc | 5.062 | 4.298 | 1.995 | 2.848 | 2.631 | 2.661 | 2.54x |
| region_batch | 12.020 | 11.837 | 12.145 | 12.692 | 12.022 | 13.803 | 1.02x |
| nqueens | 67.979 | 61.418 | 62.820 | 61.683 | 59.149 | 69.157 | 1.15x |

## 2. 结论

- **全场最快 3 项**：`actor_pingpong`（比 Go 快约 2.2x，比线程通道 C/Rust/Swift 快约 31–32x）、`sort`、`dyn_dispatch`。
- **基本同档**：`matmul` 1.03x、`btree` 1.01x、`region_batch` 1.02x、`strcat` 1.08x。
- **1.1–1.3x 差距**：`fib`、`loop_sum`、`nqueens`、`hashmap`、`hashmap_str`。
- **region_alloc**：对 C++ 2.54x，但 C++ `new/delete` 在 `-O3` 下可能被优化消除；对 C 为 1.18x，对 Go/Rust/Swift 约 1.6–1.8x。
- Go 的 `loop_sum` / `hashmap` 明显偏慢，是 gc 代码生成差异，不是计时错误。

## 3. 编译耗时（ms，单次冷编译）

| Rlyeh | C | C++ | Go | Rust | Swift |
|------:|---:|---:|---:|---:|---:|
| 241–259 | 35–46 | 36–221 | 1302–1327 | 70–158 | 148–299 |

> Rlyeh 编译时间包含 std 预置注入 + 类型/借用/区域检查 + LLVM IR 发射；LLVM 优化由 clang 在链接阶段完成。

## 4. 复现

```bash
cd examples/projects/benchmarks
python3 run.py --runs 30 --warmup 3
```

只跑指定基准：`python3 run.py --only fib,hashmap,region_batch --runs 30`。
脚本会生成 `results/benchmark_report.md` 和 `results/raw.json`。

## 5. 历史说明

0.1.0 时期 `run.py` 曾把 Rlyeh 源扩展名误写为 `rlyeh`（实际 `rl`），导致早期数值未真正编译；该问题已在 2026-08-31 修复。当前 0.2.0 数据全部由自动化脚本真实产出。
