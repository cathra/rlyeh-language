# F2 PGO 数据回灌

> **所属阶段**：阶段 F
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`rlyeh profile` 命令 + `rlyeh build --profile` 编译期注入。

## 背景

阶段 阶段 F 子任务，详见 阶段详情文档 [`stages/F.md`](../../stages/F.md)。

## 技术细节

`rlyeh profile <file.rl_profile> [--out <report.md>]`（`run_profile`，加载 PGO 画像 JSON → `PgoAdvisor::recommend_size`（p95×1.1，下限 64KiB）→ `CompilerInterface` 报告）；`rlyeh build --profile <file>` 编译期注入（加载失败仅告警不阻断）；`region_profile_report(path)` + `build_region_report(data)` 纯函数可单测。

## 验证

`profile_cmd_test.rs` 6 用例：报告元数据/下限回退/文件往返/坏 JSON/缺文件/空数据。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
