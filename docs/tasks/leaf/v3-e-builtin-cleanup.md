# V3-E：内建 desugar 清理 + 全量回归

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)
> **状态**：📋 规划
> **风险**：中
> **依赖**：V3-D（D1~D5）
> **权威来源**：`rlyeh-typecheck/check_expr.rs` 1744-2226（`try_check_adapter`）

## 目标

J3 调用点全部迁移后，删除 `try_check_adapter` 内建 desugar，消除双轨。

## 背景

双轨过渡完成后，`try_check_adapter`（及内建 desugar 相关代码）不再需要，删除以统一走 trait 方法。

## 实施情况

（未开始实施）

## 改动范围

- 删除/精简 `try_check_adapter`（check_expr.rs 1744-2226）及其调用点。
- 审计 `Vec`/`HashMap`/数组迭代的 for 循环（`check_for_*`）与适配器方法解析的协同。
- 全量回归 + 新增覆盖用例（适配器链、嵌套、空迭代器）。

## 验证

- [ ] 内建 desugar 删除后，全部适配器用例经 trait 方法路径通过。
- [ ] 全量测试（`cargo test --workspace` + `rlyeh test tests`）通过。
- [ ] clippy 零新增警告 + rustfmt 干净。
