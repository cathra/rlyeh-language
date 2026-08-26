# K1 `?` 错误传播运算符

> **所属阶段**：阶段 K
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`expr?` 在 Option/Result 上下文 desugar 为 match + return 早返回。

## 背景

阶段 阶段 K 子任务，详见 阶段详情文档 [`stages/K.md`](../../stages/K.md)。

## 技术细节

零新增 HIR 节点（复用 check_match 的 if-else 链 + tag 比较）。拆分 `check_match_with_scrutinee`（预推断 scrutinee）+ `check_question`（infer inner → Type::Named("Option"/"Result") 特判 → 构造 AST MatchArm）+ 非 Option/Result 报 Unsupported；裸无参变体 `split_variant_path` 兜底；parser 后缀循环 `?`；fmt（PREC_POSTFIX）/check（walk_expr）补分支。

## 验证

`question.{rlyeh,out}` 5 输出（含嵌套双 `?`）+ compile-pass/question_result.rl + compile-fail/question-nonoption.rl。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
