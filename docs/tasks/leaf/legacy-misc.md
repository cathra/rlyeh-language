# 遗留修复（rlyeh new / 动态切片 / String::from）

> **所属阶段**：阶段 A–F
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`rlyeh new` + Vec/数组动态切片 + `String::from` 字面量变量。

## 背景

阶段 阶段 A–F 子任务，详见 阶段详情文档 [`stages/L.md`](../../stages/L.md)。

## 技术细节

`rlyeh new <name> [--lib]`（委托 dagon `cmd_new`，Rlyeh.toml + src/main.rl|lib.rl）；`Vec<T>` 动态切片 `v[lo..<hi]`/`v[lo...hi]`/`v[lo<..hi]`（std 泛型方法 `Vec::slice` + check_slice 实例化，越界 clamp）；数组动态切片（typecheck 展开 Vec 拷贝循环 + push 实例化）；`String::from(s)` 支持绑定字面量变量（`TypeContext.local_inits` 追踪）。

## 验证

`dynamic_slice_test.rs` 7 用例 + 全量回归 111 套件全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
