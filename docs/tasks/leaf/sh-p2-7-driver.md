# SH-P2-7 前端自举 PoC（driver 自举）

> **级别**：P2（集成建设） · **风险**：🔴 高 · **状态**：🟡 进行中（M-M1 切片1(a+b) 已落地） · **归属**：0.2.0-M
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

### Rlyeh 语言踩坑点（已实证的约束，供 M-M2/M-M3 复用）
- **字符串字面量是 `string` 值类型**，需 `String::from("...")` 转成 `String` 对象才能调用 `.len`/`.get`/`.substring`；从声明为 `-> String` 的函数返回字面量必须用 `String::from` 包裹（直接 `return "x";` 会返回槽类型错配 → 运行时崩溃），但 `Vec<String>.push("x")` 可自动转换。
- `String` 的 `.len` 是**字段**（无括号），`.get(i)`/`.substring(a,b)` 是**方法**（带括号）。
- 大函数 / 长输入在默认小栈线程做 typecheck 会栈溢出；`driver` 顶层 `run_source` 与对拍 harness 均在 **64MB 栈线程**中编译+运行。
- `continue` / `while` / 元组返回 / `Vec<String>` 迭代均可用（已验证）。

## 受影响组件
`rlyeh-lexer`、`rlyeh-parser`、`rlyeh-ast`、`rlyeh-macro`（待建）、`rlyeh-driver`（编译入口）、`tests/`。

## 验证
- Rlyeh 版 lexer/parser/ast/macro 经 Rust driver 编译通过。
- 对拍测试 `tests/self-host-*`：同 `.rl` 输入，Rlyeh 版与 Rust 版 token/AST 一致。

## 状态
🟡 进行中（0.2.0 必须项，阶段 M；交付物 dogfood）。M-M1 切片1(a+b+c) 已落地：Rlyeh 版 lexer `self-host/lexer.rl`（标识符/关键字/整数/浮点/字符串(含转义)/运算符/注释/char/生命周期/`not in`/时间/原始字符串/原始标识符）经 `tests/self_host_lexer.rs` 与 Rust oracle（`--emit tokens`）差分对拍，corpus1/2/3/4/5 token 逐行一致；M-M1c 非法字符报错（✅ 2026-09-21，负向对拍）已落地；剩余 M-M2/M-M3/M-M4 待推进。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新增（前端自举 PoC，复用 K 的差分 harness 对拍） |
| 2026-09-20 | M-M1a 落地：新增 `self-host/lexer.rl`（Rlyeh 版 lexer 切片1）、`tests/self-host-lexer/{corpus1,corpus2}.rl`、`crates/rlyeh-driver/tests/self_host_lexer.rs`（与 `--emit tokens` oracle 差分对拍）；driver 新增 `--emit tokens` oracle（`emit_tokens`/`token_to_canonical`）；叶子补充 M-M1 切片计划与对拍规范、Rlyeh 语言踩坑点 |
| 2026-09-20 | M-M1b 落地：Rlyeh 版 lexer 扩展 char 字面量 / 生命周期（`'` 消歧）/ `not in`→NOTIN / 原始字符串 `r"..."`+`r#"..."#` / 原始标识符 `r#kw` / 时间字面量（`9am`/`6pm`/`22:00`/`9:30am`）；新增 `tests/self-host-lexer/corpus3.rl`，对拍 harness 扩展至 corpus3（三组全部逐行一致） |
| 2026-09-20 | M-M1c 转义解码落地：Rlyeh 版 lexer 字符串/字符分支支持 `\n \t \r \\ \" \' \xHH` 解码（ASCII 范围，`\u{...}`>127 落 U+FFFD 与 oracle 一致）；新增 `tests/self-host-lexer/corpus4.rl`，对拍 harness 扩展至 corpus4（四组全部逐行一致）。浮点字面量与非法字符报错暂推迟（见 M-M1c 说明） |
| 2026-09-21 | M-M1c② 非法字符报错落地：Rlyeh 版 lexer 非法字符兜底分支由静默 `?` token 改为 `panic!`（W 阶段内建，子进程 abort）；`tests/self_host_lexer.rs` 新增 `m_m1c2_illegal_char_panics` 负向用例，验证 oracle（`Err(InvalidChar)`）与 Rlyeh 版两侧均报错（反引号 / `~` / 裸反斜杠）；修复并行用例共享临时产物的偶发失败（加全局互斥锁 + 唯一临时文件名） |