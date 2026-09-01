# SH-P2-1 外部 crate 等价（pubgrub / clap / serde / toml / tar / flate2 / libc）

> **级别**：P2（可行但需重写 / 外部依赖） · **状态**：⏳ 规划中 · **归属**：长期跟踪
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P2-7

## 目标
为 `dagon` 包管理器与 `rlyeh-driver` 依赖的外部 Rust crate 提供 Rlyeh 侧等价方案（重写算法或经 FFI 调用 C 库），解除自举对 Rust 生态的硬依赖。

## 技术细节
- 受影响依赖（事实依据，来自 `crates/` / `dagon/` 核查）：
  - `dagon/`：`pubgrub` 0.4（版本求解）、`tar`/`flate2`（压缩归档）、`clap`/`serde`/`toml`（CLI/序列化）、`libc`（沙箱系统调用）、`sha2`/`anyhow`。
  - `rlyeh-driver` / `rlyeh-std`：`clap`/`serde`/`toml`、`libc`。
- 可移植部分：`pubgrub` 版本求解算法本身与语言无关，可重写；`toml`/`serde` 的 Rlyeh 侧等价已由 std 的 json/toml API 部分覆盖（Q/X 阶段）。
- 需 FFI 部分：`tar`/`flate2`（压缩）、`libc`（沙箱系统调用）需 Rlyeh FFI 调 C 库，或长期保留 Rust 实现。

## 受影响组件
`dagon`（manifest/registry/resolve/sandbox）、`rlyeh-driver`。

## 验证
- 单元：`dagon resolve` 用 Rlyeh 侧版本求解产出与 `pubgrub` 一致的结果。
- 集成：`rlyeh publish` 经 Rlyeh 侧实现完成打包（或经 FFI 调 Rust 实现）。

## 状态
⏳ 规划中（长期跟踪，不纳入 0.2.0 必须项）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P2-7 拆出为叶子 |
