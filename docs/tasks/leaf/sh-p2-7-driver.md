# SH-P2-7 前端自举 PoC（driver 自举）

> **级别**：P2（集成建设） · **风险**：🔴 高 · **状态**：🟡 进行中（M-M1 已落地、M-M2a 已落地、M-M2b 已落地（b1 语句骨架 + b2 模式/类型标注 + b3 元组/数组类型标注））、M-M2c 已落地（控制流 if/else/else-if/while/loop 的语句/块尾/let 初始化双形态）、M-M2d 已落地（字段访问 .field）、M-M2e 已落地（索引 `[ ]` / 调用 `( )` / 方法 `a.b(x)`）、M-M3 已落地（元组字面量 `(a, b)` / 数组字面量 `[a, b]`，shunting-yard 后缀标记 + 归约，无递归）） · **归属**：0.2.0-M
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.13

## 目标
用 Rlyeh 重写编译器前端 `lexer` + `parser` + `ast` + `macro`（依赖 A/B/C/E 落地），经 Rust `rlyeh-driver` 编译通过，并与 Rust 版前端**对拍**，证明「前端逻辑主体已是 Rlyeh 源码、可被自身工具链编译」（dogfood 交付物）。

## 技术细节
- 依赖：A 泛型 protocol/impl（前端内部 protocol）、B 嵌套模块（前端 crate 组织）、C derive 宏（AST 样板）、E `unsafe`（底层字节操作）均已落地。
- 交付物：Rlyeh 版前端源码 + 对拍测试 `tests/self-host-*`（同 `.rl` 输入，token/AST 一致）。
- 复用 K 的差分 harness 做逐步对拍。

## 风险分解（→ 中/低危）
- **M-M1（中）** 用 Rlyeh 重写 `lexer`（依赖 N 元组值 / O `if let` / P `match` 守卫），经 Rust driver 编译 + 差分对拍 token 一致。
  - **M-M1a（✅ 已落地，2026-09-20）** 切片1：标识符/全量关键字/整数（十进制·hex·bin·oct·`_`·后缀）/字符串（无转义）/单字符+多字符运算符/空白与行·块注释（嵌套）。Rlyeh 版 `self-host/lexer.rl` 经 `crates/rlyeh-driver/tests/self_host_lexer.rs` 与 Rust oracle（`--emit tokens`）对拍，corpus1/corpus2 token 逐行一致。
  - **M-M1b（✅ 已落地，2026-09-20）** 字符字面量 `'x'`、生命周期 `'r`/`'a`（`'` 后非 `'` 即生命周期）、`not in`→`NOTIN`、原始字符串 `r"..."`/`r#"..."#`、原始标识符 `r#kw`、时间字面量（`9am`→`TIME 9:0`、`6pm`→`TIME 18:0`、`22:00`→`TIME 22:0`、`9:30am`→`TIME 9:30`，canonical 不含 is_pm）。`tests/self-host-lexer/corpus3.rl` 对拍一致。
  - **M-M1c（转义解码 ✅ 已落地，2026-09-20；浮点 ✅ 已落地，2026-09-21；非法字符 ✅ 已落地，2026-09-21）** 字符串/字符转义解码（`\n`/`\t`/`\r`/`\\`/`\"`/`\'`/`\xHH`，仅 ASCII 范围；`\u{...}` 在 >127 时落 U+FFFD 替换符，与 oracle 一致）已落地，`tests/self-host-lexer/corpus4.rl` 对拍一致。① 浮点字面量：✅ 已落地（2026-09-21）——按本计划背书方式，oracle canonical 改为发射**原始拼写**（`FLOAT 1.0`/`FLOAT 1e10`/`FLOAT 2.5e-10`/`FLOAT 0.5` 等，类型后缀 `f64`/`f32` 吸收不计入），`Token::FloatLiteral` 增 `raw: String` 字段携带原始拼写，`crates/rlyeh-driver/tests/self_host_lexer.rs` 新增 corpus5 内联用例对拍一致；② 非法字符报错：✅ 已落地（2026-09-21）——Rlyeh 版 lexer 在非法字符兜底分支 `panic!`（W 阶段已落地的内建，子进程 abort），由 `tests/self_host_lexer.rs` 负向用例验证 oracle（`Err(InvalidChar)`）与 Rlyeh 版两侧均报错（反引号 / `~` / 裸反斜杠等）。
- **M-M2（中）** 用 Rlyeh 重写 `parser`（依赖 N/O/P + 递归下降），对拍 AST 一致。
  - **M-M2a（✅ 已落地，2026-09-21）** 切片1：表达式 → 规范 S-表达式 AST。`self-host/parser.rl` 采用 **shunting-yard + RPN 迭代建树（无递归）**，规避 Rlyeh 的共享 `Vec` 传递/递归语义问题，覆盖整数/标识符/一元负号/括号/二元 `+ - * / %` 与比较（`< <= > >= == !=`）/逻辑（`&& ||`）/位（`& | ^ << >>`）运算，括号透明（不产生节点）。`crates/rlyeh-driver` 新增 `emit_ast_canonical_expr` oracle（渲染为与 Rlyeh 版逐字节对齐的规范文本），`tests/self_host_parser.rs` 差分对拍一致。
  - **M-M2b（✅ 已落地，2026-09-21）** 切片2：语句/块 → 程序级规范 S-表达式 AST。`self-host/parser.rl` 在 M-M2a 基础上新增 `tokenize` + `parse_expr`（对 token 流 shunting-yard）+ `parse_program`，**迭代式 buffer 栈**处理块（`{` 入栈新 buffer、`}` 出栈包成 `(block ...)`，不引入递归）。**b1（2026-09-21）**：覆盖 `let`/`let mut`（模式仅标识符、类型标注忽略）、表达式语句（带 `;` → `(semi ...)`）、块尾裸表达式（`}` 前末位 → 裸）、`return`/`return;`，`crates/rlyeh-driver` 新增 `emit_ast_canonical` + `render_program/stmt/block_canonical`（保留 `emit_ast_canonical_expr`），`tests/self_host_parser.rs` 新增 `m_m2b_statement_ast_matches_rust_oracle` 差分对拍一致（含嵌套块、块尾表达式、return、混合 let+块+裸表达式）。**b2（2026-09-21）**：补齐 `let` **模式**（扁平元组 `(a, b)` / `_` 通配，非递归，`parse_pattern_tokens`）与**类型标注**（`i64` / `&T` / `&mut T` / `Vec<T>` 泛型，迭代式 `@GEN@` 栈，嵌套泛型 `Vec<Result<i64,String>>` 正确收束；元组类型 `(A,B)` 与数组 `[T;N]` 留待后续切片）；Rust oracle 同步新增 `render_pattern_canonical` / `render_type_canonical`；b1 既有用例（无类型标注）输出不变，新增类型/模式用例逐字节对拍一致。
  - **M-M2b3（✅ 已落地，2026-09-22）** 补齐 b2 遗留的类型标注：`parse_type_core` 扩展迭代式栈解析**元组类型** `(A, B, ...)`（`(` 压 `@TUP@`、`)` 弹出直到 `@TUP@` 包成 `(tuple-type ...)`，支持嵌套元组与元组作泛型实参 `Vec<(i64,String)>`）与**数组类型** `[T; N]`（`[` 压 `@ARR@`、`;` 压 `@ASEP@` 且其后长度表达式按字面渲染不包 `(type ...)`、`]` 弹出直到 `@ARR@` 以 `@ASEP@` 为界拆出类型 T 与长度 N 包成 `(array T N)`）；长度字面 `(int 4)` 与 oracle `render_expr_canonical` 一致；嵌套泛型 + `>>`(`shr`) 拆分在 `[Vec<Result<i64,String>>; 2]` 中验证正确。b2 移除的 `(i64,i64)` 元组类型用例已回归；`self_host_parser.rs` 的 `m_m2b` 新增元组/数组类型对拍用例（含元组内泛型、元组作数组元素、数组内嵌套泛型）逐字节一致。
  - **M-M2c（✅ 已落地，2026-09-22）** 控制流 `if`/`else`/`else if`/`while`/`loop` 的**语句形态**与**块尾表达式形态（双形态）**：Rlyeh 版 `self-host/parser.rl` 新增**迭代式控制帧栈**（`cf_kind`/`cf_cond`/`cf_then`/`cf_else`/`cf_state`/`cf_elseif`/`cf_sink`/`cf_depth` + pending-let 栈），配合既有迭代式 buffer 栈解析块体，**不引入递归**（规避 Rlyeh 递归/共享 `Vec` 语义限制）；`}` 闭合按 `cf_depth` 判定归属本帧的 then/else 块，支持嵌套控制流与 `else if` 链（`else if` 收束为嵌套 `if` 并以 `(block ...)` 包裹作为父帧 else）；`let x = if ...` / `let x: T = while ...` 等控制流作 **let 初始化表达式**经 pending-let 帧组装为 `(let <PAT> [<TYPE>] (if ...))`。Rust oracle 同步在 `render_expr_canonical` 新增 `If`/`While`/`Loop` 分支（节点 `(if <C> <then-block> [<else-block>])` / `(while <C> <body>)` / `(loop <body>)`）；`tests/self_host_parser.rs` 新增 `m_m2c_control_flow_ast_matches_rust_oracle` 差分对拍一致（含 `if/else`、`else if` 链、条件含比较链、嵌套控制流、块尾控制流、控制流后接语句 → `(semi ...)`、`let` 初始化控制流）。
  - **M-M2d（✅ 已落地，2026-09-22）** 表达式补全·字段访问 `.field`：Rlyeh 版 `self-host/parser.rl` 在 M-M2a 的 shunting-yard `parse_expr` 中新增 `.name` 后缀处理——推入 `@field@NAME` 标记入 RPN，在末位 RPN 归约阶段（`tk.get(0)==64` 分支）弹出栈顶节点包裹为 `(field <RECV> <NAME>)`，支持链式 `a.b.c`、分组 `(a+b).c`、与二元运算结合 `a.x + b.y` / `x.y == z.w`，且**不引入递归 / 新函数 / 嵌套循环**（规避 Rlyeh 前向递归与 if-表达式返回值限制）。Rust oracle 同步在 `render_expr_canonical` 新增 `FieldAccess` 分支（`(field <RECV> <FIELD>)`）；`tests/self_host_parser.rs` 新增 `m_m3a_field_access_ast_matches_rust_oracle` 差分对拍一致（含 `let` 初始化与 `if` 条件中的字段访问）。
  - **M-M2e（✅ 已落地，2026-09-22）** 表达式补全·索引 / 调用 / 方法：`self-host/parser.rl` 的 `parse_expr` 将 `[` / 调用 `( `（前接操作数判定）/ 右 `]` / 右 `)` / 调用实参分隔 `,`（仅 `call_depth>0` 上下文）统一转成 RPN 后缀标记并在末位归约阶段建树，**全程迭代无递归**（子表达式在 RPN 内直接展开，靠 opstack 括号界定的运算符冲刷，无需递归调用 `parse_expr`）。标记约定（均 `@` 起头，`f==64` 分支）：`@field@NAME`（字段，弹栈顶包裹）、`@open-index@`/`@close-index@`（索引，归约弹至 `@OPEN@` 取索引子表达式 + 接收者包裹为 `(index <RECV> <IDX>)`）、`@open-call@`/`@argsep@`/`@close-call@`（调用，归约弹至 `@OPEN@` 拆 `@ARGSEP@` 得实参 + 接收者包裹为 `(call <CALLEE> <ARG>...)`）。`(` 区分分组（`prev_op==0`）与调用（`prev_op==1` 推 `(c` 标记并 `call_depth+1`）；`,` 仅在 `call_depth>0` 作实参分隔、否则为顶层停止符。覆盖 `a[0]` / `a[i+1]` / `arr[i][j]` / `foo()` / `foo(1,2)` / `a.b(1)` / `add(1,2).scale(3)` / `f(g(x))` / 混合 `map.get(k).push(1)` / `(a+b)[i].call(x)`，与 oracle `Index`/`Call`/`MethodCall` 渲染逐字节一致（`MethodCall` 经 `(call (field <RECV> <M>) <ARGS>...)` 归一）。踩坑：`recv` 是 Rlyeh 保留关键字（通道接收），作变量名触发 typecheck 「expected pattern, found Recv」，已改名 `rcv`；一元负号 `u-` 须走 shunting-yard 运算符路径（`prec_of==100`，落于运算元之后归约），不可提前压入 rpn（否则与运算元顺序颠倒成 `(int 5)`）。`tests/self_host_parser.rs` 新增 `m_m2e_index_call_ast_matches_rust_oracle` 差分对拍一致。
  - **M-M3 表达式补全·元组/数组字面量（✅ 已落地，2026-09-22）** `self-host/parser.rl` 的 `parse_expr` 在 M-M2e 的 shunting-yard 后缀标记化基础上，新增**元组字面量** `(a, b)` 与**数组字面量** `[a, b]`：分组 `(` 统一推 `@open-paren@`（归约压 `@PARENSENT@` 哨兵，纯分组透明、仅作元组定位起点）；首个 `,` 出现时将栈顶 `(` 升级为 `(t`（元组，`tuple_depth+1`）并推 `@tupsep@`；`[` 在 primary 位（`prev_op==0`）推 `@open-arr@` + `[a` + `arr_depth+1`，在运算元后（`prev_op==1`）仍作索引；`)` / `]` 依栈顶 `(t` / `[a` 分别发 `@close-tup@` / `@close-arr@` 并降深度；`,` 依栈顶括号类型（`(c`/`(t`/`[a`/`(`）分流 `@argsep@`/`@tupsep@`/`@arrsep@` 或升级 `(→(t)`，顶层裸逗号则停止。`@close-tup@`/`@close-arr@` 归约时弹至 `@PARENSENT@`/`@ARRSENT@` 哨兵、按 `@TUPSEP@`/`@ARRSEP@` 分拆得元素包裹为 `(tuple ...)` / `(array-lit ...)`。全程迭代无递归。Rust oracle `render_expr_canonical` 新增 `TupleLit`/`ArrayLit`/`Unit` 分支（`(tuple E1 E2 ...)` / `(array-lit E1 E2 ...)` / `(unit)`）；`tests/self_host_parser.rs` 新增 `m_m3_tuple_array_literal_ast_matches_rust_oracle` 差分对拍一致（含 `(1,2)`/`(1,2,3)`/`(1+2,3)`/`((1,2))`/`[1,2,3]`/`[(1,2),3]`/`[[1,2],[3,4]]`/`foo((1,2))`/`bar([1,2,3])` 及 `let` 初始化 + 元组解构 `let (a,b)=(1,2)`）。踩坑：① Rlyeh 字符串字面量类型是 `string` 原始类型，与 `String` 拼接报 `expected string, found String`——归约构造 `(tuple ...)`/`(array-lit ...)` 的累加串须以 `String::from(...)` 初始化（与 `@close-call@` 一致）；② 分组/元组/调用/数组哨兵 `@PARENSENT@`/`@TUPSEP@`/`@ARGSEP@`/`@OPEN@`/`@ARRSENT@`/`@ARRSEP@` 非真实节点，二元运算 / `u-` / `@close-*` 收集时须**跳过**它们，否则会泄漏为操作数（如 `(a+b)*(c-d)` 曾误产出 `@PARENSENT@`）；③ 已知限制：元组元素内含嵌套分组 `(1, (a+b))` 时，内层 `@PARENSENT@` 会被误判为元组起点导致元素丢失（分组哨兵与元组起点共用 `@PARENSENT@` 所致），当前用例未覆盖，留待显式括号深度标记修复。
- **M-M3（中）** 用 Rlyeh 重写 `ast` + `macro`（依赖 C derive / I 内部可变性），对拍 AST 节点构造一致。
- **M-M4（中）** 串联 M1–M3，经 Rust `rlyeh-driver` 编译通过，并与 Rust 版前端**对拍**（同 `.rl` 输入，token/AST 一致）。
- **L1（低）** 验收即「前端逻辑主体已是 Rlyeh 源码、可被自身工具链编译」；接入 K 的三阶段 bootstrap 做 dogfood。

## M-M1 对拍规范（切片1）

### 规范化 token 格式
Rlyeh 版 `self-host/lexer.rl` 与 Rust oracle（`rlyeh run <f> --emit tokens`）输出**逐行一致**的规范化文本，每行一个 token：

- 关键字 → 关键字原文（如 `let` / `fn` / `Self` / `gc_region`）。
- 标识符 → `IDENT <文本>`；整数 → `INT <十进制值>`；字符串 → `STR <内容>`。
- 运算符 → 拼写原文（`+` `-` `*` `/` `%` `==` `!=` `<=` `>=` `&&` `||` `->` `=>` `+=` `-=` `*=` `/=` `%=` `<<` `>>` `..` `..<` `...` `<..` `@` `#` `$` `?` 等）；`not in` → `NOTIN`。
- 行尾差异由 harness 归一化（`trim_end`）处理，不计入比对。

> Rlyeh 版与 Rust 版对「值」的呈现必须一致：`INT` 打印十进制数值（hex/bin/oct 解析后的值），`STR` 打印解码后内容。新增 token 种类时两侧须同步。

## M-M2 对拍规范（切片1，S-表达式 AST）

Rlyeh 版 `self-host/parser.rl` 与 Rust oracle（`rlyeh_driver::emit_ast_canonical_expr`，**非** `{:#?}` 调试 dump）输出**逐字节一致**的规范 S-表达式文本：

- 整数 → `(int <十进制>)`；标识符 → `(var <名>)`；布尔 → `(bool <true|false>)`；字符串 → `(str <内容>)`。
- 一元负号 → `(neg <操作数>)`。
- 二元/比较运算 → `(bin <op> <左> <右>)`，`op` ∈ `add sub mul div mod and or bitand bitor bitxor shl shr lt le gt ge eq ne`（比较在 oracle AST 中为 `ComparisonChain`，统一渲染为 `bin` 以与 Rlyeh 版对齐）。
- **括号透明**：不产生节点（oracle AST 无 Paren 节点），与 shunting-yard 行为一致。
- 该格式为本次切片自定义规范；后续切片（语句/项/模式）将沿用并可扩展节点类型。

### M-M2 对拍规范（切片2，程序/语句级 S-表达式 AST）

Rlyeh 版 `self-host/parser.rl` 的 `parse_program` 与 Rust oracle（`rlyeh_driver::emit_ast_canonical`，**非** `{:#?}` dump）输出**逐字节一致**的规范 S-表达式文本：

- 程序 → `(program <STMT> ...)`；块 → `(block <STMT> ... [<FINAL-EXPR>?])`（`FINAL-EXPR` 仅块内末位、紧跟 `}` 的裸表达式，裸渲染）。
- `let` → `(let <PAT> [<TYPE>] <INIT>)`；`let mut` → `(let mut <PAT> [<TYPE>] <INIT>)`（M-M2b2 起：`<PAT>` 为标识符 / `_` / `(tuple-pat <E1> <E2> ...)` 扁平元组；`<TYPE>` 可选，存在时渲染为类型节点）。类型节点：`(type <NAME> <ARG>...)`（路径，泛型实参递归渲染）/ `(ref <T>)` / `(ref mut <T>)` / `(tuple-type <T1> <T2> ...)` / `(array <T> <N>)` / `(infer)`。`<TYPE>` 缺失时不输出（与 M-M2b1 兼容）。
- 表达式语句：带 `;` → `(semi <EXPR>)`；块内末位裸表达式（紧跟 `}`）→ 裸 `<EXPR>`（即块 `final_expr`）；顶层裸表达式 / 块后接更多语句 → `(semi <EXPR>)`。
- `return <EXPR>;` → `(semi (return <EXPR>))`；`return;` → `(semi (return))`；块内末位裸 `return` → 裸 `(return ...)`。
- 表达式中 `return` 前缀由 Rlyeh 版 `parse_expr` 识别为 `(return ...)` 节点（与 oracle `ExprKind::Return` 一致）。
- 实现：Rlyeh 版用**迭代式 buffer 栈**（遇 `{` 入栈新 buffer、遇 `}` 出栈包成 `(block ...)`，按 `nxt == "}"` 判定是否块尾 final），不引入递归，规避 Rlyeh 共享 `Vec`/递归语义限制。
- **控制流（M-M2c）**：`if`/`while`/`loop` 作语句 → `(semi (if <C> <then> [<else>]))` / `(semi (while <C> <body>))` / `(semi (loop <body>))`；作块尾表达式（紧跟 `}` 无 `;`）→ 裸 `(if ...)` / `(while ...)` / `(loop ...)`（与 oracle `AstStmt::Semi` vs 块 `final_expr` 一致）。`else if` 收束为嵌套 `if` 且整体包 `(block ...)` 作为父 `if` 的 else（`(if <C1> <then1> (block (if <C2> <then2> [<else2>])))`）。控制流作 `let` 初始化 → `(let <PAT> [<TYPE>] (if ...))`，由 pending-let 帧在控制帧收束时组装。实现：Rlyeh 版用**迭代式控制帧栈**（`cf_kind`/`cf_cond`/`cf_then`/`cf_else`/`cf_state`/`cf_elseif`/`cf_sink`/`cf_depth`）配合 buffer 栈，`}` 闭合按 `cf_depth`（`int_to_string(bufstack.len())`）判定归属本帧 then/else 块，支持嵌套与 `else if` 级联（子帧 `feif=="1"` 收束时挂到下方帧 else 并包 `(block ...)`）。

### Rlyeh 语言踩坑点（已实证的约束，供 M-M2/M-M3 复用）
- **字符串字面量是 `string` 值类型**，需 `String::from("...")` 转成 `String` 对象才能调用 `.len`/`.get`/`.substring`；从声明为 `-> String` 的函数返回字面量必须用 `String::from` 包裹（直接 `return "x";` 会返回槽类型错配 → 运行时崩溃），但 `Vec<String>.push("x")` 可自动转换。
- `String` 的 `.len` 是**字段**（无括号），`.get(i)`/`.substring(a,b)` 是**方法**（带括号）。
- 大函数 / 长输入在默认小栈线程做 typecheck 会栈溢出；`driver` 顶层 `run_source` 与对拍 harness 均在 **64MB 栈线程**中编译+运行。
- `continue` / `while` / `Vec<String>` 迭代均可用（已验证）。
- **元组字面量构造 `(a, b)` 作为表达式当前不支持**（typecheck 报「复杂被调用表达式（类型 `()` 不可调用）」）；但 `let (a, b) = ...` 解构绑定可用。需从函数返回多值时改用**结构体**（如 `struct PRes { s: String, ti: i64 }` + `PRes { s: x, ti: y }` + `r.s`/`r.ti` 字段访问），本切片 `parse_expr` 即如此实现。
- `String` 无 `to_string()` 方法；构造字符串用 `String::from("...")` 或 `+` 拼接（已实证：M-M2b 初版因此编译失败）。
- `out` 是保留关键字（region `transfer ... out of 'r` 语法），**不能**用作变量名；M-M2a 曾因此编译失败，已改用 `rpn`/`opstack` 等。
- `recv` 是保留关键字（通道接收），**不能**用作变量名；M-M2e 初版因 `let recv = ...` 触发 typecheck 「expected pattern, found Recv」，已改名为 `rcv`。`send`/`await`/`channel`/`Thread` 等亦属并发保留字，命名时避让。
- **`&&` / `||` 不短路**：Rlyeh 的逻辑与/或会**求值两侧**，不能用于边界短路（如 `i < n && arr.get(i)` 在 `i >= n` 时仍访问越界 → 段错误）。边界保护一律改用嵌套 `if`（`if i < n { if arr.get(i) == ... {} }`），M-M2b2 据此修正 `parse_type_core` 的泛型基类型前瞻与 ref-mut 判定。
- **返回 `String` 的函数禁用 `return` 提前返回**：会生成错误的 `ret i64`（函数声明 `ptr` 却返回 i64，整模块 LLVM 校验失败，所有依赖该模块的测试连带失败）。统一用「累加变量 + 尾部表达式」返回（`let mut r = ...; ...; r`），M-M2b2 据此重写 `typename_of`/`parse_pattern_tokens`/`parse_type_core`/`parse_type_tokens`，彻底消除 `return` 与 `if`-表达式作为返回值。
- **`if`/`else` 表达式作为返回值（且 then 分支含循环）会错误返回条件布尔值**：Rlyeh 的 if-表达式代码生成在 else 分支会返回**条件值**而非 else 表达式值（`ret i64` = 条件）。一律改用语句式 `if`（赋值或提前 return），不把 `if/else` 当值使用。
- **前向调用被推断为 `i64`**：被调用函数若定义在其**调用方之后**，Rlyeh 会把它当作返回 `i64`，导致调用方类型错配（同 `ret i64` vs `ptr` 故障）。被调用方须定义在调用方之前（`parse_type_core` 须在 `parse_type_tokens` 之前定义）。
- **类型泛型闭合 `>>` 词法记为右移 `shr`**：`Vec<Result<T,E>>` 的 `>>` 被 tokenize 成单个 `shr` token，导致外层泛型无法闭合（输出 `(type shr)`）。类型上下文须把 `shr` 拆成两个 `gt`（或整体上把 `>>` 视为两个 `>`）；M-M2b2 在收集类型 token 后做此拆分。
- **`Vec<i64>` 的 `get`/`pop` 返回 `ptr` 而非 `i64`**：M-M2c 初版用 `Vec<i64>` 存控制帧状态/深度，编译报「变量 `x` 类型冲突：期望 ptr，实际 i64」（`.get`/`pop` 返回指针型，与需要 i64 的 `bufstack.len() == depth` 比较冲突）。规避：所有整数态一律用 `Vec<String>` 编码——深度/索引用 `int_to_string(i64)` 转字符串后 `.push`、比较时再 `int_to_string(bufstack.len()) == cf_depth.get(tidx)`，状态机用 `"0"`/`"1"`/`"2"` 字符串、sink 用 `"stmt"`/`"let"`、else-if 子帧用 `"1"`，全程零 `Vec<i64>`。

## 受影响组件
`rlyeh-lexer`、`rlyeh-parser`、`rlyeh-ast`、`rlyeh-macro`（待建）、`rlyeh-driver`（编译入口）、`tests/`。

## 验证
- Rlyeh 版 lexer/parser/ast/macro 经 Rust driver 编译通过。
- 对拍测试 `tests/self-host-*`：同 `.rl` 输入，Rlyeh 版与 Rust 版 token/AST 一致。

## 状态
🟡 进行中（0.2.0 必须项，阶段 M；交付物 dogfood）。M-M1 切片1(a+b+c) 与 M-M2a/M-M2b（b1 语句骨架 + b2 模式/类型标注 + b3 元组/数组类型标注）/M-M2c（控制流 if/else/else-if/while/loop 语句/块尾/let 初始化双形态）已落地：Rlyeh 版 lexer `self-host/lexer.rl`（标识符/关键字/整数/浮点/字符串(含转义)/运算符/注释/char/生命周期/`not in`/时间/原始字符串/原始标识符）经 `tests/self_host_lexer.rs` 与 Rust oracle（`--emit tokens`）差分对拍 corpus1..5 逐行一致，M-M1c 非法字符报错（✅ 2026-09-21，负向对拍）已落地；Rlyeh 版 parser `self-host/parser.rl`（M-M2a 表达式切片 shunting-yard+RPN 建树；M-M2b 程序/语句切片 tokenize+迭代 buffer 栈+模式/类型解析；M-M2c 控制流切片迭代式控制帧栈+双形态+`else if` 级联+`let` 初始化；M-M2d 字段访问 `.field`；M-M2e 索引/调用/方法；M-M3 元组/数组字面量 `(a,b)`/`[a,b]`）经 `tests/self_host_parser.rs` 与 Rust oracle `emit_ast_canonical_expr`/`emit_ast_canonical`/`render_pattern_canonical`/`render_type_canonical` 差分对拍一致（含嵌套泛型 `Vec<Result<i64,String>>`、`&`/`&mut`、扁平元组模式 `(a, _)`、控制流语句/块尾/`else if` 链/控制流作 let 初始化、字段/索引/调用/方法/元组/数组字面量）；剩余 M-M4（串联编译）、M-M3 的 ast/macro 重写（当前为 oracle 对照）待推进。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新增（前端自举 PoC，复用 K 的差分 harness 对拍） |
| 2026-09-20 | M-M1a 落地：新增 `self-host/lexer.rl`（Rlyeh 版 lexer 切片1）、`tests/self-host-lexer/{corpus1,corpus2}.rl`、`crates/rlyeh-driver/tests/self_host_lexer.rs`（与 `--emit tokens` oracle 差分对拍）；driver 新增 `--emit tokens` oracle（`emit_tokens`/`token_to_canonical`）；叶子补充 M-M1 切片计划与对拍规范、Rlyeh 语言踩坑点 |
| 2026-09-20 | M-M1b 落地：Rlyeh 版 lexer 扩展 char 字面量 / 生命周期（`'` 消歧）/ `not in`→NOTIN / 原始字符串 `r"..."`+`r#"..."#` / 原始标识符 `r#kw` / 时间字面量（`9am`/`6pm`/`22:00`/`9:30am`）；新增 `tests/self-host-lexer/corpus3.rl`，对拍 harness 扩展至 corpus3（三组全部逐行一致） |
| 2026-09-20 | M-M1c 转义解码落地：Rlyeh 版 lexer 字符串/字符分支支持 `\n \t \r \\ \" \' \xHH` 解码（ASCII 范围，`\u{...}`>127 落 U+FFFD 与 oracle 一致）；新增 `tests/self-host-lexer/corpus4.rl`，对拍 harness 扩展至 corpus4（四组全部逐行一致）。浮点字面量与非法字符报错暂推迟（见 M-M1c 说明） |
| 2026-09-21 | M-M1c② 非法字符报错落地：Rlyeh 版 lexer 非法字符兜底分支由静默 `?` token 改为 `panic!`（W 阶段内建，子进程 abort）；`tests/self_host_lexer.rs` 新增 `m_m1c2_illegal_char_panics` 负向用例，验证 oracle（`Err(InvalidChar)`）与 Rlyeh 版两侧均报错（反引号 / `~` / 裸反斜杠）；修复并行用例共享临时产物的偶发失败（加全局互斥锁 + 唯一临时文件名） |
| 2026-09-21 | M-M2a 落地：新增 `self-host/parser.rl`（Rlyeh 版 parser 切片1，表达式 → 规范 S-表达式 AST；shunting-yard + RPN 迭代建树，**无递归**，规避 Rlyeh 共享 `Vec`/递归语义限制），覆盖整数/标识符/一元负号/括号/二元算术与比较/逻辑/位运算；`crates/rlyeh-driver` 新增 `emit_ast_canonical_expr` oracle（渲染为与 Rlyeh 版逐字节对齐的规范文本，非 `{:#?}` dump），`tests/self_host_parser.rs` 差分对拍一致；踩坑点补充 `out` 为保留关键字 |
| 2026-09-21 | M-M2b 落地：扩展 `self-host/parser.rl` 新增 `tokenize` + `parse_expr`（token 流 shunting-yard）+ `parse_program`（迭代式 buffer 栈处理块，不递归），覆盖 `let`/`let mut`/表达式语句/块尾裸表达式/`return`；`crates/rlyeh-driver` 新增 `emit_ast_canonical` + `render_program/stmt/block_canonical`（保留 `emit_ast_canonical_expr`），`tests/self_host_parser.rs` 新增 `m_m2b_statement_ast_matches_rust_oracle` 差分对拍一致；踩坑点补充「元组字面量构造不支持（改用结构体）/ `String` 无 `to_string()`」，并修正「块尾 final 以 `nxt == "}"` 判定」 |
| 2026-09-21 | M-M2b2 落地：补齐 `let` **模式**（`parse_pattern_tokens`：标识符 / `_` / 扁平元组 `(a, b)`，非递归）与**类型标注**（`parse_type_tokens` + `parse_type_core`：迭代式 `@GEN@` 栈处理泛型 `<...>` 收束，支持 `i64` / `&T` / `&mut T` / `Vec<T>` 嵌套泛型，元组类型 `(A,B)` 与数组 `[T;N]` 留待后续）；`crates/rlyeh-driver` 新增 `render_pattern_canonical` / `render_type_canonical` 与 `AstPattern`/`AstType` 渲染；`tests/self_host_parser.rs` 扩展 b2 用例（`Vec<Result<i64,String>>`/`&mut`/扁平元组/混合）逐字节对拍一致。实证 Rlyeh 5 项关键约束并写入踩坑点：`&&`/`||` 不短路（须嵌套 if 做边界保护）、返回 `String` 的函数禁用 `return`（须累加变量+尾部返回）、if-表达式含循环时 else 错误返回条件值、`>>` 词法记为 `shr`（类型上下文须拆成两个 `gt`）、前向调用被推断为 `i64`（被调用方须先定义）
| 2026-09-22 | M-M2b3 落地：补齐 b2 遗留类型标注——`self-host/parser.rl` 的 `parse_type_core` 扩展迭代式栈解析元组类型 `(A, B)`（`@TUP@` 标记，支持嵌套元组与元组作泛型实参 `Vec<(i64,String)>`）与数组类型 `[T; N]`（`@ARR@`/`@ASEP@` 标记，长度表达式按字面渲染不包 `(type ...)`，嵌套泛型 `>>` 经 `shr` 拆分）；b2 移除的 `(i64,i64)` 元组类型用例回归；`tests/self_host_parser.rs` 的 `m_m2b` 新增元组/数组类型对拍用例（含元组内泛型、元组作数组元素、数组内嵌套泛型）逐字节一致；oracle 侧 `render_type_canonical` 的 `Tuple`/`Array` 渲染已就绪无需改动 |
| 2026-09-22 | M-M2c 落地：控制流 `if`/`else`/`else if`/`while`/`loop` 的语句/块尾双形态——`self-host/parser.rl` 新增**迭代式控制帧栈**（`cf_kind`/`cf_cond`/`cf_then`/`cf_else`/`cf_state`/`cf_elseif`/`cf_sink`/`cf_depth` + pending-let 栈），配合迭代式 buffer 栈解析块体，`}` 闭合按 `int_to_string(bufstack.len()) == cf_depth` 判定归属本帧 then/else 块，支持嵌套控制流与 `else if` 链（子帧收束挂到下方帧 else 并包 `(block ...)`）；`let x = if ...` 等控制流作 let 初始化经 pending-let 帧组装为 `(let <PAT> [<TYPE>] (if ...))`；全程零 `Vec<i64>`（规避 `Vec<i64>` 的 `get`/`pop` 返回 ptr 的类型冲突，整数态一律 `Vec<String>` + `int_to_string` 编码）。Rust oracle `render_expr_canonical` 新增 `If`/`While`/`Loop` 分支（`(if <C> <then> [<else>])`/`(while <C> <body>)`/`(loop <body>)`）；`tests/self_host_parser.rs` 新增 `m_m2c_control_flow_ast_matches_rust_oracle` 差分对拍一致（含 `if/else`、`else if` 链、条件比较链、嵌套控制流、块尾控制流、控制流后接语句、控制流作 let 初始化） |
| 2026-09-22 | M-M2d + M-M2e 落地：表达式补全·字段访问 `.field`（M-M2d，shunting-yard 后缀 `@field@NAME` 标记 + 末位归约弹栈顶包裹为 `(field <RECV> <NAME>)`，支持链式/分组/混合、无递归）与索引/调用/方法（M-M2e，`[`/`(`/`,`/`) `/`]` 统一转 RPN 后缀标记 `@open-index@`/`@close-index@`/`@open-call@`/`@argsep@`/`@close-call@` + `@OPEN@`/`@ARGSEP@` 哨兵归约，全程迭代无递归；`prev_op==1` 的 `(` 判为调用推 `(c`+`call_depth+1`，`,` 仅在 `call_depth>0` 作实参分隔）。Rust oracle `render_expr_canonical` 含 `FieldAccess`/`Index`/`Call`/`MethodCall` 分支（`MethodCall` 归一为 `(call (field <RECV> <M>) <ARGS>...)`）；`tests/self_host_parser.rs` 新增 `m_m3a_field_access_*` / `m_m2e_index_call_*` 差分对拍一致（覆盖 `a[0]`/`a[i+1]`/`arr[i][j]`/`foo()`/`foo(1,2)`/`a.b(1)`/`add(1,2).scale(3)`/`f(g(x))`/混合 `map.get(k).push(1)`/`(a+b)[i].call(x)` 等）。踩坑：`recv` 为 Rlyeh 保留关键字（通道接收）作变量名触发 typecheck 「expected pattern, found Recv」，已改名 `rcv`；`u-` 须走 shunting-yard 运算符路径（`prec_of==100`，落于运算元之后）不可提前压 rpn |
| 2026-09-22 | M-M3 表达式补全·元组/数组字面量（`(a, b)` / `[a, b]`）落地：`self-host/parser.rl` 的 `parse_expr` 新增元组字面量（首个 `,` 将分组 `(` 升级为 `(t` 元组，`@tupsep@` 分隔，`@close-tup@` 弹至 `@PARENSENT@` 哨兵包裹 `(tuple ...)`）与数组字面量（`[` 在 primary 位推 `@open-arr@`+`[a`，`@arrsep@` 分隔，`@close-arr@` 弹至 `@ARRSENT@` 包裹 `(array-lit ...)`），全程迭代无递归；Rust oracle `render_expr_canonical` 新增 `TupleLit`/`ArrayLit`/`Unit` 分支（`(tuple ...)`/`(array-lit ...)`/`(unit)`）；`tests/self_host_parser.rs` 新增 `m_m3_tuple_array_literal_ast_matches_rust_oracle` 差分对拍一致。踩坑：字符串字面量类型是 `string` 原始类型须 `String::from` 初始化累加串；分组/元组/调用/数组哨兵须在二元/`u-`/`@close-*` 弹栈时跳过否则泄漏为操作数（`(a+b)*(c-d)` 曾误产出 `@PARENSENT@`）；已知限制：元组元素含嵌套分组 `(1,(a+b))` 时内层哨兵误判为元组起点 |