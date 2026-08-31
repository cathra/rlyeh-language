# Rlyeh — 系统级编程语言

> 版本：**0.1.0**（MVP） · 状态：活跃开发中
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

trait Area { fn area(&self) -> f64; }
impl Area for Shape { fn area(&self) -> f64 { /* ... */ } }
```

### 宏与闭包

```rlyeh
let v = vec![10, 20, 30];          // 集合宏 → Vec<T>
let m = map![1 => 10, 2 => 20];    // → HashMap<K, V>
let r = apply(|a, b| a + b, 10, 20);  // 无捕获闭包 → 函数指针（零开销）
let d: dyn Shape = &c;             // trait 对象：vtable 多态分派
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
- **JSON 序列化**：`json::stringify` / `json::parse::<T>`
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
├── examples/              # 可运行示例
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
| [CHANGELOG.md](CHANGELOG.md) | 版本变更记录 |

---

## 许可证

Rlyeh 采用双许可证：[MIT](https://opensource.org/licenses/MIT) 或 [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0)，任选其一。
