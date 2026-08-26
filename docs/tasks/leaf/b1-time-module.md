# B1 `Duration`/`Instant` 时间模块

> **所属阶段**：阶段 B
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

标准库时间模块：`Duration`/`Instant` 单位换算与时钟。

## 背景

阶段 阶段 B 子任务，详见 阶段详情文档 [`stages/B.md`](../../stages/B.md)。

## 技术细节

core.rl 末尾；底层时钟 libc `clock()` extern（POSIX CLOCKS_PER_SEC=1e6 微秒）；`Duration { micros: i64 }` + `secs()/millis()/micros()/nanos()`；`Instant { start: i64 }` + `now()`（关联函数）/`elapsed()` 返回 `Duration`。

## 验证

`time_test.rs` 3 用例：单位换算/字段访问+算术/now-elapsed 时钟差。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
