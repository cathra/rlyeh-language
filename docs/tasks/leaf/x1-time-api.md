# X1 时间 API 完整化

> **所属阶段**：阶段 X
> **状态**：✅ 已完成
> **依赖**：U6
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`Duration` 补构造器/读方法 + `Instant::duration_since` + `SystemTime`。

## 背景

阶段 阶段 X 子任务，详见 阶段详情文档 [`stages/X.md`](../../stages/X.md)。

## 技术细节

`Duration` 补构造器 `microseconds`/`nanoseconds` + 读方法 `as_secs`/`as_millis`/`as_nanos`（u64/u128 → i64 实现）+ `Instant::duration_since` + 新增 `time/system.rl` `SystemTime`（`now`/`unix_epoch`/`duration_since`）。driver `time_builtin_ir` 重构为共享 `__rlyeh_clock_now(clk_id)` + `__rlyeh_clock_monotonic`（CLOCK_MONOTONIC：Linux=1/Darwin=6）+ `__rlyeh_clock_realtime`（SystemTime 用）。关键教训：`time::system` 子模块双段路径、`module system;` 声明须置末尾。`from_secs_f64` 依赖 U6 恢复。

## 验证

`duration_full.{rlyeh,out}` 9 输出 + 全量套件 99+ 用例全绿 + cargo test。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
