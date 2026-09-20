# Comptime M3 生成式 comptime

> **级别**：P1 · **风险**：🔴 高 · **状态**：🟡 待办 · **归属**：0.3.0（自举执行阶段）
> **索引**：[`../rfc/comptime.md`](../rfc/comptime.md) §5 M3 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.26

## 目标
comptime 构造新类型/函数并经正常 codegen 编译（非字符串展开）；`@typeInfo` 反射；生成式循环（枚举/序列化代码生成）。0.3.0 用 Rlyeh 重写编译器前端/codegen 的关键赋能。

## 现状
无生成式元编程；derive 宏为文本级展开。

## 验证
corpus-comptime-3：生成式枚举/`@typeInfo` 反射对拍。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | RFC 评审通过，标注 0.3.0，建叶子 |
