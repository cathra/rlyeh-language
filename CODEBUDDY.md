# CODEBUDDY.md — Zeta 系统级编程语言

> **项目代号**：Zeta  
> **版本**：v2.0  
> **状态**：MVP 开发中  
> **目标平台**：Linux / macOS / Windows / WASM  
> **实现语言**：Rust（自举编译器，bootstrap 阶段用 Rust 实现）

---

## 1. 项目愿景

Zeta 是一门面向未来十年基础设施的**系统级编程语言**，设计目标：

| 目标 | 对标 |
|------|------|
| 内存安全、零 GC | Rust |
| 编译速度极快 | Go |
| 并发模型一等公民 | Erlang / Akka |
| 数学式语法直觉 | Python / MATLAB |
| 开发体验友好 | Go / Python |

**一句话定位**：Zeta = Rust 的安全性 + Go 的编译速度 + Erlang 的并发模型 + 数学的自然表达。

---

## 2. 项目结构

```
zeta-language/
├── CODEBUDDY.md              ← 本文件（项目总纲）
├── Cargo.toml                ← Rust 工作区配置
├── README.md                 ← 项目介绍
│
├── crates/                   ← 编译器各组件（Rust crate）
│   ├── zeta-lexer/           ← 词法分析器
│   ├── zeta-parser/          ← 语法分析器
│   ├── zeta-ast/             ← 抽象语法树定义
│   ├── zeta-hir/             ← 高级中间表示
│   ├── zeta-mir/             ← 中级中间表示
│   ├── zeta-lir/             ← 低级中间表示
│   ├── zeta-typecheck/       ← 类型检查器
│   ├── zeta-borrowck/        ← 借用检查器
│   ├── zeta-regionck/        ← 区域检查器
│   ├── zeta-codegen/         ← 代码生成（LLVM / Cranelift）
│   ├── zeta-driver/          ← 编译器驱动（CLI 入口）
│   ├── zeta-std/             ← 标准库（Zeta 源码）
│   ├── zeta-actor-runtime/   ← Actor 运行时
│   └── zeta-region-alloc/    ← 区域分配器（bump + 智能）
│
├── tools/                    ← 工具链
│   ├── zeta-fmt/             ← 代码格式化
│   ├── zeta-check/           ← Linter
│   ├── zeta-doc/             ← 文档生成
│   └── zeta-bench/           ← 基准测试框架
│
├── zep/                      ← 包管理器
│   ├── Cargo.toml
│   └── src/
│
├── docs/                     ← 语言规范文档
│   ├── grammar.md            ← 完整语法规范（EBNF）
│   ├── semantics.md          ← 语义规则
│   ├── memory-model.md       ← 分层内存管理规范
│   ├── actor-model.md        ← Actor 并发模型规范
│   └── std-lib.md            ← 标准库 API 规范
│
├── examples/                 ← 示例代码
│   ├── hello-world.zeta
│   └── arith-print.zeta
│
├── tests/                    ← 集成测试（.zeta 用例；Rust 集成测试位于各 crate 的 tests/ 目录）
│   ├── compile-pass/
│   ├── compile-fail/
│   └── run-pass/
│
├── prompts/                  ← CodeBuddy Prompt 文档
│   ├── README.md
│   ├── QUICKSTART.md
│   └── P001-P013_*.md
│
└── .github/                  ← CI/CD
    └── workflows/
        ├── ci.yml
        └── release.yml
```

---

## 3. 核心语言特性速览

### 3.1 分层内存管理

| 层级 | 机制 | 关键字 | 运行时开销 |
|------|------|--------|------------|
| L0 | 静态所有权 + 借用 + 生命周期 | `let`, `&`, `&mut` | 零开销 |
| L1 | 区域（Region） | `region 'r {}`, `in 'r`, `transfer` | 零开销（批量释放） |
| L2 | 引用计数 | `Rc<T>`, `Arc<T>` | 原子操作 |
| L3 | 可选 GC | `Gc<T>`（插件式） | GC 停顿 |

### 3.2 数学式条件判断

```zeta
// 区间内（正向比较链）
if 0 < x < 10 {}
if 0 <= x <= 10 {}
if 0 < x <= 10 {}

// 区间外（反向比较链）
if 0 > x > 10 {}    // x < 0 || x > 10
if 0 >= x >= 10 {}  // x <= 0 || x >= 10

// 集合判断（括号内为集合，范围元素展开为离散成员）
if x in (1, 3, 5) {}
if ch in ('a'..<'z', 'A'..<'Z') {}
if x in (0...10) {}  // 等价于 x == 0 || x == 1 || ... || x == 10
if x not in (0..<10) {}  // 等价于 x != 0 && x != 1 && ... && x != 9

// in 右侧裸范围 = 区间判断（与集合成员判断语义不同）
if x in 0..<10 {}  // 0 <= x < 10   [0, 10)
if x in 0...10 {}  // 0 <= x <= 10  [0, 10]
if x in 0<..10 {}  // 0 < x <= 10   (0, 10]

// 时间字面量
if hour in (9am...6pm) {}
if hour not in (6am..<10pm) {}  // 跨午夜
```

### 3.3 Actor 并发模型

```zeta
actor Counter {
    value: u32 = 0,
    
    pub fn increment(amount: u32) -> u32 {
        self.value += amount;
        self.value
    }
}

let counter = Counter::new();
let result = counter.increment(10).await;
```

### 3.4 区域系统

```zeta
// 基本用法
region 'r {
    let data = BigStruct::new() in 'r;
    process(&data);
} // 批量释放

// 转移所有权到外部
fn create_data() -> BigStruct {
    region 'r {
        let data = BigStruct::new() in 'r;
        return transfer data out of 'r;
    }
}

// 智能分配（编译器自动推断大小）
region 'r adaptive {
    for i in 0..<10000 {
        let obj = Data::new(i) in 'r;
    }
}
```

### 3.5 聚合对象与泛型

```zeta
// 枚举 + 匹配
enum Shape { Circle(f64), Rect { w: f64, h: f64 } }

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.14 * r * r,
        Shape::Rect { w, h } => w * h,
    }
}

// trait + impl + 泛型（单态化）
trait Area { fn area(&self) -> f64; }
impl Area for Shape { fn area(&self) -> f64 { /* ... */ } }

// 结构体字面量构造
let p = Point { x: 1, y: 2 };
```

### 3.6 模块系统

```zeta
mod math {
    pub const PI: f64 = 3.14159;
    pub fn square(x: i64) -> i64 { x * x }
}
use math::PI;                 // 别名导入
use math::square as sq;

// 多文件模块：mod foo; → foo.zeta / foo/mod.zeta
// 跨模块路径：mod::Enum::Variant / mod::CONST
```

### 3.7 数组与索引访问

```zeta
let arr = [10, 20, 30];        // 数组字面量（元素类型统一）
let arr2: [i64; 4] = [1, 2, 3, 4];  // 类型注解保留长度
let x = arr[0];                // 索引读取（越界编译期可查）
arr[1] = 99;                   // 索引写入（别名共享，互相可见）
let ch = s[0];                 // 字符串按字符索引（步长 1 字节）
```

---

## 4. 编译器架构

### 4.1 编译流水线

```
源代码 (.zeta)
    │
    ▼
┌─────────────────────────────────────┐
│  Lexer (zeta-lexer)                │  → Token 流
├─────────────────────────────────────┤
│  Parser (zeta-parser)              │  → AST
├─────────────────────────────────────┤
│  Name Resolution & Macro Expand     │  → HIR
├─────────────────────────────────────┤
│  Type Checker (zeta-typecheck)      │  → 类型标注的 HIR
├─────────────────────────────────────┤
│  Borrow Checker (zeta-borrowck)     │  → 借用验证
├─────────────────────────────────────┤
│  Region Checker (zeta-regionck)     │  → 区域验证 + 大小推断
├─────────────────────────────────────┤
│  MIR Lowering (zeta-mir)           │  → 控制流图
├─────────────────────────────────────┤
│  Optimization Passes                │  → 常量折叠、内联、死代码消除
├─────────────────────────────────────┤
│  LIR Lowering (zeta-lir)           │  → 目标无关低级 IR
├─────────────────────────────────────┤
│  Code Generation (zeta-codegen)     │  → LLVM IR / Cranelift IR
├─────────────────────────────────────┤
│  Backend (LLVM / Cranelift)         │  → 机器码 / WASM
└─────────────────────────────────────┘
    │
    ▼
可执行文件 / .wasm
```

### 4.2 增量编译策略

| 策略 | 粒度 | 触发条件 |
|------|------|----------|
| 模块级缓存 | 每个 .zeta 文件 | 接口哈希未变 |
| 函数级并行 | 无依赖函数 | 编译图分析 |
| 类型检查缓存 | 类型推导结果 | 输入类型未变 |
| PGO 数据复用 | 区域大小预测 | .zeta_profile 存在 |

### 4.3 目标性能

| 指标 | 目标 |
|------|------|
| 百万行全量编译 | < 30 秒 |
| 增量编译（单文件修改） | < 3 秒 |
| 编译器内存峰值 | < 2GB |
| 生成的代码性能 | 与 Rust 同级（±5%） |

---

## 5. 工具链命令

| 命令 | 功能 | 状态 |
|------|------|------|
| `zeta new <name>` | 创建新项目 | 🔲 待实现 |
| `zeta build` | 编译项目 | ✅ 可用（MVP） |
| `zeta run` | 编译并运行 | ✅ 可用（MVP） |
| `zeta test` | 运行测试 | 🔧 开发中（子命令未实现，测试经 `cargo test` 驱动） |
| `zeta bench` | 基准测试 | 🔲 待实现 |
| `zeta doc` | 生成文档 | 🔲 待实现 |
| `zeta fmt` | 代码格式化 | 🔲 待实现 |
| `zeta check` | 静态分析 | 🔲 待实现 |
| `zeta publish` | 发布包 | 🔲 待实现 |

---

## 6. 开发里程碑

### Milestone 1 — 编译器 MVP（Month 0-4）

- [x] **M1.1** Lexer 完成（支持全部关键字和运算符）
- [x] **M1.2** Parser 完成（支持完整语法）
- [x] **M1.3** AST → HIR  lowering
- [x] **M1.4** 类型检查器基础（比较链 + `in` 表达式语义 + `for` range 迭代器 desugar；泛型/trait/聚合对象特性已由后续任务补齐）
- [x] **M1.5** 借用检查器（L0 所有权系统：use-after-move + 不可变绑定赋值，区域块值传递例外；由 P012 补齐 16 测试；`&self`/`&mut self` 引用接收者方法已支持）
- [x] **M1.6** 区域系统（`zeta-region-alloc` bump allocator + 四策略 + LIFO 析构 + `execute_transfer` 所有权句柄；`zeta-regionck` 嵌套/归属/transfer 合法性 + P005 嵌套方向/PartialTransfer 语义；MIR 区域指令显式化由 P011 完成）
- [x] **M1.7** MIR + 基础优化 passes（P011：CFG lowering：if/while/loop/break/continue + 区域指令显式化；常量折叠、DCE、小函数内联）
- [x] **M1.8** 代码生成（P013：LIR 三地址码 + LLVM IR 文本后端，x86_64 Linux）
- [x] **M1.9** 能编译并运行 `hello-world.zeta`

### Milestone 2 — 生产可用（Month 4-8）

- [x] **M2.1** Actor 运行时（`zeta-actor-runtime`：工作窃取调度 + 每 Actor 互斥处理 + 有界邮箱 + ask/reply 模式 + Supervisor 监督恢复（OneForOne/AllForOne/RestartForOne + 频率限制）+ 内置 Router/Timer + 优雅排空关闭；`ActorRef`/`Runtime` API 就绪，语言级 `actor`/`spawn` 语法与标准库集成待 M2.5）
- [x] **模块系统**（嵌套 `mod {}` + `mod foo;` 多文件加载（`foo.zeta` / `foo/mod.zeta`）+ `use` 导入与别名 + 扁平符号名 `mod::item` + LLVM 引号标识符接线；typecheck 支持模块内函数/常量/结构体符号解析）
- [x] **M2.2** 包管理器 Zep（P008：pubgrub 依赖解析 + 本地/HTTP 注册表 + 打包解包 + 构建驱动，29 测试）
- [x] **M2.3** 增量编译引擎（源码/接口哈希 + LLVM IR 产物缓存 + 依赖图 + 多文件模块编译缓存）
- [ ] **M2.4** LSP 服务器（IDE 支持）
- [x] **M2.5 部分完成** 标准库核心模块：P009 完成 `Option<T>`/`Result<T,E>` 纯 Zeta 实现 + 编译器标准库搜索路径注入（`--no-std`/缓存键覆盖）+ NIO/sendfile 绑定层；**collections 已落地 `Vec<T>` + `String` + `HashMap<K,V>`**（`Vec<T>`：`new`/`with_capacity` + `push`/`pop`/`get`/`set`/`len`/`cap`/`is_empty` + `v[i]` 索引读写（`check_index` 解 `Vec<T>` 类型参数，步长 8）+ `for x in v` 容器迭代 + 自动扩容 + `contains`/`remove`/`insert`/`clear`/`find`/`sort` 常用操作 + 内建 `alloc_array`/`array_copy`/`array_free`，`vec_test.rs` 6 用例 + `vec_for_test.rs` 6 用例 + `vec_common_ops_test.rs` 6 用例 + `vec_find_sort_test.rs` 6 用例；`String`：UTF-8 字节缓冲（3 槽布局）+ `String::from("字面量")`/`new`/`with_capacity` + `println(String)`（`%.*s`）+ `len`/`cap`/`is_empty`/`get`/`push_byte`/`push_str` + `a + b` 拼接运算符 + 字节级 `s[i]` 索引（步长 1）+ `s1 == s2`/`!=` 内容相等比较（len 短路 + `bytes_eq` 内建 = `memcmp == 0`）+ `s1 < s2`/`>`/`<=`/`>=` 字典序比较（`bytes_cmp` 内建 = `memcmp` 有符号扩展 i64，desugar 为前缀 memcmp + 长度兜底）+ `substring(start, end)` 子串截取（[start, end) 字节区间 + 越界 clamp）+ `find(sub)`/`contains(sub)` 子串查找（朴素滑动窗口，未命中 -1 / 空子串 0）+ 范围切片语法 `s[lo..<hi]`/`s[lo...hi]`/`s[lo<..hi]`（typecheck 层 `check_slice` desugar 为 `String::substring`：`..<` 直通、`...` 闭区间 end+1、`<..` 左开 start+1，仅 String）/ + `to_upper`/`to_lower` 大小写转换（比较链区间 ±32，非 ASCII 不转换）+ `trim` 首尾空白剥离（双扫描 + substring）+ `starts_with`/`ends_with` 前后缀判断（逐字节比较 + 空前缀恒真 + 长于自身恒假）+ `replace` 子串替换（滑动窗口 + 空 old 特判防死循环）+ 顶层 `int_to_string`/`string_to_int` 数值互转（位权除法逐位输出 / 逐字符累加 + 负号 + 遇非数字停止）+ `split` 分割返回 `Vec<String>`（滑动窗口 + 空 sep 特判）/ `repeat` 重复拼接 / `pad_start`/`pad_end` 字节填充 / `strip_prefix`/`strip_suffix` 前后缀剥离返回 `Option<String>` / `truncate` 截断 / + 内建 `alloc_bytes`/`copy_bytes`/`bytes_eq`/`bytes_cmp`/`print_string`/`println_string`，`string_test.rs` 6 用例 + `string_eq_test.rs` 6 用例 + `string_concat_test.rs` 6 用例 + `string_cmp_test.rs` 6 用例 + `string_slice_test.rs` 6 用例 + `string_slice_syntax_test.rs` 6 用例 + `string_case_trim_test.rs` 6 用例 + `string_common_ops_test.rs` 6 用例 + `string_conv_test.rs` 6 用例 + `string_split_pad_test.rs` 6 用例 + `string_strip_trunc_test.rs` 6 用例；`HashMap<K,V>`：开放寻址线性探测 + 墓碑删除 + 翻倍 rehash（负载 1/2）+ 6 槽布局（keys/vals/states/len/used/cap）+ 内建 `hash_value`（整数键 Knuth 乘法散列 / String 键 djb2 内容哈希）+ 构造器特判展开 + `for (k, v) in m` 元组模式迭代 + `keys()`/`values()` 键值集收集 + `clear()` 完全重置（容量不变），`hashmap_test.rs` 6 用例 + `hashmap_for_test.rs` 6 用例 + `hashmap_string_key_test.rs` 7 用例 + `hashmap_keys_clear_test.rs` 6 用例）；io/net/sync 待完成
- [ ] **M2.6** 交叉编译（macOS, Windows, ARM）
- [ ] **M2.7** WASM 目标支持
- [x] **M2.8 分配器侧完成** 智能区域（P010：静态大小推断 + PGO 画像/推荐 + EWMA 自适应扩容 + 碎片/事件统计 + 编译器集成报告）；PGO 数据回灌编译流程待完成

### 已完成语言特性（无编号任务）

- [x] **聚合对象语言特性**（enum + match + impl + trait + 泛型单态化 + `struct` 字面量构造 `Point { x, y }` + `&self`/`&mut self` 引用接收者方法 + 方法调用 `Type::method()` + 泛型替换下沉模式绑定 + `Option::None` Infer 占位）
- [x] **for 循环 range 迭代器**（typecheck 层 desugar 为 `loop` + 临时边界变量 + break/continue 正确性；修复 continue 死循环）
- [x] **`for ... in` Vec 容器迭代 + `v[i]` 索引访问**（`check_for` 分派重构：range → `check_for_range`、`Vec<T>` → `check_for_vec`（typecheck 层 desugar 为索引遍历：绑定容器 `__for_v` + 缓存长度 `__for_len`（槽 1）+ 计数器 `__for_i` + `loop { if __for_i >= __for_len { break } let x = __for_v[__for_i]; __for_i += 1; body }`，递增在 continue 回跳前执行无死循环）；`check_index` 解 `Vec<T>` 类型参数并经泛型替换取元素类型（`v[i]` 读 → `HirExpr::Index` 走 IndexSet 赋值，base 为槽 0 data 指针 + 步长 8），`vec_for_test.rs` 6 用例：求和/打印/break+continue/索引读/索引写/空迭代+嵌套）
- [x] **`for (k, v) in m` HashMap 元组模式迭代**（`check_for` 分派新增 HashMap 分支 → `check_for_hashmap`：`AstPattern::Tuple` 二元标识符模式解构 + typecheck 层 desugar 为稀疏索引遍历——绑定容器 `__for_m` + 缓存容量 `__for_cap`（槽 5）+ 计数器 `__for_i` + `loop { if __for_i >= __for_cap { break } if __for_m.states[__for_i] != 1 { __for_i += 1; continue } let k = __for_m.keys[__for_i]; let v = __for_m.vals[__for_i]; __for_i += 1; body }`，`states` 跳槽跳过空槽/墓碑（0/2），K/V 各自经泛型替换取 `field_scalar_of`；`hashmap_for_test.rs` 6 用例：求和/计数/删除后跳槽/空 map/扩容 rehash 后遍历/f64 值类型；遍历顺序与插入无关，测试全部用求和或计数断言）
- [x] **`String` 内容相等比较 `==`/`!=`**（comparison.rs 单比较分支对 `String == String` desugar：绑定操作数到唯一临时变量（防重复求值）+ `s1.len == s2.len && bytes_eq(s1.data, s2.data, s1.len)`，`!=` 取 `Not` 反转；新增内建 `bytes_eq`（typecheck builtin_signature `(Infer, Infer, I64) -> Bool` / LIR BUILTIN_FUNCTIONS 返回 Bool / codegen `declare i32 @memcmp(i8*, i8*, i64)` + `call` + `icmp eq i32, 0` 存 i1）；其他聚合对象（Vec/结构体）的 `==`/`!=` 报 Unsupported（MVP 仅 String）；`string_eq_test.rs` 6 用例：同内容相等/异内容同长度不等/长度不等短路/前缀相同长度不同/中间字节不同/if 分支/复杂表达式操作数）
- [x] **修复：LIR 嵌套 Binary 比较结果错登记为 i64 槽**（既有 bug：`icmp`/`fcmp` 恒产生 i1，但 `lower_operand` 拆平嵌套 Binary 时 `fresh_temp(binary_operand_type)` 对比较运算登记 I64，导致 `let x = a < b` / `println(a == b)` 生成 `store i64 %i1` 类型错误；修复：`binary_operand_type` 保持操作数类型语义，新增 `binary_result_type`（比较/逻辑 → Bool，算术 → 操作数类型），`fresh_temp` 登记结果类型）
- [x] **`HashMap` 字符串键（djb2 内容哈希）**（check_expr.rs builtin 检查处 `hash_value(s)` 参数为 String → 特判 desugar 为纯 Zeta djb2 哈希 HIR 块：绑定 `__s` → 槽 0 data 指针 + 槽 1 len → `__h = 5381` + `loop { if __i >= __len { break } let __b = __data[__i]（字节索引步长 1，LIR 层 zext i64）；__h = __h * 33 + __b; __i += 1 }` → 返回 I64；同内容字符串恒同哈希保证探测链正确，乘法 LLVM mul wrapping 回绕；无新增内建/LIR/codegen 改动，泛型实例化 `HashMap<String,V>` 后 `hash()`/`find()` 键比较 `==` 走内容相等）；`hashmap_string_key_test.rs` 7 用例：insert/get/contains_key/同内容不同对象哈希一致/同键覆盖/remove 墓碑/12 键扩容 rehash/未命中/for 元组迭代）\n- [x] **`HashMap` 键值集/清空：`keys()` + `values()` + `clear()`**（core.zeta 纯 Zeta 泛型 impl 方法；`keys()`/`values()` 稀疏遍历全部槽位收集 `states == 1`（存活）的键/值为 `Vec<K>`/`Vec<V>`——`let mut ks: Vec<K> = Vec::new();` 带泛型参数注解定型（`Vec::new()` Infer 占位需上下文，`grow` 的 `[K; 0]` 先例），遍历顺序与插入无关（开放寻址），同槽位遍历保证 keys 与 values 顺序一致，墓碑/空槽跳过；`clear()` 完全重置——重开三数组（容量不变）替换旧缓冲并 free，`len`/`used` 归零，墓碑与旧数据全部丢弃（与 `remove` 逐键墓碑不同，后续插入即全新探测链，无墓碑堆积拖慢））；`hashmap_keys_clear_test.rs` 6 用例：clear 基本（3 键清空后 len/is_empty/contains_key 全空 + 重新插入可查）/clear 保持容量（cap 8 清空后仍 8 + 复用 4 键不扩容无墓碑残留）/keys 基本（5 键扩容后键集 sort 逐位断言 10→50）/keys String 键（字典序 sort + 与 values 等长同遍历）/values 基本（重复值 + sort 断言 + 与键数一致）/组合（insert + remove 墓碑后 keys/values 只含存活 4 键 + clear 终态键集空））\n- [x] **`Option`/`Result` 补充方法：`expect` + `is_err` + `unwrap_or`**（core.zeta 纯 Zeta 泛型 impl 方法；`Option::expect(msg)`/`Result::expect(msg)` Some/Ok 返回载荷、None/Err 以 `loop {}` 充当崩溃替代（消息参数保留对齐 Rust 语义，MVP 不打印）；`Result::is_err` match 分派 Ok→0/Err→1（与 is_ok 互补）；`Option/Result::unwrap_or(default)` match 分派 Some/Ok 返回载荷、None/Err 返回 default）；为让 expect 的 `msg: String` 参数可引用，`struct String` 定义前移文件顶部（类型符号顺序解析无前向引用）；`option_result_test.rs` 6 用例：Option expect Some 路径（i64/String 双实例化 + 参与算术与比较）/Option unwrap_or 默认值（Some 自身/None 默认/i64 实例化）/Result is_err（Ok→0/Err→1/与 is_ok 互补和恒 1）/Result unwrap_or（Ok 载荷/Err 默认/i64 实例化）/Result expect Ok 路径（载荷 + 算术比较）/组合（带显式类型注解的 Result<i64,i64>/Option<i64> 混合链路 + expect 载荷走 String 方法））\n- [ ] **编译器已知 bug：枚举聚合载荷 + 方法返回路径**（`Option<String>`/`Result<String,E>` 等**聚合载荷**实例化下，`unwrap_or` 的 match 分支返回 default（参数/局部 String）会得到损坏值（垃圾指针数字）；`expect` 的 Some/Ok 分支返回载荷正常、i64 实例化全部正常。根因线索：`field_scalar_of(聚合)=Ptr`——枚举载荷以指针存于 `1 + max_fields` 槽（String 载荷仅 1 字段槽），match 解构恢复聚合的「返回非载荷来源聚合」路径在 codegen 层损坏。纯 Zeta 全部变体（result 变量/match 空分支/局部中转/if+unwrap）均无法绕过；**String 场景请用 `expect`**，`unwrap_or` 仅标量实例化可靠。另：main/顶层函数 `match` 解构具体实例化枚举的聚合载荷报 `expected String, found T`（泛型替换未下沉到用户级 match，仅 std 泛型方法体路径正常）；`Option::Some(10)` 等部分 Infer 类型参数枚举 + 泛型方法实例化需显式类型注解。修复待编译器深水区排查）
- [x] **`String` 拼接 `+` 运算符 + `push_str`**（core.zeta 纯 Zeta 实现 `fn push_str(&mut self, other: String)`：逐字节 `other.data[i]` → `push_byte`（自动扩容迁移），值参数 3 槽拷贝共享缓冲；check_expr.rs `ExprKind::Binary` 分支对 `Add` + 双侧 String 特判 desugar 为 `let __s = a; __s.push_str(b); __s`——`__s` 为 mutable 临时绑定，`push_str` 经 `find_impl_for_method` + `instantiate_impl_method` 实例化注册函数体（mono_instances 防递归重复），返回左侧操作数类型；链式 `a + b + c` 左结合递归展开；`is_string_type` 提升为 `pub(crate)` 供 check_expr 复用）；`string_concat_test.rs` 6 用例：基本拼接/链式拼接/拼接结果字节索引/内容相等比较/push_str 直接调用/多次拼接触发扩容（cap 6→12→24））
- [x] **`String` 字典序比较 `<`/`>`/`<=`/`>=`**（新增内建 `bytes_cmp`（typecheck 签名 `(Infer, Infer, I64) -> I64` / LIR BUILTIN_FUNCTIONS 返回 I64 / codegen `call i32 @memcmp` + `sext i32 to i64` 保留符号）；comparison.rs 单比较分支对 String 排序操作跳过数值类型检查直接 desugar：`string_lt_hir` 绑定 `__a`/`__b` → 槽 1 长度 + min（`if __la >= __lb { __n = __lb }` 控制流赋值）+ `__c = bytes_cmp(__a.data, __b.data, __n)` → `__c < 0 || (__c == 0 && __la < __lb)`；`>` 交换操作数、`<=`/`>=` 取 `Not` 反转）；`string_cmp_test.rs` 6 用例：等长前缀不同定大小/前缀相同长度兜底/完全相等四方向/不等四方向 + == 回归/拼接复杂表达式操作数/if 分支 + 三串取最大）
- [x] **`String` 子串/查找：`substring` + `find` + `contains`**（core.zeta 纯 Zeta 方法；`substring(start, end)` 截取 [start, end) 字节区间——start/end 越界 clamp 到 [0, len]、start >= end 空串，`String::new()` 特判展开 + 逐字节 `push_byte`（自动扩容）拷贝到新缓冲，原字符串不受影响；`find(sub)` 朴素滑动窗口匹配返回首次出现下标（未命中 -1、空子串 0），`result` 变量返回规避 while 块后 `-1` 被解析为减法；`contains(sub)` = `find(sub) >= 0`）；`string_slice_test.rs` 6 用例：基本截取/越界 clamp/find 命中-未命中-空子串/contains 真假 + 自身包含/`find` 下标喂 `substring` 组合提取/拼接 + 子串链式）
- [x] **`String` 范围切片语法 `s[lo..<hi]`/`s[lo...hi]`/`s[lo<..hi]`**（parser 本就支持 `[` 内 range 表达式（RANGE 优先级 infix），`ExprKind::Index` 的 index 为 `ExprKind::Range` 时 typecheck 特判走 `check_slice`（`infer_expr` 对 Range 仅返回 Unit，须在索引推断前分派）；`check_slice` 仅放行 String（`comparison::is_string_type`），数组/Vec 动态切片报 Unsupported，下界/上界要求整数，边界按区间运算符调整后 desugar 为 `HirExpr::Call "String::substring"`（`instantiate_impl_method` 注册函数体，subst 空）——`..<` 左闭右开直通、`...` 双闭 end+1、`<..` 左开右闭 start+1 & end+1；切片结果仍为 String，可链式再切片/拼接/比较，`find` 下标可直接作边界）；`string_slice_syntax_test.rs` 6 用例：三种 range 运算符边界语义/越界 clamp/切片 + == + len/切片链式/拼接 + 切片组合/变量与表达式边界（`find` 下标 + `p + 5`））
- [x] **`String` 大小写/裁剪：`to_upper` + `to_lower` + `trim`**（core.zeta 纯 Zeta 方法；`to_upper`/`to_lower` 逐字节 push_byte 拷贝到新缓冲，比较链区间判断（`97 <= b <= 122` / `65 <= b <= 90`）±32 转换，非 ASCII 多字节不转换；`trim` 剥离首尾空白（空格 32/制表 9/换行 10/回车 13）——正向扫描跳过开头空白找 start、反向扫描跳过结尾空白找 end（嵌套单条件 while + 标志变量，规避 while 头 `&&` 复合条件），再 `substring` 返回新缓冲，全空白串返回空串）；`string_case_trim_test.rs` 6 用例：基本转换（大小写互转 + 再转还原）/混合大小写 + 数字标点原样/空串与单字符/基本裁剪（多空白 + 仅首尾 + 内部空格保留）/全空白 → 空串 + 再裁剪幂等/链式组合（trim + to_upper + substring + find + contains））
- [x] **`String` 常用操作：`starts_with` + `ends_with` + `replace`**（core.zeta 纯 Zeta 方法；`starts_with`/`ends_with` 逐字节比较 + 标志变量，空前缀/空后缀恒 true、目标长于自身恒 false、大小写敏感，result 变量模式返回；`replace(old, new)` 滑动窗口扫描——命中处 `push_str(new)` 追加并跳过 `old.len`（不回溯），否则追加原字节，空 `old` 特判返回自身拷贝防死循环，未命中返回新拷贝原串不受影响）；`string_common_ops_test.rs` 6 用例：前缀基本（命中/不命中/空串/长于自身/全等/大小写敏感）/后缀基本（同构）/前后缀 + 嵌套 if 组合 + substring 提取协议 + find 定位路径/替换基本（单次/多次/未命中原样/内容相等/原串不变量）/替换边界（空 old 拷贝/空 new 删除/old == new/替换串更长）/链式组合（trim + replace + 前后缀 + substring + find + contains））
- [x] **`String` 数值转换：`int_to_string` + `string_to_int`**（core.zeta 顶层自由函数，i64 与 String 互转；`int_to_string(n)` 负数加 '-'（45）前缀 + 0 特判输出 '0'，先算位数 ndigits 再以位权 place（10^(ndigits-1)）从最高位 `(v / place) % 10` 逐位输出，全部 i64 算术；`string_to_int(s)` 逐字符累加 `result * 10 + (c - 48)`，'-' 前缀为负，遇非数字字符停止解析，空串/无数字返回 0）；`string_conv_test.rs` 6 用例：整数转字符串基本（零特判/多位数/拼接结果）/负数（负号前缀 + 数值正确）/转换结果 + String 操作组合（len/starts_with/ends_with/拼接/substring 跳过负号）/字符串转整数基本（前导零/可参与算术）/边界（负数/空串/遇非数字停止/开头非数字/前导空白停止）/往返 + 算术组合（数字→字符串→数字一致/负数往返/乘法除法取模结果转字符串/解析参与比较））
- [x] **`Vec<T>` 常用操作：`contains` + `remove` + `insert` + `clear`**（core.zeta 纯 Zeta 泛型 impl 方法，走 `unify` 统一类型参数 + `instantiate_impl_method` 单态化路径；`contains(x)` O(n) 遍历 `self.data[i] == x`——`==` 在实例化后按具体类型比较（i64 数值相等 / String 内容相等）；`remove(i)` 保存返回值 + 其后元素逐位前移（浅拷贝槽）+ len -= 1，越界（i >= len）以 `loop {}` 充当崩溃替代；`insert(i, x)` 从尾部反向搬移避免正向覆盖 + len += 1 + 容量不足先 grow，i == len 等价 append；`clear()` 仅重置 len（槽区残留被后续 push 覆盖写，无需释放））；`vec_common_ops_test.rs` 6 用例：contains 基本（命中/未命中/空 Vec/多出现）/contains String 元素（内容相等/大小写敏感/子串不等于元素）/remove 基本（删中间/头部/尾部 + 返回值 + 剩余顺序）/remove 序列（连续删除到空 + 复用 push）/insert 基本（中间/头部/尾部插入 + 连续插入扩容 cap 4→8→16）/混合组合（push + insert + remove + contains + clear 全链路最终断言））
- [x] **`Vec<T>` 查找/排序：`find` + `sort`**（core.zeta 纯 Zeta 泛型 impl 方法；`find(x)` result 变量模式返回首个相等元素下标，命中 `i = self.len` 提前终止保住首个下标，未命中 -1（对齐 String::find）；`sort()` 原地选择排序——外循环固定 `min = i`、内循环 `self.data[j] < self.data[min]` 扫最小值，`min != i` 时临时变量三行 swap，O(n²) 非稳定，`<` 在实例化后按具体类型比较（i64 数值 / String 字典序））；`vec_find_sort_test.rs` 6 用例：find 基本（首个命中非末个/头部/尾部/未命中 -1/空 Vec）/find String 元素（内容相等下标/未命中/大小写敏感）/sort 基本（乱序升序 + len 不变 + 首尾）/sort 边界（已排序不动/单元素/空 Vec/重复元素升序）/sort String（字典序 apple→pear）/组合（sort + find 排序后下标 + remove + contains + len））
- [x] **`String` 分割/重复/填充：`split` + `repeat` + `pad_start` + `pad_end`**（core.zeta 纯 Zeta 方法；`split(sep)` 滑动窗口匹配返回 `Vec<String>`——命中处 `substring(start, i)` 推段、跳过 `sep.len` 不回溯，空 sep 特判返回整体自身拷贝防死循环，连续分隔符产生空串段、尾部分隔符尾空段（每段新缓冲）；`repeat(count)` 循环逐字节 push_byte 拷贝（`push_str` 参数为值 String，`&self` 无法直传，改用与 pad 同构的拷贝循环）；`pad_start`/`pad_end(total, pad)` pad 字节（如 48='0'/45='-'）重复填充到总长，total <= len 原样拷贝，空串可填充）；`string_split_pad_test.rs` 6 用例：split 基本（逗号多段/不含分隔符单段/len + get 访问）/split 边界（空 sep 整体拷贝/连续分隔符空串段/尾部分隔符尾空段/空串）/split + 操作组合（to_upper/len/拼接/前后缀）/repeat 基本（3 次/0 次空/1 次自身/内容相等）/pad 基本（左填充 00042/右填充 42000/负号填充 42--/不足原样/空串填充 000）/组合链式（trim + split + to_upper + repeat + pad_start/pad_end 全链路））\n- [x] **`String` 前后缀剥离/截断：`strip_prefix` + `strip_suffix` + `truncate`**（core.zeta 纯 Zeta 方法；`strip_prefix(prefix)`/`strip_suffix(suffix)` 逐字节窗口比较（与 split 同构），命中返回去掉前后缀的剩余部分（新缓冲）、未命中 `Option::None`——`let mut result: Option<String> = Option::None;` 显式注解定型（`Option::None` Infer 占位需上下文），空前缀/空后缀恒命中返回自身拷贝、长于自身恒 None；`truncate(new_len)` 直接委托 `substring(0, new_len)`——负数 clamp 空串、超长整体拷贝、返回新缓冲不影响 self）；`string_strip_trunc_test.rs` 6 用例：strip_prefix 基本（命中剩余 llo/未命中 is_none/完全相等剩余空串）/strip_prefix 边界（空前缀整体拷贝/长于自身/部分前缀/首字节相同后续不同）/strip_suffix 基本（命中剩余 hel/未命中/完全相等空串）/strip_suffix 边界（空后缀整体拷贝/扩展名剥离 filename/长于自身/尾段相近非后缀）/truncate 基本（前 3 字节/0 空串/负数空串/恰好等长/超长整体拷贝/原串不受影响）/组合（trim + strip_prefix + to_upper/pad_end 填充/truncate + repeat/前后各剥一次取 middle））
- [x] **修复：LIR 跨函数返回类型推断——「返回用户函数调用结果」的函数被误判为 i64**（既有 bug：`infer_function_types` 在函数内独立推断，用户 `Call` 的 target 类型不解析（占位 i64），导致 `String::trim` 这类末尾为 `let result = self.substring(...); result` 的函数返回类型被固化 I64，而实际值（substring 返回值 = Ptr i8*）不符，clang 报 `ret i8*` 与函数结果类型 `i64` 不符；修复：提取 `resolve_call_target_types`（lower_function 原 307-341 跨函数解析逻辑），`lower_program` 建立 ret_types 后新增**迭代重算循环**——用 ret_types 解析各函数调用目标 → `infer_return_type` 重算 → 重建 ret_types，迭代至稳定（上限 8 遍）处理链式依赖，第二遍 lower_function 的 return_type 取重算后的全程序表；`string_case_trim_test.rs` 首现该 bug 并随修复全绿）
- [x] **修复：MIR `lower_if` 条件为含控制流的块表达式时「基本块已有终止符」**（既有 bug：`lower_if` 求值 cond 后强制 `self.cur = entry_id` 再发 `CondJump`，当 cond 为含 if 的块（如 `if y > m` 中 `y > m` desugar 出的 min 计算块）时入口块已被内层 if 闭合导致断言崩溃；修复：cond 求值后记录收尾块 `emit_id`，`CondJump` 从收尾块发出——无分支时收尾块 == 入口块行为不变，有分支时正确承载分叉；`lower_while` 本为"发到当前块"模式天然兼容）
- [x] **跨模块路径表达式**（`mod::Enum::Variant` 多段路径 + 模块常量引用 `mod::CONST` + match 模式多段路径 `AstPattern::EnumPath`）
- [x] **数组字面量与索引访问**（AST `ArrayLit` + HIR `Index`/`IndexSet` + MIR/LIR `IndexGet`/`IndexSet` + LLVM GEP 代码生成；数组元素步长 8 字节、字符串字符步长 1 字节；`resolve_ast_type` 保留数组长度 `[T; N]`）
- [x] **标准库 `Vec<T>` 动态数组**（`core.zeta` 纯 Zeta 实现 + 内建 `alloc_array`/`array_copy`/`array_free` 三处登记与 LLVM 映射；`[T; 0]` 长度约定 = 动态数组指针；Vector 3 槽布局 `data(指针)/len/cap`；自动扩容翻倍 + memcpy 迁移；泛型方法实例化 `push`/`pop`/`get`/`set` 全链路可用）
- [x] **标准库 `String` 动态字符串**（`core.zeta` 纯 Zeta 实现，UTF-8 字节缓冲 3 槽布局 `data([u8;0])/len/cap`；构造器 `from`/`new`/`with_capacity` 编译器特判展开（`alloc_bytes` + `copy_bytes` 内建）；`println(String)` 经 `print_string`/`println_string` 内建以 `printf("%.*s", len, data)` 输出；`u8` 字节数组与 String 索引统一步长 1 字节 + load i8/zext、store 前 trunc 的 1 字节读写；修复 `check_if` 非数值分支类型合并 bug）
- [x] **标准库 `HashMap<K,V>` 开放寻址哈希表**（`core.zeta` 纯 Zeta 实现，6 槽布局 `keys([K;0])/vals([V;0])/states([i64;0])/len/used/cap`；`states` 0=空 1=占用 2=墓碑；线性探测 + 墓碑删除不破坏探测链；负载因子 used/cap ≥ 1/2 时翻倍 rehash（新开三数组 + 重哈希，`alloc_array` 需类型注解定型）；`hash_value` 内建三处登记（typecheck `(Infer)->I64` / LIR 返回 I64 / codegen `mul i64, 2654435761` Knuth 乘法散列，MVP 仅支持整数键）；`HashMap::new`（cap 8）/`with_capacity(n)` 编译器特判展开；API：`insert`（同键覆盖）/`get`/`remove`/`contains_key`/`len`/`cap`/`is_empty`；修复块尾 `while ... }` 后直接跟 `-1` 被 parser 解析为减法的隐患（find 改用 `result` 变量返回））

### Milestone 3 — 生态繁荣（Month 8-12）

- [ ] **M3.1** 数据库驱动（SQLite, PostgreSQL, MySQL）
- [ ] **M3.2** HTTP 框架
- [ ] **M3.3** 序列化库（JSON, Protobuf）
- [ ] **M3.4** 嵌入式支持（RTOS, 裸机）
- [ ] **M3.5** GPU 计算后端
- [ ] **M3.6** 官方教程 + 认证考试

---

## 7. 编码规范

### 7.1 Rust 代码（编译器实现）

- 遵循 `rustfmt` 默认配置
- 每个 crate 必须有 `lib.rs` 或 `main.rs`
- 公共 API 必须有文档注释（`///`）
- 测试覆盖率目标：核心模块 ≥ 80%

### 7.2 Zeta 代码（标准库 + 示例）

- 使用 `zeta fmt` 格式化（语言确定后实现）
- 文件扩展名：`.zeta`
- 模块声明：`mod foo { ... }`
- 导入：`use foo::bar;`

### 7.3 提交规范

```
feat(lexer): add support for 'in' keyword
fix(parser): handle edge case in comparison chain
docs(grammar): clarify in-set precedence
test(borrowck): add test for partial move
refactor(mir): simplify CFG construction
```

格式：`<type>(<scope>): <description>`

| Type | 用途 |
|------|------|
| feat | 新功能 |
| fix | Bug 修复 |
| docs | 文档变更 |
| test | 测试相关 |
| refactor | 重构 |
| perf | 性能优化 |
| chore | 构建/工具链 |

---

## 8. 测试策略

### 8.1 单元测试

每个 crate 内 `tests/` 目录或 `#[test]` 标注。

```rust
// crates/zeta-lexer/src/lib.rs
#[test]
fn test_comparison_chain_tokens() {
    let source = "if 0 < x < 10 {}";
    let tokens = lex(source);
    assert_eq!(tokens.len(), 8); // if, 0, <, x, <, 10, {, }
}
```

### 8.2 集成测试

`tests/` 目录下的 `.zeta` 文件，通过 `zeta test` 运行。

```
tests/
├── compile-pass/    ← 应该编译通过的用例
│   ├── basic-types.zeta
│   ├── region-basic.zeta
│   └── actor-simple.zeta
├── compile-fail/    ← 应该编译失败的用例（含预期错误信息）
│   ├── borrowck-double-mut.zeta
│   ├── region-escape.zeta
│   └── comparison-chain-invalid.zeta
└── run-pass/         ← 编译并运行，检查输出
    ├── hello-world.zeta
    ├── sort-bench.zeta
    └── actor-pingpong.zeta
```

### 8.3 基准测试

```rust
// crates/zeta-lexer/benches/lexer_bench.rs
use criterion::{black_box, Criterion};

fn bench_large_file(c: &mut Criterion) {
    let source = std::fs::read_to_string("large_input.zeta").unwrap();
    c.bench_function("lex_large_file", |b| {
        b.iter(|| lex(black_box(&source)))
    });
}
```

### 8.4 模糊测试

```rust
// crates/zeta-parser/fuzz/fuzz_target.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        let _ = parse(source);  // 不应该 panic
    }
});
```

---

## 9. CI/CD 流水线

### 9.1 GitHub Actions 配置要点

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace --all-features
      - run: cargo bench --no-run
      
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: cargo clippy --workspace -- -D warnings
      - run: cargo fmt --all -- --check
      
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install cargo-fuzz
      - run: cargo fuzz run lexer_fuzz -- -max_total_time=60
```

### 9.2 发布流程

1. 更新 `CHANGELOG.md`
2. 打 tag：`git tag v0.x.0`
3. CI 自动构建各平台二进制
4. 上传到 GitHub Releases
5. 更新 `zep` 注册表中的版本信息

---

## 10. 贡献指南

### 10.1 如何贡献

1. Fork 本仓库
2. 创建分支：`git checkout -b feat/your-feature`
3. 编写代码 + 测试
4. 确保 `cargo test` 和 `cargo clippy` 通过
5. 提交 PR，描述变更内容和动机

### 10.2 优先级标签

| 标签 | 含义 |
|------|------|
| `good-first-issue` | 适合新手 |
| `compiler-core` | 编译器核心组件 |
| `std-lib` | 标准库 |
| `tooling` | 工具链 |
| `language-design` | 语言设计讨论 |
| `performance` | 性能相关 |

### 10.3 决策流程

- **小改动**（bug fix、文档）：直接 PR
- **中改动**（新语法特性）：先开 Issue 讨论，再 PR
- **大改动**（内存模型变更、新层级）：RFC 流程

---

## 11. 关键设计决策记录

### ADR-001：比较链方向反转表示区间外

- **决策**：`0 > x > 10` 表示 `x < 0 || x > 10`
- **理由**：数学直觉对称，零学习成本
- **替代方案**：`outside` 等关键词 → 被否决（繁琐）

### ADR-002：区域默认使用 bump allocator + 链表扩容

- **决策**：区域内 bump allocate，溢出时分配新块（翻倍），不拷贝旧数据
- **理由**：扩容 O(1)，总开销远低于逐个 malloc
- **替代方案**：realloc 拷贝 → 被否决（大数据时拷贝开销大）

### ADR-003：Transfer 是编译期操作

- **决策**：`transfer` 不拷贝数据，只转移所有权簿记
- **理由**：零运行时开销，对象仍在原堆位置
- **约束**：不能 transfer 引用，不能部分 transfer

### ADR-004：编译器后端选择 LLVM + Cranelift 双后端

- **决策**：开发期用 Cranelift（编译快），发布用 LLVM（优化强）
- **理由**：兼顾开发体验和生产性能

---

## 12. 参考资源

| 资源 | 链接/说明 |
|------|-----------|
| Rust 参考手册 | 所有权系统设计参考 |
| LLVM 文档 | 后端代码生成 |
| Cranelift 文档 | 快速 JIT 编译 |
| PubGrub 算法论文 | 包管理器依赖解析 |
| Immix GC 论文 | 可选 GC 层级参考 |
| Erlang Actor 模型 | 并发模型设计参考 |

---

## 13. 联系与沟通

| 渠道 | 用途 |
|------|------|
| GitHub Issues | Bug 报告、功能请求 |
| GitHub Discussions | 设计讨论、Q&A |
| Discord | 实时沟通 |
| 邮件列表 | 正式 RFC 讨论 |

---

> **最后更新**：2026-08-20  
> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
