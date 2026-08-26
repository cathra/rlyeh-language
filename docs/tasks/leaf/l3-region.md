# L3 region 选项接线

> **所属阶段**：阶段 L
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

语言级 `region` 选项完整语法 + region 指令接线 `rlyeh-region-alloc` C ABI 运行时。

## 背景

阶段 阶段 L 子任务，详见 阶段详情文档 [`stages/L.md`](../../stages/L.md)。

## 技术细节

`strategy (bump)` 选项（AST RegionStrategy::Bump）；`HirRegionOptions`→MIR→LIR 传递；`AllocInRegion{size}`/`RegionExit{name}` 节点；匿名区域 `__anon_region_N`。运行时接线（rlyeh-region-alloc cabi.rs）：`rlyeh_region_enter/alloc/transfer/exit`。PGO 回灌：`--profile` → `region 'r adaptive` 初始容量。关键教训：Apple clang GEP 独立指令、`--force` 缓存。

## 验证

`region_alloc.{rlyeh,out}` 10 输出 + 全量用例全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
