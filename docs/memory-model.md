# Rlyeh 内存模型规范

> 版本：0.1.0  
> 最后更新：2026-08-24

> **⚠️ 实现状态**：L1 区域系统（§3）**已实现**（bump 分配 + 批量释放 + `adaptive`/`with_size`/`strategy (bump)` +
> **运行时接线（L3 ✅）**——region 指令调用 `rlyeh-region-alloc` C ABI 层（`rlyeh_region_enter`/`rlyeh_region_alloc`/
> `rlyeh_region_transfer`/`rlyeh_region_exit`），聚合对象 `in 'r` 经 bump 分配器分配（值镜像浅拷贝，不注册析构），
> `transfer x out of 'r` 标记所有权移出；PGO 数据回灌（F2，`.rl_profile`，简化格式）注入 `adaptive` 初始容量）；
> L0（§2）已实现（值拷贝/移动语义 + 方法接收者 + `&`/`&mut` 引用与宽松借用检查 G1 ✅ + `Box` K2 ✅；
> `Copy` trait 规划中）；L2 引用计数（§4）已实现（K3 ✅：`Rc`/`Arc`/`Weak`，含 `strong_count`/`downgrade`/
> `try_unwrap`）；L3 可选 GC（§5）已实现（K4 ✅：保守标记-清除 `Gc<T>`，多线程/增量回收规划中）。
> 区域用法见 [`guide.md`](./guide.md) §8。

## 相关文档

| 类型 | 文档 | 说明 |
|------|------|------|
| 项目总纲 | [CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 |
| 设计文档 | [04_所有权与借用检查器](design/04_所有权与借用检查器.md) / [05_区域内存管理系统](design/05_区域内存管理系统.md) | 模块设计 |
| 实现纪要 | [附录 A](#附录-a实现纪要)（原 P004/P005/P010，已归档至 [design/prompts/](design/prompts/)） | 区域系统落地状态 |

---

## 1. 概述

Rlyeh 提供**分层内存管理**，从底层到高层依次为：

| 层级 | 名称 | 机制 | 开销 | 确定性 |
|------|------|------|------|----------|
| L0 | 静态所有权 | 编译期分析 | 零 | 完全确定 |
| L1 | 区域 (Region) | Bump allocator + 批量释放 | 极低 | 区域边界确定 |
| L2 | 引用计数 | Rc/Arc | 原子操作 | 不确定 |
| L3 | 追踪 GC | 标记-清除 | GC 停顿 | 不确定 |

---

## 2. L0：静态所有权

### 2.1 核心规则

1. 每个值有且仅有一个所有者。
2. 所有者离开作用域时，值被销毁。
3. 赋值和传参默认转移所有权（move）。
4. 实现了 `Copy` 的类型除外。

### 2.2 内存布局

**栈分配**（大小编译期已知）：
```
Stack Frame:
┌──────────────┐
│  Return Addr │
├──────────────┤
│  Local Vars  │ ← 编译器确定偏移量
├──────────────┤
│  Arguments   │
└──────────────┘
```

**堆分配**（大小运行时确定；`Box` 已实现（K2 ✅，含 `Box::leak` T3a ✅ 返回 `*mut T`），堆分配经 std `Vec`/`String` 内部完成）：
```rlyeh
let boxed = Box::new(42);  // 目标语法：堆上分配 8 字节
// boxed 本身在栈上（指针 + 元数据），指向堆上的数据
```

### 2.3 Drop 顺序

变量按**逆声明顺序**析构：

```rlyeh
{
    let a = Resource::new("a");
    let b = Resource::new("b");
    let c = Resource::new("c");
}
// 析构顺序：c → b → a
```

---

## 3. L1：区域系统

### 3.1 设计目标

- 大量临时对象的高效管理
- 批量分配、批量释放
- 编译期可推断大部分大小
- 运行时自适应兜底

### 3.2 内存布局

```
Region Memory:

Block 1 (初始块, 64KB)
┌──────────────────────────────┐
│  bump_pointer →             │
│  ┌─────────┐                │
│  │ Object A │                │
│  ├─────────┤                │
│  │ Object B │                │
│  ├─────────┤                │
│  │ Object C │                │
│  ├─────────┤                │
│  │   ...    │                │
│  ├─────────┤                │
│  │ free     │                │
│  └─────────┘                │
└──────────────────────────────┘
         │
         ▼ (扩容时)
Block 2 (128KB)
┌──────────────────────────────┐
│  Object D │ Object E │ ...  │
└──────────────────────────────┘
         │
         ▼
Block 3 (256KB)
┌──────────────────────────────┐
│  ...                        │
└──────────────────────────────┘
```

### 3.3 Bump Allocator 算法

```rust
struct BumpAllocator {
    start: *mut u8,
    current: *mut u8,  // bump pointer
    end: *mut u8,
}

impl BumpAllocator {
    fn allocate(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        // 对齐 current 指针
        let aligned = align_up(self.current, align);
        let new_current = aligned + size;
        
        if new_current <= self.end {
            self.current = new_current;
            Some(aligned)
        } else {
            None  // 空间不足，需要扩容
        }
    }
    
    fn reset(&mut self) {
        self.current = self.start;
    }
}
```

**性能**：每次分配仅 3-5 条指令，无锁、无链表查找。

> **codegen 内联 bump 快路径**：LLVM 后端对 `AllocInRegion` 生成内联 bump 序列（对齐 + 越界检查 +
> 指针递增），**不调用运行时函数**，仅越界时才 `call rlyeh_region_alloc`（慢路径扩容）。为进一步消除
> 热循环中每次 bump 对 Region 头（base/cursor/limit 三字段）的访存，codegen 实施**循环级 region 状态提升**
> （见 [附录 A.4](#a4-循环级-region-状态提升2026-08-24) 单 bump 点与 [附录 A.6](#a6-批量提升多-bump-点聚合2026-08-24)
> 多 bump 点批量聚合）：preheader 快照 → header phi 维护提升状态 → latch 快路径零访存（仅寄存器运算）→
> 循环退出写回 cursor 一次。

### 3.4 析构管理

```rust
struct Region {
    blocks: Vec<MemoryBlock>,
    current_block: usize,
    destructors: Vec<DestructorEntry>,  // 需要析构的对象列表
}

struct DestructorEntry {
    ptr: *mut u8,
    drop_fn: unsafe fn(*mut u8),
}

impl Region {
    /// 区域结束时调用
    unsafe fn destroy(&mut self) {
        // 1. 逆序调用所有析构函数
        for entry in self.destructors.iter().rev() {
            (entry.drop_fn)(entry.ptr);
        }
        
        // 2. 重置析构列表
        self.destructors.clear();
        
        // 3. 释放所有内存块
        for block in self.blocks.drain(..) {
            deallocate(block.start, block.size);
        }
    }
}
```

### 3.5 扩容策略

```rust
enum GrowthStrategy {
    /// 精确大小，不允许扩容
    Exact(usize),
    
    /// 固定倍数扩容
    Multiply(f64),  // 如 2.0 表示每次翻倍
    
    /// 线性增长
    Linear(usize),  // 每次增加固定大小
    
    /// 自适应（基于历史数据）
    Adaptive {
        min_size: usize,
        max_size: usize,
        growth_history: Vec<usize>,
    },
}

impl GrowthStrategy {
    fn next_size(&self, current_size: usize, needed: usize) -> usize {
        match self {
            GrowthStrategy::Exact(n) => *n,
            GrowthStrategy::Multiply(factor) => {
                max(*factor * current_size as f64, needed as f64 * 2.0) as usize
            }
            GrowthStrategy::Linear(inc) => {
                current_size + inc
            }
            GrowthStrategy::Adaptive { .. } => {
                // 基于历史增长率预测
                // 详见 3.6 节
            }
        }
    }
}
```

### 3.6 自适应算法

```rust
struct AdaptiveAllocator {
    blocks: Vec<MemoryBlock>,
    current: usize,
    growth_history: Vec<GrowthEvent>,
    allocation_samples: Vec<usize>,  // 每次分配的采样
}

struct GrowthEvent {
    timestamp: Instant,
    old_size: usize,
    new_size: usize,
    reason: GrowthReason,
}

enum GrowthReason {
    BlockFull { requested: usize, available: usize },
    HintFromProfiler { suggested: usize },
}

impl AdaptiveAllocator {
    fn calculate_next_size(&self, needed: usize) -> usize {
        let n = self.growth_history.len();
        
        if n == 0 {
            // 首次扩容：基于当前分配模式猜测
            let avg_obj = self.average_object_size();
            let estimated_count = self.allocation_samples.len() * 2;  // 假设翻倍
            max(avg_obj * estimated_count, needed * 4)
        } else if n < 3 {
            // 数据不足：保守翻倍
            max(self.current_block_size() * 2, needed * 2)
        } else {
            // 使用指数加权移动平均 (EWMA)
            let alpha = 0.3;  // 平滑因子
            let mut ewma = self.growth_history[0].new_size as f64;
            
            for event in &self.growth_history[1..] {
                let ratio = event.new_size as f64 / event.old_size as f64;
                ewma = alpha * (ewma * ratio) + (1.0 - alpha) * ewma;
            }
            
            // 加上安全余量
            let predicted = (ewma * 1.5) as usize;
            max(predicted, needed * 2)
        }
    }
    
    fn average_object_size(&self) -> usize {
        if self.allocation_samples.is_empty() {
            return 64;  // 默认猜测
        }
        self.allocation_samples.iter().sum::<usize>() / self.allocation_samples.len()
    }
}
```

### 3.7 编译期大小推断

编译器在 HIR 阶段分析区域内的分配：

```rlyeh
// 源码
region 'r {
    let a = Point::new() in 'r;     // 大小已知：32 bytes
    let b = String::new() in 'r;    // 大小已知：24 bytes（堆指针 + len + cap）
    for i in 0..<100 {
        let item = Item::new(i) in 'r;  // 循环：100 × 48 bytes
    }
}
```

编译器生成的中间表示：

```rust
// 编译器推断的区域描述
RegionPlan {
    scope: "src/main.rs:42",
    static_size: Some(4856),  // 32 + 24 + 100*48 = 4856
    dynamic_factor: None,       // 无动态不确定因素
    strategy: GrowthStrategy::Exact(4856),
}
```

### 3.8 PGO 数据格式

> **实现状态**：✅ 已实现（F2，`rlyeh profile` + `rlyeh build --profile`）。实际格式为 `.rl_profile`
> JSON（含各区域 size_stats 的 p50/p95/mean/max 等），建议初始容量 = p95×1.1（下限 64KiB）。
> 与下述目标格式一致（示例中 `min/max/avg/p50/p95/p99` 字段实现为 `min/max/mean/p50/p95/p99`）。

```json
{
  "version": 1,
  "regions": {
    "src/main.rs:42:process_data": {
      "call_count": 15234,
      "size_stats": {
        "min": 4096,
        "max": 1048576,
        "avg": 262144,
        "p50": 131072,
        "p95": 786432,
        "p99": 983040
      },
      "growth_stats": {
        "avg_growth_count": 2.3,
        "avg_growth_factor": 2.8,
        "max_growth_factor": 4.0
      }
    }
  }
}
```

编译器使用 P95 值作为 `initial_size`，确保 95% 的调用不需要扩容。

---

## 4. L2：引用计数

> **实现状态**：`Rc<T>`/`Arc<T>` 已实现（K3 ✅：编译器内建，`Rc::new`/`clone`/`strong_count`/`weak_count`/`downgrade`/`try_unwrap`/`Weak::upgrade` + 解引用/字段/方法/索引自动剥层）。
>
> **MVP 布局注记**：规范语义布局为 `RcInner = { strong, weak, value }`；MVP 编译器实现改为 **`T` 值区自堆首槽起（槽 0..`n`，`n = slot_count(T)`，与 `Box<T>` 同构，剥层零差异）+ 尾部计数槽（槽 `n` = strong、槽 `n + 1` = weak）**，分配 `(n + 2)` 个 8 字节槽。计数槽为普通整数（8 字节槽，FieldScalar::Int）：`Arc` 与 `Rc` 同构，`AtomicUsize` 原子性规划中。MVP 无自动 drop（编译器不生成析构代码），计数只增不减，显式释放语义规划中（与 `Box`/`Vec`/`String` 一致）。

### 4.1 Rc<T>（单线程）

```rust
struct Rc<T> {
    ptr: *mut RcInner<T>,
}

struct RcInner<T> {
    strong_count: usize,  // 非原子（单线程）
    weak_count: usize,
    value: T,
}

// clone: strong_count += 1
// drop:  strong_count -= 1, 若为 0 则释放
```

### 4.2 Arc<T>（多线程）

```rust
struct Arc<T> {
    ptr: *mut ArcInner<T>,
}

struct ArcInner<T> {
    strong_count: AtomicUsize,  // 原子操作
    weak_count: AtomicUsize,
    value: T,
}

// clone: fetch_add(1, Acquire)
// drop:  fetch_sub(1, Release), 若为 0 则释放
```

### 4.3 循环引用检测

Rlyeh 提供 `Weak<T>` 打破循环：

```rlyeh
struct Node {
    parent: Weak<Node>,    // 不增加引用计数
    children: Vec<Rc<Node>>,
}
```

---

## 5. L3：可选 GC

> **实现状态**：✅ MVP 已实现（K4，`rlyeh-gc-runtime` 保守标记-清除）。
> 目标设计（Immix 增量标记-清除 + 复制、write barrier、逃逸限制）见下，MVP 注记见 §5.3。

### 5.1 设计（目标）

- 采用 Immix 算法（增量标记-清除 + 复制）
- 仅在显式启用的代码块中可用
- 与 L0/L1 对象互不干扰

```rlyeh
// 在 GC 区域内分配
gc_region {
    let obj = Gc::new(ExpensiveObject::new());
    // obj 由 GC 管理
    // 离开 gc_region 时触发 GC 周期
}
```

### 5.2 安全边界（目标）

- GC 管理的对象不能包含非 GC 管理的指针
- GC 对象与区域对象之间的引用需要 write barrier

### 5.3 MVP 注记（K4，2026-08）

当前实现为**保守标记-清除**（非增量、非复制），目标 API 与设计细节见上。

**布局与生命周期协议**（与编译器内建 `Gc<T>` 一致，实现见 `rlyeh-gc-runtime`）：

- `Gc<T>` 栈上 1 槽（Ptr）指向堆 **1-槽包装**（`Alloc{slots:1}`，槽 0 存 `GcInner` 基址）；对象 = `slot_count(T)` 个 8 字节槽的连续堆块，`T` 值区自堆首槽起，与 `Box<T>` 完全同构（解引用 / 字段 / 方法 / 索引剥层零差异）。
- `gc_region` 块 desugar 为固定调用序列：`rlyeh_gc_region_begin()`（`epoch += 1`）→ `rlyeh_gc_alloc(n)`（malloc 对象 + 注册块表 + 记录当前 epoch）→ `rlyeh_gc_escape(ptr)`（块返回值登记逃逸 root，空指针空操作）→ `rlyeh_gc_collect()`（从逃逸 root 标记 → 清除 `epoch` 匹配的未标记对象 → 存活对象提升为 root → `epoch -= 1`）。
- **epoch 分层**：块外分配对象 `epoch` 恒小于任何块 → 永不回收（MVP 泄漏语义）；块内对象仅回收本块未标记的；跨块存活的引用链经"存活提升"连续保护（内层逃逸对象在后续块 collect 中为 root）。
- **保守扫描**：值区内任意槽位值等于已注册对象基址即视为引用（线性查找），标量误判为安全漏回收。

**已知限制**（与目标设计的差距）：

- 非增量（stop-the-world）、单线程无锁（全局状态 `SyncUnsafeCell`，编译产物为单线程串行调用约定）；多线程 GC（全局锁 / 线程局部堆）规划中。
- 递归标记（DFS），深引用图可能爆栈。
- `gc_region` 结束后块内对象失效（逃逸限制，无悬空指针防护）；跨块逃逸对象及其引用图泄漏至程序结束。
- 无 write barrier（保守扫描规避精确性要求）。
- 分配器约束：运行时全部动态内存**只用 `libc::malloc` / `libc::free`**（对象块 + 元数据链表），不用 Rust 堆分配与 `realloc`——macOS C 主程序环境实测 `RawVec` / `realloc` 在 `libc::malloc` 之后调用触发 `libsystem_malloc` 的 `mfm_alloc` 崩溃（`_os_unfair_lock_unowned_abort` / SIGKILL），详见 `rlyeh-gc-runtime` 模块头注释。

---

## 6. 跨层级交互

### 6.1 层级转换规则

| 转换 | 允许 | 机制 |
|------|------|------|
| L0 → L1 | ✅ | `in 'r` 关键字 |
| L1 → L0 | ✅ | `transfer ... out of` |
| L0 → L2 | ✅ | `Rc::new(x)` |
| L2 → L0 | ✅ | `Rc::try_unwrap(x)` |
| L1 → L2 | ⚠️ | 需要先 transfer 到 L0 |
| L2 → L1 | ❌ | 不允许（Arc 生命周期不确定） |
| 任何 → L3 | ✅ | `Gc::new(x)` |
| L3 → 任何 | ❌ | GC 对象不能逃逸 |

### 6.2 Transfer 安全性证明

```
定理：transfer x out of 'r 是安全的

证明：
1. x 在区域内分配，区域拥有 x 的所有权
2. transfer 从区域的析构列表中移除 x
3. 区域的内存分配器标记 x 的位置为"已迁出"
4. x 的所有权转移到接收方
5. 区域结束时，不会释放 x 的内存（已被标记）
6. 接收方负责最终释放 x
7. 因此不存在悬空指针或双重释放
```

---

## 7. 性能模型

### 7.1 分配延迟

| 操作 | 平均延迟（CPU cycles） |
|------|------------------------|
| 栈分配 | 0-1 |
| 区域分配（bump） | 3-5 |
| malloc（glibc） | 50-200 |
| Rc::new | 30-80 |
| Arc::new | 50-150 |
| GC 分配 | 10-30（但分摊后有 GC 停顿） |

### 7.2 释放延迟

| 操作 | 延迟 |
|------|------|
| 栈释放 | 0（栈指针移动） |
| 区域释放（无析构） | O(1)（重置指针） |
| 区域释放（有析构） | O(k)，k = 需析构的对象数 |
| 逐个 free | O(n)，n = 对象数 |
| GC 周期 | 不确定（毫秒级停顿） |

### 7.3 内存开销

| 机制 | 每对象开销 |
|------|------------|
| 栈 | 0 |
| 区域 | 0（仅 bump 指针） |
| malloc | 8-16 bytes（堆元数据） |
| Rc | 16 bytes（计数 + 对齐） |
| Arc | 16 bytes + 原子操作开销 |
| GC | 8-16 bytes（标记位 + 对齐） |

### 7.4 区域循环分配实测（2026-08-24）

基准用例（`examples/projects/benchmarks/region_alloc/region_alloc.rl`，100 万次循环、每次 32B 聚合对象 `in 'r` bump 分配，
30 次运行取中位数，Apple Silicon arm64 / clang -O3）：

| 版本 | 100 万次 bump 耗时 | 热循环 Region 头访存/迭代 |
|------|--------------------|----------------------------|
| 优化前（无循环提升） | 6.847 ms | 3 次（`ldp [x19,#8]` + `ldr [x19]` + `str [x19,#8]`） |
| 优化后（循环提升，单块不扩容） | 3.258 ms | 0 次（仅慢路径 reload + 循环退出写回 1 次） |

**结论**：循环级 region 状态提升将 bump 分配热路径的 Region 头访存完全消除，分配本身收敛为纯寄存器运算
（对齐 `and` + 越界 `cmp` + 基址 `add`）。上表"优化后"为热循环零访存的**理论形态**（单块不扩容场景）；
**完整语义**（含附录 A.5 慢路径 cursor 回写修复、13 次真实扩容）下的最终基准见下表。

#### Rlyeh 各 region 策略对比（同机同构 100 万次 32B 对象，10 次取中位数）

| 策略 | 语法 | 块数（扩容次数） | 内存峰值 | 耗时 (ms) | 较最优 |
|------|------|:---:|:---:|-----:|:---:|
| plain（默认 bump） | `region 'r {}` | 14（13 次，1B→17.5MB） | 17.5MB | 5.512 | +3.4% |
| adaptive（EWMA 画像） | `region 'r adaptive {}` | 21（20 次，调优初值） | ~17.5MB | 5.558 | +4.3% |
| `with_size (32MB)` | `region 'r with_size (33554432) {}` | 1（0 次） | 32MB | **5.331** | — |
| `with_size (4KB)` | `region 'r with_size (4096) {}` | 9（8 次） | 26.8MB | 5.434 | +1.9% |
| `strategy (bump)`（显式 bump） | `region 'r strategy (bump) {}` | 14（13 次） | 17.5MB | 5.420 | +1.7% |

对照基线：空进程（无 region）2.755ms、空 `region` 3.202ms、空 `with_size(32MB)` 2.993ms——region 进出开销 < 0.5ms；
单次 bump 分配成本约 **2.8 ns**（扣除进程基线后 1M 次分配 ~2.76ms）。

**洞察**：

1. **五种策略耗时全部落在 5.33–5.56ms（~4% 内）**——循环级状态提升后热循环分配为纯寄存器 bump，
   策略差异只作用于慢路径扩容次数，被热路径完全掩盖；`adaptive` 的慢路径 EWMA 记账（20 次 vs 13 次扩容）
   造成 < 0.3ms 的差距，扩容次数在提升后不再是性能敏感项。
2. `with_size (32MB)` 零扩容最快；`with_size (4KB)` 以最小预分配 + 8 次快速扩容居中——印证
   "预分配越大越好"的传统直觉在提升后不成立；日常代码可放心使用默认 `region`，仅在已知精确上限时
   用 `with_size (N)` 规避全部扩容。

#### 批量分配（多 bump 点，region_batch 基准：100 万循环 × 每次 4×32B 对象，真实写带宽对照）

| 实现 | 分配方式 | 耗时 (ms) | 较 C |
|------|----------|-----:|:---:|
| Rlyeh（批量提升，P4） | 寄存器 bump：单次溢出检查 + 4 派生 | 13.684 | **1.04x** |
| Rlyeh（批量 bump，P3，A/B 对照） | 每次迭代 Region 头访存 + 单次 bump | 13.39 | ~1.0x（内部对照） |
| Rust | 手动 bump + `write_volatile` | 13.265 | 1.00x |
| C | 手动 bump + `volatile` 写读 | 13.218 | 1.0x |
| C++ | 手动 bump + `volatile` 写读 | 13.220 | 1.00x |
| Go | 手动 bump（unsafe 指针写） | 13.531 | 1.02x |
| Swift | 手动 bump + escape 黑盒读 | 15.303 | 1.16x |

> 注：本表为同环境交替 A/B 对照（P3/P4 21 轮取中位，跨语言 7 轮取中位）。**对照修复说明**：旧 C/Rust/Swift
> 对照为 `malloc`/`free` 与 `Box::new`/class 分配循环——bump 内存不 escape 时 LLVM 证明 store 全为死代码并
> **整体消除**（C 热循环汇编只剩 10 条 SIMD 纯计算指令、零内存访问），测出 2.9/4.99/4.19ms 的「纯计算假数据」，
> 造成 Rlyeh 4.8x 的假差距。修复后各语言强制真实内存写（volatile / `write_volatile` / escape 黑盒），
> 128MB 线性写为带宽受限场景：**Rlyeh 与 C/C++/Rust 持平（1.04x，噪声内并列最快）**——批量提升已将
> 热循环压到内存带宽极限，逐 bump 检查全部摊薄。完整跨语言对比见
> [`benchmarks/README.md`](../examples/projects/benchmarks/README.md)。

---

## 附录 A：实现纪要

> **说明**：本节提炼自开发任务书（原 `prompts/P004`/`P005`/`P010`，2026-08-24 归档至 [`design/prompts/`](design/prompts/)），
> 记录 L1 区域系统的实际落地状态、关键决策与已知限制，供后续维护参考。

### A.1 区域系统运行时与检查器（对应 P004，2026-08-20 ✅）

- **运行时**（`rlyeh-region-alloc`）：bump 分配器（块链表 + bump 指针），支持默认增长、
  `adaptive`（EWMA 预测）、`with_size (N)`（精确预分配）、`strategy (bump)` 四策略；LIFO 析构
  （`DestructorRegistry`，region 退出按注册逆序调用）。
- **检查器**（`rlyeh-regionck`）：区域嵌套合法性、`in 'r` 对象归属、`transfer` 方向（§6.2）、
  区域存活期/引用逃逸（escape）检查。
- **落地偏差**：检查器未拆分 `escape.rs`/`transfer.rs`，全部并入 `checker.rs`；`GrowthStrategy`
  以 struct 变体承载四策略；`Region::new()` 无参 + 默认增长参数；错误 `line`/`col` 占位 0；
  `in 'r` 在 AST 中以包裹节点（`InRegion`）表达；引用逃逸检查待 HIR 引入引用节点后启用（防御性）。

### A.2 Transfer 语义（对应 P005，2026-08-20 ✅）

- **运行时**：`Region::execute_transfer<T>(&mut self, ptr) -> &'static mut T`（移除析构 + 标记已迁出 +
  返回所有权句柄）；`is_transferred(ptr)` 状态查询；`mark_transferred` 为底层簿记入口。
- **检查器**：`OuterRegionTransfer`（transfer 源必须是当前活跃区域栈栈顶，内层转外层报错）；
  `PartialTransfer`（transfer 非变量表达式/调用结果，无法静态判定归属）。
- **落地偏差 / 已知限制**：
  - `execute_transfer` 归属 region 侧（原稿设计在 TransferChecker 内；运行时操作涉 `DestructorRegistry`
    内部字段，放 Region 更内聚）；
  - 未公开 `remove_destructor`（`mark_transferred` 内部完成"移除析构 + 标记"，保留单一入口防泄漏态）；
  - `CannotTransferReference`/`UnsizedTransfer` 为**防御性变体**（typecheck 对 `&`/字段/元组索引
    直接报 Unsupported，这些语法无法到达 regionck）；
  - **transfer 后内存语义（ADR-003）**：零拷贝，对象仍位于区域块内；接收方须在区域 `destroy` **之前**
    读取/修改/移出；销毁后句柄失效（真实"搬移到外部堆"由 MIR/代码生成阶段完成，规划中）；
  - `use-after-move` 检查依赖借用检查器（semantics.md 附录 A.2），MVP 未覆盖。

### A.3 智能区域分配器（对应 P010，2026-08-20 ✅）

- **架构**：`StaticSizer`（编译期静态大小推断）→ `PgoAdvisor`（Profile 画像）→ `SizeAdvisor`
  （综合决策初始容量）→ `AdaptiveBumpAllocator`（bump + 链表扩容 + EWMA 预测 + 碎片统计）→
  `StatsCollector`（分配/扩容统计 + PGO 数据输出）。
- **落地**：静态大小推断 + PGO 画像回灌 `adaptive` 初始容量 + EWMA 自适应扩容 + 碎片统计 +
  criterion 基准；性能测试 28 项全过。

### A.4 循环级 region 状态提升（codegen 优化，2026-08-24 ✅）

- **动机**：`AllocInRegion` 在 LLVM 后端生成内联 bump 快路径（对齐 + 越界检查 + 指针递增），仅越界时
  `call rlyeh_region_alloc`。但慢路径 `Region::grow()`（`rlyeh-region-alloc/src/region.rs`）扩容时会改写
  Region 头的 base/cursor/limit 三字段——LLVM 无法证明 call 不写 Region 头，LICM 失效，热循环每次 bump
  均从内存重读三字段（反汇编为 `ldp [x19,#8]`/`ldr [x19]`/`str [x19,#8]` 3 次访存/迭代）。
- **方案**（`rlyeh-codegen/src/llvm.rs`）：识别规范 while 循环并做循环级状态提升——
  - **保守命中条件**（`find_loop_promo`）：恰一条回边（`dom[src] ∋ dst`，迭代数据流 O(n²·E)）、非自循环、
    唯一 preheader、体内无 `RegionEnter`/`RegionExit`、无嵌套回边、latch 恰一个 `AllocInRegion` bump、
    恰一条退出边（源为 header、退出目标唯一前驱为 header）。
  - **preheader 快照**：terminator 前 load handle → `%p{base,cur,lim}` SSA 值。
  - **header phi**：`%h{base,cur,lim} = phi [ %p.., %pre ], [ %n.., %latch ]`，循环内状态完全寄存器化。
  - **latch bump**：快路径零访存（对齐 `and` + 越界 `cmp %new, %hlim` + 基址 `add %hbase`）；慢路径
    load handle + call + reload 三字段；join 处 phi 回传快/慢路径新状态（`%n{base,cur,lim}`）。
  - **退出写回**：循环退出块入口 `store %hcur, cursor`，Region 头状态最终一致。
- **关键修复**：header phi 的回边 entry 必须引用 LLVM CFG 中 latch 的**真实回边块**——bump split
  （fast/slow/join/cont 多 label）后为预分配的 join/cont label（发射 header 时 `self.label()` 回填
  `PromoCtx`），而非 LIR 块号；否则 clang verifier 报 `PHI node entries do not match predecessors` /
  `Instruction does not dominate all uses`（初版即踩此坑）。
- **验证**：单块不扩容场景 100 万次 32B bump 实测 6.847 ms → 3.258 ms（中位数 30 次，约 2.1 倍，见 §7.4）；
  热循环反汇编零 `[x19]` 访存；完整语义（含 A.5 修复、真实扩容）下五种策略 5.33–5.56ms，见 §7.4 策略对比表；
  97 个集成测试全通过，无回归。

### A.5 慢路径 cursor 回写修复（2026-08-24 ✅）

- **缺陷**：循环提升后 bump 状态完全寄存器化，但寄存器 `%cur` 仅在循环**退出**时写回 Region 头一次。
  当慢路径被触发（`call rlyeh_region_alloc`，如 plain 第 14 块、自适应第 21 块）时，运行时基于 Region 头中
  **过时的 cursor/limit** 判定空间，可能返回与既有寄存器状态重叠的地址（或误判容量不足重复扩容），
  语义错误且内存利用率失真。实测（调试日志）：block_count 停在 9、总容量 72KB、used≈48KB 但循环 100 万次
  （应为 14 块、17.5MB），峰值 RSS 仅 2.3MB（应为 ~34MB）。
- **修复**（`rlyeh-codegen/src/llvm.rs`，`emit_region_bump_promoted`）：在慢路径 `call rlyeh_region_alloc`
  **之前**先 `store i64 %hcur_{hdr}, i64* %{cv}` 将寄存器状态写回 Region 头，令运行时基于最新状态扩容，
  call 返回后 reload 三字段继续提升。快路径与循环退出写回不变。
- **验证**：修复后 100 万次循环慢路径恰好 15 次调用（14 块扩容 + 1 次对齐补块），峰值 RSS 34.6MB；
  200 万循环用例输出与 plain/ws32m 逐值一致（`999000000`）；97 个集成测试全通过。
- **调试备忘**：`Region` 的 `base`/`cursor`/`limit` 为私有字段（E0616），运行时诊断经公开
  `capacity()`/`used()`/`block_count()` 读取。

### A.6 批量提升（多 bump 点聚合，2026-08-24 ✅）

- **动机**：A.4 单对象提升覆盖循环内**单个** bump 点；批量场景（每次迭代分配多个对象）下每个 bump
  点仍独立发射独立对齐/越界检查/指针推进，Region 头访存与检查冗余、代码体积膨胀。
- **实现**（`rlyeh-codegen/src/llvm.rs`）：
  - `find_loop_promo` 收集 latch 内**同 region 的全部** `AllocInRegionDirect` bump（多 bump 批量提升），
    提升状态**整组共享一组 phi**（`%hcur_{hdr}`/`%hbase_{hdr}`/`%hlim_{hdr}`）；
  - 发射期 `try_emit_region_promo_batch` 窗口聚合：首个 bump 做一次对齐 + 一次越界检查 + 一次推进
    `total = Σ size`，各对象经 `gep` 派生（`__tmp0 = base+off0`、`__tmp1 = base+off0+size0` …）；
    慢路径一次 `call rlyeh_region_alloc` 扩容；
  - 窗口收集与 P3 批量 bump（`try_emit_region_batch`）共享穿插跳过规则（`FieldSet`/`Assign`/`Binary`
    无副作用穿插可跳过），但**不排除** promo 命中。
- **约束**：仅 `AllocInRegionDirect`（值镜像 `AllocInRegion` 需 memcpy 发射，不聚合，散落 bump 与单发
  提升的回边 phi 重复定义冲突）；bump 全部位于 latch；bump 之间仅无副作用穿插（窗口兼容预检）。
- **关键修复（span 越界 bug）**：窗口兼容预检须限定在**首个至末个 bump 的区间内**——末个 bump 之后
  的 `FieldGet`（如 `sum = sum + x1.a + x2.a + x3.a + x4.a` 读取分配结果）属合法跟随，不得触发拒绝。
  初版 `in_span` 置位后不复位导致 span 扩展到块尾，**所有含字段读取的批量循环被整体拒绝**（批量基准
  19.5ms 退化至 P3 水平，IR 无 `%hcur_` phi）。
- **验证**：`region_batch` 基准（100 万循环 × 每次 4×32B 对象，`examples/projects/benchmarks/region_batch/`）；
  IR 确认一组 `%hcur_`/`%ncur_` phi 命中、慢路径 `rlyeh_region_alloc` 保持 2 处（扩容 + 对齐补块）；
  同环境交替 A/B（交替测 21 轮取中位）：**P4 批量提升 12.70 ms vs P3 批量 bump 13.39 ms（+5.5%）**；
  输出与 C/Go/Rust/Swift 对照严格一致（`2000497500000`）。
- **对照修复（DSE 假差距揭穿，2026-08-24 追加）**：旧跨语言对照为 `malloc`/`free`（C/C++）、
  `Box::new`（Rust）、class 分配（Swift）——bump 内存不 escape 时 LLVM 证明所有 store 为死代码并整体消除
  （C 热循环汇编仅剩 10 条 SIMD 纯计算指令、零内存访问），测出「纯计算假数据」（C 2.62/2.91ms、Rust 4.99ms、
  Swift 4.19ms），造成 Rlyeh 4.8x 假差距。修复：各语言对照统一为**手动 bump + 强制真实内存写**
  （C/C++ `volatile` 写读、Rust `write_volatile`/`read_volatile`、Go `unsafe` 指针写、Swift 指针写 + escape 黑盒读），
  与 region 线性 bump 语义对齐。修复后 128MB 线性写为带宽受限场景（§7.4 表）：**Rlyeh 13.684 vs
  C 13.218 / C++ 13.220 / Rust 13.265（1.04x，噪声内并列最快）**；P3/P4 内部 A/B（+5.5%）仍为真实收益。
  `region_alloc` 单对象场景修复后 Rlyeh 6.095 vs C 5.407（1.13x）——剩余差距为 region 语义必需的
  逐对象越界检查（~1ns/迭代），非 codegen 缺陷。

---

> **维护者**：Rlyeh Language Team  
> **License**：MIT / Apache-2.0
