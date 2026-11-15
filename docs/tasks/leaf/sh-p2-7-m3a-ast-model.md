# SH-P2-7 M-M3a 类型化 AST 模型 + 节点级对拍

> **级别**：P2（集成建设） · **风险**：🟠 中 · **状态**：✅ 已完成（2026-10-19） · **归属**：0.2.0-M
> **索引**：[sh-p2-7-driver.md](./sh-p2-7-driver.md) · **计划**：[development-plan-0.2.0.md](../../development-plan-0.2.0.md) §7.7

## 目标
在 Rlyeh 侧定义**类型化 AST 模型**（替代「直接拼规范 S-表达式串」），并与 Rust AST 做**节点级**对拍，为 M-M3b（parser 切换 AST 构造）提供载体。

## 技术细节
- 新增 `self-host/ast.rl`：建议扁平 arena（Node 含 head 与子节点索引串），规避递归/Box 与 `Vec<i64>` 取 ptr 的既有坑。
- 能力：`parse_ast(text)` 由规范 S-表达式文本构造 AST；`ast_kinds(...)` 输出前序 head 序列；（可选）`render_ast` 回渲染。
- Rust oracle：`crates/rlyeh-driver/src/lib.rs` 追加 `emit_ast_kinds`，节点命名严格镜像既有 `render_*_canonical`。
- 对拍：`crates/rlyeh-driver/tests/self_host_ast.rs`——`emit_ast_kinds(src)` vs Rlyeh `ast_kinds(emit_ast_canonical(src))` 逐行一致。

## 验证
- 新测试通过（表达式 + 程序级 corpus：int/var/neg/bin、field/index/call/tuple/array-lit、let/block/semi/if/while/loop/for/match/fn/struct/enum/generics 等）。
- 记录实现中印证/新发现的 Rlyeh 语言约束。

## 交付（2026-10-19）
- `self-host/ast.rl`（264 行）：扁平 arena 类型化 AST（`AstNode{head,kids,kind}` + `Ast{nodes,root}`）；`parse_ast`（canonical 文本 → arena，括号唯一定界，`str` 载荷含空格/空串原样保留）、`ast_kinds`（前序 head）、`render_ast`（逐字节回渲染）、`ast_node_count`/`ast_kind_count`；全程迭代无递归。
- `crates/rlyeh-driver/src/lib.rs` 末尾纯追加 `emit_ast_kinds` / `emit_ast_kinds_expr`（遍历 Rust AST，命名镜像 `render_*_canonical`；每行一个 head）。
- `crates/rlyeh-driver/tests/self_host_ast.rs`（342 行）：表达式 33 + 程序/项 42 corpus + dogfood 示例；每 corpus 断言 kinds 逐行一致 + roundtrip 逐字节一致，另含覆盖度自检。
- 覆盖 head：program/int/var/neg/bin/field/index/call/tuple/array-lit/str/bool/path/unit/let/block/semi/if/while/loop/for/range/match/arm/fn/params/param/struct/enum/variant/generics/return/ref/infer/tuple-pat/tuple-type/array/type。
- 新 Rlyeh 约束：`Vec<T>.pop()` 返回 `Option<T>`；`String::trim()` 返回 `&str`（无 `.len` 字段，需 `String::from` 包裹）；struct 须先定义后使用；`Vec<Struct>.get(i).field` 与 struct literal `.push` 可用；`out` 仍为保留字。
- 限制：含 `(`/`)` 的字符串内容无法按括号定界解析（oracle canonical 不转义，属既有规范限制），未纳入 corpus。

## 验证结果
`cargo test -p rlyeh-driver --test self_host_ast` → 4 passed, 0 failed（约 47–60s）。

## 状态
✅ 已完成（2026-10-19）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-10-19 | 新建（M-M3 拆分：M-M3a） |
| 2026-10-19 | 完成并验证：`self-host/ast.rl` + `emit_ast_kinds` + `self_host_ast.rs`（4/4 通过，kinds + roundtrip） |
| 2026-11-14 | SH-P2-7 收尾复核：M-M3a 4/4 通过；M-M3c/M-M4 完成后 ast.rl 同时作为 dogfood 四层对拍中的 kinds/roundtrip 层 |
