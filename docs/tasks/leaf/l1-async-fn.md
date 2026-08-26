# L1 普通函数 `async fn`/`await`

> **所属阶段**：阶段 L
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

MVP 同步语义，零编译器代码改动。

## 背景

阶段 阶段 L 子任务，详见 阶段详情文档 [`stages/L.md`](../../stages/L.md)。

## 技术细节

`async fn` 关键字经 parser（AstFn.is_async）解析后由 typecheck 忽略（编译为普通同步函数）；`expr.await`（ExprKind::Await）typecheck 直接 `infer_expr(inner)` 求值（`.await` 为语法标记）。`async fn foo(x) -> T` ≡ `fn foo(x) -> T`、`foo().await` ≡ `foo()`；支持嵌套 await、String/数组返回、表达式中间 await。

## 验证

`async-fns.{rlyeh,out}` 6 输出 + 全量 42 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
