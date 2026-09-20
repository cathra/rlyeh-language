# V3-D5：删除内建 desugar 特判 + 全量回归

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-D 细分子任务）
> **状态**：✅ 已完成（数组/`Vec` 内建 desugar 保留——数组非命名类型语言限制，非死代码）
> **风险**：低（收尾清理，机制已全部走 protocol 方法）
> **依赖**：V3-D4
> **权威来源**：`rlyeh-typecheck/check_expr.rs` 1744（`try_check_adapter`）、`tests/run-pass`

## 目标

删除 `try_check_adapter` 中已迁移适配器的内建 desugar 特判代码，完成全量回归。

## 背景

V3-D4 双轨过渡后所有调用点已走 protocol 方法，本子任务删除残留内建分支，收敛为单一实现路径。

## 改动范围

- **typecheck**：`try_check_adapter` 内建 desugar 分支**保留**（仅服务数组/`Vec<T>` 接收者）——自定义迭代器已返回 `None` 走通用 protocol 方法解析，数组/`Vec` 因**语言限制**（Rlyeh 数组为内建非命名类型，无法 `impl Iterator`）须保留内建。非死代码，grep 确认无对自定义迭代器的残留内建分支。
- **测试**：全量回归（146 用例）+ 适配器链专项用例。

## 验证

- [x] 无对自定义迭代器的残留内建 desugar 分支（自定义迭代器走 protocol 方法）。
- [x] 全量测试通过（146 用例），适配器链输出与迁移前一致。
- [x] 编译告警清理（build 无 warning）。

## 为什么是低风险

仅删已死分支 + 回归，机制已稳定走 protocol 方法路径，无新逻辑。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-D（高风险）细化拆分而来 |
