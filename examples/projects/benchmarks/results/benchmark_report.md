# Zeta 语言性能对比基准报告

> 生成时间: 2026-08-24 11:37  |  每次运行 warmup 1 次 + 正式 5 次取中位数

## 环境

- `uname -m` → arm64
- `sw_vers -productVersion` → 27.0
- `clang --version` → Apple clang version 21.0.0 (clang-2100.3.30.1)
- `swiftc --version` → Apple Swift version 6.4 (swiftlang-6.4.0.30.4 clang-2100.3.30.1)
- `rustc --version` → rustc 1.96.0 (ac68faa20 2026-05-25)
- `sysctl -n machdep.cpu.brand_string` → Apple M5 Pro
- `sysctl -n hw.ncpu` → 18

## 编译优化级别

| 语言 | 编译器/优化 |
|------|-------------|
| Zeta | `zeta build`（无优化管线，clang O0 汇编——当前真实水平） |
| C    | `clang -O3` |
| C++  | `clang++ -O3` |
| Swift| `swiftc -O` |
| Rust | `rustc -O` |

## 结果（毫秒，中位数，越低越好）

| 基准 | Zeta | C | C++ | Swift | Rust |
|------|-----|-----|-----|-----|-----|
| fib | 5.994 | 3.609 | 3.555 | 4.716 | 3.953 |
| loop_sum | 83.823 | 2.146 | 2.077 | 24.864 | 2.582 |
| matmul | 69.997 | 12.256 | 12.139 | 13.684 | 11.930 |
| strcat | 4.951 | 2.140 | 2.277 | 4.111 | 2.333 |
| hashmap | 26.838 | 4.594 | 8.261 | 8.457 | 5.619 |
| sort | 42.535 | 2.440 | 2.579 | 3.071 | 2.639 |

## 相对速度（以 Zeta = 1.0 为基准，越高表示比 Zeta 快）

| 基准 | Zeta | C | C++ | Swift | Rust |
|------|-----|-----|-----|-----|-----|
| fib | 1.0x | 1.7x | 1.7x | 1.3x | 1.5x |
| loop_sum | 1.0x | 39.1x | 40.4x | 3.4x | 32.5x |
| matmul | 1.0x | 5.7x | 5.8x | 5.1x | 5.9x |
| strcat | 1.0x | 2.3x | 2.2x | 1.2x | 2.1x |
| hashmap | 1.0x | 5.8x | 3.2x | 3.2x | 4.8x |
| sort | 1.0x | 17.4x | 16.5x | 13.9x | 16.1x |

## 基准说明

- **fib**: fib(30) 双递归（函数调用 + 整数运算）
- **loop_sum**: 1 亿次 i64 循环累加（整数运算 + 分支）
- **matmul**: 256x256 f64 矩阵乘法（浮点 + 内存访问）
- **strcat**: 字符串拼接 10 万次（缓冲扩容）
- **hashmap**: 20 万 insert + 20 万 get，i64 键（哈希表）
- **sort**: LCG 生成 5000 个 i64 排序（排序算法）

## 方法学与注意事项

- 所有语言实现逻辑严格一致（跨语言输出已逐项核对一致）。
- Zeta 当前无优化管线（最小 MIR 优化 + clang O0 汇编），其余语言均为发布级优化，本报告反映的是各语言**当前编译器的真实水平**，而非 Zeta 的理论上限。
- 排序基准中 Zeta `Vec::sort_by` 当前实现为 O(n²) 选择排序（std-lib §3.1），C/C++/Swift/Rust 均为 O(n log n) 标准库排序——该差异属标准库实现差距，非语法层能力差距。
- 进程启动开销已含在计时内（各语言一致）。
- Go 本机未安装，暂缺；安装后可在本报告框架内补测。
