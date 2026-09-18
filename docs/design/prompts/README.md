# Rlyeh 开发任务书索引（归档）

> **目录定位**：本目录归档 Rlyeh 编译器/工具链从零起步的 13 份开发任务书（Prompt）及其执行记录。
> **归档说明（2026-08-24）**：P001–P013 **已全部完成**；各任务书的持久内容（实现决策、落地偏差、
> 已知限制、关键 bug）已提炼并入 `docs/` 对应权威文档的**"附录 A：实现纪要"**，本目录仅保留原始任务书供追溯。
> 新功能开发以 [`../../docs/`](../../) 权威规范与 [`../../CODEBUDDY.md`](../../../CODEBUDDY.md) 为准。
> 执行记录：阶段 A–Z 见 [`../../docs/development-plan.md`](../../development-plan.md)（阶段 A–F 已完成 + §6 剩余任务消解 G–L / M–T / U–Z）。
> 设计稿 → 权威规范 → 实现任务 三方映射见 [`../README.md`](../README.md)（本目录上一级为设计稿归档区）。

---

## 1. Prompt 索引（P001–P013，全部完成 ✅）

| 编号 | 文件 | 模块 | 对应设计稿 | 预估工期 |
|------|------|------|-----------|----------|
| P001 | [P001_词法分析器核心.md](./P001_词法分析器核心.md) | `rlyeh-lexer` | [design/01](../01_词法分析器.md) | 3-5 天 |
| P002 | [P002_语法分析器核心.md](./P002_语法分析器核心.md) | `rlyeh-parser` | [design/02](../02_语法分析器.md) | 5-7 天 |
| P003 | [P003_比较链语义分析.md](./P003_比较链语义分析.md) | `rlyeh-typecheck` | [design/03](../03_类型系统.md) / [design/06](../06_比较链与条件判断.md) | 3-5 天 |
| P004 | [P004_区域系统实现.md](./P004_区域系统实现.md) | `rlyeh-regionck` | [design/05](../05_区域内存管理系统.md) | 5-7 天 |
| P005 | [P005_Transfer语义实现.md](./P005_Transfer语义实现.md) | `rlyeh-regionck` | [design/05](../05_区域内存管理系统.md) | 3-5 天 |
| P006 | [P006_Actor运行时.md](./P006_Actor运行时.md) | `rlyeh-actor-runtime` | [design/07](../07_Actor并发模型.md) | 5-7 天 |
| P007 | [P007_增量编译引擎.md](./P007_增量编译引擎.md) | `rlyeh-driver` | [design/09](../09_工具链设计.md) | 5-7 天 |
| P008 | [P008_包管理器Zep.md](./P008_包管理器Zep.md) | `dagon` | [design/09](../09_工具链设计.md) | 5-7 天 |
| P009 | [P009_标准库核心模块.md](./P009_标准库核心模块.md) | `rlyeh-std` | [design/10](../10_标准库规划.md) | 5-7 天 |
| P010 | [P010_智能区域分配器.md](./P010_智能区域分配器.md) | `rlyeh-region-alloc` | [design/05](../05_区域内存管理系统.md) | 5-7 天 |
| P011 | [P011_MIR中间表示实现.md](./P011_MIR中间表示实现.md) | `rlyeh-mir` | [design/08](../08_编译器后端与代码生成.md) | 5-7 天 |
| P012 | [P012_L0借用检查器实现.md](./P012_L0借用检查器实现.md) | `rlyeh-borrowck` | [design/04](../04_所有权与借用检查器.md) | 3-5 天 |
| P013 | [P013_LLVM后端与代码生成.md](./P013_LLVM后端与代码生成.md) | `rlyeh-lir`+`rlyeh-codegen`+`rlyeh-driver` | [design/08](../08_编译器后端与代码生成.md) | 7-10 天 |

**总计**：约 59-98 天（单人），2-3 个月（3-4 人团队）

---

## 2. 依赖关系

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

> 注：P006 的精确前置依赖为 P004；P007 的精确前置依赖为 P002-P005；P010 的精确前置依赖为 P004 + P007（P007 已隐含 P004，故图中从 P007 引出）；P012 的前置为 P005（Transfer 语义提供所有权转移路径）；P011 的前置为 P004 + P005 + P012（MIR 显式化区域与 transfer 指令）；P013 的前置为 P011（LIR lowering 依赖 MIR CFG）。

---

## 3. 使用方式（历史指引）

> 以下流程为 P001–P013 开发阶段的用法；**当前所有 Prompt 均已完成**。
> 新增开发任务请直接参考 `docs/development-plan.md`（阶段 A–Z）的阶段划分，不必回放历史任务书。

1. 在下方索引中选择要执行的 Prompt
2. 全选复制对应 `Pxxx_*.md` 文件内容
3. 粘贴到 CodeBuddy，按文档要求生成代码
4. 跑通测试后进入下一个 Prompt

---

## 4. 进度追踪（历史记录，已全部完成）

| Prompt | 状态 | 完成日期 | 测试通过率 | 备注 |
|--------|------|----------|------------|------|
| P001 | ✅ 完成 | 2026-08-19 | 35/35 (100%) | 20 单元 + 14 集成 + 1 doc |
| P002 | ✅ 完成 | 2026-08-19 | 62/62 (100%) | 44 单元 + 15 集成 + 3 伪模糊；clippy 零警告；fuzz 发现并修复 const EOF panic |
| P003 | ✅ 完成 | 2026-08-19 | 100% | 比较链 + in 表达式语义 |
| P004 | ✅ 完成 | 2026-08-19 | 100% | bump allocator + 四策略 + LIFO 析构 + regionck 嵌套/归属/transfer |
| P005 | ✅ 完成 | 2026-08-20 | 100% | transfer 嵌套方向/PartialTransfer 语义；遗留 use-after-move 由 P012 补齐 |
| P006 | ✅ 完成 | 2026-08-20 | 15/15 (100%) | Actor 运行时（工作窃取调度 + 邮箱互斥 + ask/reply + Supervisor 恢复 + Router/Timer + 优雅关闭）；补齐 M2.1 |
| P007 | ✅ 完成 | 2026-08-20 | 24/24 (100%) | 增量编译（源码/接口哈希 + 多版本产物缓存 + 损坏恢复 + 依赖图 + 多文件模块缓存）；补齐 M2.3 |
| — | ✅ 完成 | 2026-08-20 | 7/7 (100%) | 模块系统（嵌套 `mod` + `use` 导入别名 + `mod foo;` 多文件加载 + 扁平符号名 + 模块内符号解析）——parser/typecheck/driver/codegen 跨层联动 |
| P008 | ✅ 完成 | 2026-08-20 | 29/29 (100%) | 包管理器 Dagon（pubgrub 依赖解析 + 本地/HTTP 注册表 + 打包解包 + 构建驱动）；补齐 M2.2 |
| — | ✅ 完成 | 2026-08-20 | 10/10 (100%) | 聚合对象语言特性（enum + match + impl + protocol + 泛型单态化） |
| — | ✅ 完成 | 2026-08-20 | 62/62 (100%) | 聚合对象语言特性扩展（`impl<T>` 泛型 self、语句式 while/for/loop/region、struct 字面量、`&self`/`&mut self` 方法、match 引用解构、loop Never、泛型替换下沉模式） |
| — | ✅ 完成 | 2026-08-20 | 63/63 (100%) | 标准库预置接入 + 聚合类型全链路修复（stdlib 搜索路径 + LIR 类型传播 + MIR Never 分支 phi） |
| P009 | ✅ 完成 | 2026-08-20 | 6/6 (100%) | 标准库核心类型（Option/Result 纯 Rlyeh 实现于 core.rl）+ 编译器标准库搜索路径接入 |
| — | ✅ 完成 | 2026-08-20 | 9/9 (100%) | for 循环 range 迭代器（desugar 为 loop + 临时边界变量；修复 continue 死循环） |
| — | ✅ 完成 | 2026-08-20 | 11/11 (100%) | 跨模块路径表达式（`mod::Enum::Variant`）+ 模块常量引用 + match 多段路径模式 |
| — | ✅ 完成 | 2026-08-20 | 12/12 (100%) | 索引访问 `a[i]` 与数组字面量（Alloc + 逐元素 FieldSet；GEP+bitcast+load；步长 8/1 字节）——M2.5 collections 硬前提 |
| P010 | ✅ 完成 | 2026-08-20 | 28/28 (100%) | 智能区域（SmartRegion：静态大小推断 + PGO 画像 + EWMA 自适应扩容 + 碎片统计 + criterion 基准）；补齐 M2.8 分配器侧 |
| P011 | ✅ 完成 | 2026-08-20 | 18/18 (100%) | MIR CFG lowering + 常量折叠/DCE/内联；修复 new_block 终止符错位 bug |
| P012 | ✅ 完成 | 2026-08-20 | 16/16 (100%) | L0 借用检查（use-after-move + 不可变赋值）；补齐 M1.5 |
| P013 | ✅ 完成 | 2026-08-20 | 18/18 (100%) | LIR 三地址码 + LLVM IR 文本 + driver 端到端（clang 汇编/链接/运行）；补齐 M1.8/M1.9 |

**状态标记**：⏳ 待开始 | 🔧 进行中 | ✅ 完成 | ❌ 阻塞

> 阶段 A–F 的扩展任务（模块系统、io/net、FFI、actor 语言级接线、标准库扩展、交叉编译/WASM/发布、LSP、PGO 回灌）执行记录见 [`../docs/development-plan.md`](../../development-plan.md)。

---

## 5. 快速验证（MVP 现状）

```bash
# 创建并运行新项目
rlyeh new hello_world
cd hello_world
rlyeh run src/main.rl        # 预期输出：Hello, Rlyeh!

# 手动编译 hello world（内建打印，无宏）
echo 'fn main() { println("Hello, Rlyeh!"); }' > test.rl
rlyeh run test.rl            # 预期输出：Hello, Rlyeh!

# 测试比较链与集合判断
echo 'fn main() {
  let x = 5;
  if 0 < x < 10 { println("in range"); }
  if x in (1, 3, 5) { println("in set"); }
}' > test2.rl
rlyeh run test2.rl
# 预期输出：in range\nin set

# 测试区域系统（区域名用单引号 'r，打印用 println(x) 而非 println!("{}", x)）
echo 'fn main() {
  region '\''r'\'' {
    let x = 42 in '\''r'\'';
    println(x);
  }
}' > test3.rl
rlyeh run test3.rl
# 预期输出：42

# 测试动态切片（数组 / Vec）
echo 'fn main() {
  let arr = [10, 20, 30, 40, 50];
  let a = arr[1..<3];
  println(a.len());
  println(a.get(0));
}' > test4.rl
rlyeh run test4.rl
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
}' > test5.rl
rlyeh run test5.rl
# 预期输出：10

# 全量回归
cargo test --workspace        # 全部通过（当前 111 套件）
```

---

## 6. 通用约定

所有 Prompt 共享以下约定：

### 代码风格

- Rust 2021 edition
- `cargo fmt` 格式化
- `cargo clippy -- -D warnings` 零警告
- 公共 API 必须有 `///` 文档注释
- 错误类型使用 `thiserror`

### 测试要求

- 每个公开函数至少一个单元测试
- 每个模块至少一个集成测试
- 性能敏感模块必须有 `criterion` 基准测试
- 解析器必须有 `libfuzzer` 模糊测试

### 文件头模板

```rust
//! # rlyeh-xxx
//!
//! 简要描述本模块的功能。
//!
//! ## 使用示例
//!
//! ```rust
//! // 示例代码
//! ```

#![warn(missing_docs)]
#![warn(unsafe_code)]
```

---

## 7. 项目文档导航

| 文档 | 路径 | 说明 |
|------|------|------|
| 项目总纲 | [../CODEBUDDY.md](../../../CODEBUDDY.md) | 项目全景 + 工具链命令 + 特性速览 |
| 文档导航（docs 入口） | [../docs/README.md](../../README.md) | docs/ 文档地图 + 阅读顺序 + 一致性规则 |
| 开发计划（阶段 A–Z） | [../docs/development-plan.md](../../development-plan.md) | 阶段 A–F 已完成 + §6 剩余任务消解 G–L / M–T / U–Z |
| 语言教程 | [../docs/guide/index.md](../../guide/index.md) | 面向读者（示例均可运行） |
| 语法规范 | [../docs/grammar.md](../../grammar.md) | EBNF 语法（含规划） |
| 语义规则 | [../docs/semantics.md](../../semantics.md) | 类型/求值规则 |
| 内存模型 | [../docs/memory-model.md](../../memory-model.md) | 分层内存 |
| Actor 模型 | [../docs/actor-model.md](../../actor-model.md) | 并发规范 |
| 标准库 API | [../docs/std-lib.md](../../std-lib.md) | API 规范（已实现/规划） |
| 早期设计稿归档 | [../docs/design/README.md](../README.md) | 设计稿 ↔ 权威规范 ↔ 任务映射 |

---

> **维护者**：Rlyeh Language Team
> **License**：MIT / Apache-2.0
