# V3-E：内建 desugar 清理 + 全量回归

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)
> **状态**：✅ 已完成（双轨定案：自定义迭代器走 protocol 默认方法；数组/`Vec` 保留内建——数组非命名类型语言限制）
> **风险**：中
> **依赖**：V3-D（D1~D5）
> **权威来源**：`rlyeh-typecheck/check_expr.rs` 1744-2226（`try_check_adapter`）

## 目标

J3 调用点全部迁移后，删除 `try_check_adapter` 内建 desugar，消除双轨。

## 背景

双轨过渡完成后，`try_check_adapter`（及内建 desugar 相关代码）不再需要，删除以统一走 protocol 方法。

## 实施情况

（已完成，2026-08-27）双轨定案：`try_check_adapter` 保留数组/`Vec` 内建分支（`check_iterator_adapter`），自定义迭代器返回 `None` 走通用 protocol 方法解析（惰性默认方法）。数组非命名类型（无法 `impl Iterator`）为语言限制，内建分支非死代码。

## 改动范围

- **双轨定案**：`try_check_adapter` 对 map/filter/fold/collect/take/skip——自定义迭代器走 protocol 默认方法（惰性）；数组/`Vec<T>` 保留内建 desugar（返回 Vec 兼容）。
- 审计 `check_for_*`（数组/Vec/自定义迭代器 for 循环）与适配器方法解析协同：数组/Vec 内建、自定义走 `next()` protocol 方法。
- 全量回归 + 新增覆盖用例（`v3d1_lazy_adapters`/`v3d2_map_lazy`/`v3d3_fold_lazy`）。

## 验证

- [x] 自定义迭代器适配器经 protocol 方法路径通过（惰性）。
- [x] 数组/Vec 适配器内建 desugar 通过（返回 Vec，兼容）。
- [x] 全量测试（146 用例）+ `cargo build` 无 warning。
