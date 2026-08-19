# Zeta 语言语义规则

> 版本：v2.0  
> 最后更新：2026-08-19

## 相关文档

| 类型 | 文档 | 说明 |
|------|------|------|
| 项目总纲 | [CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 |
| 设计文档 | [03_类型系统](../03_类型系统.md) / [04_所有权与借用检查器](../04_所有权与借用检查器.md) / [06_比较链与条件判断](../06_比较链与条件判断.md) | 模块设计 |
| 实现任务 | [P003](../prompts/P003_比较链语义分析.md) | 类型检查 + 比较链 |

---

## 1. 所有权与借用

### 1.1 所有权规则

1. 每个值有且只有一个所有者（owner）。
2. 当所有者离开作用域时，值被自动销毁（调用 `Drop`）。
3. 赋值或传参时，所有权默认转移（move）。
4. 实现了 `Copy` trait 的类型除外（按位拷贝）。

```zeta
let s1 = String::from("hello");
let s2 = s1;       // s1 所有权转移给 s2，s1 不再有效
// println!(s1);    // 编译错误

let x = 42;        // i32 实现了 Copy
let y = x;         // x 被拷贝，x 仍然有效
println!("{}", x); // OK
```

### 1.2 借用规则

1. 同一时刻，要么有**多个不可变引用**，要么有**一个可变引用**。
2. 引用必须始终有效（生命周期 ≤ 被引用对象的生命周期）。

```zeta
let mut data = vec![1, 2, 3];

let r1 = &data;     // 不可变引用
let r2 = &data;     // 另一个不可变引用，OK
// let r3 = &mut data; // 错误：已有不可变引用

println!("{} {}", r1[0], r2[0]);

let r3 = &mut data;  // OK，r1 和 r2 不再被使用
r3.push(4);
```

### 1.3 生命周期省略规则

当函数签名中只涉及一个输入生命周期时，编译器自动推断：

```zeta
// 完整写法
fn first_word<'a>(s: &'a str) -> &'a str { ... }

// 省略写法（编译器自动推断 'a）
fn first_word(s: &str) -> &str { ... }
```

---

## 2. 区域系统语义

### 2.1 基本规则

1. 区域内的对象生命周期绑定到区域。
2. 区域结束时，所有对象被批量释放。
3. 区域内实现了 `Drop` 的对象，先调用析构函数，再回收内存。
4. 区域内的引用不能逃逸到区域外（除非通过 `transfer`）。

### 2.2 分配语义

```zeta
region 'r {
    let x = 42 in 'r;           // i32 在区域内分配
    let s = String::new() in 'r; // String 在区域内分配
}
// x 和 s 的内存被批量回收
// 如果 s 实现了 Drop，先调用 String::drop()
```

### 2.3 Transfer 语义

```zeta
fn create() -> BigStruct {
    region 'r {
        let data = BigStruct::new() in 'r;
        
        // transfer 的语义：
        // 1. 从区域的析构列表中移除 data
        // 2. 区域内存分配器中标记该位置为"已迁出"
        // 3. data 的所有权转移到调用者
        // 4. 区域结束时不会释放 data 的内存
        return transfer data out of 'r;
    }
}
// 调用者负责最终释放 data
```

### 2.4 区域嵌套

```zeta
region 'outer {
    let a = Data::new() in 'outer;
    
    region 'inner {
        let b = Data::new() in 'inner;
        // b 的生命周期绑定到 'inner
        // a 在 'inner 内仍然有效（'outer 包含 'inner）
    }
    // b 在这里已经被释放
    // a 仍然有效
}
// a 在这里被释放
```

### 2.5 区域扩容策略

| 策略 | 行为 | 适用场景 |
|------|------|----------|
| `exact(N)` | 精确 N 字节，溢出则 panic | 已知精确大小 |
| `with_size(N)` | 初始 N 字节，溢出时链表扩容 | 大致已知大小 |
| `allow_growth(factor)` | 溢出时按 factor 倍数扩容 | 动态大小 |
| `adaptive` | 运行时根据分配模式自适应 | 完全未知 |

---

## 3. 比较链语义

### 3.1 正向链（区间内）

所有运算符方向一致，表示逻辑 AND：

```
a < b < c   →   a < b && b < c
a <= b <= c →   a <= b && b <= c
a < b <= c  →   a < b && b <= c
```

### 3.2 反向链（区间外）

所有运算符方向一致且为 `>`，表示逻辑 OR：

```
a > b > c   →   a > b && b > c   →   但语义解释为：b 在 a 和 c 之外
             →   等价于  b < a || b > c  （当 a < c 时）
```

**具体规则**：

```
对于表达式  L > x > R  （假设 L < R）：
    语义 =  x < L || x > R

对于表达式  L >= x >= R （假设 L < R）：
    语义 =  x <= L || x >= R
```

编译器在语义分析阶段检测 L 和 R 的常量值，如果 L < R，则按区间外解释；否则报错。

### 3.3 非法比较链

```
a < b > c    →  ERROR: inconsistent comparison direction
a < b < c > d →  ERROR: direction change at c
a == b == c  →  ERROR: ambiguous chained equality
```

### 3.4 类型要求

比较链中所有表达式必须实现 `PartialOrd`。不同类型之间需要存在隐式转换。

---

## 4. `in` 表达式语义

### 4.1 离散集合

```
x in (a, b, c)  →  x == a || x == b || x == c
```

编译器优化：
- 元素 ≤ 5 个：展开为 `||` 链
- 元素 > 5 个且有序：生成二分查找
- 元素 > 5 个且无序：生成哈希集合

### 4.2 范围集合

```
x in (0..10)    →  x >= 0 && x < 10
x in [0, 10]    →  x >= 0 && x <= 10
x in (0, 10]    →  x > 0 && x <= 10
```

### 4.3 混合集合

```
x in (1..10, 20, 30)  →  (x >= 1 && x < 10) || x == 20 || x == 30
```

### 4.4 补集

```
x not in (1, 2, 3)    →  x != 1 && x != 2 && x != 3
x not in (0..10)       →  x < 0 || x >= 10
```

---

## 5. Actor 模型语义

### 5.1 Actor 状态隔离

每个 Actor 实例拥有独立的状态。Actor 内部状态只能通过消息传递访问。

```zeta
actor BankAccount {
    balance: f64 = 0.0,
    
    pub fn deposit(amount: f64) {
        self.balance += amount;
    }
    
    pub fn withdraw(amount: f64) -> Result<(), String> {
        if self.balance < amount {
            return Err("Insufficient funds");
        }
        self.balance -= amount;
        Ok(())
    }
    
    pub fn get_balance() -> f64 {
        self.balance
    }
}

// 使用
let account = BankAccount::new();
account.deposit(100.0).await;
let balance = account.get_balance().await;
```

### 5.2 消息传递语义

- 消息发送是**异步**的（返回 `Future`）。
- 消息按发送顺序处理（FIFO）。
- Actor 内部是**单线程**执行的（无锁）。
- 崩溃隔离：一个 Actor 崩溃不影响其他 Actor。

### 5.3 Supervisor 策略

```zeta
supervisor {
    strategy: OneForOne,  // 或 AllForOne
    max_restarts: 3,
    within: Duration::seconds(30),
    
    children: [
        BankAccount::new(),
        Logger::new(),
    ]
}
```

---

## 6. 错误处理语义

### 6.1 Result 传播

`?` 运算符自动传播错误：

```zeta
fn read_config() -> Result<Config, IoError> {
    let content = std::fs::read_to_string("config.toml")?;
    let config = parse_config(&content)?;
    Ok(config)
}
// 等价于：
fn read_config() -> Result<Config, IoError> {
    let content = match std::fs::read_to_string("config.toml") {
        Ok(s) => s,
        Err(e) => return Err(e),
    };
    let config = match parse_config(&content) {
        Ok(c) => c,
        Err(e) => return Err(e),
    };
    Ok(config)
}
```

### 6.2 自动错误类型转换

`?` 会自动调用 `Into::into()` 进行错误类型转换：

```zeta
enum MyError {
    Io(IoError),
    Parse(ParseError),
}

impl From<IoError> for MyError {
    fn from(e: IoError) -> Self { MyError::Io(e) }
}

fn process() -> Result<(), MyError> {
    let content = read_file()?;  // IoError 自动转换为 MyError
    Ok(())
}
```

### 6.3 永不返回类型 `!`

```zeta
fn abort() -> ! {
    loop {}  // 永不返回
}

fn main() {
    let x: i32 = match some_condition {
        true => 42,
        false => abort(),  // abort() 返回 !，可以匹配任何类型
    };
}
```

---

## 7. 泛型与特化语义

### 7.1 单态化

泛型函数在编译期为每个具体类型生成一个副本：

```zeta
fn identity<T>(x: T) -> T { x }

// 单态化后生成：
// fn identity_i32(x: i32) -> i32 { x }
// fn identity_string(x: String) -> String { x }
```

### 7.2 特化

```zeta
// 通用实现
fn sort<T: Ord>(data: &mut [T]) { /* 通用排序 */ }

// 对 u32 特化
@specialize(for T = u32)
fn sort(data: &mut [u32]) {
    // 使用基数排序（更快）
}
```

### 7.3 编译期反射

```zeta
impl Serialize for Point {
    fn serialize(&self) -> JsonValue {
        let mut json = Map::new();
        @for field in Self::fields() {
            json.insert(field.name, field.get_value(self).serialize());
        }
        JsonValue::Object(json)
    }
}
```

`@for` 在编译期展开为对每个字段的显式访问。

---

## 8. 内存模型

### 8.1 分层内存架构

```
┌─────────────────────────────────────────────┐
│  L3: GC Heap (可选)                        │
│  - 追踪式 GC，用于脚本层                    │
│  - 周期性标记-清除或 Immix 算法             │
├─────────────────────────────────────────────┤
│  L2: Reference Counted                     │
│  - Rc<T> / Arc<T>                          │
│  - 显式选择，用于共享所有权                  │
│  - 原子操作开销                            │
├─────────────────────────────────────────────┤
│  L1: Region Allocator                      │
│  - Bump pointer 分配                       │
│  - 批量释放 O(1)                          │
│  - 链表式扩容                             │
├─────────────────────────────────────────────┤
│  L0: Static Ownership                      │
│  - 编译期确定所有权                        │
│  - 零运行时开销                            │
│  - 栈分配 + 堆分配（Box）                  │
└─────────────────────────────────────────────┘
```

### 8.2 数据竞争规则

- 如果一个类型实现了 `Send`，它可以安全地转移到另一个线程。
- 如果一个类型实现了 `Sync`，它可以安全地被多个线程共享（通过 `&T`）。
- 编译器在编译期验证所有并发访问的安全性。

```zeta
// 自动推导
struct SafeData { value: i32 }  // 自动实现 Send + Sync（i32 是 Send + Sync）

// 不安全的类型
struct RawPointer(*mut u8);  // 不实现 Send 和 Sync
```

---

## 9. 执行模型

### 9.1 调用约定

- 默认：系统 V ABI（Linux/macOS）或 Microsoft x64 ABI（Windows）
- `extern "C"`：C ABI 兼容
- `extern "Zeta"`：Zeta 内部 ABI（支持尾调用优化）

### 9.2 栈布局

```
┌─────────────────────┐
│  Return Address     │
├─────────────────────┤
│  Saved Registers    │
├─────────────────────┤
│  Local Variables    │
├─────────────────────┤
│  Function Args      │
└─────────────────────┘
```

### 9.3 异步执行

`async fn` 返回 `Future`，由执行器（executor）调度：

```zeta
async fn fetch_all(urls: &[&str]) -> Vec<Result<Data, Error>> {
    let mut tasks = vec![];
    for url in urls {
        tasks.push(fetch(url));  // 创建 Future
    }
    join_all(tasks).await  // 并发等待所有任务
}
```

---

> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
