# SH-P0-7 `match` 守卫 + 范围/或模式

> **级别**：P0（阻塞前端 PoC） · **风险**：🔴 中高 · **状态**：✅ 已完成 · **归属**：0.2.0-P
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.16

## 目标
支持 `match` 守卫 `p if cond => ..`、范围模式 `0..=10` / `0..<10`、或模式 `A | B`，覆盖解析器字符分类、类型检查器判别分支等密集 `match` 逻辑。

## 现状
- `grammar.md` 无守卫/范围/或模式的 `match` 产生式；`match` 基础 + 模式解析已存在。
- 范围语义已由 `in 0..<10` 比较链/区间判断实现（CODEBUDDY §3.2），可复用。

## 风险分解（→ 中/低危）
- **M1（中）** `match x { p if cond => .. }` 守卫：解析守卫表达式，desugar 为「绑定模式变量到临时 + `if cond { arm_body } else { 下一 arm }`」，零新增 IR。
- **M2（中）** 范围模式 `0..=10` / `0..<10` / `0...10`：复用 `in` 区间判断语义生成比较链；与字面量 arm 共存。
- **M3（中）** 或模式 `A | B`：复用 union 优先级规则（`&T | &mut U` 解析），展开为「任一子模式命中即进入 arm」。
- **M4（低）** 守卫 + 范围/或组合、多重守卫链。
- **L1（低）** 差分对拍 Rust 参考。

## 受影响组件
`rlyeh-parser`（pattern/guard）、`rlyeh-typecheck`（match 检查）、`rlyeh-codegen`（desugar）。

## 验证
- 解析器字符分类 `match c { 'a'..='z' | 'A'..='Z' if !escaped => .. }` 重写通过差分。
- 单元：范围模式 + 守卫混合 `match`。

## 缺口复核（2026-09-02，动手前）
叶子「现状」称守卫 / 范围 / 或模式均缺失，复核发现**三项缺口层次各不相同**：

| 项 | 规划描述 | 复核结果 |
|----|---------|---------|
| **M1 守卫** `p if cond` | 缺失 | **语法与 typecheck 均已存在**，但**有真实缺陷**：绑定语句位于臂体块内、晚于条件求值，致引用绑定的守卫读到未初始化值（`Option::Some(v) if v > 10` 恒假 → 静默落到后续臂）。→ 本轮**修复** |
| **M2 范围模式** | 缺失 | `AstPattern::Range` 已有 AST 变体、parser 已解析（含单测），**仅 `check_pattern` 显式 Unsupported**（「范围模式在 MVP 阶段」）。→ 缺口只在 typecheck |
| **M3 或模式** `A \| B` | 缺失 | **确为全链路缺口**（AST 无变体、parser 不支持）。→ 本轮**实现** |

## 实现纪要（2026-09-02）

### M1 守卫修复：绑定必须在守卫之前求值
`check_match_with_scrutinee` 中臂条件原为 `模式条件 && 守卫`。两处障碍：
- **绑定晚于条件**：`binds` 在 `then_block` 内，条件先求值 → 守卫读到未初始化槽。
- **`&&` 在 MIR 非短路**：`lower_expr` 对 `Binary` 两侧无条件求值，故「把绑定塞进块表达式作 `&&` 右操作数」会让绑定被无条件执行（指针类负载上还会解引用未初始化值）。

改为以 **`if <模式条件> { <绑定>; <守卫> } else { false }` 作条件**——借用 HIR `If`
的真实 CFG 分叉获得短路语义：绑定与守卫仅在模式命中后求值。
代价：绑定被求值两次（条件内一次、臂体内一次），均为纯字段读取 / 取址，无副作用。

### M2 范围模式（typecheck）
`AstPattern::Range` 分支：边界经 `infer_expr` 求值后，类型校验与 HIR 构造分别
复用比较链的 `check_comparison` / `compare_hir`（两者由 private 提升为
`pub(crate)`），故模式侧语义与 `x in lo..<hi` **完全一致**——排序仅放行数值与
字符，其余报 `MissingPartialOrd`。支持 `lo..<hi` / `lo...hi` / `lo<..hi` 三种开闭。

### M3 或模式（AST + parser + typecheck + 工具链）
- **AST**：新增 `AstPattern::Or(Vec<AstPattern>)`。
- **parser**：新增 `parse_or_pattern`。**不可并入 `parse_pattern` 本身**——闭包参数
  列表以 `|` 作分隔符与结束符（`|x, y| ..`），让模式吞 `|` 会误食参数列表结束符
  （与类型注解处不收集 `|` 联合同理）。仅用于 match 臂与 `if let` / `while let`
  的模式位置（`||` 是独立 token `Token::OrOr`，不会被 `BitOr` 误匹配）。
- **typecheck**：绑定**只取首个备选**（其变量注册在当前臂作用域内）；其余备选在
  **临时作用域**内检查、仅取条件、注册随即丢弃（条件只依赖 scrutinee，不引用被
  丢弃的槽名）。一致性按**源码层变量名**校验（`util::pattern_bind_names`）——不可
  用 `bound_tys` 比较，因其存的是会随作用域深度 mangle 的槽名。
- **工具链**：`rlyeh-fmt` 打印 `A | B`；`rlyeh-check` 的 `bind_pattern` 按首个备选绑定。

### M4 组合
守卫 + 范围 / 或模式由上述分支天然组合（守卫与模式条件合流为单一条件表达式）。

### 连带修复：codegen `Assign` 未按目标槽类型转换
`LirStmt::Assign { target, value: Local(src) }` 原按**源**类型宽度直接 store：
bool 形参与具名 bool 局部的槽是 `i64`（8 字节），而按值推断的 bool 临时槽是
`i1`（1 字节），`store i64 %v, i1* %t` 会**写穿 1 字节槽破坏相邻栈**——守卫值
并入 bool 临时槽即经此路径（表现为守卫恒假）。修复：源与目标类型不同时插入
`icmp ne i64 %v, 0`（I64→Bool）/ `zext i1 %v to i64`（Bool→I64）；其余组合保持
原样以免波及既有路径。

### 已知限制
- **守卫仅对可反驳模式可用**：标识符 / `_` 兜底模式无匹配条件可合并，报
  `对兜底模式使用守卫条件`（与 Rust 的 `x if cond` 不同，属 MVP 约束）。
- **或模式不支持不可反驳备选**（`_ | 1` 报错），且各备选须绑定同名同序变量。
- **范围模式仅数值与字符**（与 `in` 区间判断一致）；边界须为字面量。
- 守卫求值仍受「MIR `&&` 非短路」影响——本实现已用 `If` 绕开，但**守卫与模式
  条件之外的复合布尔表达式仍是非短路的**（既有语义，非本项引入）。

### 验收
- `tests/run-pass/match_guard_range_or.{rl,out}`（28 行输出）：范围三种开闭与边界 /
  守卫四路分派（含引用绑定 `Option::Some(v) if v > 10`）/ 或模式字面量与字符范围 /
  **或模式绑定同名变量**（`Shape::Circle(r) | Shape::Square(r)`）/ 或模式 + 守卫 /
  范围 + 守卫引用被匹配变量 / `if let` 或模式。
- `tests/compile-fail/match-{or-bind-mismatch,or-irrefutable,guard-on-catchall,range-nonord}.rl`
  四类门禁。
- 新用例经「篡改 `.out` → 套件如期失败 → 恢复复验通过」确认被收录；
  `match_guard_range_or.out` 纳入 `.gitignore` 白名单供 CI 精确比对。
- 全量 `cargo test --workspace -- --test-threads=1` 无失败。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从解析器/类型检查器重写依赖中拆出 |
| 2026-09-02 | 缺口复核（M1 已存在但有绑定不可见缺陷 / M2 仅缺 typecheck / M3 全链路缺失）；修复 M1、实现 M2/M3/M4；连带修复 codegen `Assign` 槽类型转换；run-pass + 4 项 compile-fail 用例与全量回归 |
