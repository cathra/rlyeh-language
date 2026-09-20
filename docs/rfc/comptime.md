# RFC：Rlyeh `comptime` 编译期求值与元编程系统

| 字段 | 内容 |
|------|------|
| 状态 | 评审通过（Accepted）；已并入 0.2.0 计划（阶段 Z：M0/M1 落 0.2.0，M2/M3/M4 留 0.3.0）；待并入 `grammar.md` / `semantics.md` |
| 日期 | 2026-09-20 |
| 范围 | 语言**编译期求值（CTFE）/ 元编程**能力：编译期代码执行、值/类型一等公民、生成式代码。不影响现有 LLVM 后端与运行时语义模型，仅在编译管线中新增「comptime 解释器」与「comptime→AST/类型 注入」环节。 |
| 关联文档 | [`grammar.md`](../grammar.md) §2（声明/表达式）、[`semantics.md`](../semantics.md)（类型/求值）、[`docs/tasks/leaf/sh-p2-9-const-static.md`](../tasks/leaf/sh-p2-9-const-static.md)（既有 `const`/`static` 折叠）、[`docs/performance.md`](../performance.md)（基准）、[`self-hosting/feasibility.md`](../self-hosting/feasibility.md)（0.3.0 自举） |
| 任务拆解 | 见 §9（M0–M4，建议挂 `tasks/leaf/comptime-*.md`） |

---

## 1. 背景与动机

Rlyeh 当前已具备：

- **`const` 常量折叠**（0.2.0-V 落地）：模块级 `const` 表达式在编译期求值为值，但**仅限常量表达式折叠**，不支持「在编译期执行任意函数/控制流」。
- **泛型（单态化）**（0.2.0-A）：类型参数 `T` 经单态化展开，但**值参数（如数组长度、缓冲区大小）尚不能作为泛型参数**。
- **声明式宏 / derive 宏**（0.2.0-C）：`vec!`/`arr!`/`println!`/`format!` 与 `#[derive(...)]` 走字符串/模式展开，是「文本级」元编程，**无类型安全、无调试栈、与 IDE/诊断割裂**。

三者在「需要编译期计算或代码生成」的场景下各自有缺口，且宏路线难以支撑 **0.3.0 自举**（自举要求编译器前端、codegen 表、序列化等能在编译期由语言自身生成）。

**核心主张**：引入统一的 **`comptime` 编译期求值系统**——把「编译期能跑的代码」作为语言一等公民，用**类型安全、可调试、可差分测试**的解释执行取代字符串宏展开。设计综合 Zig `comptime`、Rust `const fn`、Jai/Odin 的编译期泛型。

### 1.1 典型场景

```rlyeh
// 值泛型：数组长度由编译期值参数决定（当前不支持）
fn make_buffer(comptime n: i64) -> [i64; n] { [0; n] }

// 类型参数：编译期传入类型，生成特化实现
fn zero(comptime T: type) -> T { /* 由 T 决定构造 */ }

// 生成式代码：编译期生成一组相关类型/函数（自举关键能力）
comptime {
    for lang in ["rust", "c", "go"] {
        // 生成 enum Variant + match 分支
    }
}

// 编译期嵌入数据（构建期确定、零运行时 IO）
const TABLE = @embedFile("lookup.bin");
```

---

## 2. 目标与非目标

**目标（Goals）**
1. 在编译期执行**任意受控代码**（算术、控制流、循环、函数调用、聚合构造），结果注入 AST/类型。
2. 支持**值参数泛型**与**类型作为值**（`comptime T: type`），消除「长度/尺寸只能运行时化」的强制堆分配。
3. 提供**生成式元编程**：comptime 代码可构造新类型/函数并经正常 codegen 编译（非字符串展开）。
4. **可差分测试**：comptime 求值行为可由 gold oracle 对拍，复用 M-M1 lexer 的对拍方法论。
5. **可调试**：comptime 求值错误映射回源码 span，拥有编译期调用栈。

**非目标（Non-goals，本期）**
- 运行期反射（`typeid`/RTTI 在运行期查询类型信息）——本期只做编译期自省。
- 编译期多线程 / 编译期 IO 网络访问——comptime 执行须**确定性、封闭（hermetic）**。
- 用 comptime 替换全部既有宏——声明式/derive 宏保留，长期可作为 comptime 的语法糖。

---

## 3. 与现有 `const` / `static` / 泛型 / 宏的关系

| 既有能力 | 与 comptime 的关系 |
|----------|-------------------|
| `const` 折叠 | **comptime 的子集**：`const X = expr;` 等价于「在 comptime 上下文求值 `expr` 并冻结为值」。保留 `const` 语法，语义统一到 comptime 求值引擎。 |
| `static` / `static mut` | 运行期 data 段符号；`comptime` 不得使用（除非仅取其类型/地址的编译期已知部分）。 |
| 泛型单态化 | comptime 类型参数 `comptime T: type` 在单态化前由解释器求出具体类型，再走既有单态化；值参数 `comptime n` 在 codegen 前固化为常量。 |
| 声明式宏 `vec!`/`arr!` | 简单聚合字面量继续保留；复杂生成式需求迁移到 comptime。 |
| `#[derive(...)]` | 长期可用 comptime 表达（在 comptime 中反射 struct 字段并生成 impl）；本期 derive 宏保留。 |

---

## 4. 设计

### 4.1 语法

```rlyeh
// (A) comptime 块：块内语句在编译期求值，结果（值/类型）注入
comptime {
    let n = 1 << 4;          // 编译期计算 16
    // 可定义仅在编译期可见的局部
}

// (B) comptime 函数：实参在编译期已知时可被解释执行
comptime fn sq(x: i64) -> i64 { x * x }

// (C) comptime 参数修饰符
fn buf(comptime n: i64) -> [i64; n] { [0; n] }
fn id(comptime T: type, v: T) -> T { v }

// (D) comptime 类型/值绑定（可选，M3 引入）
comptime const N = 1024;
comptime var counter = 0;    // 编译期可变状态（用于生成式计数）
```

- **触发规则**：`comptime fn` 在「全部实参均为编译期已知」时被解释执行；否则退化为普通运行期函数（函数体需能同时被解释与 codegen）。
- **`comptime` 块**整体在编译期求值；其内部所有表达式必须可编译期求值，否则报 `comptime 上下文无法求值` 错误。

### 4.2 求值模型：comptime 解释器

采用 **HIR/MIR 树遍历解释器**（类比 Rust 的 const-eval over MIR、Zig 的 comptime interpreter），而非为 comptime 单独生成 LLVM：

1. comptime 代码先经**正常类型检查**（复用 `rlyeh-typecheck`），得到类型正确的 HIR。
2. **`rlyeh-comptime`（新增 crate / 或并入 driver）** 对 HIR 做树遍历求值，产出 **comptime 值**（见 §4.3）。
3. 求值结果回填：值回填为 AST 字面量/常量；类型回填为类型注解/单态化输入，交回既有 codegen。

**可求值的 HIR 子集（M1 最小集）**：字面量、算术/逻辑/比较、变量绑定、分支（`if`）、循环（`for`/`while`）、函数调用（限 comptime fn 与内建）、元组/结构体/枚举构造、数组下标、字段访问。

**内建（builtins，M1–M3 渐进引入）**：`@typeOf`、`@sizeOf`、`@typeInfo`（M3 反射）、`@embedFile`（M4）、类型强制转换 `@as(T, v)`。

### 4.3 值与类型的一等公民

comptime 解释器需一套**值表示（comptime value model）**：

- 标量：`i64`/`f64`/`bool`/`char` 等。
- 聚合：结构体（`{field: value}`）、元组、数组（`[v; n]`）、枚举变体。
- **类型**：`type` 是一类特殊的 comptime 值；`comptime T: type` 形参接收类型值，可在块内用于构造/分发。

类型值不参与运行期内存布局计算之外的事；它只是「编译期已知的一个类型」，用于在 codegen 前实例化。

### 4.4 效果与沙箱（Effects & Hermeticity）

comptime 执行须**确定性、封闭**：

- **允许**：纯计算、类型构造、编译期已知文件的嵌入（`@embedFile`，路径在编译期解析）。
- **禁止**：运行期 IO、网络、时钟、随机数（`@rand` 不在 comptime 可用）、修改运行期全局状态。
- 违反即 `comptime 上下文含非确定性效果` 错误。该约束保证 comptime 结果可复现、可缓存、可对拍。

### 4.5 错误与诊断

- 求值失败（如除零、越界、效果违例）报**编译期错误**，带 comptime 调用栈与首个出错表达式的源码 span。
- 与 §8 差分测试对齐：oracle 与实现在「同一输入」上必须产出同一值或同一错误类别。

---

## 5. 实现路线（M0–M4）

| 阶段 | 主题 | 风险 | 说明 |
|------|------|------|------|
| **M0** | comptime 解释器基础设施 | 🔴 高 | 新增 `rlyeh-comptime`：comptime 值模型 + HIR 树遍历求值（字面量/算术/控制流/局部/调用/聚合）；复用既有 `const` 折叠逻辑作为起点。 |
| **M1** | `comptime` 块 / 函数求值 | 🔴 高 | 接入类型检查后 HIR；`comptime { }` 与 `comptime fn` 在编译期求值并回填 AST；最小内建集。差分对拍 harness。 |
| **M2** | 泛型值/类型参数 | 🟠 中 | `comptime n: i64` / `comptime T: type`；数组长度由 comptime 值确定；类型级分发接入既有单态化。 |
| **M3** | 生成式 comptime | 🔴 高 | comptime 构造新类型/函数并经正常 codegen 编译；`@typeInfo` 反射；生成式循环（如枚举/序列化代码生成）。 |
| **M4** | 编译期自省与嵌入 | 🟡 低 | `@embedFile`、编译期反射、构建期配置；沙箱路径校验。 |

**推荐前序**：M0→M1 为「可用 CTFE」最小闭环（对标 Rust `const fn`）；M2 补齐值泛型；M3/M4 为自举赋能。

---

## 6. 与 0.3.0 自举的关系

- **M0/M1（CTFE 最小闭环）** 可在 **0.2.0 末段**落地，作为自举前置能力验证。
- **M2/M3（值泛型 + 生成式）** 是 **0.3.0 用 Rlyeh 重写编译器前端/codegen** 的关键：codegen 指令表、parser 状态表、序列化样板等可在编译期由 Rlyeh 自身生成，显著降低自举手写量。
- 自举可行性评估（`self-hosting/feasibility.md`）应将 comptime 列为 **P1 级能力缺口**，并在 0.3.0 计划新增对应阶段。

---

## 7. 风险与开放问题

1. **解释器性能**：纯树遍历对重计算 comptime（如编译期大循环）可能慢；长期可对「热 comptime」JIT 到 LLVM，但本期以树遍历为基线。
2. **comptime 值与运行期值的表示桥接**：解释器产出的 comptime 值须无损转回 AST 字面量/类型，浮点与聚合的序列化需谨慎。
3. **类型流与单态化耦合**：comptime 求出的类型如何干净注入 `rlyeh-parser`→`rlyeh-ast`→`rlyeh-codegen` 既有管线，是 M2/M3 的主要工程风险。
4. **`const fn` 是否合并**：建议 `const` 折叠直接复用 comptime 引擎，避免两套求值路径。
5. **效果沙箱边界**：`@embedFile` 的路径/内容是否计入编译缓存键（影响增量重建）需约定。

---

## 8. 验证计划（差分对拍）

复用 M-M1 lexer 的对拍方法论：以一个**独立 gold oracle**（Rust 侧参考解释器，或同一套 HIR 的参考实现）对 comptime 程序求值，与 Rlyeh `comptime` 解释器结果逐值/逐错误对齐。

- **corpus-comptime-1**（M1）：`comptime fn` 算术/控制流/递归（如 `comptime fn fib(n) {...}`）、`comptime { }` 块求值。
- **corpus-comptime-2**（M2）：值泛型 `buf(8)`/`buf(16)` 产出不同长度数组；类型分发。
- **corpus-comptime-3**（M3）：生成式枚举/`@typeInfo` 反射。

测试形态：`tests/self-host-comptime/` 下存放 corpus，新增 `tests/.../self_host_comptime.rs` 驱动对拍，要求 oracle 与实现**逐 token/逐值一致**。

---

## 9. 任务拆解（建议 leaf）

| 子任务 | 对应阶段 | 交付物 |
|--------|----------|--------|
| `comptime-m0-interp` | M0 | comptime 值模型 + HIR 解释器骨架 + 单测 |
| `comptime-m1-blocks` | M1 | `comptime` 块/函数求值 + 回填 + 内建最小集 + 对拍 harness |
| `comptime-m2-generics` | M2 | 值/类型参数 + 单态化接入 |
| `comptime-m3-generative` | M3 | 生成式类型/函数 + `@typeInfo` |
| `comptime-m4-reflect` | M4 | `@embedFile` + 编译期自省 + 沙箱 |

> **下一步**：本 RFC 评审通过后，① 并入 `grammar.md`/`semantics.md` 对应章节；② 在 `tasks/leaf/` 落地 `comptime-m0..m4` 叶子并挂入0.2.0 末段 M0/M1。
