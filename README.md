# Rlyeh — 系统级编程语言

> 版本：**0.1.0**（MVP，已收口） · **0.2.0**（自举能力补齐，规划中） · 状态：0.1.0 已收口，0.2.0 规划中
> 仓库：<https://gitee.com/cathra/rlyeh-language.git>
> 目标平台：Linux / macOS / Windows / WASM
> 实现语言：Rust（自举编译器，bootstrap 阶段用 Rust 实现）

Rlyeh 是一门面向未来十年基础设施的**系统级编程语言**：

| 目标 | 对标 |
|------|------|
| 内存安全、零 GC | Rust |
| 编译速度极快 | Go |
| 并发模型一等公民 | Erlang / Akka |
| 数学式语法直觉 | Python / MATLAB |
| 开发体验友好 | Go / Python |

**一句话定位**：Rlyeh = Rust 的安全性 + Go 的编译速度 + Erlang 的并发模型 + 数学的自然表达。

---

## 多语言交叉对比

Rlyeh 的全部对比均在**同一台机器、同一套基准、同一份源码逻辑**下实测（[`examples/projects/benchmarks/`](examples/projects/benchmarks/)），跨语言输出逐项自动核对，不存在「只跑一侧」的宣传数据。

### 1. 语言设计对比

与主流语言的**设计取向**对照（非性能，性能见 §2）：

| 维度 | **Rlyeh** | Rust | Go | Erlang / Elixir | Python |
|------|-----------|------|----|-----------------|--------|
| 类型系统 | 静态 · `protocol` + 泛型单态化 | 静态 · `trait` + 泛型单态化 | 静态 · `interface` + 泛型 | 动态（typespec 仅文档） | 动态 |
| 内存管理 | **分层可选**：编译期所有权 / 借用（L0）· region 批量释放（L1）· `Rc`/`Arc`（L2）· 可选 GC（L3） | 编译期所有权 / 借用 + `Rc`/`Arc`，无 GC | 并发三色标记 GC | 每进程独立堆 + GC | 引用计数 + 分代 GC |
| 并发模型 | **Actor 一等公民**（消息 + 监督重启）+ `sync` 原语（Mutex / RwLock / Atomic / channel） | 线程 + `Send`/`Sync` + `async` | goroutine + channel | Actor 进程 + 监督树 | GIL + `asyncio` / 线程 |
| 崩溃 / 容错 | Actor 监督策略自动重启（OneForOne / AllForOne / RestartForOne） | `panic` / `Result` 传播 | `panic` / `recover` | 「let it crash」+ 监督树 | 异常 |
| 数学式条件 | 比较链 `0 < x < 10`；`in` 集合 / 范围 / 时间字面量 `9am...6pm` | 无 | 无 | 无 | 链式比较（`0 < x < 10`） |
| 编译后端 | LLVM（clang -O3 发布级管线） | LLVM / Cranelift | 自研 gc 后端 | BEAM 字节码 | CPython 字节码 / JIT |

### 2. 运行时性能对比（13 基准 × 6 语言）

- **环境**：Apple M5 Pro（arm64，18 核）· macOS 27.0 · Apple clang 21.0.0 · Go 1.27.0 · Swift 6.4 · rustc 1.96.0
- **方法**：每基准每语言 warmup 1 次 + 正式 10 次取平均（ms，含进程启动）；各语言均开发布级优化（`clang -O3` / `clang++ -O3` / `go build` / `swiftc -O` / `rustc -O` / `rlyeh build`）
- **数据源**：[`results/benchmark_report.md`](examples/projects/benchmarks/results/benchmark_report.md)（2026-09-01 自动生成）

| 基准 | Rlyeh | C | C++ | Go | Rust | Swift |
|------|-----:|-----:|-----:|-----:|-----:|-----:|
| fib | 5.071 | 4.988 | 4.962 | **4.317** | 5.371 | 5.998 |
| loop_sum | 3.633 | 3.593 | **3.566** | 25.572 | 3.791 | 25.555 |
| matmul | **12.195** | 13.105 | 13.210 | 12.513 | 13.043 | 14.733 |
| strcat | 3.824 | 3.798 | 4.161 | **2.749** | 3.845 | 5.111 |
| hashmap | 7.539 | **6.519** | 9.133 | 16.581 | 6.662 | 9.144 |
| sort | 3.807 | 3.767 | 3.667 | **2.956** | 3.879 | 4.553 |
| actor_pingpong | **6.783** | 181.481 | 176.910 | 12.156 | 149.204 | 164.067 |
| btree | 4.052 | 3.836 | 3.787 | **2.825** | 4.001 | 3.759 |
| hashmap_str | 8.121 | 4.828 | 4.290 | **3.573** | 4.688 | 4.896 |
| dyn_dispatch | **3.762** | 13.083 | 13.223 | 4.994 | 3.775 | 25.666 |
| region_alloc | 6.793 | 6.248 | 3.882 | **3.065** | 4.423 | 4.717 |
| region_batch | 11.665 | 11.974 | 11.707 | **10.814** | 11.577 | 13.815 |
| nqueens | 59.428 | 58.341 | 59.268 | 58.912 | **56.220** | 67.079 |

*(单位 ms，越低越好；**加粗**为该行最快语言。)*

**相对速度**（以 Rlyeh = 1.0 为基准，>1 表示该语言比 Rlyeh 快）：

| 基准 | C | C++ | Go | Rust | Swift |
|------|-----:|-----:|-----:|-----:|-----:|
| fib | 1.0x | 1.0x | 1.2x | 0.9x | 0.8x |
| loop_sum | 1.0x | 1.0x | 0.1x | 1.0x | 0.1x |
| matmul | 0.9x | 0.9x | 1.0x | 0.9x | 0.8x |
| strcat | 1.0x | 0.9x | 1.4x | 1.0x | 0.7x |
| hashmap | 1.2x | 0.8x | 0.5x | 1.1x | 0.8x |
| sort | 1.0x | 1.0x | 1.3x | 1.0x | 0.8x |
| **actor_pingpong** | 0.0x | 0.0x | 0.6x | 0.0x | 0.0x |
| btree | 1.1x | 1.1x | 1.4x | 1.0x | 1.1x |
| **hashmap_str** | 1.7x | 1.9x | 2.3x | 1.7x | 1.7x |
| dyn_dispatch | 0.3x | 0.3x | 0.8x | 1.0x | 0.1x |
| **region_alloc** | 1.1x | 1.7x | 2.2x | 1.5x | 1.4x |
| region_batch | 1.0x | 1.0x | 1.1x | 1.0x | 0.8x |
| nqueens | 1.0x | 1.0x | 1.0x | 1.1x | 0.9x |

**结论**

- **3 项全场最快**：`actor_pingpong`（6.78ms，**超 Go 1.8x、超 C/Rust/Swift 22–27x**）、`dyn_dispatch`（3.76ms，与 Rust 持平、超 Go 1.3x）、`matmul`（12.20ms）。
- **约 1.0x 持平**：`loop_sum`、`nqueens`、`region_batch` 与最快语言差距在 1.1x 内。
- **相对落后项**：`hashmap_str`（2.3x，瓶颈在 `format!` 键构造的分配次数）、`region_alloc`（2.2x，差距主要是 region 语义必需的逐对象越界检查 ~1ns/迭代）、`strcat`/`btree`（1.4x）。
- `actor_pingpong` 中 C/C++/Rust/Swift 侧为**双线程双通道同步往返**（mutex/condvar、mpsc），Rlyeh 侧为 Actor ask 同步往返（同线程 fast path 零调度）——这正是 Actor 作为**语言一等公民**的价值。

**编译耗时**（单次全量冷编译，`rlyeh build --force` 绕开增量缓存）：

| | Rlyeh | C | C++ | Go | Rust | Swift |
|---|---:|---:|---:|---:|---:|---:|
| 区间 (ms) | 223–290 | 42–79 | 41–269 | 44–193 | 77–181 | 156–700 |

Rlyeh 处于 C++/Swift 区间；「编译速度对标 Go」的目标需靠**增量缓存 / 惰性 LLVM 后端**兑现（规划中）。

### 3. region 内存策略对比

同机同构 100 万次 32B 对象分配，10 次取中位数（[`examples/projects/benchmarks/README.md`](examples/projects/benchmarks/README.md)）：

| 策略 | 语法 | 扩容次数 | 内存峰值 | 耗时 (ms) |
|------|------|:---:|:---:|-----:|
| plain（默认 bump ×2） | `region 'r {}` | 13 | 17.5MB | 4.644 |
| adaptive（EWMA 画像） | `region 'r adaptive {}` | 20 | ~17.5MB | 4.698 |
| `with_size (32MB)` | `region 'r with_size (33554432) {}` | 0 | 32MB | 4.780 |
| `with_size (4KB)` | `region 'r with_size (4096) {}` | 8 | 26.8MB | 4.907 |
| `strategy (bump)` | `region 'r strategy (bump) {}` | 13 | 17.5MB | **4.625** |

对照基线：空进程 2.06ms、空 `region` 2.43ms（进出开销 0.37ms）；**无 region 的逐次堆分配 9.77ms —— region 分配约为其 1/2**。结论：热循环分配已被优化为纯寄存器 bump（约 **2.6 ns/次**），五种策略耗时落在 4.6–4.9ms、差异在噪声内，日常代码可放心用默认 `region`。

> 复现：`cd examples/projects/benchmarks && python3 run.py`（`--runs 10` 更稳，`--only fib,matmul` 指定基准）。

---

## 核心特性

### 分层内存管理（零 GC 起步）

| 层级 | 机制 | 关键字 | 运行时开销 |
|------|------|--------|------------|
| L0 | 静态所有权 + 借用 + 生命周期 | `let`, `&`, `&mut` | 零开销 |
| L1 | 区域（Region） | `region 'r {}`, `in 'r`, `transfer` | 零开销（批量释放） |
| L2 | 引用计数 | `Rc<T>`, `Arc<T>` | 原子操作 |
| L3 | 可选 GC | `Gc<T>`（插件式） | GC 停顿 |

### 数学式条件判断

```rlyeh
if 0 < x < 10 {}          // 区间内（正向比较链）
if 0 > x > 10 {}          // 区间外：x < 0 || x > 10
if ch in ('a'..<'z', 'A'..<'Z') {}   // 集合判断
if x not in (0..<10) {}   // x != 0 && ... && x != 9
if hour in (9am...6pm) {} // 时间字面量（分钟单位）
```

### Actor 并发模型（一等公民）

```rlyeh
actor Counter {
    value: i64 = 0,

    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }
}

let counter = Counter::new();              // 普通 spawn
let result = counter.increment(10).await;  // ask 同步往返
send counter.increment(1);                 // fire-and-forget

let supervised = Counter::new_supervised(0); // 受监督：崩溃后自动重建重启
```

### 区域系统

```rlyeh
region 'r {
    let data = BigStruct::new() in 'r;   // 区域内分配
    process(&data);
} // 批量释放

fn create_data() -> BigStruct {
    region 'r {
        let data = BigStruct::new() in 'r;
        return transfer data out of 'r;  // 所有权转移
    }
}
```

### 聚合对象与泛型

```rlyeh
enum Shape { Circle(f64), Rect { w: f64, h: f64 } }

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.14 * r * r,
        Shape::Rect { w, h } => w * h,
    }
}

protocol Area { fn area(&self) -> f64; }
impl Shape: Area { fn area(&self) -> f64 { /* ... */ } }
```

### 宏与闭包

```rlyeh
let v = vec![10, 20, 30];          // 集合宏 → Vec<T>
let m = map![1 => 10, 2 => 20];    // → HashMap<K, V>
let r = apply(|a, b| a + b, 10, 20);  // 无捕获闭包 → 函数指针（零开销）
let d: dyn Shape = &c;             // 协议对象：vtable 多态分派
```

### 更多已实现能力

- **函数一等值**：`fn(T) -> R` 类型、函数值绑定与间接调用、作实参/返回值
- **引用与借用**：`&T`/`&mut T`、裸指针 `*const T`/`*mut T`、严格借用检查（BorrowConflict / DanglingReference）、`ref`/`ref mut` 模式
- **错误传播**：`?` 运算符（Option / Result 上下文）
- **所有权层级**：`Box<T>` / `Rc<T>` / `Arc<T>` / `Gc<T>`（含 `gc_region` 保守标记-清除）
- **迭代器**：数组 / Vec / 自定义迭代器接入 `for`，`map`/`filter`/`fold`/`collect`/`take`/`skip` 适配器
- **模块系统**：`module` / `import`，多文件模块（扁平名字空间）
- **async/await**：`async fn` desugar 为 Future 状态机 + `block_on` 驱动
- **FFI 与 WASM**：`extern fn` 全链路打通；`wasm32-wasip1` 目标支持（actor 运行时 + WASI 单线程同步）
- **序列化**：`json::stringify` / `json::parse::<T>`；`toml::to_string` / `toml::from_str::<T>`（标量 / 数组 / Vec / 嵌套表 `[section]` / HashMap 内联表，支持 round-trip）
- **可恢复解析**：`json::try_parse::<T>` / `toml::try_parse::<T>` 返回 `Result<T, serde::JsonError | serde::TomlError>`（非法输入走 `Err`，`?` 传播）
- **格式化宏**：`println!` / `print!` / `format!` / `dbg!` / `eprintln!`

---

## 快速开始

### 构建

需要 Rust 工具链（stable），以及 `clang`（LLVM 汇编管线依赖）。

```bash
cargo build --release -p rlyeh-driver   # 构建 rlyeh CLI
```

### 运行第一个程序

```rlyeh
// hello-world.rl
fn main() {
    println("Hello, Rlyeh!");
}
```

```bash
rlyeh run examples/hello-world.rl     # 或 rlyeh build + 执行产物
```

### 运行测试

```bash
cargo test --workspace -- --test-threads=1
rlyeh test tests                       # 集成测试（.rl 用例，扫描 compile-pass / compile-fail / run-pass）
```

---

## 项目结构

```
rlyeh-language/
├── crates/                # 编译器各组件（Rust crate）
│   ├── rlyeh-lexer/        # 词法分析器
│   ├── rlyeh-parser/       # 语法分析器
│   ├── rlyeh-ast/          # 抽象语法树
│   ├── rlyeh-hir/          # 高级中间表示
│   ├── rlyeh-mir/          # 中级中间表示
│   ├── rlyeh-lir/          # 低级中间表示
│   ├── rlyeh-typecheck/    # 类型检查器
│   ├── rlyeh-borrowck/     # 借用检查器
│   ├── rlyeh-regionck/     # 区域检查器
│   ├── rlyeh-codegen/      # 代码生成（LLVM / Cranelift）
│   ├── rlyeh-driver/       # 编译器驱动（CLI 入口）
│   ├── rlyeh-std/          # 标准库（Rlyeh 源码）
│   ├── rlyeh-actor-runtime/# Actor 运行时
│   ├── rlyeh-region-alloc/ # 区域分配器（bump + 智能）
│   ├── rlyeh-gc-runtime/   # 可选 GC 运行时
│   └── rlyeh-lsp/          # 语言服务器（LSP）
├── tools/                 # rlyeh-fmt / rlyeh-check / rlyeh-doc / rlyeh-bench
├── dagon/                   # 包管理器
├── docs/                  # 权威规范 + 开发进度（入口：docs/README.md）
├── examples/              # 可运行示例（含 projects/benchmarks 多语言性能基准）
├── skills/                # rlyeh-language 技能（SKILL.md + references/，随工具链分发）
└── tests/                 # 集成测试（.rl 用例）
```

---

## 文档导航

| 文档 | 内容 |
|------|------|
| [docs/guide/index.md](docs/guide/index.md) | 语言教程（每章独立文档，示例均可运行） |
| [docs/manual/index.md](docs/manual/index.md) | 语言参考（每章独立文档：词法/类型/运算符/标准库 API/工具链命令速查） |
| [docs/manual/std/index.md](docs/manual/std/index.md) | 标准库详述（每个类型的成员/方法/用例） |
| [docs/tutorial/index.md](docs/tutorial/index.md) | 新手教程（安装 → 第一个程序 → 发布项目） |
| [docs/grammar.md](docs/grammar.md) | 完整语法规范（EBNF） |
| [docs/semantics.md](docs/semantics.md) | 语义规则 |
| [docs/memory-model.md](docs/memory-model.md) | 分层内存管理规范 |
| [docs/actor-model.md](docs/actor-model.md) | Actor 并发模型规范 |
| [docs/module-system.md](docs/module-system.md) | 模块系统规范 |
| [docs/std-lib.md](docs/std-lib.md) | 标准库 API 规范 |
| [docs/development-plan.md](docs/development-plan.md) | 开发计划（阶段 A–F + 剩余任务消解 G–L / M–T / U–Z） |
| [examples/projects/benchmarks/results/benchmark_report.md](examples/projects/benchmarks/results/benchmark_report.md) | 多语言性能基准完整报告（13 基准 × 6 语言） |
| [CHANGELOG.md](CHANGELOG.md) | 版本变更记录 |

---

## 许可证

Rlyeh 采用双许可证，使用者可任选其一：

- [MIT License](LICENSE-MIT)（<https://opensource.org/licenses/MIT>）
- [Apache License, Version 2.0](LICENSE-APACHE)（<https://www.apache.org/licenses/LICENSE-2.0>）

除非你明确另行说明，否则任何有意提交以纳入本项目的贡献，均按上述双许可证授权，且不附加任何额外条款或条件。
