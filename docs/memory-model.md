# Zeta 内存模型规范

> 版本：v2.0  
> 最后更新：2026-08-19

## 相关文档

| 类型 | 文档 | 说明 |
|------|------|------|
| 项目总纲 | [CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 |
| 设计文档 | [04_所有权与借用检查器](../04_所有权与借用检查器.md) / [05_区域内存管理系统](../05_区域内存管理系统.md) | 模块设计 |
| 实现任务 | [P004](../prompts/P004_区域系统实现.md) / [P005](../prompts/P005_Transfer语义实现.md) / [P010](../prompts/P010_智能区域分配器.md) | 区域系统实现 |

---

## 1. 概述

Zeta 提供**分层内存管理**，从底层到高层依次为：

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

**堆分配**（大小运行时确定）：
```zeta
let boxed = Box::new(42);  // 堆上分配 8 字节
// boxed 本身在栈上（指针 + 元数据），指向堆上的数据
```

### 2.3 Drop 顺序

变量按**逆声明顺序**析构：

```zeta
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

```zeta
// 源码
region 'r {
    let a = Point::new() in 'r;     // 大小已知：32 bytes
    let b = String::new() in 'r;    // 大小已知：24 bytes（堆指针 + len + cap）
    for i in 0..100 {
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

Zeta 提供 `Weak<T>` 打破循环：

```zeta
struct Node {
    parent: Weak<Node>,    // 不增加引用计数
    children: Vec<Rc<Node>>,
}
```

---

## 5. L3：可选 GC

### 5.1 设计

- 采用 Immix 算法（增量标记-清除 + 复制）
- 仅在显式启用的代码块中可用
- 与 L0/L1 对象互不干扰

```zeta
// 在 GC 区域内分配
gc_region {
    let obj = Gc::new(ExpensiveObject::new());
    // obj 由 GC 管理
    // 离开 gc_region 时触发 GC 周期
}
```

### 5.2 安全边界

- GC 管理的对象不能包含非 GC 管理的指针
- GC 对象与区域对象之间的引用需要 write barrier

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

---

> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
