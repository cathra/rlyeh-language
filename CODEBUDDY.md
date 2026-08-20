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
│   └── zeta-std/             ← 标准库（Zeta 源码）
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
│   ├── http-server.zeta
│   ├── actor-chat.zeta
│   └── region-bench.zeta
│
├── tests/                    ← 集成测试
│   ├── compile-pass/
│   ├── compile-fail/
│   └── run-pass/
│
├── prompts/                  ← CodeBuddy Prompt 文档
│   ├── README.md
│   ├── QUICKSTART.md
│   └── P001-P010_*.md
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
| `zeta test` | 运行测试 | 🔧 开发中 |
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
- [x] **M1.4** 类型检查器基础（比较链 + `in` 表达式语义；泛型/trait 待扩展）
- [x] **M1.5** 借用检查器（L0 所有权系统：use-after-move + 不可变绑定赋值，区域块值传递例外；借用互斥规则待 `&` 语法支持）
- [x] **M1.6** 区域系统（`zeta-region-alloc` bump allocator + 四策略 + LIFO 析构 + `execute_transfer` 所有权句柄；`zeta-regionck` 嵌套/归属/transfer 合法性 + P005 嵌套方向/PartialTransfer 语义；MIR lowering 待 P007+）
- [x] **M1.7** MIR + 基础优化 passes（CFG lowering：if/while/loop/break/continue + 区域指令显式化；常量折叠、DCE、小函数内联）
- [x] **M1.8** 代码生成（LLVM 后端，x86_64 Linux）
- [x] **M1.9** 能编译并运行 `hello-world.zeta`

### Milestone 2 — 生产可用（Month 4-8）

- [x] **M2.1** Actor 运行时（`zeta-actor-runtime`：工作窃取调度 + 每 Actor 互斥处理 + 有界邮箱 + ask/reply 模式 + Supervisor 监督恢复（OneForOne/AllForOne/RestartForOne + 频率限制）+ 内置 Router/Timer + 优雅排空关闭；`ActorRef`/`Runtime` API 就绪，语言级 `actor`/`spawn` 语法与标准库集成待 M2.5）
- [x] **模块系统**（嵌套 `mod {}` + `mod foo;` 多文件加载（`foo.zeta` / `foo/mod.zeta`）+ `use` 导入与别名 + 扁平符号名 `mod::item` + LLVM 引号标识符接线；typecheck 支持模块内函数/常量/结构体符号解析）
- [ ] **M2.2** 包管理器 Zep（依赖解析 + 注册表）
- [x] **M2.3** 增量编译引擎（源码/接口哈希 + LLVM IR 产物缓存 + 依赖图 + 多文件模块编译缓存）
- [ ] **M2.4** LSP 服务器（IDE 支持）
- [ ] **M2.5** 标准库核心模块（collections, io, net, sync）
- [ ] **M2.6** 交叉编译（macOS, Windows, ARM）
- [ ] **M2.7** WASM 目标支持
- [ ] **M2.8** PGO 支持

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

> **最后更新**：2026-08-19  
> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
