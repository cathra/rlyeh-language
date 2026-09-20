# TCL2-2 错误检测置信提升

> **级别**：P1 · **风险**：🔴 高 · **状态**：🟡 待办（边界硬化已落地） · **归属**：0.2.0-AB
> **索引**：[`../rfc/tcl2-certification.md`](../rfc/tcl2-certification.md) §5.2 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.28

## 目标
- UB 清单 `docs/ub-inventory.md`：枚举 UB 来源并标注防护（静态拒绝/运行期 trap/文档豁免）。
- 整数溢出语义：明确默认策略（认证/调试 panic，发布可配 wrap 且文档化），消除 std 内部未检查路径。
- **边界检查全覆**：Vec/`String`/`&str`/切片直接 `arr[i]` 越界已注入 `call @abort()` 确定性中止（已落地，eh-8 M4，2026-09-20）；区间/切片检查与裸指针解引用约束待补。
- 未初始化读防护：栈槽默认零初始化或借用检查兜底。
- MISRA 类 lint（升级 `rlyeh-check`）：危险隐式转换、指针别名、动态内存、函数纯度、禁止丢弃返回值。
- 确定性：safe 子集禁非确定性（`clock`/`random`/调度敏感）；落实 comptime hermetic 约束。

## 现状
溢出未定义、UB 清单缺、`rlyeh-check` 仅基础；边界硬化已落地。

## 验证
UB 清单闭环；溢出/边界在认证模式 `abort`；`rlyeh-check` 认证级 lint 全绿；unsafe 审计零未记录项。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | RFC 评审通过，并入 0.2.0-AB，建叶子；边界硬化标注已落地 |
