# Zeta 语言语义规则

> 版本：0.1.0  
> 最后更新：2026-08-22

> **⚠️ 实现状态**：本文为**目标语义规范**。所有权/借用（§1）、`?` 运算符（§6）、闭包（§10）、
> 引用类型等均已实现（G1 引用与借用 / G2 `&str` / G3 裸指针 / H1–H5 闭包与一等函数 / K1 `?`）。
> `Copy` trait 与生命周期严格验证仍为规划（MVP：值拷贝/移动语义 + 宽松借用检查，`'a` 语法接受后丢弃）。
> 已实现语义的教程见 [`guide.md`](./guide.md)，已知限制见其 §13。

## 相关文档

| 类型 | 文档 | 说明 |
|------|------|------|
| 项目总纲 | [CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 |
| 设计文档 | [03_类型系统](design/03_类型系统.md) / [04_所有权与借用检查器](design/04_所有权与借用检查器.md) / [06_比较链与条件判断](design/06_比较链与条件判断.md) | 模块设计 |
| 模块系统 | [module-system.md](./module-system.md) | §11 模块系统语义的完整设计（语法/编译模型/演进路线） |
| 实现纪要 | [附录 A](#附录-a实现纪要)（原 P003/P012，已归档至 [design/prompts/](design/prompts/)） | 比较链语义 + L0 借用检查落地状态 |

---

## 1. 所有权与借用

> **MVP 状态**：本节为**目标语义（规划）**。MVP 已实现：`let s2 = s1` 为值拷贝（对象为堆数据
> 拷贝，见 §8 聚合对象），对象修改需 `let mut`；`&self`/`&mut self` 方法接收者、`&x`/`&mut x` 表达式、
> `&T`/`&mut T` 参数与返回、`*` 解引用与借用检查（G1 ✅）；`Copy` trait 与生命周期严格验证规划中。

### 1.1 所有权规则（目标）

1. 每个值有且只有一个所有者（owner）。
2. 当所有者离开作用域时，值被自动销毁（调用 `Drop`）。
3. 赋值或传参时，所有权默认转移（move）。
4. 实现了 `Copy` trait 的类型除外（按位拷贝）。

```zeta
let s1 = String::from("hello");
let s2 = s1;       // 目标：s1 所有权转移给 s2，s1 不再有效（MVP：值拷贝，s1 仍可读）
let x = 42;        // 目标：i32 实现了 Copy
let y = x;         // x 被拷贝，x 仍然有效
println(x);        // MVP 内建打印（无宏 / 无格式化占位符）
```

### 1.2 借用规则（G1 ✅ 已实现，目标语义）

> `&` 引用表达式与 `&T` 参数类型已实现（G1 ✅），本示例为目标语义：

1. 同一时刻，要么有**多个不可变引用**，要么有**一个可变引用**。
2. 引用必须始终有效（生命周期 ≤ 被引用对象的生命周期）。

```zeta
// 目标语法（已实现，G1 ✅）：
let mut data = vec![1, 2, 3];

let r1 = &data;     // 不可变引用
let r2 = &data;     // 另一个不可变引用，OK
// let r3 = &mut data; // 错误：已有不可变引用

println(r1[0]);     // 打印（MVP 内建）

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

### 1.4 MVP 值语义（实现）

> **实现状态（2026-08-22）**：✅ 已实现。借用规则（§1.2）为规划目标，MVP 实际采用下述值语义。

- String / Vec 为 **3 槽值**（`ptr` / `len` / `cap`），`let b = a` 拷贝 3 槽，但**底层缓冲共享**（数组别名写入互相可见，见 §8.2）。
- **字符串拼接 `a + b` 为深拷贝**：desugar 为 `a.clone()` + `push_str(b)`，结果与左操作数隔离——`push_str` / 扩容不会污染左操作数（A3 修复，消除共享缓冲别名隐患）。
- **`String::from` 支持运行期内容**（G2）：`String` 变量 → desugar 为 `s.clone()` 深拷贝；`&str` 视图 → 读 data/len 槽深拷贝。
- **`&str` 只读借用视图**（G2）：`s.as_str()` 返回零拷贝视图（运行时 = 指向 String 对象的瘦指针，复用 G1 聚合引用）；`&str` 支持 `len()` / 字节索引 / `substring`（拷贝）/ 内容比较，可作参数（`&String` 兼容传入）与返回值。
- 动态切片（`arr[1..<3]` / `v[lo...hi]`）返回**全新缓冲**（元素按值拷贝），非引用视图（`substring` 同理保持拷贝返回，零拷贝以 `as_str()` 视图体现）。
- 越界索引：字面量数组越界编译期可查；动态切片越界自动 clamp 到 `[0, len]`，`start >= end` 返回空。

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

### 4.2 集合内范围元素（离散展开）

`in` 右侧为**括号集合**时，集合内的范围元素展开为离散成员
（要求上下界为编译期整数常量；时间字面量可归一化为分钟值）：

```
x in (0..<10)   →  x == 0 || x == 1 || ... || x == 9     // [0, 10) 展开
x in (0...10)   →  x == 0 || x == 1 || ... || x == 10    // [0, 10] 展开
x in (0<..10)   →  x == 1 || x == 2 || ... || x == 10    // (0, 10] 展开
```

展开后同样应用 4.1 的优化策略（`||` 链 / 二分查找 / 哈希集合）。

### 4.3 裸范围（区间判断）

`in` 右侧为**裸范围**（不带括号）时表示区间判断，语义与集合不同：

```
x in 0..<10   →  x >= 0 && x < 10    // 左闭右开 [0, 10)
x in 0...10   →  x >= 0 && x <= 10   // 闭区间 [0, 10]
x in 0<..10   →  x > 0 && x <= 10    // 左开右闭 (0, 10]
```

### 4.4 混合集合

集合内可混合单值与范围元素，范围元素仍按离散展开：

```
x in (1..<10, 20, 30)  →  (x == 1 || ... || x == 9) || x == 20 || x == 30
```

### 4.5 补集

```
x not in (1, 2, 3)    →  x != 1 && x != 2 && x != 3
x not in (0..<10)     →  x != 0 && x != 1 && ... && x != 9    // 集合补（离散）
x not in 0..<10       →  x < 0 || x >= 10                     // 区间补（裸范围）
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

> **MVP 状态**：`Option`/`Result` 枚举与 `unwrap/unwrap_or/expect` 等已实现；`?` 运算符已实现（K1 ✅，
> desugar 为 `match` + 早返回）；`Into::into()` 自动转换规划中；`std::fs` 模块已实现（N3 ✅）。

### 6.1 Result 传播（K1 ✅ 已实现）

`?` 运算符自动传播错误（desugar 为 `match expr { Ok(v) => v, Err(e) => return Err(e) }`）：

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

### 6.2 自动错误类型转换（规划）

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

## 8. 数组与索引语义

### 8.1 数组字面量

```zeta
let arr = [10, 20, 30];            // 元素类型统一（i64）
let arr2: [i64; 4] = [1, 2, 3, 4]; // 类型注解保留长度 [T; N]
```

- 数组字面量 `[a, b, c]` 编译为"槽区分配 + 逐元素写入"块，数组值为**槽区指针**（堆共享）。
- 所有元素类型必须一致（`compatible_with` 统一），空数组 `[]` 与元素类型不一致为编译错误。
- 数组类型 `[T; N]` 的长度 `N` 必须是 `>= 0` 的整数字面量。

### 8.2 索引读取与写入

```zeta
let x = arr[0];   // 索引读取
arr[1] = 99;      // 索引写入（仅纯赋值；复合赋值暂不支持）
```

- 索引表达式 `a[i]` 要求 `a` 为数组（`Type::Array`）或字符串（`Type::Str`），`i` 必须为整数类型。
- **数组元素步长 8 字节**（`mul i64 idx, 8` + GEP）；**字符串字符步长 1 字节**（直接 GEP）。
- 数组值 = 槽区指针，**别名写入互相可见**（同一数组的多个引用写同一槽区）。
- 嵌套索引 `m[1][0]` 链式访问合法。

---

## 9. 内存模型

### 9.1 分层内存架构

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

### 9.2 数据竞争规则

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

## 10. 执行模型

### 10.1 调用约定

- 默认：系统 V ABI（Linux/macOS）或 Microsoft x64 ABI（Windows）
- `extern "C"`：C ABI 兼容
- `extern "Zeta"`：Zeta 内部 ABI（支持尾调用优化）

### 10.2 栈布局

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

### 10.3 异步执行（S1 ✅ 已实现 MVP）

> **MVP 状态**：普通 `async fn` 已实现（S1 ✅：`Future`/`Poll`/`block_on` + 状态机 desugar + `.await`
> 挂起/恢复，参数 `i64`、返回 `i64`/`()`）；`join_all`/`timeout`/`sleep` 已实现（S2 ✅）。事件驱动执行器与
> 泛型 `Future` 关联输出（`Output` 固定 `i64`、`Pin`/`Context`）规划中。actor 方法的 `async` + `.await`/
> `send` 为独立机制（消息往返，见 [`guide.md`](./guide.md) §9）。

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

## 11. 模块系统语义（规划）

> 完整设计（语法 / 编译模型 / 包集成 / 演进路线）见 [`module-system.md`](./module-system.md)。以下为语义要点。
> v1.1：关键字 `mod` → `module`、`use` → `import`；模块为**扁平名字空间**（无 `crate` / `super` / `self`）；可见性仅 `pub` / 私有两档。

### 11.1 模块名空间与符号表

模块为**扁平名字空间**：模块名全局唯一，无层级寻址。内联模块（`module m { }`）与外部文件模块（`module m;` → `m.zeta` / `m/module.zeta`，现有 ✅）同处一个模块名空间。跨模块可达性 = 模块名路径（`m::item`、`外层::内层::item`）＋ `import` 别名注入。

- 类型与函数/常量同一命名空间；`import` 别名冲突报 `NameConflict`（规划）。
- 符号名编码（`module_name::item` 扁平串）为代码生成契约，不随模块演进改变（现有 ✅）。

### 11.2 名称解析顺序（规划）

对名称引用 `R`，按序解析：

1. 本地作用域：参数 / 局部绑定 / 模式绑定（现有 ✅）。
2. 当前模块符号与直接 `import` 别名（现有 ✅）。
3. 模块名路径：首段查扁平模块名空间，命中则逐段下钻（现有 `module_prefix` 拼接，扩展为名字空间表查找）；未命中 → 下一步。
4. glob 注入（`import m::*`）：不遮蔽显式绑定（规划）。
5. 未找到 → `NameNotFound`（带候选提示，规划）。

> 无 `crate` / `super` / `self` 前缀分支——路径首段恒为模块名，扁平解析，无相对层级。

### 11.3 可见性规则（规划）

- 默认私有：符号仅当前模块及其子模块可达（MVP 现状为全部可达，P2 收紧）。
- `pub`：整个程序 / 依赖图内公开。**不支持** `pub(crate)` / `pub(super)`（扁平名字空间下无层级概念）。
- 子模块可访问祖先私有项（向下开放、向上封闭）。
- 跨模块访问私有 → `PrivateItem` 错误。

### 11.4 歧义与遮蔽（规划）

- 显式符号优先于 glob 同名符号；两个 glob 同名在使用处报歧义；`import` 别名与本地冲突报错；嵌套模块同名符号最近者优先（现有行为保留）；模块名与符号名同名时符号优先。

### 11.5 循环引用

- 文本加载期文件级循环（`module a;` ↔ `module b;`）现有 ✅ 拒绝（visited 集合）。
- 模块图期环 → `CyclicModule` 错误（规划，P3）。
- 函数体交叉调用不构成编译错误（符号表统一解析，规划声明）。

---

## 附录 A：实现纪要

> **说明**：本节提炼自开发任务书（原 `prompts/P003`/`P012`，2026-08-24 归档至 [`design/prompts/`](design/prompts/)），
> 记录语义的实际落地状态、关键决策与已知限制，供后续维护参考。

### A.1 比较链与 `in` 表达式语义（对应 P003，2026-08-19 ✅）

- **比较链**：`0 < x < 10` 语义为 `0 < x && x < 10`（自左向右串联，每段共享中间变量）；
  反向链 `0 > x > 10` 语义为 `x < 0 || x > 10`（区间外）；支持混合 `<=`/`<`、`>=`/`>`。
- **`in` 集合**：`x in (1, 3, 5)` 展开为 `x == 1 || x == 3 || x == 5`；范围元素 `('a'..<'z')` 按步长展开为离散成员。
- **`in` 裸范围（区间判断）**：`x in 0..<10` ≡ `0 <= x < 10`；`x in 0...10` 双闭；`x in 0<..10` 左开右闭。
- **时间字面量区间**：`hour in (9am...6pm)` 分钟值区间判断，跨午夜自动拆段（`6am..<10pm` 分两段）。
- **类型推断**：比较链两端隐式统一为 `f64`（整数自动提升）；`in` 集合要求成员类型一致。

### A.2 L0 借用检查器（对应 P012，2026-08-20 ✅）

- **检查范围（MVP）**：`use-after-move`（`let s2 = s1; use s1` 报错）与**对不可变绑定的赋值**
  （`let x = 1; x = 2` 报错）；覆盖简单赋值、函数参数传递、返回值场景的部分子集。
- **错误类型**：`UseAfterMove` / `AssignToImmutable`（`line`/`col` 字段占位 0，HIR 无 Span）。
- **落地偏差 / 已知限制**：
  - 重复使用同一已 move 变量时逐次报错（**不合并去重**）；
  - 一般化 move 追踪未覆盖所有场景（如字段级 move、闭包捕获）——超出 MVP 范围；
  - 借用检查器**未接线进 driver**（`zeta check` 的 L0 检查与类型检查为独立通路），
    MVP 编译流水线默认不执行借用检查；
  - 与 L1 区域系统（`transfer`）的交互语义由 regionck 承担，见 memory-model.md 附录 A.2。

---

> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
