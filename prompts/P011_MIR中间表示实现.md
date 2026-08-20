# P011: MIR 中间表示实现

> **模块路径**：`crates/zeta-mir/`（新建）  
> **预估工期**：5-7 天  
> **前置依赖**：P004（区域系统）、P005（Transfer 语义）、HIR + typecheck  
> **输出**：HIR → MIR 的 CFG lowering + 基础优化 passes  
> **状态**：✅ MVP 已实现（2026-08-20）

---

## 任务描述

将类型检查后的 HIR 降低为 MIR（中级中间表示）控制流图，并实现三个基础优化 pass：

1. **CFG 结构**：基本块 + 终止符（`Return` / `Jump` / `CondJump`）
2. **表达式求值**：所有表达式降为局部变量 + 指令（类 `_tN` 临时变量）
3. **控制流 lowering**：`if` / `while` / `loop` / `break` / `continue` 的 CFG 展开
4. **区域操作显式化**：`RegionEnter` / `RegionExit` / `AllocInRegion` / `Transfer` 成为一等指令
5. **优化 passes**：常量折叠、不可达块删除 + 死赋值消除（DCE）、小函数内联

---

## 数据结构

```rust
// crates/zeta-mir/src/lib.rs

/// MIR 程序 = 一组函数
pub struct MirProgram {
    pub functions: Vec<MirFunction>,
}

pub struct MirFunction {
    pub name: String,
    pub params: Vec<Local>,
    pub blocks: Vec<BasicBlock>,   // blocks[0] 为入口块
}

pub struct BasicBlock {
    pub stmts: Vec<MirStmt>,
    pub terminator: Option<MirTerminator>,  // 允许 None（lower 未完成/错误态）
}

/// 三地址码指令
pub enum MirStmt {
    Assign { target: Local, value: MirValue },
    Call { target: Local, callee: String, args: Vec<MirValue> },
    RegionEnter { region: String },
    RegionExit { region: String },
    AllocInRegion { target: Local, region: String },
    Transfer { place: Local, region: String },
}

/// 右值
pub enum MirValue {
    Unit,
    Literal(MirLiteral),
    Place(Local),
    Binary { op: MirBinaryOp, lhs: MirValue, rhs: MirValue },
    Unary { op: MirUnaryOp, value: MirValue },
}

/// 终止符
pub enum MirTerminator {
    Return(Option<MirValue>),
    Jump(usize),                       // 目标基本块
    CondJump { cond: MirValue, then: usize, otherwise: usize },
}
```

---

## Lowering 规则

### 表达式 → 临时变量

- 所有表达式求值结果放入 `_tN`（N 自增）临时变量，`Assign` 指令发射到当前块
- 变量引用直接映射为 `Place(x)`（不产生指令）

### 复合表达式展开

| HIR 表达式 | MIR 展开 |
|-----------|---------|
| `x += 1` | `x = x + 1`（`Binary Add`） |
| `x in (1, 3, 5)` | `x==1 \|\| x==3 \|\| x==5` |
| `x in 0..<10` | `x>=0 && x<=9`（取反包 `Not`） |
| `a && b` / `a \|\| b` | `Binary And/Or` |
| 一元 `!x` | `Unary Not` |

### 控制流

```
if cond { A } else { B } C;:
    ┌─ entry ────────────────┐
    │ t = cond               │
    │ condjump t, then, else │
    └───┬──────────┬─────────┘
        │          │
   ┌─ then ─┐  ┌─ else ─┐
   │ A      │  │ B      │
   │ jump m │  │ jump m │
   └───┬────┘  └───┬────┘
       └─────┬─────┘
        ┌─ merge ─┐
        │ ...C... │
        └─────────┘

while cond { body }:   // entry → head(cond) ⇄ body，出口 after
loop { body }:         // entry → body，出口 after
```

- `break` → `Jump(after)`；`continue` → `Jump(head)`
- 已终止块后的语句在 lowering 阶段跳过（不可达）
- `while` / `loop` / `if` 作为表达式时，块值写入结果临时变量，merge/after 块读取

### 区域操作

```
region 'r { let x = 5 in 'r; transfer x out of 'r; x }:
    region_enter 'r
    _t0 = 5
    alloc_in_region _t0, 'r
    x = _t0
    transfer x, 'r
    region_exit 'r
    return x
```

---

## 优化 Passes（`passes::optimize()`）

执行顺序：**常量折叠 → DCE → 小函数内联 → DCE**

### 1. 常量折叠（`const_fold.rs`）

- 折叠 `Assign` 右值的常量运算：整数四则/比较、布尔逻辑、字符比较
- **不折叠**：除零 / 模零（保守保留，避免语义改变）、浮点运算
- 条件为常量的 `CondJump` → `Jump`（折叠条件 + 定向跳转）

### 2. 死代码消除（`dce.rs`）

- 入口块图遍历删除不可达块，重映射所有跳转目标
- 后向活跃扫描：删除对结果从未被读取的 `_tN` 临时变量的赋值

### 3. 小函数内联（`inline.rs`）

- 仅内联：单基本块、无区域操作、调用点直接展开
- 参数替换为实参，临时变量改名 `_iN` 避免冲突
- 预克隆内联函数体避免借用冲突（E0502）

---

## 测试清单

### `crates/zeta-mir/tests/mir_lower_test.rs`（11 项）

- 字面量与算术、二元运算临时变量化、布尔/字符常量
- 比较链 `0 < x < 10` 的链式求值
- `if` 有/无 else 的 CFG 结构（then/else/merge 块 id 与跳转）
- `while` 四块结构（head 条件 + 回跳）、`loop` 三块结构
- `break` / `continue` 跳转目标、不可达语句跳过
- 复合赋值展开 `x += 1` → `x = x + 1`
- `in` 集合/范围展开、`SetLookup` / `RangeCheck` 否定形态
- 函数调用与参数传递、多函数 lowering
- 区域操作序列（enter/alloc/bind/transfer/exit）

### `crates/zeta-mir/tests/mir_pass_test.rs`（7 项）

- 常量折叠（算术、比较、逻辑、字符）、除零不折叠
- 不可达块删除、死赋值消除（活跃变量后向扫描）
- 小函数内联（调用点展开、参数替换、临时变量改名）
- 优化流水线端到端（含 while 循环程序）

---

## 配套改动（本任务触及的其它 crate）

| crate | 改动 |
|-------|------|
| `zeta-hir` | 新增 `HirExpr::While { cond, body }`、`HirExpr::Loop { body }`、`HirExpr::Assign { target, op, value }` 与 `HirAssignOp` 枚举 |
| `zeta-typecheck` | `For` 改为显式 `Unsupported`（提示用 while）；`While` / `Loop` 生成真实节点并检查循环体；`Assign` 生成 `HirExpr::Assign`（不再静默丢弃） |
| `zeta-regionck` | 适配新 HIR 节点（`While` / `Loop` 遍历体、`Assign` 检查右值） |
| `zeta-parser` | 修复 `true` / `false` 布尔字面量解析（lexer 产出 `Token::True` / `Token::False`，parser 原先只处理 `Token::BoolLiteral`） |

---

## 验收标准

| 标准 | 要求 |
|------|------|
| 所有测试通过 | 100%（全 workspace 176 项） |
| 零警告 | `cargo clippy --workspace --all-targets` |
| 格式 | `cargo fmt --all -- --check` |
| 区域指令 | enter/exit/alloc/transfer 全部显式化 |
| 优化正确性 | 折叠不改变语义（除零保守保留） |

---

## 完成后下一步

进入 **P007_增量编译引擎.md**，或按里程碑推进 **M1.8 代码生成（LLVM 后端）**。

## 实施记录（2026-08-20）

- `zeta-mir` 从占位 crate 落地为真实实现：`lib.rs`（数据结构）+ `lower.rs`（HIR → CFG）+ `passes/`（三个优化 pass）。
- **关键 bug 修复**：`new_block()` 会切换当前块，导致 `lower_if` / `lower_while` / `lower_loop` 在创建分支块后把终止符发射到了错误的块（`Jump` 被发射到 after 块）。修复为：创建块前记录 entry，创建完毕后切回 entry 再发射终止符。
- 常量条件 `CondJump` → `Jump` 的定向跳转在折叠 pass 内完成。
- 内联 pass 预克隆函数体（`Vec<(String, MirFunction)>`）解决 `&mut program.functions` 迭代时的借用冲突（E0502）。
- `MirValue` 无 `Default`，`mem::take` 改用 `mem::replace(value, MirValue::Unit)`。

## 落地偏差说明

- **简化 SSA**：未引入真正的 SSA/φ 节点；`if` / `while` / `loop` 各分支写入同一结果变量、merge 块读取，模拟 φ 语义。后续优化 pass 需要时再升级。
- **`break` / `continue` 的边界检查未做**：MVP 允许 `break` 出现在非循环上下文中（typecheck 未校验），lowering 中表现为跳转到失效块 id。待 typecheck 补上下文校验。
- **`transfer` 后仍可使用变量**：MIR 阶段不做 use-after-transfer 检查（依赖借用检查器/区域检查器，P007 范围）。
- **循环条件中含 `return` / `break` 的表达式**：`?` 提前返回会使 entry 块未终止（边缘情况，MVP 接受）。
- **MIR 尚未接线到编译器驱动**：`zeta-driver` 流水线仍停留在 regionck 之后（M1.8 代码生成时接线）。

## 测试清单（新增 18 项）

- `mir_lower_test.rs`（11 项）：覆盖上述 lowering 规则全部分支。
- `mir_pass_test.rs`（7 项）：三个 pass 单测 + 优化流水线端到端。

全 workspace 测试通过（含 zeta-mir 18 项），clippy 0 警告，`cargo fmt --check` 通过。
