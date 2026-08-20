# Zeta CodeBuddy 快速开始指南 (5 分钟版)

> 本文件帮助你在 5 分钟内了解 Zeta 项目并启动第一个 Prompt。

---

## 1. 项目是什么？

Zeta 是一门**系统级编程语言**，设计目标是：

| 对标 | 能力 |
|------|------|
| Rust | 内存安全、零 GC |
| Go | 编译速度极快 |
| Erlang | Actor 并发模型 |
| Python | 数学式语法直觉 |

---

## 2. 项目结构一览

```
zeta-language/
├── CODEBUDDY.md              ← 项目总纲（你正在看的上一级文档）
├── Cargo.toml                ← Rust 工作区配置
├── docs/                     ← 语言规范文档
│   ├── grammar.md            ← EBNF 语法
│   ├── semantics.md         ← 语义规则
│   ├── memory-model.md      ← 分层内存管理
│   ├── actor-model.md       ← Actor 并发模型
│   └── std-lib.md            ← 标准库 API
├── prompts/                  ← ⭐ 核心交付物
│   ├── README.md           ← Prompt 索引（本目录入口）
│   ├── QUICKSTART.md      ← 本文件
│   ├── P001_词法分析器核心.md
│   ├── P002_语法分析器核心.md
│   ├── P003_比较链语义分析.md
│   ├── P004_区域系统实现.md
│   ├── P005_Transfer语义实现.md
│   ├── P006_Actor运行时.md
│   ├── P007_增量编译引擎.md
│   ├── P008_包管理器Zep.md
│   ├── P009_标准库核心模块.md
│   ├── P010_智能区域分配器.md
│   ├── P011_MIR中间表示实现.md
│   ├── P012_L0借用检查器实现.md
│   └── P013_LLVM后端与代码生成.md
├── crates/                   ← 编译器各组件（Rust crate）
├── tools/                    ← 工具链
├── zep/                      ← 包管理器
└── examples/                 ← 示例代码
```

---

## 3. 执行流程（给 CodeBuddy 的指令）

### 第一步：打开 Prompt 文件

用编辑器打开 `prompts/P001_词法分析器核心.md`。

### 第二步：全选复制

选中文件的**全部内容**（从第一行到最后一行）。

### 第三步：粘贴给 CodeBuddy

在 CodeBuddy 对话窗口中粘贴，发送。

### 第四步：等待 CodeBuddy 生成代码

CodeBuddy 会：
1. 创建 `crates/zeta-lexer/` 目录结构
2. 编写所有源代码文件
3. 编写所有测试文件
4. 运行 `cargo test` 验证
5. 报告结果

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

**不要跳步！** 每个 Prompt 依赖前一个的输出。

---

## 5. 每个 Prompt 的预期产出

| 编号 | 产出 | 验证方式 |
|------|------|----------|
| P001 | `crates/zeta-lexer/` 可工作的词法分析器 | `cargo test` 全部通过 |
| P002 | `crates/zeta-parser/` 可工作的语法分析器 | `cargo test` 全部通过 |
| P003 | `crates/zeta-typecheck/` 比较链检查器 | `cargo test` 全部通过 |
| P004 | `crates/zeta-region-alloc/` + `zeta-regionck/` | `cargo test` 全部通过 |
| P005 | Transfer 语义完整实现 | `cargo test` 全部通过 |
| P006 | `crates/zeta-actor-runtime/` 运行时 | `cargo test` 全部通过 |
| P007 | `crates/zeta-driver/` 增量编译 | `cargo test` 全部通过 |
| P008 | `zep/` 包管理器 | `cargo test` 全部通过 |
| P009 | `crates/zeta-std/` 标准库 | `zeta test` 全部通过 |
| P010 | 智能区域分配器（最终版） | `cargo test` 全部通过 |
| P011 | `crates/zeta-mir/` + `zeta-lir/` MIR/LIR | `cargo test` 全部通过 |
| P012 | `crates/zeta-borrowck/` L0 借用检查器 | `cargo test` 全部通过 |
| P013 | `crates/zeta-codegen/` LLVM 后端 + driver | `zeta run` 输出正确 |

---

## 6. 进度追踪

在 `prompts/README.md` 的进度表格中更新状态：

| Prompt | 状态 | 完成日期 | 测试通过率 |
|--------|------|----------|------------|
| P001 | ⏳ 待开始 | - | - |
| P002 | ⏳ 待开始 | - | - |
| ... | ... | ... | ... |

**状态标记**：⏳ 待开始 | 🔧 进行中 | ✅ 完成 | ❌ 阻塞

---

## 7. 常见问题

### Q: CodeBuddy 报错了怎么办？

1. 检查错误信息，看是编译错误还是测试失败
2. 把错误信息反馈给 CodeBuddy，让它修复
3. 如果反复失败，检查前序 Prompt 是否完全通过

### Q: 可以并行执行多个 Prompt 吗？

**不建议。** 虽然 P006 和 P007 之间没有硬依赖，但 CodeBuddy 一次处理一个任务质量更高。

### Q: 测试失败了要紧吗？

- 1-2 个非关键测试失败 → 可以让 CodeBuddy 修复后继续
- 大量测试失败 → 回到上一个 Prompt 检查

### Q: 需要多少时间？

| 团队规模 | 预估总工期 |
|----------|----------|
| 1 人 | 46-76 天 |
| 2 人 | 25-40 天 |
| 3-4 人 | 15-25 天（2-3 个月）|

---

## 8. 快速验证（全部完成后）

```bash
# 编译 hello world
echo 'fn main() { println!("Hello, Zeta!"); }' > test.zeta
zeta build test.zeta
./test
# 预期输出：Hello, Zeta!

# 测试比较链
echo 'fn main() {
  let x = 5;
  if 0 < x < 10 { println!("in range"); }
}' > test2.zeta
zeta build test2.zeta
./test2
# 预期输出：in range

# 测试区域系统
echo 'fn main() {
  region "r" {
    let x = 42 in "r";
    println!("{}", x);
  }
}' > test3.zeta
zeta build test3.zeta
./test3
# 预期输出：42
```

---

## 9. 文档导航

| 文档 | 路径 | 说明 |
|------|------|------|
| 项目总纲 | [../CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 + 里程碑 |
| Prompt 索引 | [README.md](./README.md) | 13 个 Prompt 的索引 |
| 语法规范 | [../docs/grammar.md](../docs/grammar.md) | EBNF 语法定义 |
| 语义规则 | [../docs/semantics.md](../docs/semantics.md) | 类型/求值规则 |
| 内存模型 | [../docs/memory-model.md](../docs/memory-model.md) | 分层内存管理 |
| Actor 模型 | [../docs/actor-model.md](../docs/actor-model.md) | 并发模型规范 |
| 标准库 API | [../docs/std-lib.md](../docs/std-lib.md) | API 规范 |

---

> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
