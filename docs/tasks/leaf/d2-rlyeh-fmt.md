# D2 `rlyeh fmt` 格式化器

> **所属阶段**：阶段 D
> **状态**：✅ 已完成
> **依赖**：D1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`rlyeh fmt` 格式化器（tools/rlyeh-fmt）。

## 背景

阶段 阶段 D 子任务，详见 阶段详情文档 [`stages/D.md`](../../stages/D.md)。

## 技术细节

解析为 AST 后按统一规范重建：顶层项空一行、块类表达式多行展开、表达式按优先级表重排括号保证语义不变（Assign<Or<And<BitOr<BitXor<BitAnd<Compare<Shift<Add<Mul<Cast<Unary<Postfix）、字符串/字符按 lexer 转义规则重编码、浮点强制保留小数点、时间字面量还原；`AstStmt::Expr`=带分号/`Semi`=无分号；self 接收者 `&self`/`&mut self` 特例。CLI `rlyeh fmt <file> [--check] [-w] [--indent N]`。

## 验证

`rlyeh-fmt` 9 单测 + `fmt_check_test.rs` 集成（round-trip 幂等、格式化后编译运行语义不变）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
