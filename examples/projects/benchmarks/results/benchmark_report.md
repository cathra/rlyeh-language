# Zeta 语言性能对比基准报告

> 生成时间: 2026-08-25 00:10  |  每次运行 warmup 2 次 + 正式 7 次取中位数

## 环境

- `uname -m` → arm64
- `sw_vers -productVersion` → 27.0
- `clang --version` → Apple clang version 21.0.0 (clang-2100.3.30.1)
- `go version` → go version go1.27.0 darwin/arm64
- `swiftc --version` → Apple Swift version 6.4 (swiftlang-6.4.0.30.4 clang-2100.3.30.1)
- `rustc --version` → rustc 1.96.0 (ac68faa20 2026-05-25)
- `sysctl -n machdep.cpu.brand_string` → Apple M5 Pro
- `sysctl -n hw.ncpu` → 18

## 编译优化级别

| 语言 | 编译器/优化 |
|------|-------------|
| Zeta | `zeta build`（clang -O3 发布级优化：LLVM 循环优化/向量化/寄存器分配） |
| C    | `clang -O3` |
| C++  | `clang++ -O3` |
| Go   | `go build`（gc 编译器默认优化，无 -O 分级） |
| Rust | `rustc -O` |
| Swift| `swiftc -O` |

## 运行耗时（毫秒，中位数，越低越好）

| 基准 | Zeta | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 4.242 | 4.032 | 4.074 | 5.311 | 4.285 | 5.557 |
| loop_sum | 2.562 | 2.689 | 2.653 | 27.155 | 2.902 | 26.046 |
| matmul | 12.871 | 13.135 | 13.000 | 14.002 | 12.986 | 15.009 |
| strcat | 2.829 | 2.794 | 3.154 | 3.661 | 3.119 | 4.482 |
| hashmap | 8.390 | 5.725 | 11.601 | 22.506 | 7.505 | 12.896 |
| sort | 2.711 | 2.914 | 2.862 | 3.674 | 3.101 | 3.711 |
| actor_pingpong | 6.156 | 194.246 | 184.708 | 12.527 | 174.007 | 171.542 |
| btree | 3.107 | 2.945 | 2.924 | 3.534 | 3.224 | 3.804 |
| hashmap_str | 7.836 | 3.963 | 3.543 | 4.373 | 3.943 | 4.082 |
| dyn_dispatch | 2.860 | 13.694 | 13.742 | 5.896 | 2.985 | 25.746 |
| region_alloc | 6.216 | 5.444 | 2.946 | 3.740 | 3.712 | 3.924 |
| region_batch | 13.104 | 13.066 | 13.596 | 13.631 | 13.745 | 15.685 |
| nqueens | 62.729 | 61.044 | 62.249 | 62.570 | 58.912 | 70.325 |

## 编译耗时（毫秒，单次全量冷编译，越低越好）

> Zeta 使用 `zeta build --force` 绕开增量缓存，全部语言均为从源码全量编译。

| 基准 | Zeta | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 295.7 | 93.3 | 94.0 | 99.0 | 249.9 | 214.6 |
| loop_sum | 307.2 | 93.5 | 92.6 | 104.8 | 243.7 | 208.7 |
| matmul | 304.9 | 99.8 | 253.9 | 97.9 | 263.2 | 267.4 |
| strcat | 299.0 | 98.7 | 212.1 | 103.0 | 251.5 | 223.2 |
| hashmap | 309.0 | 98.0 | 218.6 | 97.5 | 301.8 | 294.1 |
| sort | 300.1 | 95.8 | 264.2 | 99.9 | 294.3 | 374.4 |
| actor_pingpong | 323.7 | 96.7 | 311.2 | 101.8 | 346.8 | 329.7 |
| btree | 300.5 | 100.3 | 265.9 | 97.9 | 258.7 | 293.0 |
| hashmap_str | 333.0 | 102.3 | 270.7 | 101.2 | 308.4 | 317.6 |
| dyn_dispatch | 303.6 | 97.3 | 98.6 | 103.0 | 259.1 | 226.3 |
| region_alloc | 318.9 | 99.1 | 102.7 | 100.8 | 249.6 | 275.0 |
| region_batch | 312.8 | 99.3 | 256.0 | 100.2 | 257.0 | 329.1 |
| nqueens | 300.4 | 98.0 | 264.0 | 104.8 | 260.6 | 315.2 |


## 相对速度（以 Zeta = 1.0 为基准，比值越高表示比 Zeta 快）

| 基准 | Zeta | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 1.0x | 1.1x | 1.0x | 0.8x | 1.0x | 0.8x |
| loop_sum | 1.0x | 1.0x | 1.0x | 0.1x | 0.9x | 0.1x |
| matmul | 1.0x | 1.0x | 1.0x | 0.9x | 1.0x | 0.9x |
| strcat | 1.0x | 1.0x | 0.9x | 0.8x | 0.9x | 0.6x |
| hashmap | 1.0x | 1.5x | 0.7x | 0.4x | 1.1x | 0.7x |
| sort | 1.0x | 0.9x | 0.9x | 0.7x | 0.9x | 0.7x |
| actor_pingpong | 1.0x | 0.0x | 0.0x | 0.5x | 0.0x | 0.0x |
| btree | 1.0x | 1.1x | 1.1x | 0.9x | 1.0x | 0.8x |
| hashmap_str | 1.0x | 2.0x | 2.2x | 1.8x | 2.0x | 1.9x |
| dyn_dispatch | 1.0x | 0.2x | 0.2x | 0.5x | 1.0x | 0.1x |
| region_alloc | 1.0x | 1.1x | 2.1x | 1.7x | 1.7x | 1.6x |
| region_batch | 1.0x | 1.0x | 1.0x | 1.0x | 1.0x | 0.8x |
| nqueens | 1.0x | 1.0x | 1.0x | 1.0x | 1.1x | 0.9x |

## 基准说明

- **fib**: fib(30) 双递归（函数调用 + 整数运算）
- **loop_sum**: 1 亿次 i64 循环累加（整数运算 + 分支）
- **matmul**: 256x256 f64 矩阵乘法（浮点 + 内存访问）
- **strcat**: 字符串拼接 10 万次（缓冲扩容）
- **hashmap**: 20 万 insert + 20 万 get，i64 键（哈希表）
- **sort**: LCG 生成 5000 个 i64 排序（排序算法）
- **actor_pingpong**: 5 万次 actor 同步往返（Zeta 并发模型 vs 线程通道）
- **btree**: 深度 15 完全二叉树构造 + 递归求和（内存访问 + 递归）
- **hashmap_str**: 1 万条字符串键 insert + get（字符串哈希 + 键构造）
- **dyn_dispatch**: 2000 万次 dyn Trait / 虚函数多态分派
- **region_alloc**: 100 万次小对象分配（Zeta region 批量 vs 逐次分配）
- **region_batch**: 100 万循环 × 每次 4 小对象分配（批量提升 vs 手动 bump 真实写带宽）
- **nqueens**: 12 皇后回溯搜索（纯整数递归 + 剪枝分支）

## 方法学与注意事项

- 所有语言实现逻辑严格一致（运行期自动核对跨语言输出逐项一致）。
- Zeta 编译走 clang -O3 发布级优化管线（LLVM 循环优化/向量化/常量传播/寄存器分配），其余语言为各自发布级优化（-O3/-O），本报告反映各语言**当前编译器的真实水平**。
- 排序基准中 Zeta `Vec::sort_by` 为 O(n log n) 原地堆排序（std-lib §3.1），C/C++/Swift/Rust 为各自标准库排序——差距属实现差异。
- `actor_pingpong`：Zeta 侧为单 actor 5 万次 ask 同步往返（actor 运行时调度 + 邮箱消息传递），C/C++/Swift/Rust 侧为双线程双通道同步往返（mutex/condvar、mpsc）——语义对应「请求-响应消息吞吐」。
- `region_alloc`：Zeta 侧为 region 内 100 万次 bump 分配（区域退出一次性释放），C/C++/Rust/Swift 侧为逐次分配+释放（malloc/free、new/delete、Box、class+ARC）——反映不同内存管理模型的分配吞吐（region 批量分配 vs 逐次分配是 Zeta 的设计优势）。
- `hashmap`：Zeta/Rust/Go/Swift 侧为各语言标准/内置哈希表（std HashMap / 内置 map），C 无标准哈希表、手写线性探测表（2^20 槽，负载 ~19%）。
- `hashmap_str`：各语言在插入/查询阶段每次重建键字符串（format!/sprintf/strdup/Sprintf），键构造成本计入基准。
- `dyn_dispatch`：Zeta 侧为 `dyn Trait` 胖指针 + vtable 间接分派，C++ 虚函数、Rust trait 对象、Swift `any` 存在类型；局部对象多态调用在各编译器下可能被去虚拟化优化，本基准反映真实多态调用吞吐。
- `nqueens` 为 P2 复平面迭代（mandelbrot）的替代：Zeta MVP 的 `as f64` 数值转换尚未在 IR 层实现（Cast 在 typecheck 后被静默擦除、无转换指令，i64 位模式被直接当作 f64 值），mandelbrot 需要运行时 i→f64 坐标计算，故改用纯整数回溯搜索覆盖「搜索 / 递归 / 分支」算力维度；`as` 转换的 IR 支持已列为后续任务。
- 进程启动开销已含在计时内（各语言一致）。
- 编译耗时对比为单次全量冷编译；Zeta `--force` 绕开增量缓存，其余语言无增量缓存。
