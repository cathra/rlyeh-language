# SH-P0-6 `if let` / `while let` 模式控制流

> **级别**：P0（阻塞前端 PoC） · **风险**：🔴 高 · **状态**：✅ 已完成 · **归属**：0.2.0-O
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.15

## 目标
支持 `if let` / `while let` 模式控制流，使解析器可用 `if let Some(tok) = next() { .. }` 处理 `Option` 返回。语言当前**完全缺失**该语法，解析器/类型检查器重写（`lexer`/`parser` 的 `Option` 返回处理）均依赖此项。

## 现状
- `grammar.md` 无 `if let` / `while let` 产生式；基础 `if`/`while`/`let`/`match` 已存在。
- `Option`/`Result` 已支持（`K1` `?` 运算符已实现），但无「模式绑定于条件位置」的语法。

## 风险分解（→ 中/低危）
- **M1（中）** `if let Pat = expr { .. } else { .. }`：解析为 `let Pat = expr; if /* 绑定成功 */ { .. } else { .. }`，绑定成功判定由模式匹配生成，零新增 IR。
- **M2（中）** `while let Pat = expr { .. }`：循环头绑定 + 条件重评估，复用 M1 模式匹配。
- **M3（低）** `if let`/`while let` 嵌套与链（`if let a = .. && let b = ..` 暂不计，先单模式）。
- **L1（低）** 差分对拍 Rust 参考实现，确认语义一致。

## 受影响组件
`rlyeh-parser`（stmt/expr 解析）、`rlyeh-typecheck`（模式 + 条件作用域）、`rlyeh-codegen`（desugar）。

## 验证
- 解析器 `if let Some(tok) = lexer.next() { .. }` 重写通过差分对拍。
- 单元：`if let`/`while let` 处理 `Option` 返回，结果正确。

## 缺口复核（2026-09-02，动手前）
叶子「现状」称该语法「**完全缺失**」，复核确认**语法层确为缺口**（`grammar.md` 无产生式、
parser 无分支），但**地基完备**，使实现可降为纯 desugar：
- `match` 已具备（含枚举 tag 判别、负载绑定、字面量模式、通配符 `_` 臂）；
- `Option`/`Result` + `?` 已支持，`while let` 所需的 `loop`/`break`/`continue` 已具备；
- 故 **M1/M2 无需新增任何 IR 节点**，全部在 parser 层展开为既有节点。

## 实现纪要（2026-09-02）

### 落点
`crates/rlyeh-parser/src/expr/control.rs`（`parse_if_expr` / `parse_while_expr` 各加
`Token::Let` 分支）；`crates/rlyeh-parser/src/expr/mod.rs` 补入 `MatchArm` 导入。
**typecheck / codegen 零改动**。

### O1：`if let`
```text
if let Pat = e { A } else { B }   ⟶   match e { Pat => { A }, _ => { B } }
```
- 缺 `else` 时兜底空块（求值 `Unit`），保证 match 始终穷尽。
- `else if` / `else if let` 链由 `parse_if_expr` **递归**处理（内层 if 作为 `_` 臂体），
  故链式与嵌套无需额外代码。
- 因 desugar 结果为 `match`（表达式），`let v = if let .. { x } else { 0 };` 天然可用。

### O2：`while let`
```text
while let Pat = e { A }   ⟶   loop { match e { Pat => { A }, _ => break } }
```
- 每轮**重新求值** `e`；模式不再匹配即 `break` 跳出。
- 体内 `break` / `continue` 落在 `loop` 上，语义与 Rust 一致（已用例验证）。

### 已知限制
- **模式能力继承 `match` 臂**：元组 / 结构体模式在 match 位置尚不支持，
  `if let (a, b) = t` 显式报 `unsupported syntax: 元组 / 结构体模式在 MVP 阶段`
  （可解构绑定 `let (a, b) = e;` 走 `check_stmt` 的不可反驳路径，不受此限）。
- **无 let 链**：`if let a = .. && let b = ..`（M3）按规划「先单模式」暂不支持。
- `match` 守卫（SH-P0-7）落地后，`if let` 可经由 match 顺带获得守卫能力。

### 验收
- `tests/run-pass/if_while_let.rl`（+`.out`）：匹配成功 / 走 else / 无 else /
  `else if` 链 / 嵌套 / 作表达式用 / 字面量模式 / `while let` 取通道 /
  `else if let` 链 / `while let` 体内 `continue`+`break`。
- `tests/compile-fail/if-let-tuple-pattern.rl`：元组模式显式报错门禁。
- 两项均已确认被套件收录（篡改 `.out` 与 `expect:` 片段后套件如期失败，恢复后复验通过）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从解析器/类型检查器重写依赖中拆出（语言完全缺失）；补齐缺失叶子文档 |
| 2026-09-02 | 缺口复核（语法确为缺口但地基完备）；O1/O2 以 parser 层 desugar 落地（零新增 IR）；run-pass + compile-fail 用例与全量回归 |