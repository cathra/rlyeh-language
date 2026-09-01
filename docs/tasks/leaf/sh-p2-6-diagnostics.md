# SH-P2-6 诊断信息质量对齐

> **级别**：P2（集成建设） · **状态**：⏳ 规划中 · **归属**：0.2.0-L
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md)（诊断质量未在缺口中单列）

## 目标
Rlyeh 版编译器复刻 Rust 参考实现的 **span 级诊断质量**（文件名/行/列/长度 + 结构化错误码 + 建议），保证自举后开发者体验不退化（评审发现原评估未单列此能力）。

## 技术细节
- 当前 Rlyeh 诊断已有 Span 概念（lexer/parser 阶段），但 `typecheck`/`borrowck`/`regionck` 的错误信息丰富度需对齐 Rust 版。
- 需建设：
  - **L1** span 级错误定位（多文件/宏展开后映射）。
  - **L2** 结构化诊断（稳定错误码 + 修复建议 + 相关 span 标注）。
  - **L3** 与 Rust 参考实现诊断文本**对拍**（同一错误输入，诊断结构等价）。

## 受影响组件
`rlyeh-typecheck` / `rlyeh-borrowck` / `rlyeh-regionck` / `rlyeh-parser` 诊断输出、`rlyeh-check`（lint）。

## 验证
- 对拍：错误用例经 Rust/Rlyeh 编译器产出诊断结构（错误码 + span）一致。

## 状态
⏳ 规划中（0.2.0 必须项，阶段 L）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新增（评审发现：诊断质量对齐未在原评估缺口中单列） |
