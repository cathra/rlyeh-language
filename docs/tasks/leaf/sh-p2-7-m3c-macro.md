# SH-P2-7 M-M3c macro 最小对拍

> **级别**：P2（集成建设） · **风险**：🟠 中 · **状态**：✅ 已完成（2026-11-14） · **归属**：0.2.0-M
> **索引**：[sh-p2-7-driver.md](./sh-p2-7-driver.md) · **计划**：[development-plan-0.2.0.md](../../development-plan-0.2.0.md) §7.7

## 目标
在 Rlyeh 侧实现**最小 macro 能力**（`macro_rules` 子集展开），并与 Rust 侧展开结果逐 token 对拍，补齐计划级 M-M3 的 macro 部分。

## 技术细节
- **oracle**：以 `crates/rlyeh-macro`（既有 `macro_rules` 展开器）为唯一事实源。driver 新增 `src/macro_oracle.rs`（72 行），暴露：
  - `emit_macro_expand(matcher_src, transcriber_src, invocation_src)`：单规则；
  - `emit_macro_expand_rules(matcher_srcs, transcriber_srcs, invocation_src)`：多规则按序取首个完全匹配。
  - 输入均为**组内容**（已剥外层定界符），底层经 `rlyeh_lexer` 词法 + `rlyeh_macro::parse_matcher/parse_transcriber/expand`；输出复用 driver 既有 `token_to_canonical`，每行一个规范 token。
- **Rlyeh 侧**：`self-host/macro.rl`（876 行），复用 `self-host/lexer.rl` 的 `lex`（Rlyeh 无 include，harness 按 lexer.rl + macro.rl 拼接）。
- **实现形态**：matcher/transcriber 各自解析为扁平 arena（`MNode{kind,a,b,kids}`），元变量绑定表 `MBind` 以调用 token 区间 `[cs,ce)` 记录捕获，支持 delim/repeat 作用域快照与提交/回滚；展开为迭代式 sink 收集。

## 覆盖子集（43 组对拍用例）
- 单规则 37 组、多规则 6 组；规则按顺序首个完全消费调用 token 者胜出。
- 字面 token 精确匹配、嵌套定界组（`()`/`[]`/`{}`）。
- 元变量：`$x:ident` / `$x:ty` / `$x:tt` / `$x:expr`（expr 为 token 级优先级爬升捕获；ty/tt 的 MVP 语义与 Rust 侧一致）。
- 重复：`$($x),*`（0+）/ `$($x),+`（1+）/ `$($x),?`（0/1，含可选分隔符），分隔符可选；重复片段内多 token；重复 + 定界组。
- 空 matcher / 空 transcriber / 空调用、原始标识符 `r#type`、比较/方法链/范围表达式捕获。

## 明确不支持（与 Rust oracle 的 MVP 限制同源）
- hygiene、前向引用、`$crate`；
- 嵌套重复（`$($($x)*)*`）；
- 其它元变量种类（`pat`/`stmt`/`path`/`item` 等，Rust oracle 亦返回错误）；
- `$x:ty` 只捕获单个 token（`Vec<i64>` 只捕获 `Vec`，与 `rlyeh-macro` 一致）。

## 实证的 Rlyeh 约束
- **函数递归可用**：本文件的 matcher 解析 / 匹配 / 展开均自递归，经 driver 编译运行验证；此前的 parser 切片出于风险规避未用递归，但并非语言限制。
- `Vec<String>` 承载 token 流与 arena；元变量捕获以字符串区间记录，避免 `Vec<i64>` 的 `get`/`pop` 返回 `ptr` 类型冲突。
- 返回 `String` 的函数继续遵守「累加变量 + 尾部返回」；`&&`/`||` 不短路，边界判断用嵌套 `if`。
- 结构体需先定义后使用；被调函数先于调用方定义。

## 验证结果
- `cargo test -p rlyeh-driver --test self_host_macro` → **2 passed, 0 failed**（49.75s）。
- `cargo test -p rlyeh-driver --test self_host_parser` → **15 passed, 0 failed**（446.61s，M-M3b/M-M3c 共用 driver 无回归）。

## 状态
✅ 已完成（2026-11-14）。M-M3（ast + macro）三切片全部收口。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-10-19 | 新建（M-M3 拆分：M-M3c）；启动：rlyeh-macro 为 oracle + 最小 macro_rules 子集 |
| 2026-11-14 | 完成并验证：`self-host/macro.rl` + `macro_oracle.rs` + `self_host_macro.rs`（2/2，43 组用例）；记录递归可用等实证约束 |
