# SH-P2-5 分阶段自举 + 差分测试基础设施

> **级别**：P2（集成建设） · **风险**：🔴 高 · **状态**：⏳ 规划中 · **归属**：0.2.0-K
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.11

## 目标
建立「Rust 引导器编译 Rlyeh 版组件 → 三阶段 bootstrap 校验 → 差分测试 harness → 快照测试」的自举与对拍基础设施，作为 0.3.0 自举的工程骨架（0.2.0 内先落地 harness 与 PoC 级对拍）。

## 技术细节
- Rust 版编译器始终作为 bootstrap 编译器，先编译 Rlyeh 写的 `lexer`/`parser`/…/`typecheck`/`codegen`/`driver`。
- 差分 harness：同一 `.rl` 程序分别经 Rust 参考编译器与 Rlyeh 编译器编译，比较 IR 文本 / 可执行行为 / 诊断输出。
- 快照测试：AST/HIR/MIR/LIR 关键节点序列化快照，回归比对。
- 三阶段 bootstrap：① Rust 编译 Rlyeh 编译器；② Rlyeh 编译器编译自身得 `rlyeh₂`；③ `rlyeh₂` 编译自身得 `rlyeh₃`，`rlyeh₂` 与 `rlyeh₃` 字节/行为一致（经典自举校验）。

## 风险分解（→ 中/低危）
- **K-M1（中）** 引导器（Rust driver）编译 Rlyeh 版**单组件**（如 `lexer`），经差分 harness 对拍。
- **K-M2（中）** 扩展为**双组件**（lexer + parser），验证组件间接口在 Rlyeh 侧一致。
- **K-M3（高→中）** 三阶段 bootstrap 校验：`rlyeh₂` ≡ `rlyeh₃`（字节/行为一致）。
- **K-M4（中）** 差分测试 harness 完善：IR 文本 / 行为 / 诊断三维比对，覆盖编译器多阶段产物。
- **K-M5（低）** 快照测试：AST/HIR/MIR/LIR 序列化快照回归比对。
- **L1（低）** 0.2.0 内先落地 harness 与 PoC 级对拍（lexer/parser），逐步扩展至全编译器。

## 受影响组件
`rlyeh-driver`（bootstrap 入口）、全部编译器 crate（被测对象）、`tests/`（自举/差分用例）。

## 验证
- 三阶段 bootstrap 产出 `rlyeh₂` 与 `rlyeh₃` 字节/行为一致。
- 差分 harness 对拍 Rust 参考与 Rlyeh 编译器产物（IR/行为/诊断）一致。

## 状态
⏳ 规划中（0.2.0 必须项，阶段 K）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新增（评审发现：原评估遗漏分阶段自举 + 差分测试基础设施） |