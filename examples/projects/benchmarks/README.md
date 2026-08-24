# Zeta 性能对比基准（benchmarks）

Zeta 语言与 **C / C++ / Go / Swift / Rust** 的全方位性能对比用例集。每个基准在所有语言中实现**逻辑严格一致**（跨语言输出已逐项核对，run.py 运行期自动校验），统一编译、统一计时，公平对比。

## 基准维度（12 项）

| 基准 | 内容 | 考察点 |
|------|------|--------|
| `fib` | fib(30) 双递归 | 函数调用开销 + 整数运算 |
| `loop_sum` | 1 亿次 i64 循环累加 | 整数运算 + 分支 |
| `matmul` | 256×256 f64 矩阵乘法 | 浮点运算 + 内存访问 |
| `strcat` | 字符串拼接 10 万次 | 字符串缓冲扩容 |
| `hashmap` | 20 万 insert + 20 万 get（LCG i64 键） | 哈希表 |
| `sort` | LCG 生成 5000 个 i64 排序 | 排序算法 + 标准库 |
| `actor_pingpong` | 5 万次 actor 同步往返 | **Actor 并发模型**（消息调度吞吐） |
| `btree` | 深度 15 完全二叉树构造 + 递归求和 | 层次内存访问 + 递归 |
| `hashmap_str` | 1 万条字符串键 insert + get | 字符串哈希 + 运行时键构造 |
| `dyn_dispatch` | 2000 万次多态分派 | **dyn Trait / 虚函数 vtable** |
| `region_alloc` | 100 万次小对象分配 | **region 批量分配 vs 逐次分配** |
| `nqueens` | 12 皇后回溯搜索 | 深度搜索 + 递归 + 剪枝分支 |

## 目录结构

```
benchmarks/
├── run.py                  # 统一编译 + 计时 + 输出校验脚本
├── README.md
├── fib|loop_sum|matmul|strcat|hashmap|sort|
│   actor_pingpong|btree|hashmap_str|dyn_dispatch|region_alloc|nqueens/
│   ├── <name>.zeta         # Zeta 实现
│   ├── <name>.c            # C 实现（clang -O3）
│   ├── <name>.cpp          # C++ 实现（clang++ -O3）
│   ├── <name>.go           # Go 实现（go build）
│   ├── <name>.swift        # Swift 实现（swiftc -O）
│   ├── <name>.rs           # Rust 实现（rustc -O）
│   └── bench_<lang>        # 编译产物（按语言隔离）
└── results/
    ├── benchmark_report.md # 对比报告（自动生成）
    └── raw.json            # 原始数据（run_ms + compile_ms）
```

## 运行方式

```bash
python3 run.py                     # 完整跑 12 基准 × 6 语言
python3 run.py --runs 10           # 增加正式运行次数（更稳）
python3 run.py --only fib,matmul   # 只跑指定基准
python3 run.py --skip-zeta-build   # 跳过 Zeta 重编译（复用已有二进制）
```

计时策略：每基准每语言 **warmup 1 次 + 正式 5 次取中位数**（毫秒），进程启动开销各语言一致。
编译计时：单次全量冷编译（Zeta 用 `--force` 绕开增量缓存，其余语言无增量缓存）。
输出校验：构建后首次运行捕获各语言 stdout，跨语言不一致自动告警（浮点按数值容差）。

## 编译优化级别（如实声明）

| 语言 | 编译器/优化 | 说明 |
|------|------------|------|
| Zeta | `zeta build` | **clang -O3 发布级优化**（LLVM 循环优化/向量化/常量传播/寄存器分配） |
| C / C++ | `clang -O3` / `clang++ -O3` | 发布级优化 |
| Go | `go build` | gc 编译器默认优化（无 -O 分级） |
| Swift | `swiftc -O` | 发布级优化 |
| Rust | `rustc -O` | 发布级优化 |

## 最新结果摘要（Apple M5 Pro，2026-08-24，发布级优化后，含 Go 对比）

| 基准 | Zeta | C | C++ | Go | Rust | Swift | Zeta/最优 |
|------|-----:|---:|---:|---:|-----:|------:|:---:|
| fib (ms) | 4.645 | 3.872 | 3.839 | 5.090 | 4.215 | 5.126 | 1.21x |
| loop_sum (ms) | 3.719 | 2.619 | 2.233 | 26.337 | 2.704 | 25.670 | 1.67x |
| matmul (ms) | 13.306 | 12.724 | 12.756 | 13.639 | 12.590 | 14.291 | 1.06x |
| strcat (ms) | 3.286 | 2.408 | 2.752 | 3.198 | 2.993 | 3.886 | 1.36x |
| hashmap (ms) | 11.086 | 5.166 | 9.578 | 19.021 | 6.188 | 10.340 | 2.15x |
| sort (ms) | 3.431 | 2.637 | 3.310 | 3.292 | 3.003 | 3.387 | 1.30x |
| actor_pingpong (ms) | 341.140 | 186.265 | 179.796 | 12.045 | 157.111 | 166.406 | 28.3x |
| btree (ms) | 3.549 | 2.542 | 2.789 | 2.954 | 2.607 | 3.271 | 1.40x |
| hashmap_str (ms) | 5.297 | 3.587 | 3.155 | 3.851 | 3.764 | 3.400 | 1.68x |
| dyn_dispatch (ms) | 15.481 | 12.328 | 12.105 | 5.317 | 2.745 | 24.983 | 5.64x |
| region_alloc (ms) | 8.188 | 2.231 | 2.474 | 3.329 | 3.235 | 3.399 | **3.67x（vs C，较 5.92x 收窄）** |
| nqueens (ms) | 63.248 | 61.030 | 62.433 | 61.953 | 59.098 | 68.974 | 1.07x |

### 关键洞察

1. **12 项基准整体贴近 C**：`fib` 1.21x、`nqueens` 1.07x、`matmul` 1.06x；`strcat` 1.36x、`loop_sum` 1.67x（clang 对纯整数循环的强度削减优势）；`hashmap`（2.15x）与 `hashmap_str`（1.68x）落后各语言标准哈希表/键构造。
2. **Go 加入对比（go1.27.0，gc 编译器默认优化）**：
   - `loop_sum`（26.3ms）与 Swift（25.7ms）同量级——gc 编译器不对纯整数累加循环做 -O3 级强度削减/向量化，与 clang/rustc 拉开约 7–11x 差距；
   - `actor_pingpong`（12.0ms）**远超其它语言**（C 186ms）——无缓冲 channel 单生产者/消费者有锁竞争的 runtime 快速路径（~0.24μs/往返），而 pthread condvar 每次往返需 futex 唤醒；这反映 Go 并发原语的工程优化水平；
   - `dyn_dispatch`（5.3ms）快于 C（12.3ms）——Go 接口的 itab 间接调用 + 栈内对象逃逸优化后的低开销；
   - `region_alloc`（3.33ms）与 C++/Rust/Swift 同级——Go 逃逸分析将循环内对象栈分配/消除，未体现逐次分配成本。
3. **`region_alloc` 本轮大幅优化（5.92x → 3.67x vs C）**：内联 bump 快路径（codegen 直接读写 `Region` 首部 cursor/limit，仅溢出才走运行时扩容）+ 字面量直接构造（消除中间堆临时与值镜像 memcpy）+ 热路径去统计——Zeta 14.21ms → ~8.2ms（全量连跑下含系统噪声）。
4. **编译耗时（单次全量冷编译）**：Zeta 239–256ms/基准（LLVM 全量管线），vs C 75–84ms（约 3.1x）、Go 75–83ms（约 3.2x）、Rust 170–265ms、C++ 69–255ms、Swift 180–330ms——Zeta 处 C++/Swift 区间；「编译速度对标 Go」的目标需靠增量缓存 / 惰性 LLVM 后端兑现。
5. **剩余差距与后续方向**：`hashmap`（2.15x）可换 Robin Hood / 二次探测；`actor_pingpong`（28.3x）为 actor 运行时每次 ask 的调度+消息槽往返成本，可减少往返（批量/批处理协议）；`dyn_dispatch`（5.64x）Zeta vtable 分派可进一步内联化；`as f64` 数值转换 IR 支持（恢复 mandelbrot 复平面算力基准）。

## 复现与验证

- 逻辑等价性：所有语言输出逐项一致（`832040` / `4999999950000000` / `200000` / `19999900000` / sort 首尾值相同 / `1250025000` / `2166712927200` / `49995000` / `70000000` / `499500000` / `14200`）。
- 运行期自动校验：`run.py` 捕获各语言 stdout 对比，不一致即告警（浮点按数值容差，因各语言 f64 打印精度不同）。
- LCG 随机数使用统一常数（MMIX：A=6364136223846793005, C=1442695040888963407），Zeta 的 i64 乘法为 wrapping 语义（已实测），C/C++ 用无符号算术保证无 UB。
