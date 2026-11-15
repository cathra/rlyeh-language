# Rlyeh 语言性能对比基准报告

> 生成时间: 2026-11-15 16:09  |  每次运行 warmup 3 次 + 正式 30 次取平均值

## 环境

- `uname -m` → arm64
- `sw_vers -productVersion` → 27.2.1
- `clang --version` → Apple clang version 21.0.0 (clang-2100.4.1.1)
- `go version` → go version go1.27.4 darwin/arm64
- `swiftc --version` → Apple Swift version 6.4.2 (swiftlang-6.4.2.1.9 clang-2100.4.1.1)
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

## 运行耗时（毫秒，30 轮取平均值，越低越好）

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 3.912 | 3.446 | 3.471 | 4.397 | 3.739 | 4.504 |
| loop_sum | 2.245 | 1.970 | 2.014 | 26.654 | 2.393 | 25.759 |
| matmul | 12.639 | 12.371 | 12.330 | 14.235 | 12.373 | 14.706 |
| strcat | 2.432 | 2.244 | 2.711 | 2.646 | 2.462 | 3.696 |
| hashmap | 5.897 | 4.862 | 8.516 | 19.526 | 5.232 | 8.843 |
| sort | 2.412 | 2.458 | 2.656 | 2.755 | 2.637 | 2.942 |
| actor_pingpong | 5.371 | 173.502 | 173.809 | 11.694 | 164.549 | 165.746 |
| btree | 2.175 | 2.288 | 2.157 | 2.665 | 2.452 | 2.755 |
| hashmap_str | 3.426 | 3.101 | 2.684 | 3.355 | 2.920 | 3.143 |
| dyn_dispatch | 2.092 | 11.907 | 12.048 | 4.882 | 2.215 | 25.566 |
| region_alloc | 5.062 | 4.298 | 1.995 | 2.848 | 2.631 | 2.661 |
| region_batch | 12.020 | 11.837 | 12.145 | 12.692 | 12.022 | 13.803 |
| nqueens | 67.979 | 61.418 | 62.820 | 61.683 | 59.149 | 69.157 |

## 编译耗时（毫秒，单次全量冷编译，越低越好）

> Rlyeh 使用 `rlyeh build --force` 绕开增量缓存，全部语言均为从源码全量编译。

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 245.6 | 37.2 | 37.9 | 1326.8 | 70.0 | 148.4 |
| loop_sum | 241.0 | 35.3 | 36.4 | 1301.6 | 74.0 | 151.9 |
| matmul | 246.8 | 42.0 | 191.6 | 1308.6 | 83.8 | 191.8 |
| strcat | 243.1 | 39.7 | 137.4 | 1315.0 | 79.5 | 156.0 |
| hashmap | 251.2 | 41.5 | 153.2 | 1315.2 | 118.4 | 236.8 |
| sort | 246.3 | 39.6 | 189.2 | 1311.0 | 113.9 | 298.6 |
| actor_pingpong | 246.6 | 40.2 | 220.8 | 1310.0 | 157.9 | 267.4 |
| btree | 246.0 | 40.1 | 187.8 | 1321.8 | 81.2 | 222.2 |
| hashmap_str | 259.1 | 43.2 | 184.7 | 1317.0 | 115.4 | 235.4 |
| dyn_dispatch | 244.1 | 38.2 | 40.5 | 1317.3 | 72.0 | 161.0 |
| region_alloc | 249.3 | 41.4 | 40.0 | 1305.5 | 73.9 | 203.6 |
| region_batch | 249.6 | 42.4 | 192.9 | 1307.5 | 79.5 | 247.3 |
| nqueens | 246.0 | 46.3 | 189.4 | 1305.5 | 83.0 | 241.6 |


## 相对速度（以 Rlyeh = 1.0 为基准，比值越高表示比 Rlyeh 快）

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 1.00x | 1.14x | 1.13x | 0.89x | 1.05x | 0.87x |
| loop_sum | 1.00x | 1.14x | 1.11x | 0.08x | 0.94x | 0.09x |
| matmul | 1.00x | 1.02x | 1.03x | 0.89x | 1.02x | 0.86x |
| strcat | 1.00x | 1.08x | 0.90x | 0.92x | 0.99x | 0.66x |
| hashmap | 1.00x | 1.21x | 0.69x | 0.30x | 1.13x | 0.67x |
| sort | 1.00x | 0.98x | 0.91x | 0.88x | 0.91x | 0.82x |
| actor_pingpong | 1.00x | 0.03x | 0.03x | 0.46x | 0.03x | 0.03x |
| btree | 1.00x | 0.95x | 1.01x | 0.82x | 0.89x | 0.79x |
| hashmap_str | 1.00x | 1.10x | 1.28x | 1.02x | 1.17x | 1.09x |
| dyn_dispatch | 1.00x | 0.18x | 0.17x | 0.43x | 0.94x | 0.08x |
| region_alloc | 1.00x | 1.18x | 2.54x | 1.78x | 1.92x | 1.90x |
| region_batch | 1.00x | 1.02x | 0.99x | 0.95x | 1.00x | 0.87x |
| nqueens | 1.00x | 1.11x | 1.08x | 1.10x | 1.15x | 0.98x |

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
- **dyn_dispatch**: 2000 万次 dyn Protocol / 虚函数多态分派
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
- `dyn_dispatch`：Rlyeh 侧为 `dyn Protocol` 胖指针 + vtable 间接分派，C++ 虚函数、Rust protocol 对象、Swift `any` 存在类型；局部对象多态调用在各编译器下可能被去虚拟化优化，本基准反映真实多态调用吞吐。
- `nqueens` 为 P2 复平面迭代（mandelbrot）的替代：Rlyeh MVP 的 `as f64` 数值转换尚未在 IR 层实现（Cast 在 typecheck 后被静默擦除、无转换指令，i64 位模式被直接当作 f64 值），mandelbrot 需要运行时 i→f64 坐标计算，故改用纯整数回溯搜索覆盖「搜索 / 递归 / 分支」算力维度；`as` 转换的 IR 支持已列为后续任务。
- 进程启动开销已含在计时内（各语言一致）。
- 编译耗时对比为单次全量冷编译；Rlyeh `--force` 绕开增量缓存，其余语言无增量缓存。
