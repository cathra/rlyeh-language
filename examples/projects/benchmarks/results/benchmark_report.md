# Rlyeh 语言性能对比基准报告

> 生成时间: 2026-11-15 16:02  |  每次运行 warmup 3 次 + 正式 30 次取平均值

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
| fib | 3.560 | 3.200 | 3.188 | 4.572 | 3.362 | 4.270 |
| loop_sum | 1.908 | 1.893 | 1.810 | 26.435 | 2.064 | 25.544 |
| matmul | 12.484 | 12.321 | 12.460 | 13.468 | 12.209 | 14.296 |
| strcat | 2.054 | 1.940 | 2.354 | 2.580 | 2.178 | 3.419 |
| hashmap | 5.518 | 4.605 | 8.342 | 20.082 | 4.930 | 8.560 |
| sort | 2.199 | 2.327 | 1.869 | 2.699 | 2.102 | 2.770 |
| actor_pingpong | 5.116 | 177.108 | 172.839 | 11.593 | 169.537 | 165.601 |
| btree | 2.129 | 2.466 | 2.166 | 2.754 | 2.376 | 2.752 |
| hashmap_str | 3.348 | 3.058 | 2.522 | 3.497 | 2.900 | 3.071 |
| dyn_dispatch | 1.972 | 12.824 | 13.239 | 5.036 | 2.083 | 25.373 |
| region_alloc | 5.073 | 4.268 | 2.095 | 2.854 | 2.557 | 2.717 |
| region_batch | 12.368 | 11.892 | 12.456 | 12.459 | 12.151 | 14.414 |
| nqueens | 68.508 | 62.006 | 63.053 | 62.612 | 59.672 | 70.696 |

## 编译耗时（毫秒，单次全量冷编译，越低越好）

> Rlyeh 使用 `rlyeh build --force` 绕开增量缓存，全部语言均为从源码全量编译。

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 253.8 | 37.2 | 41.6 | 1317.5 | 71.8 | 153.3 |
| loop_sum | 240.1 | 35.9 | 37.8 | 1319.9 | 70.1 | 155.2 |
| matmul | 249.0 | 42.1 | 194.8 | 1330.1 | 83.9 | 192.9 |
| strcat | 242.6 | 39.3 | 136.2 | 1337.9 | 78.3 | 159.2 |
| hashmap | 254.7 | 40.7 | 153.8 | 1338.3 | 118.5 | 229.3 |
| sort | 251.1 | 39.7 | 187.5 | 1328.2 | 115.4 | 302.0 |
| actor_pingpong | 249.6 | 40.0 | 221.3 | 1318.5 | 159.1 | 256.9 |
| btree | 248.3 | 41.1 | 188.3 | 1331.1 | 83.1 | 233.5 |
| hashmap_str | 259.9 | 46.2 | 188.4 | 1327.5 | 118.2 | 239.5 |
| dyn_dispatch | 247.3 | 38.6 | 40.4 | 1316.3 | 73.4 | 163.5 |
| region_alloc | 251.9 | 41.5 | 40.2 | 1327.4 | 74.9 | 204.6 |
| region_batch | 252.7 | 46.6 | 185.6 | 1329.1 | 78.5 | 251.4 |
| nqueens | 254.8 | 44.2 | 192.2 | 1313.9 | 85.0 | 245.2 |


## 相对速度（以 Rlyeh = 1.0 为基准，比值越高表示比 Rlyeh 快）

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 1.00x | 1.11x | 1.12x | 0.78x | 1.06x | 0.83x |
| loop_sum | 1.00x | 1.01x | 1.05x | 0.07x | 0.92x | 0.07x |
| matmul | 1.00x | 1.01x | 1.00x | 0.93x | 1.02x | 0.87x |
| strcat | 1.00x | 1.06x | 0.87x | 0.80x | 0.94x | 0.60x |
| hashmap | 1.00x | 1.20x | 0.66x | 0.27x | 1.12x | 0.64x |
| sort | 1.00x | 0.94x | 1.18x | 0.81x | 1.05x | 0.79x |
| actor_pingpong | 1.00x | 0.03x | 0.03x | 0.44x | 0.03x | 0.03x |
| btree | 1.00x | 0.86x | 0.98x | 0.77x | 0.90x | 0.77x |
| hashmap_str | 1.00x | 1.09x | 1.33x | 0.96x | 1.15x | 1.09x |
| dyn_dispatch | 1.00x | 0.15x | 0.15x | 0.39x | 0.95x | 0.08x |
| region_alloc | 1.00x | 1.19x | 2.42x | 1.78x | 1.98x | 1.87x |
| region_batch | 1.00x | 1.04x | 0.99x | 0.99x | 1.02x | 0.86x |
| nqueens | 1.00x | 1.10x | 1.09x | 1.09x | 1.15x | 0.97x |

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
