# SH-P2-7 前端自举 PoC（driver 自举）

> **级别**：P2（集成建设） · **风险**：🔴 高 · **状态**：⏳ 规划中 · **归属**：0.2.0-M
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.13

## 目标
用 Rlyeh 重写编译器前端 `lexer` + `parser` + `ast` + `macro`（依赖 A/B/C/E 落地），经 Rust `rlyeh-driver` 编译通过，并与 Rust 版前端**对拍**，证明「前端逻辑主体已是 Rlyeh 源码、可被自身工具链编译」（dogfood 交付物）。

## 技术细节
- 依赖：A 泛型 trait/impl（前端内部 trait）、B 嵌套模块（前端 crate 组织）、C derive 宏（AST 样板）、E `unsafe`（底层字节操作）均已落地。
- 交付物：Rlyeh 版前端源码 + 对拍测试 `tests/self-host-*`（同 `.rl` 输入，token/AST 一致）。
- 复用 K 的差分 harness 做逐步对拍。

## 风险分解（→ 中/低危）
- **M-M1（中）** 用 Rlyeh 重写 `lexer`（依赖 N 元组值 / O `if let` / P `match` 守卫），经 Rust driver 编译 + 差分对拍 token 一致。
- **M-M2（中）** 用 Rlyeh 重写 `parser`（依赖 N/O/P + 递归下降），对拍 AST 一致。
- **M-M3（中）** 用 Rlyeh 重写 `ast` + `macro`（依赖 C derive / I 内部可变性），对拍 AST 节点构造一致。
- **M-M4（中）** 串联 M1–M3，经 Rust `rlyeh-driver` 编译通过，并与 Rust 版前端**对拍**（同 `.rl` 输入，token/AST 一致）。
- **L1（低）** 验收即「前端逻辑主体已是 Rlyeh 源码、可被自身工具链编译」；接入 K 的三阶段 bootstrap 做 dogfood。

## 受影响组件
`rlyeh-lexer`、`rlyeh-parser`、`rlyeh-ast`、`rlyeh-macro`（待建）、`rlyeh-driver`（编译入口）、`tests/`。

## 验证
- Rlyeh 版 lexer/parser/ast/macro 经 Rust driver 编译通过。
- 对拍测试 `tests/self-host-*`：同 `.rl` 输入，Rlyeh 版与 Rust 版 token/AST 一致。

## 状态
⏳ 规划中（0.2.0 必须项，阶段 M；交付物 dogfood）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新增（前端自举 PoC，复用 K 的差分 harness 对拍） |