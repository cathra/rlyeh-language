# D3 `rlyeh bench` 基准框架

> **所属阶段**：阶段 D
> **状态**：✅ 已完成
> **依赖**：D2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`rlyeh bench` 基准测试框架（tools/rlyeh-bench）。

## 背景

阶段 阶段 D 子任务，详见 阶段详情文档 [`stages/D.md`](../../stages/D.md)。

## 技术细节

纯 std 无外部依赖；`BenchOptions { warmup, runs, quiet }`；`bench_executable` 多次启动进程 Instant 计时；`bench_source` 接受编译回调；`BenchReport` 输出平均/中位/最小/最大/标准差/吞吐（ops/s）；CLI `rlyeh-bench <file.rl|exe> [--runs N] [--warmup N] [--out <路径>] [--quiet]`。

## 验证

`rlyeh-bench` 3 单测 + `doc_bench_test.rs` 集成。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
