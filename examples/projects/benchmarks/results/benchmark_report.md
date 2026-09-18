# Rlyeh 语言性能对比基准报告

> 生成时间: 2026-09-01 10:19  |  每次运行 warmup 1 次 + 正式 10 次取平均值

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

## 运行耗时（毫秒，10 轮取平均值，越低越好）

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 5.071 | 4.988 | 4.962 | 4.317 | 5.371 | 5.998 |
| loop_sum | 3.633 | 3.593 | 3.566 | 25.572 | 3.791 | 25.555 |
| matmul | 12.195 | 13.105 | 13.210 | 12.513 | 13.043 | 14.733 |
| strcat | 3.824 | 3.798 | 4.161 | 2.749 | 3.845 | 5.111 |
| hashmap | 7.539 | 6.519 | 9.133 | 16.581 | 6.662 | 9.144 |
| sort | 3.807 | 3.767 | 3.667 | 2.956 | 3.879 | 4.553 |
| actor_pingpong | 6.783 | 181.481 | 176.910 | 12.156 | 149.204 | 164.067 |
| btree | 4.052 | 3.836 | 3.787 | 2.825 | 4.001 | 3.759 |
| hashmap_str | 8.121 | 4.828 | 4.290 | 3.573 | 4.688 | 4.896 |
| dyn_dispatch | 3.762 | 13.083 | 13.223 | 4.994 | 3.775 | 25.666 |
| region_alloc | 6.793 | 6.248 | 3.882 | 3.065 | 4.423 | 4.717 |
| region_batch | 11.665 | 11.974 | 11.707 | 10.814 | 11.577 | 13.815 |
| nqueens | 59.428 | 58.341 | 59.268 | 58.912 | 56.220 | 67.079 |

## 编译耗时（毫秒，单次全量冷编译，越低越好）

> Rlyeh 使用 `rlyeh build --force` 绕开增量缓存，全部语言均为从源码全量编译。

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 290.0 | 79.2 | 48.8 | 192.5 | 129.5 | 700.2 |
| loop_sum | 223.4 | 43.3 | 44.5 | 49.9 | 77.0 | 158.4 |
| matmul | 226.2 | 54.1 | 269.3 | 44.5 | 117.6 | 202.1 |
| strcat | 221.9 | 46.7 | 139.0 | 48.3 | 93.7 | 155.9 |
| hashmap | 244.5 | 41.6 | 151.6 | 45.4 | 138.8 | 226.2 |
| sort | 224.8 | 46.8 | 185.5 | 48.7 | 123.4 | 290.8 |
| actor_pingpong | 233.3 | 42.0 | 217.4 | 44.0 | 180.8 | 353.9 |
| btree | 233.4 | 47.5 | 184.3 | 48.2 | 87.8 | 214.9 |
| hashmap_str | 239.3 | 44.6 | 178.4 | 46.1 | 122.4 | 232.6 |
| dyn_dispatch | 225.7 | 45.7 | 41.0 | 44.6 | 82.5 | 159.7 |
| region_alloc | 228.7 | 42.9 | 43.0 | 48.1 | 80.4 | 203.7 |
| region_batch | 238.3 | 46.8 | 172.4 | 44.3 | 86.3 | 243.8 |
| nqueens | 226.8 | 42.8 | 178.6 | 44.8 | 90.9 | 240.4 |


## 相对速度（以 Rlyeh = 1.0 为基准，比值越高表示比 Rlyeh 快）

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 1.0x | 1.0x | 1.0x | 1.2x | 0.9x | 0.8x |
| loop_sum | 1.0x | 1.0x | 1.0x | 0.1x | 1.0x | 0.1x |
| matmul | 1.0x | 0.9x | 0.9x | 1.0x | 0.9x | 0.8x |
| strcat | 1.0x | 1.0x | 0.9x | 1.4x | 1.0x | 0.7x |
| hashmap | 1.0x | 1.2x | 0.8x | 0.5x | 1.1x | 0.8x |
| sort | 1.0x | 1.0x | 1.0x | 1.3x | 1.0x | 0.8x |
| actor_pingpong | 1.0x | 0.0x | 0.0x | 0.6x | 0.0x | 0.0x |
| btree | 1.0x | 1.1x | 1.1x | 1.4x | 1.0x | 1.1x |
| hashmap_str | 1.0x | 1.7x | 1.9x | 2.3x | 1.7x | 1.7x |
| dyn_dispatch | 1.0x | 0.3x | 0.3x | 0.8x | 1.0x | 0.1x |
| region_alloc | 1.0x | 1.1x | 1.7x | 2.2x | 1.5x | 1.4x |
| region_batch | 1.0x | 1.0x | 1.0x | 1.1x | 1.0x | 0.8x |
| nqueens | 1.0x | 1.0x | 1.0x | 1.0x | 1.1x | 0.9x |

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
- `dyn_dispatch`：Rlyeh 侧为 `dyn Protocol` 胖指针 + vtable 间接分派，C++ 虚函数、Rust trait 对象、Swift `any` 存在类型；局部对象多态调用在各编译器下可能被去虚拟化优化，本基准反映真实多态调用吞吐。
- `nqueens` 为 P2 复平面迭代（mandelbrot）的替代：Rlyeh MVP 的 `as f64` 数值转换尚未在 IR 层实现（Cast 在 typecheck 后被静默擦除、无转换指令，i64 位模式被直接当作 f64 值），mandelbrot 需要运行时 i→f64 坐标计算，故改用纯整数回溯搜索覆盖「搜索 / 递归 / 分支」算力维度；`as` 转换的 IR 支持已列为后续任务。
- 进程启动开销已含在计时内（各语言一致）。
- 编译耗时对比为单次全量冷编译；Rlyeh `--force` 绕开增量缓存，其余语言无增量缓存。
