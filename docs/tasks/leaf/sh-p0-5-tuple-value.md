# SH-P0-5 元组值构造 + 解构（多返回值）

> **级别**：P0（阻塞前端 PoC） · **风险**：🔴 中高 · **状态**：✅ 已完成（单元验证） · **归属**：0.2.0-N
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.14

## 目标
支持元组**值构造** `(a, b, c)`、解构绑定 `let (a, b) = e`、函数**多返回值** `fn f() -> (i64, String)`，使解析器可用 `(token, rest)` 风格重写（当前 std `Channel` 因「未提供多元组返回」退化为 `ChannelPair` 结构体，即此缺口的旁证）。

## 现状（已具备地基）
- parser 已解析元组**类型** `(A, B)` → `AstType::Tuple`；typecheck 已有 `Type::Tuple` 并参与单态化/替换/字段计数（`field.rs:561`）。
- `ExprKind::Unit` / `Type::Unit`（单元 `()`）已支持。
- **缺失**：元组值字面量解析、解构模式（`index_enum.rs:765` 报 `元组 / 结构体模式在 MVP 阶段` Unsupported）、函数多返回与 codegen 发射。

## 风险分解（→ 中/低危）
- **M1（中）** 元组字面量 `(a, b, c)` 解析 + HIR/MIR/LIR：复用现有聚合槽布局，按位置命名字段 `f0/f1/...`（与枚举负载字段同构），codegen 发射为 N 槽聚合。
- **M2（中）** 解构绑定 `let (a, b) = e` / `let (a, _, c) = e`：pattern 层 `AstPattern::Tuple` 支持标识符/`_`/嵌套，逐字段 `Assign` 到临时再绑定。
- **M3（中）** 函数多返回值 `fn f() -> (i64, String)` + 调用点 `let (x, y) = f()`：返回聚合经现有多槽返回机制；尾表达式/显式 `return (x, y)` 复用 M1/M2。
- **M4（低）** 元组参与 `for (k, v) in map` 既有特判一致化（当前 `iter.rs:549` 已特判二元元组模式）。
- **L1（低）** 差分测试（K 阶段）对拍 Rust 参考实现，确认内存布局/ABI 一致。

## 受影响组件
`rlyeh-lexer` / `rlyeh-parser`（PoC 重写）、`rlyeh-typecheck`（pattern/expr）、`rlyeh-codegen`（聚合发射）。

## 验证
- Rlyeh 侧 `(tok, rest)` 风格重写 lexer/parser 主体，经 Rust driver 编译 + 差分对拍 token/AST 一致。
- 单元：多返回函数 `fn split() -> (i64, String)` + 解构接收，结果正确。

## 缺口复核（2026-09-02，动手前）
叶子「现状」称「元组值字面量解析、解构模式、函数多返回与 codegen 发射」三项全缺，
复核发现**仅解构（M2）为真缺口**：
- **M1 元组字面量已具备**：`tests/run-pass/tuple_value.rl` 已验证 `(10, 20, 30)`
  构造 + `.f0/.f1/.f2` 按位置字段访问 + 异构元组（含 String）
  （`field.rs:17-37` 的 `strip_prefix('f')` 分支）。
- **M3 多返回已具备**：`fn split(x: i64) -> (i64, i64) { (x, x * 2) }` 可编译运行
  （返回聚合经现有多槽返回机制），此前只是**被 M2 阻塞**而无法解构接收
  （std `Channel` 退化为 `ChannelPair` 结构体的根因即此）。
- **M2 解构缺失**：`let (a, b) = e;` 报「复杂 let 绑定模式（元组 / 结构体等）」。

故本叶子实际交付为 M2（M4 一并回归确认）。

## 实现纪要（2026-09-02）

### M2：元组解构绑定
- 落点：`rlyeh-typecheck/src/check_stmt.rs`。
- **签名改造**：`HirStmt` 无 Block 变体，而解构需展开为多条 `Let`
  （临时变量承载元组值 + 各元素分别绑定）。原 `check_stmt` 返回单条
  `(HirStmt, Type)`，已改为返回 **`(Vec<HirStmt>, Type)`**；唯一调用方
  `check_expr/block.rs:33` 由 `push` 改为 `extend`（其余语句恒为单条，无行为变化）。
- **展开形态**（与 `t.f0` 字段访问同构，见 `field.rs:29-36`）：
  ```text
  __tup_N = e;              // init 只求值一次（避免重复副作用）
  a = FieldGet(__tup_N, 0); // 按位置取字段，ty = field_scalar_of(elem_ty)
  c = FieldGet(__tup_N, 2);
  ```
- **支持形态**：标识符绑定、`_` 通配符（跳过该位置但仍占用下标）、
  整体 `let mut (a, b) = ..`（可变标志作用到各元素）、异构元组、
  `&` 到元组（剥一层引用后解构）、init 为函数调用（多返回值解构接收）。
- **门禁**：元数不匹配报 `WrongType`（`expected N 元元组, found M 元解构模式`）；
  非元组类型报 `expected 元组（N 元）`；嵌套解构 `let ((a,b), c) = ..` 显式报
  Unsupported（不静默误绑定）。

### 已知限制
- **元素模式仅支持标识符与 `_`**：嵌套解构、字面量 / 枚举模式均不支持
  （`let (mut a, b) = ..` 亦不可——parser 不接受元组模式内的 `mut`）。
- 结构体 / 枚举解构模式仍为 Unsupported（原 catch-all 分支保留）。
- 无 `..` 剩余模式（`let (a, ..) = t;`）。

### 验收
- `tests/run-pass/tuple_destructure.rl`（+`.out`）：基本解构 / 通配符 / 异构 /
  多返回值解构接收 / `mut` 解构 / 三元组。
- `tests/compile-fail/tuple-destructure-{arity,non-tuple,nested}.rl`：三类门禁。
- 回归：`for (k, v) in map`（`iter.rs` 的二元元组模式特判，走另一路径）不受影响，
  `hashmap_iter_pairs.rl` 输出不变。
- 全量 `cargo test --workspace -- --test-threads=1` 740 用例全绿。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从评估报告漏判项中拆出（类型层已就绪，值/解构/多返回缺失） |
| 2026-09-02 | 复核确认 M1/M3 已具备，实现 M2 元组解构绑定（`check_stmt` 改为返回语句序列）；三类门禁用例 + 全量 740 全绿 |
