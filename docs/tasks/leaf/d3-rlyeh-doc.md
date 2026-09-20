# D3 `rlyeh doc` 文档生成器

> **所属阶段**：阶段 D
> **状态**：✅ 已完成
> **依赖**：D2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`rlyeh doc` 文档生成器（tools/rlyeh-doc）。

## 背景

阶段 阶段 D 子任务，详见 阶段详情文档 [`stages/D.md`](../../stages/D.md)。

## 技术细节

按源码位置提取 `///` 文档注释（连续行合并，`//!` 文件级文档）；输出 Markdown：标题 + 目录 + 按类别分组正文（函数/结构体/枚举/Protocol/impl/Actor/常量/模块/其他）；签名重建独立实现（`item_signature`/`fn_signature`/`fmt_type`/`fmt_param`，self 接收者特判）。CLI `rlyeh doc <file.rl> [--out <file.md>] [--title <标题>]`。

## 验证

`rlyeh-doc` 5 单测 + `doc_bench_test.rs` 集成。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
