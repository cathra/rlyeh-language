# TCL2-1 TD 分析与影响面收敛

> **级别**：P1 · **风险**：🔴 高 · **状态**：🟡 待办 · **归属**：0.2.0-AB
> **索引**：[`../rfc/tcl2-certification.md`](../rfc/tcl2-certification.md) §5.1 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.28

## 目标
产出 `docs/tool-impact-analysis.md`：枚举各 crate 输出，标注 TI/TnI 与危害场景（codegen 误编译、typecheck 漏检、区域/GC 运行时错误）；收敛 TI 面：审计 0.2.0-E `unsafe` 契约、最小化注入内建信任边界、对 codegen 关键路径独立评审。

## 现状
无 TI/TnI 清单，TD 分析缺失（资质起点）。

## 验证
TD 分析报告通过评审；unsafe 审计零未记录项。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | RFC 评审通过，并入 0.2.0-AB，建叶子 |
