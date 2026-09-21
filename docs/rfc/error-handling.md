# RFC：Rlyeh 错误处理机制完善计划

| 字段 | 内容 |
|------|------|
| 状态 | 评审通过（Accepted）；已并入 0.2.0 计划（阶段 AA：eh-1…eh-8）。**进度（2026-09-21）**：eh-1 ✅、eh-2 ✅、**eh-3 ✅**（组合子全量落地：非闭包 / 函数值型 / 嵌套泛型 / 引用侧 / 惰性零参闭包；连带修复 4 处编译器缺口——typecheck「fn 形参泛型推断」、parser+typecheck「嵌套 self 类型」、parser「`||` 零参闭包前缀分派」、typecheck「闭包字面量反推方法泛型」）、eh-8 边界硬化（M4）✅ |
| 日期 | 2026-09-20 |
| 范围 | 语言/标准库层的**可恢复错误处理**与**不可恢复失败（panic）策略**完善：错误类型体系、`?` 自动转换、错误组合子、`Error` protocol 完整化、泛型错误类型、错误 derive、错误聚合、以及面向 ASIL/TCL2 的确定性 panic 策略。 |
| 关联文档 | [`std-lib.md` §12](../std-lib.md)（现状）、[`docs/tasks/leaf/sh-p1-6-question-from.md`](../tasks/leaf/sh-p1-6-question-from.md)（?+From）、[`docs/tasks/leaf/k1-question.md`](../tasks/leaf/k1-question.md)（? 运算符）、[`docs/rfc/tcl2-certification.md`](./tcl2-certification.md)（ASIL/TCL2 关联）、[`docs/manual/std/result.md`](../manual/std/result.md) |
| 任务拆解 | 见 §8（eh-1…eh-8） |

---

## 1. 现状：Rlyeh 现在如何进行错误处理

Rlyeh **没有异常（exception）机制**，错误通过返回类型显式表达：

1. **代数数据类型作为错误载体**
   - `Option<T>`（`Some`/`None`）表达"有/无"；`Result<T, E>`（`Ok`/`Err`）表达"成功值/失败原因"（`std-lib.md` §2.1/§2.2）。
   - 调用方必须 `match` 处理 `Err`/`None` 才能取用内部值——编译器强制，杜绝 C 式"忘记检查返回值"。

2. **`?` 运算符（K1 ✅）+ `From` 自动转换（已核实落地）**
   - `Result` 上下文：`expr?` desugar 为 `match { Ok(v) => v, Err(e) => return Err(From::from(e)) }`（`rlyeh-typecheck/src/check_expr/index_enum.rs` `check_question`，lines 454–478）。
   - `Option` 上下文：早返回 `None`；若目标为 `Result<T, E>` 同样经 `From` 转换。
   - **已落地**：当错误类型 `E1 ≠ E2` 时注入 `From::<E1>::from(__e)`，要求存在 `impl From<E1> for E2`（依赖 `rlyeh-typecheck/src/check_expr/call.rs` P6c 的 protocol 关联函数调用机制，2026-08-29）。叶子 `sh-p1-6`（?+From）标记完成**属实**。
   - **定义侧已核实落地**：`-> Self` 返回（U4）与用户自定义 `impl From<X> for Y { fn from(...) -> Self }` **均已支持**（run-pass：`self_return.rl` / `mem_take.rl` / `error_conversion.rl`；std 内置 `impl IoError: From<IoErrorKind>` 见 `io/error.rl` 69–82）。`std-lib.md` §12 标注 U4"未支持"**已过时**。

3. **错误组合子（部分实现）**
   - `Option`：`from`/`map`/`and_then`/`unwrap`/`unwrap_or`。
   - `Result`：`map`/`map_err`/`and_then`/`propagate`（等价 `?`）/`unwrap`/`unwrap_or`/`expect`。
   - 缺：`or_else`/`unwrap_or_else`/`expect_err`/`ok()`/`err()`/`transpose`/`flatten` 等常用组合子。

4. **具体错误类型**
   - `IoError`/`IoErrorKind`（M1 ✅，`io/error.rl`）：IO 错误及其分类。
   - `TimeoutError`（`future::timeout` 返回）。
   - 无统一的"库错误"基类或泛型错误类型（`Box<dyn Error>`/`anyhow` 式）。

5. **`Error` protocol（M2a ✅，Y6 已部分落地）**
   - `fn message(&self) -> String`（M2a ✅）。
   - `fn source(&self) -> Option<&dyn Error>`（Y6）：**方法已落地**（`io/error.rl` `impl IoError: Error` 已实现 `source`，但当前返回 `Option::None` 占位，尚未接入真实根因链）。
   - 缺：`source()` 的真实根因填充、`kind()` 分类方法、以及所有 std 错误类型补齐 `source()`。

6. **不可恢复失败（panic）**
   - `unwrap`/`expect` 失败、actor 方法返回 `-1`（崩溃信号）、OOM → 运行时 **panic（MVP 直接 abort）**（`manual/std/result.md`）。
   - **无文档化、可配置的 panic 策略**（无 `panic=abort`/unwind 开关、`no_panic` 标注、或安全关键路径的 unwind 禁令）。

---

## 2. 现状盘点与缺口

| 能力 | 现状 | 缺口 |
|------|------|------|
| 错误载体 Option/Result | ✅ | — |
| `?` 运算符 | ✅ 含 From 自动转换（已核实） | 调用转换 + U4 定义侧均已落地（见 eh-1） |
| 组合子 | 🟠 部分 | `or_else`/`unwrap_or_else`/`transpose`/`flatten` 等缺失 |
| 具体错误类型 | 🟠 IoError/TimeoutError | 无泛型错误类型 / 库错误基类 |
| `Error` protocol | ✅ `message()`+`source()`（已核实，`source` 暂返回 `None` 占位） | 缺真实根因链 / `kind()` |
| `From`/`Into` 错误转换 | ✅ 已实现（U4 落地，run-pass 验证） | — |
| 错误 derive | ❌ | 手写错误枚举样板冗长 |
| 错误聚合 / `try` 块 | ❌ | `Vec`/迭代器错误累积、`try` 块缺失 |
| context / 背链 | ❌ | 无错误上下文附加 |
| panic 策略 | 🟠 MVP 直接 abort（未文档化） | 无可配置策略 / `no_panic` / ASIL 友好 |

---

## 3. 完善目标与原则

- **零隐式控制流**：保持"错误即返回值"，不引入异常式 unwind。
- **`?` 透明传播**：借 `From` 实现分层错误经 `?` 自动转换（对齐 Rust）。
- **可组合**：补全组合子、错误聚合、`context` 链。
- **可认证（呼应 TCL2/ASIL B+C）**：panic 语义**确定且可文档化**，安全关键路径可强制 `abort`/禁 unwind、可标注 `no_panic`；错误路径不得引入 UB。

---

## 4. 改进项

### 4.1 `?` + `From` 自动转换（eh-1，已核实落地 ✅）
- **核实结论**：调用侧自动转换**已实现**（`check_question` 在 `E1 ≠ E2` 时注入 `From::<E1>::from(__e)`，依赖 `call.rs` P6c 的 `From::from`/`Into::into` protocol 关联函数调用机制）。`sh-p1-6` 标记完成属实。
- **定义侧亦已落地**：`-> Self` 返回（U4）与用户自定义 `impl From<X> for Y { fn from(...) -> Self }` **均支持**（run-pass：`self_return.rl`/`mem_take.rl`/`error_conversion.rl`；std 内置 `impl IoError: From<IoErrorKind>` 见 `io/error.rl`）。`std-lib.md` §12 标注 U4"未支持"**已过时**。
- 无 `From` impl 时退化为现有 `return Err(e)`，向后兼容。
- 交付：**修正 `std-lib.md` §12 过时标注**（U4"未支持"已不准确）+ 对拍验证。

### 4.2 `Error` protocol 完整化（eh-2，Y6）
- 增加 `fn source(&self) -> Option<&dyn Error>`（错误链/根因式）。
- 可选 `fn kind(&self) -> ErrorKind`（分类，供程序分支处理）。
- 让 `IoError` 等实现 `source()`（如 IO 错误包装底层 `errno`）。

### 4.3 组合子补全（eh-3，✅ 2026-09-21 落地）
- `Result`：`or_else`/`unwrap_or_else`/`expect_err`/`ok()`/`err()`/`transpose`（Result<Option> ↔ Option<Result>）/`flatten`。**全部 ✅**
- `Option`：`ok_or`/`ok_or_else`/`filter`/`flatten`/`transpose`。**全部 ✅**（另附 `map`/`and_then`/`or_else`/`unwrap_or_else`/`copied`/`cloned`）
- 均为 inherent 方法（`core/module.rl` 的 `impl<T> Option<T>` / `impl<T, E> Result<T, E>` 及嵌套 / 引用侧 impl），零新增 IR。
- **核实校正**：本节早期清单中的 `Result::propagate()` 从未实现，且与 `?` 运算符（K1 ✅）语义重复，**决定不提供**；`Option::from` 亦不存在（已从 `std-lib.md` §2.1 移除）。
- **连带修复的编译器缺口**（详见 [`../tasks/leaf/eh-3-combinators.md`](../tasks/leaf/eh-3-combinators.md)）：
  1. **typecheck**：`fn(..)` 形参中的方法级泛型推断（`Option::map(inc)` 的 `U`）；
  2. **parser + typecheck**：嵌套泛型 self 类型 `impl<T> Option<Option<T>>`（`flatten`/`transpose` 前置）；
  3. **parser**：`|| expr` 零参闭包**表达式前缀位置未分派**（`unwrap_or_else(|| ..)` 报 `unexpected token: found OrOr`）；
  4. **typecheck**：闭包字面量实参对方法泛型的反推——形参泛型可由接收者 / 其它实参反推时（`map(|x| ..)`）直接可用；泛型**仅出现在闭包返回位置**时（`ok_or_else(|| ..)` 的 `E`、`Result::or_else(|| ..)` 的 `F`）由闭包体推断类型回填。

### 4.4 泛型错误类型（eh-4，anyhow 式便捷）
- 提供 `DynError`（内部 `Box<dyn Error + Send + Sync>` 或等价）作为"我不关心具体类型、只想传播"的便捷错误类型，降低样板。
- 提供 `Result<T>` 别名（`Result<T, DynError>`）。
- 可选：`anyhow!`/`bail!` 风格宏用于构造/早返错误。

### 4.5 错误 derive（eh-5，thiserror 式）
- `#[derive(Error)]`：为枚举/结构体自动实现 `Error` + `message()`（基于 `#[error("...")]` 消息模板）+ `source()`（基于 `#[source]`/`from` 字段）。
- 大幅减少手写错误枚举样板，是库作者友好性的关键。

### 4.6 错误聚合 / `try` 块（eh-6）
- `try` 块：`try { ...; Ok(x) }` 内部 `?` 冒泡到块尾 `Result`（对齐 Rust `try`/返回块）。
- 迭代器/集合错误累积：`Iterator::collect::<Result<Vec<T>, E>>()`、`Vec` 批量 `?`；可选 `try_for_each`。
- 多错误合并类型（记录首个/全部错误）。

### 4.7 context / 背链附加（eh-7，可选）
- `Result::with_context(f)` / `Context` protocol：为错误附加调用现场信息（如文件路径、行号），便于诊断。
- 与 `source()` 链组合形成完整诊断。

### 4.8 panic 策略与可认证性（eh-8，呼应 TCL2/ASIL）
- **文档化 panic 模型**：明确哪些操作 panic（unwrap/expect/数组越界/整数溢出在认证模式/actor -1/OOM）。
- **可配置策略**：`panic = abort | unwind`（默认 abort）；安全关键编译模式强制 `abort`。
- **`no_panic` 标注**：函数级 `no_panic` 注解 + 静态检查（或不提供，依赖对拍证明无 panic 路径），供 ASIL 关键函数使用。
- **禁止安全关键路径隐式 unwind**：与 `tcl2-certification.md` §5.2 确定性/边界硬化协同——越界、溢出在认证模式改为 `abort` 而非 UB。

---

## 5. 分阶段计划（里程碑）

| 阶段 | 内容 | 关联既有任务 |
|------|------|--------------|
| M1（近期） | eh-1 `?`+From 核实/收尾 + eh-3 组合子补全 | U4、sh-p1-6 |
| M2 | eh-2 `Error::source` + eh-4 `DynError`/别名 | Y6 |
| M3 | eh-5 `#[derive(Error)]` + eh-6 `try`/聚合 | derive 框架（已有 JSON derive 先例） |
| M4（认证向） | eh-7 context + eh-8 panic 策略/`no_panic` | TCL2 |

---

## 6. 验证

- **单元/集成**：分层错误 `?` 经 `From` 传播对拍（`sh-p1-6` 用例扩展）；组合子各方法 run-pass；`#[derive(Error)]` 生成正确 `message`/`source`；`try` 块与 `collect::<Result<_>>` 对拍。
- **差分对拍**：复用 M-M1 harness，错误路径语义与参考行为对齐。
- **认证证据**：panic 策略文档 + `no_panic` 标注用例 + 安全关键模式 `abort` 验证（配合 `tcl2-certification.md` §5.3）。

---

## 7. 与 TCL2/ASIL 的关系

错误处理直接服务于 **ISO 26262 TCL2 / ASIL B+C**（见 `tcl2-certification.md`）：
- **确定性失败**：明确且可文档化的 panic（默认 abort），避免未定义行为与隐藏控制流，降低 TD（工具影响）。
- **错误路径无 UB**：边界/溢出在认证模式 `abort` 而非 UB（§4.8 + §5.2 边界硬化）。
- **可审查性**：`Result` 强制显式处理，配合 `rlyeh-check` lint（见 TCL2 §5.2），使错误传播可被静态审查，提升"错误检测置信"。

---

## 8. 任务拆解（建议 leaf，`docs/tasks/leaf/`）

| 子任务 | 对应 § | 交付物 |
|--------|--------|--------|
| `eh-1-question-from` | 4.1 | 已落地复核 + **修正 std-lib.md §12 过时标注（U4"未支持"）+ 对拍** |
| `eh-2-error-source` | 4.2 | `source()` 真实根因链填充 + 全 std 错误类型补齐 + 可选 `kind()`（Y6 收尾） |
| `eh-3-combinators` | 4.3 | 组合子补全 |
| `eh-4-dyn-error` | 4.4 | `DynError` + `Result<T>` 别名 + 宏 |
| `eh-5-derive-error` | 4.5 | `#[derive(Error)]` |
| `eh-6-try-aggregation` | 4.6 | `try` 块 + 迭代器错误累积 |
| `eh-7-context` | 4.7 | context/背链附加 |
| `eh-8-panic-policy` | 4.8 | panic 策略文档 + `panic=abort` + `no_panic` |

> **下一步**：① `sh-p1-6`（`?`+From 调用侧）**与 U4 定义侧均已核实落地**，`std-lib.md` §12 标注已过时；② 评审通过后，在 `tasks/leaf/` 落地 `eh-1…eh-8`（eh-1/eh-2 转为"修正过时标注 + 填充 `source` 根因链"收尾项），并入 0.2.0 计划。
