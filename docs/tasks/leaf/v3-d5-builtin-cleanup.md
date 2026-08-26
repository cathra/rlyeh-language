# V3-D5：删除内建 desugar 特判 + 全量回归

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-D 细分子任务）
> **状态**：📋 规划
> **风险**：低（收尾清理，机制已全部走 trait 方法）
> **依赖**：V3-D4
> **权威来源**：`rlyeh-typecheck/check_expr.rs` 1744（`try_check_adapter`）、`tests/run-pass`

## 目标

删除 `try_check_adapter` 中已迁移适配器的内建 desugar 特判代码，完成全量回归。

## 背景

V3-D4 双轨过渡后所有调用点已走 trait 方法，本子任务删除残留内建分支，收敛为单一实现路径。

## 改动范围

- **typecheck**：删除 map/filter/fold/collect/take/skip 的内建 desugar 特判分支（若 V3-D4 已逐步删除则确认无残留）。
- **测试**：全量回归 + 适配器链专项用例。

## 验证

- [ ] 无残留内建 desugar 分支（grep 确认）。
- [ ] 全量测试通过，适配器链输出与迁移前一致。
- [ ] 编译告警/死代码清理。

## 为什么是低风险

仅删已死分支 + 回归，机制已稳定走 trait 方法路径，无新逻辑。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-D（高风险）细化拆分而来 |
