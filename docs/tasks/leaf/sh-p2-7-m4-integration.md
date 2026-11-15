# SH-P2-7 M-M4 driver 串联 + 扩大真实示例 dogfood

> **级别**：P2（集成建设） · **风险**：🟠 中 · **状态**：✅ 已完成（2026-11-14） · **归属**：0.2.0-M
> **索引**：[sh-p2-7-driver.md](./sh-p2-7-driver.md) · **计划**：[development-plan-0.2.0.md](../../development-plan-0.2.0.md) §7.7

## 目标
把 lexer + ast + parser 三层自举前端**串联**为单一编译单元，经 Rust `rlyeh-driver` 编译运行，并对更多真实 `.rl` 示例做端到端对拍，形成 SH-P2-7 的 dogfood 交付证据。

## 技术细节
- 新增 `crates/rlyeh-driver/tests/self_host_dogfood.rs`（295 行）：
  - `frontend_src()` 按 `self-host/lexer.rl` → `self-host/ast.rl` → `self-host/parser/*.rl`（文件名序）拼接；Rlyeh 无 `include`，与 stdlib 平铺单元策略一致；
  - 单次 `IncrementalDriver` 编译 + 运行，一次产出四层结果：`lex(src)` 规范化 token、`parse_program(src)` 规范 S-表达式 AST、`ast_kinds(parse_ast(canonical))` 前序节点 head、`render_ast(parse_ast(canonical))` roundtrip；
  - 四项分别对拍 Rust oracle：`emit_tokens_str` / `emit_ast_canonical` / `emit_ast_kinds` / canonical 原文；
  - 每个语料使用唯一临时源文件，64MB 栈线程编译，测试内加锁串行（复用 driver 临时产物）。
- **符号消歧**：lexer 与 parser 各自定义 `is_ident_cont`，单编译单元会重名；parser 侧（`00_prelude.rl`/`01_tokenize.rl`）改名 `p_is_ident_cont`，语义不变。
- 覆盖清单 `COVERED` 与未覆盖清单 `UNSUPPORTED` 内置为常量，`m_m4_dogfood_manifest_is_complete` 锁定「至少 16 个语料（examples ≥8、tests/run-pass ≥8）+ 至少 8 类未覆盖语法」。

## Dogfood 语料（16 个真实文件）
### examples/（8）
- `hello-world.rl`、`arith-print.rl`（既有 dogfood）
- `by-chapter/01-what-is-rlyeh.rl`、`by-chapter/02-quick-start.rl`、`by-chapter/11-targets-toolchain.rl`
- `projects/benchmarks/fib/fib.rl`（递归 + return + if）
- `projects/smoke/util/mod.rl`（pub fn + String）
- `std-demos/02-collections/str_value.rl`（字符串字面量/方法/拼接/比较）

### tests/run-pass/（8）
- `hello.rl`、`arith.rl`（最小程序 + 算术/控制流/布尔）
- `tuple_value.rl`（元组字面量 + 字段访问 + 异构元组）
- `v2a_str_semantics.rl`（&str 类型/方法/索引）
- `v2c_print.rl`（String/&str/trim 打印链）
- `path_ops.rl`（Path 方法链 + `let _` 通配）
- `stdout_stderr.rl`（stdout/stderr 对象 + 方法链）
- `sleep_join.rl`（Duration/Thread/if-else + 返回 i64）

## 明确未覆盖语法（自举 parser 尚未实现）
| 语法族 | 说明 |
|--------|------|
| module/import/use | 模块系统与导入 |
| impl/trait/protocol | trait/impl 块与协议声明 |
| closure | 闭包字面量 `|x| ..` 与迭代器适配器 |
| async/await/actor | 异步、await、actor/spawn |
| region/gc_region | 区域内存语法 |
| struct-literal | 结构体字面量 `S { x: 1 }`、结构体/枚举解构模式 |
| enum-pattern | 枚举变体模式、`if let`/`while let` 复杂模式 |
| slice/ref-expr | 切片类型 `&[T]`、表达式位置一元 `&`/`*` |
| compound-assign | 复合赋值 `+=`/`-=`、裸赋值语句 |
| macro/attribute | `vec!`/`assert!` 等宏调用、`#[..]` 属性、derive |
| const/static/type | const/static/类型别名、where 约束 |
| float-escape | 浮点字面量、含转义的字符串/字符字面量 |

逐项根因：M-M8 只接入 `struct`/`enum` 声明而未接入 impl/trait、模块与结构体字面量；parser 的 `parse_expr` 尚无闭包/`?`/`as`/一元 `&`/`*`；tokenize 仅处理整数、不含转义的字符串与布尔字面量，未接浮点/char/转义解码；`parse_program` 只处理 `let` 与表达式语句，无裸赋值与复合赋值。

### 语料筛选中的代表性排除项
- `v2d_str_functions.rl`：含 `" \t "` 转义，Rlyeh tokenizer 当前不解码 `\t`（输出字面反斜杠+t），对拍暴露该限制；已登记为 float-escape 族，不纳入 covered。
- `open_mode.rl` / `question.rl` / `fs_ops.rl`：分别依赖枚举变体模式、`?`、`Ok(v)` 模式，均属 enum-pattern/其它未覆盖族。
- `array_for.rl` / `loop_sum.rl` / `sort.rl`：依赖 `+=` 或裸赋值，属 compound-assign。

## 验证结果
- `cargo test -p rlyeh-driver --test self_host_dogfood` → **2 passed, 0 failed**（35.56s；16 个真实语料四层对拍全绿）。
- 同一 harness 覆盖的 token 对拍同时验证 lexer 层；AST/kinds/roundtrip 同时验证 parser 真·arena 与 ast.rl 类型化模型。
- parser 辅助函数改名后 `cargo test -p rlyeh-driver --test self_host_parser` → **15 passed, 0 failed**（446.61s）。

## 状态
✅ 已完成（2026-11-14）。SH-P2-7 前端自举 PoC 的 M-M4 收尾完成；三阶段 bootstrap（K-M1..K-M3）按 SH-P2-5 推迟至 0.3.0，不属本轮。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-10-19 | 新建（M-M4） |
| 2026-11-14 | 完成并验证：`self_host_dogfood.rs` 单编译单元串联 lexer+ast+parser，16 个真实语料四层对拍（2/2，35.56s）；parser `p_is_ident_cont` 改名；登记 12 类未覆盖语法与排除项 |
