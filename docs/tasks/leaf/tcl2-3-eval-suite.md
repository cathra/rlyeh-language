# TCL2-3 评估资质测试套件

> **级别**：P1 · **风险**：🔴 高 · **状态**：🟡 待办 · **归属**：0.2.0-AB（部分延续 0.3.0）
> **索引**：[`../rfc/tcl2-certification.md`](../rfc/tcl2-certification.md) §5.3 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.28

## 目标
- 差分/对拍：扩展 M-M1 harness 到 codegen（Rlyeh 输出 vs 参考/oracle），覆盖溢出/边界/确定性。
- 规范符合性套件：以 `semantics.md`/`grammar.md` 为基准的可执行规范测试。
- 回归基线：固化 253/255/330 套件为资质回归（每次变更全绿）。
- 模糊测试：AFL/libFuzzer + Sanitizer 对编译器/运行时做无 UB 模糊。
- 目标硬件测试：生成代码在 MCU/ECU 行为验证。

## 现状
仅 lexer 对拍 + 回归，缺规范符合性/codegen 差分/模糊。

## 验证
§5.3 套件全绿（差分对拍、规范符合性、模糊无 UB、目标硬件测试）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | RFC 评审通过，并入 0.2.0-AB，建叶子 |
