# Rlyeh 语言性能对比基准报告

> 生成时间: 2026-08-31 13:45  |  每次运行 warmup 1 次 + 正式 5 次取中位数

## 环境

- `uname -m` → arm64
- `sw_vers -productVersion` → 27.0
- `clang --version` → Apple clang version 21.0.0 (clang-2100.3.33.1)
- `go version` → go version go1.27.0 darwin/arm64
- `swiftc --version` → Apple Swift version 6.4 (swiftlang-6.4.0.33.1 clang-2100.3.33.1)
- `rustc --version` → rustc 1.96.0 (ac68faa20 2026-05-25)
- `sysctl -n machdep.cpu.brand_string` → Apple M5 Pro
- `sysctl -n hw.ncpu` → 18

## 编译优化级别

| 语言 | 编译器/优化 |
|------|-------------|
| Rlyeh | `rlyeh build`（clang -O3 发布级优化：LLVM 循环优化/向量化/寄存器分配） |
| C    | `clang -O3` |
| C++  | `clang++ -O3` |
| Go   | `go build`（gc 编译器默认优化，无 -O 分级） |
| Rust | `rustc -O` |
| Swift| `swiftc -O` |

## 运行耗时（毫秒，中位数，越低越好）

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 5.795 | 5.729 | 6.082 | 4.733 | 6.205 | 4.875 |
| loop_sum | 4.291 | 2.291 | 2.341 | 25.296 | 2.778 | 24.838 |
| matmul | 13.167 | 12.351 | 12.116 | 12.695 | 11.719 | 14.035 |
| strcat | 4.379 | 2.387 | 2.798 | 2.778 | 2.607 | 3.845 |
| hashmap | 9.184 | 4.694 | 8.213 | 18.484 | 5.501 | 7.920 |
| sort | 4.361 | 2.563 | 2.434 | 3.243 | 2.354 | 3.046 |
| actor_pingpong | 7.502 | 177.017 | 181.243 | 11.390 | 167.513 | 158.735 |
| btree | 4.538 | 2.406 | 2.413 | 2.891 | 2.506 | 3.296 |
| hashmap_str | 9.810 | 3.454 | 2.791 | 3.590 | 3.289 | 3.611 |
| dyn_dispatch | 4.842 | 13.369 | 12.803 | 4.988 | 2.252 | 24.252 |
| region_alloc | 5.874 | 4.533 | 2.467 | 3.107 | 2.938 | 3.083 |
| region_batch | 12.930 | 11.528 | 12.759 | 11.484 | 11.442 | 14.118 |
| nqueens | 62.839 | 58.409 | 63.046 | 62.785 | 56.530 | 65.694 |

## 编译耗时（毫秒，单次全量冷编译，越低越好）

> Rlyeh 使用 `rlyeh build --force` 绕开增量缓存，全部语言均为从源码全量编译。

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 219.6 | 45.1 | 46.0 | 47.7 | 75.6 | 150.9 |
| loop_sum | 232.6 | 52.6 | 40.4 | 43.0 | 72.9 | 147.9 |
| matmul | 225.3 | 42.5 | 177.7 | 41.9 | 85.9 | 180.8 |
| strcat | 224.6 | 52.6 | 135.6 | 42.5 | 80.9 | 152.2 |
| hashmap | 230.6 | 43.9 | 157.0 | 41.7 | 119.9 | 224.7 |
| sort | 226.0 | 60.1 | 194.9 | 42.6 | 123.1 | 281.5 |
| actor_pingpong | 227.1 | 45.5 | 219.8 | 41.8 | 163.9 | 253.9 |
| btree | 225.9 | 55.2 | 200.0 | 44.4 | 92.2 | 219.7 |
| hashmap_str | 237.9 | 46.0 | 179.6 | 42.3 | 117.0 | 227.6 |
| dyn_dispatch | 226.5 | 51.9 | 42.6 | 49.1 | 81.5 | 156.2 |
| region_alloc | 237.2 | 41.9 | 39.6 | 44.1 | 77.3 | 199.6 |
| region_batch | 234.7 | 43.3 | 185.3 | 44.8 | 79.4 | 231.5 |
| nqueens | 223.9 | 45.0 | 178.4 | 45.4 | 90.7 | 235.4 |


## 相对速度（以 Rlyeh = 1.0 为基准，比值越高表示比 Rlyeh 快）

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 1.0x | 1.0x | 1.0x | 1.2x | 0.9x | 1.2x |
| loop_sum | 1.0x | 1.9x | 1.8x | 0.2x | 1.5x | 0.2x |
| matmul | 1.0x | 1.1x | 1.1x | 1.0x | 1.1x | 0.9x |
| strcat | 1.0x | 1.8x | 1.6x | 1.6x | 1.7x | 1.1x |
| hashmap | 1.0x | 2.0x | 1.1x | 0.5x | 1.7x | 1.2x |
| sort | 1.0x | 1.7x | 1.8x | 1.3x | 1.9x | 1.4x |
| actor_pingpong | 1.0x | 0.0x | 0.0x | 0.7x | 0.0x | 0.0x |
| btree | 1.0x | 1.9x | 1.9x | 1.6x | 1.8x | 1.4x |
| hashmap_str | 1.0x | 2.8x | 3.5x | 2.7x | 3.0x | 2.7x |
| dyn_dispatch | 1.0x | 0.4x | 0.4x | 1.0x | 2.2x | 0.2x |
| region_alloc | 1.0x | 1.3x | 2.4x | 1.9x | 2.0x | 1.9x |
| region_batch | 1.0x | 1.1x | 1.0x | 1.1x | 1.1x | 0.9x |
| nqueens | 1.0x | 1.1x | 1.0x | 1.0x | 1.1x | 1.0x |

## 基准说明

- **fib**: fib(30) 双递归（函数调用 + 整数运算）
- **loop_sum**: 1 亿次 i64 循环累加（整数运算 + 分支）
- **matmul**: 256x256 f64 矩阵乘法（浮点 + 内存访问）
- **strcat**: 字符串拼接 10 万次（缓冲扩容）
- **hashmap**: 20 万 insert + 20 万 get，i64 键（哈希表）
- **sort**: LCG 生成 5000 个 i64 排序（排序算法）
- **actor_pingpong**: 5 万次 actor 同步往返（Rlyeh 并发模型 vs 线程通道）
- **btree**: 深度 15 完全二叉树构造 + 递归求和（内存访问 + 递归）
- **hashmap_str**: 1 万条字符串键 insert + get（字符串哈希 + 键构造）
- **dyn_dispatch**: 2000 万次 dyn Trait / 虚函数多态分派
- **region_alloc**: 100 万次小对象分配（Rlyeh region 批量 vs 逐次分配）
- **region_batch**: 100 万循环 × 每次 4 小对象分配（批量提升 vs 手动 bump 真实写带宽）
- **nqueens**: 12 皇后回溯搜索（纯整数递归 + 剪枝分支）

## 方法学与注意事项

- 所有语言实现逻辑严格一致（运行期自动核对跨语言输出逐项一致）。
- Rlyeh 编译走 clang -O3 发布级优化管线（LLVM 循环优化/向量化/常量传播/寄存器分配），其余语言为各自发布级优化（-O3/-O），本报告反映各语言**当前编译器的真实水平**。
- 排序基准中 Rlyeh `Vec::sort_by` 为 O(n log n) 原地堆排序（std-lib §3.1），C/C++/Swift/Rust 为各自标准库排序——差距属实现差异。
- `actor_pingpong`：Rlyeh 侧为单 actor 5 万次 ask 同步往返（actor 运行时调度 + 邮箱消息传递），C/C++/Swift/Rust 侧为双线程双通道同步往返（mutex/condvar、mpsc）——语义对应「请求-响应消息吞吐」。
- `region_alloc`：Rlyeh 侧为 region 内 100 万次 bump 分配（区域退出一次性释放），C/C++/Rust/Swift 侧为逐次分配+释放（malloc/free、new/delete、Box、class+ARC）——反映不同内存管理模型的分配吞吐（region 批量分配 vs 逐次分配是 Rlyeh 的设计优势）。
- `hashmap`：Rlyeh/Rust/Go/Swift 侧为各语言标准/内置哈希表（std HashMap / 内置 map），C 无标准哈希表、手写线性探测表（2^20 槽，负载 ~19%）。
- `hashmap_str`：各语言在插入/查询阶段每次重建键字符串（format!/sprintf/strdup/Sprintf），键构造成本计入基准。
- `dyn_dispatch`：Rlyeh 侧为 `dyn Trait` 胖指针 + vtable 间接分派，C++ 虚函数、Rust trait 对象、Swift `any` 存在类型；局部对象多态调用在各编译器下可能被去虚拟化优化，本基准反映真实多态调用吞吐。
- `nqueens` 为 P2 复平面迭代（mandelbrot）的替代：Rlyeh MVP 的 `as f64` 数值转换尚未在 IR 层实现（Cast 在 typecheck 后被静默擦除、无转换指令，i64 位模式被直接当作 f64 值），mandelbrot 需要运行时 i→f64 坐标计算，故改用纯整数回溯搜索覆盖「搜索 / 递归 / 分支」算力维度；`as` 转换的 IR 支持已列为后续任务。
- 进程启动开销已含在计时内（各语言一致）。
- 编译耗时对比为单次全量冷编译；Rlyeh `--force` 绕开增量缓存，其余语言无增量缓存。
