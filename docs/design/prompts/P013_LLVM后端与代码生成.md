# P013 — LLVM 后端与代码生成

> **状态**：✅ 已完成（MVP）
> **完成日期**：2026-08-20
> **前置**：P011（MIR 中间表示）、P012（区域系统）
> **里程碑**：M1.8（代码生成）+ M1.9（hello-world 运行）
> **目标**：打通编译流水线最后两环 —— LIR 三地址码 → LLVM IR 文本 → clang 汇编链接 → 可执行文件，使 `zeta run hello-world.zeta` 端到端可用。

---

## 1. 任务描述

### 1.1 背景

M1.7（MIR + 优化）完成后，编译流水线只剩代码生成一环：

```
源代码 → Lexer → Parser → Typecheck → Borrowck → Regionck → MIR(+优化) → ??? → 可执行文件
```

本任务补齐 `???` 部分：新增三个 crate：

| crate | 职责 |
|-------|------|
| `zeta-lir` | 低级中间表示（三地址码）+ MIR → LIR lowering |
| `zeta-codegen` | LIR → LLVM IR 文本生成 |
| `zeta-driver` | 编译器驱动（CLI）：串联全流水线 + 调用 clang 汇编/链接 |

### 1.2 验收标准

- [x] MIR 程序可降低为三地址码 LIR（含局部变量类型推断）
- [x] LIR 可生成合法 LLVM IR 文本，clang 可汇编链接成可执行文件
- [x] `zeta run examples/hello-world.zeta` 输出 `Hello, Zeta!`
- [x] 支持算术/比较/逻辑/字符/字符串/浮点/布尔，以及用户函数调用与递归调用
- [x] 内建 `print` / `println` 映射到 `printf`，按参数类型分派格式串
- [x] 控制流（if/while/loop/break/continue）经 MIR CFG → LLVM 基本块正确生成
- [x] 区域指令（RegionEnter/Exit/AllocInRegion/Transfer）在 LLVM 后端显式忽略（MVP）
- [x] 全 workspace 测试通过（230 个）+ clippy 零警告 + rustfmt 干净

---

## 2. 设计决策

### ADR-LLVM-001：非 SSA 槽式存储

每个局部变量在函数入口块 `alloca` 一个槽，访问时 `load` / `store`。

- **优点**：天然支持分支合并（各分支写同一变量、合并块读取），无需构造 φ 节点；LLVM 后端在 `-O1` 以上自动执行 `mem2reg` 提升为 SSA。
- **代价**：无优化时指令数偏多，MVP 阶段可接受。

### ADR-LLVM-002：LLVM IR 文本直出

不引入 inkwell 等 LLVM C API 绑定，直接生成 LLVM IR 文本，交给 clang 汇编链接。

- **优点**：零额外依赖、编译期快、错误信息直接暴露 IR 文本（便于用 clang 复现定位）。
- **代价**：失去 LLVM 类型安全校验；MVP 阶段由编译器自身保证 IR 合法。

### ADR-LLVM-003：命名寄存器 `%rN`

临时寄存器统一用命名寄存器 `%rN`（跨函数递增计数）。

- **背景**：LLVM 中命名寄存器同样占用函数内的数值编号序列。若混用裸数字 `%4` 与命名寄存器，会触发 `instruction expected to be numbered` 错误。
- **决策**：所有临时值一律 `%rN`，规避编号冲突。

### ADR-LLVM-004：`main` 特化

Zeta 的 `main` 函数生成 `define i32 @main()`，返回 `ret i32 0`。

- **理由**：链接进可执行文件需要 C 风格的 `main` 入口；返回值固定为 0（MVP 不支持退出码）。

### ADR-LLVM-005：区域指令后端忽略

`RegionEnter / RegionExit / AllocInRegion / Transfer` 在 LLVM 后端直接忽略。

- **理由**：MVP 的分配语义（bump allocator + LIFO 析构）由 `zeta-region-alloc` 运行时库提供，LLVM IR 生成阶段不展开；后续由专用后端或运行时接线。

---

## 3. 实现内容

### 3.1 `zeta-lir`（新建）

**数据结构**（`src/lib.rs`）：

- `LirProgram { functions }` / `LirFunction { name, params, return_type, locals, blocks }`
- `LirBlock { stmts, terminator }`
- `LirStmt`：`Assign` / `Binary` / `Unary` / `Call` / `RegionEnter` / `RegionExit` / `AllocInRegion` / `Transfer`
- `LirOperand`：`Local` / `Int` / `Float` / `Char` / `Bool` / `String` / `Unit`
- `LirTerminator`：`Return` / `Jump` / `CondJump`
- `LirType`：`I64` / `F64` / `Bool` / `Char` / `Str` / `Unit`

**类型推断**（`src/lower.rs`）：

- `infer_function_types`：对每个函数不动点迭代，直到局部变量类型稳定
- 二元运算结果类型：`And` / `Or` → `Bool`；比较 → `Bool`；算术 → 操作数类型（浮点优先）
- `Binary.ty` 统一为**操作数类型**，比较/逻辑结果固定 `bool`（LLVM store 时按目标变量类型，避免 i1/i64 槽错配）
- 用户函数跨函数类型在全部函数推断完成后解析

**Lowering**：

- 嵌套表达式拆平为临时变量（三地址化）
- 内建函数 `print` / `println`（`BUILTIN_FUNCTIONS`）保留参数、走普通调用路径

### 3.2 `zeta-codegen`（新建）

`src/llvm.rs`：`LlvmEmitter`（sigs / globals / reg_counter），`generate_llvm()` 入口。

- **内建打印**：`print` / `println` → `printf`；格式串按类型分派（`%s` / `%lld` / `%f` / `%c`）；布尔经 `select` 选 `"true"` / `"false"` 字符串指针；字符 `zext i8 → i32`（变参整型提升）
- **字符串**：全局常量 `@.str.N` + `getelementptr`；转义支持 `\\`、`\"`、`\n`、`\r`、`\t`、非打印字节 `\XX`
- **浮点字面量**：按 `f64::to_bits()` 生成十六进制 `0x...` 位模式
- **Unary**：`Neg` → `fneg`（浮点）/ `sub i64 0, x`；`Not` → `xor i1 true, x`
- **终止符**：`Return`（main 特化 `ret i32 0`）/ `Jump` / `CondJump`（`br i1`）
- **错误**（`src/error.rs`）：`UnsupportedType` / `UndefinedFunction` / `InvalidMain`

### 3.3 `zeta-driver`（新建）

`src/lib.rs`：

- `compile_to_llvm(source)`：lexer → parser → typecheck → borrowck → regionck → MIR(+优化) → LIR → LLVM IR
- `build_executable(source)`：IR 落盘临时目录（`zeta-mvp-{pid}-{nanos}`）→ 探测 clang → `clang -O1 ir -o exe`
- `run_source(source)`：build + 执行 + 捕获输出
- clang 探测屏蔽输出（`.stdout(Stdio::null())`），避免 `clang --version` 污染 stdout

`src/main.rs`：CLI 入口 —— `zeta run <file>` / `zeta build <file>` / `--version`。

### 3.4 配套改动

- `zeta-typecheck/src/check_expr.rs`：新增 `pub const BUILTIN_FUNCTIONS`；Call 分支在宏分支前处理内建（保留参数、返回 `Unit`）
- `zeta-mir/src/passes/dce.rs`：修复 **既有 bug** —— 副作用调用（`print`/`println` 等 `Call`）被 `remove_dead_assignments` 误删；修复为 Call 永不删除、参数标记活跃

---

## 4. 测试

| 文件 | 覆盖点 | 数量 |
|------|--------|------|
| `zeta-lir/tests/lir_lower_test.rs` | 三地址运算、比较 CondJump、内建调用保留参数、区域标注、类型推断、嵌套拆平 | 6 |
| `zeta-codegen/tests/llvm_test.rs` | 字符串/printf、用户函数、icmp+CondJump、not+select、转义、未定义函数报错 | 6 |
| `zeta-driver/tests/driver_test.rs` | **端到端**：clang 汇编/链接/运行，断言精确 stdout 输出 | 6 |
| `zeta-mir` 既有测试 | DCE 副作用调用修复回归 | — |

**结果**：全 workspace 230 个测试全部通过；`cargo clippy --workspace --all-targets` 零警告；`cargo fmt --all -- --check` 干净。

**端到端示例**（`examples/`）：

- `hello-world.zeta`：`fn main() { println("Hello, Zeta!"); }` → 输出 `Hello, Zeta!`
- `arith-print.zeta`：函数加法 + 内建打印 + if 比较 → 输出精确匹配

---

## 5. 调试记录

| 问题 | 根因 | 修复 |
|------|------|------|
| 编译出的程序不打印任何东西 | DCE 把 `println` 调用（target 为临时变量且不活跃）误删 | 副作用 Call 永不删除，参数标记活跃 |
| `fn add(a, b)` 报 Comma 语法错误 | Zeta 要求参数带类型标注 | 示例改为 `fn add(a: i64, b: i64) -> i64` |
| LLVM store 到 i1 槽报错 | `Binary.ty` 与操作数类型混用 | `Binary.ty` 统一为操作数类型；store 按目标变量类型 |
| `instruction expected to be numbered` | 手写 `%4` 与命名寄存器编号冲突 | 全部改用命名寄存器 `%rN`（跨函数递增） |
| clang 探测输出混入 stdout | `clang --version` 未屏蔽 | `.stdout(Stdio::null()).stderr(Stdio::null())` |
| LIR 类型推断 match 不完整 | `infer_function_types` 缺区域指令分支 | 补 `RegionEnter/Exit/AllocInRegion/Transfer => {}` |

**关键调试方法**：测试失败时打印生成的 IR → 复现 clang 错误 → 落盘 `.ll` 文件直接用 clang 定位（如寄存器编号冲突）。

---

## 6. 后续工作

### 6.1 本任务遗留

- [ ] 用户函数可重定义检查（当前 `sigs` 表后写覆盖）
- [ ] 全局变量 / 常量支持
- [ ] `&` 借用语法 → LLVM 指针语义
- [ ] 递归深度校验 / 栈溢出保护
- [ ] WASM 目标（Milestone 2.7）

### 6.2 建议下一步

- **P007 增量编译引擎**（模块级缓存 + 函数级并行）—— 编译速度目标（百万行 < 30s）的核心
- **M2.1 Actor 运行时**（`actor` 关键字 → 运行时调度）

---

## 7. 实施记录

| 日期 | 内容 |
|------|------|
| 2026-08-19 | 搭建 `zeta-lir`（三地址码 + MIR→LIR lowering + 类型推断），6 个单测 |
| 2026-08-19 | 搭建 `zeta-codegen`（LLVM IR 文本 + 内建 printf），6 个单测 |
| 2026-08-19 | 搭建 `zeta-driver`（CLI + clang 端到端），6 个端到端测试；typecheck 注册内建 |
| 2026-08-19 | 修复 DCE 副作用调用误删 bug；修复 LIR 类型推断 match 缺失分支 |
| 2026-08-20 | 全 workspace 230 测试通过；clippy / rustfmt 干净；hello-world 命令行运行验证通过 |
