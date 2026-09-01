# SH-P2-2 进程调用 / 外部工具链 FFI

> **级别**：P2（可行但需重写 / 外部依赖） · **状态**：⏳ 规划中 · **归属**：0.2.0-D
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P2-8 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.4

## 目标
提供 `system`/`exec` 外部命令调用与输出捕获能力，使 Rlyeh 侧 `assemble()` 能调 `clang`/`rust-lld`/`wasm-ld` 完成汇编链接，解锁**后端 assemble 自举**（保留"生成 LLVM IR 文本 + 调 clang"策略，见评估报告 §6 路径 A）。

## 技术细节
- 当前 Rlyeh 0.1.0 低阶 `extern "C"` 已实现，但**进程调用**能力需确认/补齐。
- 受影响 Rust 代码（事实依据）：`rlyeh-driver/src/lib.rs:432` `assemble()` 写 `main.ll` 后 `Command::new(&clang).arg("-O3")...`（第 469–494 行）；wasm 走 `assemble_wasm`，musl 跨平台走 `assemble_cross_elf`（`rust-lld`）。
- 子任务（对应 0.2.0-D）：D1 `system`/`exec` 外部命令 / D2 捕获 stdout/stderr / D3 对接 driver assemble。

## 受影响组件
`rlyeh-driver`（`assemble` / `assemble_wasm` / `assemble_cross_elf`）、`rlyeh-codegen`（IR 文本发射，已由字符串逻辑承载）。

## 验证
- 单元：Rlyeh 侧 `exec("echo", ["hi"])` 返回退出码与捕获输出。
- 集成：Rlyeh 侧驱动对一份 `.rl` 生成可执行文件并运行通过（对拍 Rust driver）。

## 状态
⏳ 规划中（0.2.0 必须项，阶段 D）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P2-8 拆出为叶子 |
