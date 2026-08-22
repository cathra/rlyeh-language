# Zeta CodeBuddy 快速开始指南 (5 分钟版)

> 本文件帮助你快速了解 Zeta 项目：项目结构、Prompt 执行流程与快速验证方法。
> P001–P013 开发 Prompt 已全部完成（MVP 已可用），本文件用于新成员上手与现状验证。

---

## 1. 项目是什么？

Zeta 是一门**系统级编程语言**，设计目标是：

| 对标 | 能力 | MVP 现状 |
|------|------|----------|
| Rust | 内存安全、零 GC | 分层内存模型（L0 部分 / L1 区域已实现；Rc/Arc/Gc 规划中） |
| Go | 编译速度极快 | 编译器 Rust 实现（bootstrap），增量编译已实现 |
| Erlang | Actor 并发模型 | `actor` 语言级构造 + 监督重启已实现 |
| Python | 数学式语法直觉 | 比较链 `0 < x < 10`、集合判断 `x in (1, 3, 5)` 已实现 |

**当前状态**：MVP 开发中（阶段 A–F 全部完成，`cargo test --workspace` 全绿；见 [`docs/development-plan.md`](../docs/development-plan.md)）。

---

## 2. 项目结构一览

```
zeta-language/
├── CODEBUDDY.md              ← 项目总纲（架构、工具链命令、执行记录）
├── CHANGELOG.md              ← 版本变更记录
├── Cargo.toml                ← Rust 工作区配置
├── docs/                     ← 语言规范 + 开发文档
│   ├── guide.md              ← 语言教程（面向读者，示例均可运行）
│   ├── grammar.md            ← EBNF 语法规范（目标语法，含规划）
│   ├── semantics.md          ← 语义规则（目标规范，含规划）
│   ├── memory-model.md       ← 分层内存管理规范
│   ├── actor-model.md        ← Actor 并发模型规范
│   ├── std-lib.md            ← 标准库 API 规范（已实现 / 规划）
│   ├── development-plan.md   ← 开发计划（阶段 A–F，权威执行记录）
│   └── design/               ← 早期设计稿归档
├── prompts/                  ← CodeBuddy 开发 Prompt
│   ├── README.md             ← Prompt 索引（本目录入口）
│   ├── QUICKSTART.md         ← 本文件
│   └── P001–P013_*.md        ← 各阶段开发任务（已全部完成）
├── crates/                   ← 编译器各组件（含 zeta-lsp 等 15 个 crate）
├── tools/                    ← 工具链（zeta-fmt / zeta-check / zeta-doc / zeta-bench）
├── zep/                      ← 包管理器
├── examples/                 ← 示例代码
└── tests/                    ← 集成测试用例（compile-pass / compile-fail / run-pass）
```

---

## 3. 执行流程（历史指引，P001–P013 已全部完成）

> 以下流程是 P001–P013 开发阶段的用法。**当前所有 Prompt 均已完成**，若要新增开发任务，直接参考 `README.md` 或 `docs/development-plan.md` 的阶段划分，而不必回放历史 Prompt。

### 第一步：打开 Prompt 文件

用编辑器打开 `prompts/P001_词法分析器核心.md`。

### 第二步：全选复制

选中文件的**全部内容**（从第一行到最后一行）。

### 第三步：粘贴给 CodeBuddy

在 CodeBuddy 对话窗口中粘贴，发送。

### 第四步：等待 CodeBuddy 生成代码

CodeBuddy 会：创建 crate 目录结构 → 编写源码与测试 → 运行 `cargo test` 验证 → 报告结果。

### 第五步：验证通过 → 进入下一个 Prompt

打开 `prompts/P002_语法分析器核心.md`，重复上述流程。

---

## 4. 依赖关系图

```
P001 ──→ P002 ──→ P003 ──→ P004 ──→ P005 ──→ P012 ──→ P011 ──→ P013
                                       │
                              P006 ←───┤
                                       │
                        P007 ──→ P008 ──→ P009
                           │
                           └───→ P010
```

**关键路径**：P001 → P002 → P003 → P004 → P005 → P007 → P008 → P009（P012/P011/P013 为编译器主线支路）

> 注：P006 的精确前置依赖为 P004；P007 的精确前置依赖为 P002-P005；P010 的精确前置依赖为 P004 + P007（P007 已隐含 P004，故图中从 P007 引出）；P012 的前置为 P005；P011 的前置为 P004 + P005 + P012；P013 的前置为 P011。

---

## 5. 每个 Prompt 的预期产出（完成情况见 README.md）

| 编号 | 产出 | 验证方式 | 状态 |
|------|------|----------|------|
| P001 | `crates/zeta-lexer/` 可工作的词法分析器 | `cargo test` 全部通过 | ✅ |
| P002 | `crates/zeta-parser/` 可工作的语法分析器 | `cargo test` 全部通过 | ✅ |
| P003 | `crates/zeta-typecheck/` 比较链检查器 | `cargo test` 全部通过 | ✅ |
| P004 | `crates/zeta-region-alloc/` + `zeta-regionck/` | `cargo test` 全部通过 | ✅ |
| P005 | Transfer 语义完整实现 | `cargo test` 全部通过 | ✅ |
| P006 | `crates/zeta-actor-runtime/` 运行时 | `cargo test` 全部通过 | ✅ |
| P007 | `crates/zeta-driver/` 增量编译 | `cargo test` 全部通过 | ✅ |
| P008 | `zep/` 包管理器 | `cargo test` 全部通过 | ✅ |
| P009 | `crates/zeta-std/` 标准库 | `zeta test` 全部通过 | ✅ |
| P010 | 智能区域分配器（最终版） | `cargo test` 全部通过 | ✅ |
| P011 | `crates/zeta-mir/` + `zeta-lir/` MIR/LIR | `cargo test` 全部通过 | ✅ |
| P012 | `crates/zeta-borrowck/` L0 借用检查器 | `cargo test` 全部通过 | ✅ |
| P013 | `crates/zeta-codegen/` LLVM 后端 + driver | `zeta run` 输出正确 | ✅ |

> 阶段 A–F 的执行记录与遗留问题清单见 [`docs/development-plan.md`](../docs/development-plan.md)。

---

## 6. 快速验证（MVP 现状）

```bash
# 创建并运行新项目
zeta new hello_world
cd hello_world
zeta run src/main.zeta        # 预期输出：Hello, Zeta!

# 手动编译 hello world（内建打印，无宏）
echo 'fn main() { println("Hello, Zeta!"); }' > test.zeta
zeta run test.zeta            # 预期输出：Hello, Zeta!

# 测试比较链与集合判断
echo 'fn main() {
  let x = 5;
  if 0 < x < 10 { println("in range"); }
  if x in (1, 3, 5) { println("in set"); }
}' > test2.zeta
zeta run test2.zeta
# 预期输出：in range\nin set

# 测试区域系统（区域名用单引号 'r，打印用 println(x) 而非 println!("{}", x)）
echo 'fn main() {
  region '\''r'\'' {
    let x = 42 in '\''r'\'';
    println(x);
  }
}' > test3.zeta
zeta run test3.zeta
# 预期输出：42

# 测试动态切片（数组 / Vec）
echo 'fn main() {
  let arr = [10, 20, 30, 40, 50];
  let a = arr[1..<3];
  println(a.len());
  println(a.get(0));
}' > test4.zeta
zeta run test4.zeta
# 预期输出：2\n20

# 测试 Actor 并发（ask 往返 + fire-and-forget）
echo 'actor Counter {
  value: i64 = 0,
  pub fn increment(amount: i64) -> i64 { self.value += amount; self.value }
}
fn main() {
  let c = Counter::new();
  println(c.increment(10).await);
  send c.increment(1);
}' > test5.zeta
zeta run test5.zeta
# 预期输出：10

# 全量回归
cargo test --workspace        # 全部通过（当前 111 套件）
```

---

## 7. 文档导航

| 文档 | 路径 | 说明 |
|------|------|------|
| 项目总纲 | [../CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 + 工具链命令 + 执行记录 |
| 语言教程 | [../docs/guide.md](../docs/guide.md) | 面向读者的教程（示例均可运行） |
| Prompt 索引 | [README.md](./README.md) | 13 个 Prompt 的索引与进度 |
| 语法规范 | [../docs/grammar.md](../docs/grammar.md) | EBNF 语法定义（目标语法，含规划） |
| 语义规则 | [../docs/semantics.md](../docs/semantics.md) | 类型/求值规则 |
| 内存模型 | [../docs/memory-model.md](../docs/memory-model.md) | 分层内存管理 |
| Actor 模型 | [../docs/actor-model.md](../docs/actor-model.md) | 并发模型规范 |
| 标准库 API | [../docs/std-lib.md](../docs/std-lib.md) | API 规范（已实现 / 规划） |
| 开发计划 | [../docs/development-plan.md](../docs/development-plan.md) | 阶段 A–F 执行记录 |

---

> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
