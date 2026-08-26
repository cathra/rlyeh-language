# I1 声明式宏

> **所属阶段**：阶段 I
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

新 crate `rlyeh-macro`：`macro_rules!` matcher/transcriber。

## 背景

阶段 阶段 I 子任务，详见 阶段详情文档 [`stages/I.md`](../../stages/I.md)。

## 技术细节

matcher/transcriber token 解析、匹配 `$x:expr`/`ident`/`ty`/`tt` 四类元变量 + `$(`...`)` 重复 `*`/`+`/`?`，`MacroError(String)`；lexer 新增 `$`/`?` token（Dollar/Question，`!` 保留 not 一元运算符）；parser 集成——`macro_rules!` 定义注册 + `name!` 调用（内置宏 → MacroCall AST；用户宏 → expand_macro 递归展开 + 子 Parser from_tokens，MAX_MACRO_DEPTH=64）；AST 新增 `ExprKind::MacroCall`。

## 验证

rlyeh-macro 12 单测 + compile-pass/macro.rl + run-pass/macro.{rlyeh,out} + 116 套件。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
