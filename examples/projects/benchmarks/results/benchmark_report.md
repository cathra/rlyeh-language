# Zeta 语言性能对比基准报告

> 生成时间: 2026-08-24 18:58  |  每次运行 warmup 1 次 + 正式 5 次取中位数

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
| fib | 4.645 | 3.872 | 3.839 | 5.090 | 4.215 | 5.126 |
| loop_sum | 3.719 | 2.619 | 2.233 | 26.337 | 2.704 | 25.670 |
| matmul | 13.306 | 12.724 | 12.756 | 13.639 | 12.590 | 14.291 |
| strcat | 3.286 | 2.408 | 2.752 | 3.198 | 2.993 | 3.886 |
| hashmap | 11.086 | 5.166 | 9.578 | 19.021 | 6.188 | 10.340 |
| sort | 3.431 | 2.637 | 3.310 | 3.292 | 3.003 | 3.387 |
| actor_pingpong | 341.140 | 186.265 | 179.796 | 12.045 | 157.111 | 166.406 |
| btree | 3.549 | 2.542 | 2.789 | 2.954 | 2.607 | 3.271 |
| hashmap_str | 5.297 | 3.587 | 3.155 | 3.851 | 3.764 | 3.400 |
| dyn_dispatch | 15.481 | 12.328 | 12.105 | 5.317 | 2.745 | 24.983 |
| region_alloc | 8.188 | 2.231 | 2.474 | 3.329 | 3.235 | 3.399 |
| nqueens | 63.248 | 61.030 | 62.433 | 61.953 | 59.098 | 68.974 |

## 编译耗时（毫秒，单次全量冷编译，越低越好）

> Zeta 使用 `zeta build --force` 绕开增量缓存，全部语言均为从源码全量编译。

| 基准 | Zeta | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 238.7 | 77.5 | 71.4 | 82.6 | 170.2 | 179.8 |
| loop_sum | 243.7 | 76.2 | 68.8 | 75.3 | 175.3 | 181.7 |
| matmul | 242.5 | 78.9 | 219.3 | 78.1 | 187.8 | 225.4 |
| strcat | 242.1 | 78.8 | 173.1 | 79.7 | 181.2 | 184.0 |
| hashmap | 256.4 | 75.8 | 190.1 | 78.2 | 223.5 | 259.6 |
| sort | 241.8 | 81.3 | 226.0 | 78.6 | 218.1 | 329.8 |
| actor_pingpong | 243.6 | 77.4 | 255.3 | 78.2 | 264.7 | 288.4 |
| btree | 241.4 | 81.5 | 225.0 | 77.4 | 195.1 | 257.5 |
| hashmap_str | 248.0 | 83.9 | 222.1 | 76.7 | 222.3 | 265.3 |
| dyn_dispatch | 245.5 | 74.7 | 76.3 | 79.6 | 174.7 | 190.8 |
| region_alloc | 248.9 | 77.0 | 77.0 | 80.0 | 176.8 | 237.6 |
| nqueens | 239.4 | 78.9 | 227.0 | 82.2 | 188.4 | 283.7 |


## 相对速度（以 Zeta = 1.0 为基准，比值越高表示比 Zeta 快）

| 基准 | Zeta | C | C++ | Go | Rust | Swift |
|------|-----|-----|-----|-----|-----|-----|
| fib | 1.0x | 1.2x | 1.2x | 0.9x | 1.1x | 0.9x |
| loop_sum | 1.0x | 1.4x | 1.7x | 0.1x | 1.4x | 0.1x |
| matmul | 1.0x | 1.0x | 1.0x | 1.0x | 1.1x | 0.9x |
| strcat | 1.0x | 1.4x | 1.2x | 1.0x | 1.1x | 0.8x |
| hashmap | 1.0x | 2.1x | 1.2x | 0.6x | 1.8x | 1.1x |
| sort | 1.0x | 1.3x | 1.0x | 1.0x | 1.1x | 1.0x |
| actor_pingpong | 1.0x | 1.8x | 1.9x | 28.3x | 2.2x | 2.1x |
| btree | 1.0x | 1.4x | 1.3x | 1.2x | 1.4x | 1.1x |
| hashmap_str | 1.0x | 1.5x | 1.7x | 1.4x | 1.4x | 1.6x |
| dyn_dispatch | 1.0x | 1.3x | 1.3x | 2.9x | 5.6x | 0.6x |
| region_alloc | 1.0x | 3.7x | 3.3x | 2.5x | 2.5x | 2.4x |
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
