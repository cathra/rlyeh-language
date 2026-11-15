# SH-P2-7 M-M3b parser 切换 AST 构造

> **级别**：P2（集成建设） · **风险**：🟠 中 · **状态**：✅ 已完成（2026-10-19） · **归属**：0.2.0-M
> **索引**：[sh-p2-7-driver.md](./sh-p2-7-driver.md) · **计划**：[development-plan-0.2.0.md](../../development-plan-0.2.0.md) §7.7

## 目标
在 M-M3a 模型定型后，把 `self-host/parser/*.rl` 的解析结果由「直接拼规范串」改为「构造 AST 节点 → 统一 render」，并保持既有 14 组差分对拍逐字节不变。

## 技术细节
- 依赖：M-M2（parser 已完成且已拆分）、M-M3a（AST 模型 + 节点级 oracle）。
- 分步：表达式层 → 语句/块层 → 项层；每步保持 `self_host_parser.rs` 全绿。
- 注意：保持函数定义顺序（Rlyeh 前向调用被推断为 i64）与现有碎片顺序一致。

## 交付（2026-10-19，方案 A：真·arena 全层贯通）
- `self-host/ast.rl` 264→355 行：`ast_push_atom/list/leaf`、`ast_cat`、`render_node`、`ast_list_node_count`；`render_ast` 内联渲染（规避结构体 Vec 字段再传 `&Vec` 形参的异常 IR）。
- `self-host/parser/02_expr.rl` 486→567：`parse_expr_arena` RPN 归约直接建 `AstNode`、栈存节点索引；path 原地改 var→path 防孤儿。
- `03_types.rl` 345→393（类型/模式/match 模式节点化）、`04_items.rl` 270→308（param/params/header/generics/struct/enum）、`05_program.rl` 568→600（buffer 栈存子节点索引，全节点组装后 `render_ast`）、`00_prelude.rl` 244→247（`PRes.id`）。
- 测试：`self_host_parser.rs` 新增 `m_m3b_parser_arena_nodes_match_ast_model`（①文本==oracle；②parser arena 列表节点数==ast.rl `parse_ast` 列表节点数；③`ast_kinds(parse_ast(text))==emit_ast_kinds`；含 dogfood 示例）。

## 验证结果
- `cargo test -p rlyeh-driver --test self_host_parser`：**15/15 通过**（14 组原有文本对拍逐字节不变 + 新节点级测试），501.33s。
- `cargo test -p rlyeh-driver --test self_host_ast`：**4/4 通过**，58.99s。

## 新实证约束
- `&mut Vec<AstNode>` 可跨函数多层传递并反复调用；顶层局部 arena 需 block-scope 或作为 worker 形参。
- 结构体形参的 Vec 字段再传 `&Vec` 形参生成异常 IR（运行段错误）→ `render_ast` 内联渲染规避。
- `parse_ast` 连续原子合并使总节点数有 ±差，故按列表节点数对拍；`parse_ast` 收到非 S-表达式（root=-1）会段错误。

## 状态
✅ 已完成（2026-10-19）。遗留：`00_prelude.rl` 旧 `parse()`（M-M2a 遗留、无调用者）建议后续清理。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-10-19 | 新建（M-M3 拆分：M-M3b） |
| 2026-10-19 | 完成并验证：真·arena 全层贯通（parser 15/15 + ast 4/4），新增节点级断言 |
| 2026-11-14 | SH-P2-7 收尾复核：parser 15/15；parser 辅助函数 `is_ident_cont`→`p_is_ident_cont` 以支持 M-M4 中 lexer+ast+parser 单编译单元串联（语义不变） |
