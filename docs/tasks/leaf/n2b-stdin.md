# N2b stdin 增强

> **所属阶段**：阶段 N
> **状态**：✅ 已完成
> **依赖**：M
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

stdin 读行增强。

## 背景

阶段 阶段 N 子任务，详见 阶段详情文档 [`stages/N.md`](../../stages/N.md)。

## 技术细节

`read_to_string`/`lines`（迭代行读取，复用 J1 循环形态）。测试套件无 stdin 注入，`stdin_enhance.rl` 覆盖空 stdin 路径；多行/跨 256B 缓冲逻辑经 `rlyeh build` 产物 + shell 管道手动验证。

## 验证

`stdin_enhance.rl` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
