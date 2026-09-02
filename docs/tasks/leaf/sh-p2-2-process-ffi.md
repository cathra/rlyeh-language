# SH-P2-2 进程调用 / 外部工具链 FFI

> **级别**：P2（可行但需重写 / 外部依赖） · **状态**：🟢 核心落地（D1/D2 + D3 印证） · **归属**：0.2.0-D
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P2-8 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.4

## 目标
提供 `system`/`exec` 外部命令调用与输出捕获能力，使 Rlyeh 侧 `assemble()` 能调 `clang`/`rust-lld`/`wasm-ld` 完成汇编链接，解锁**后端 assemble 自举**（保留"生成 LLVM IR 文本 + 调 clang"策略，见评估报告 §6 路径 A）。

## 技术细节
- 当前 Rlyeh 0.1.0 低阶 `extern "C"` 已实现，但**进程调用**能力需确认/补齐。
- 受影响 Rust 代码（事实依据）：`rlyeh-driver/src/lib.rs:432` `assemble()` 写 `main.ll` 后 `Command::new(&clang).arg("-O3")...`（第 469–494 行）；wasm 走 `assemble_wasm`，musl 跨平台走 `assemble_cross_elf`（`rust-lld`）。
- 子任务（对应 0.2.0-D）：D1 `system`/`exec` 外部命令 / D2 捕获 stdout/stderr / D3 对接 driver assemble。

## 实现纪要（2026-09-02，核心落地）
- 新增 `crates/rlyeh-std/rlyeh/process/module.rl`（`module process;`，core.rl 注册 + 顶层 import）：
  - **D1 `system(cmd)`**：经既有 libc 原语 `popen`+`pclose` 实现（不新增 extern），返回归一化退出码 `(waitpid_status >> 8) & 0xFF`。
  - **D2 `exec(cmd)` → `Output{ status, stdout }`**：`popen("r")` + `fread` 块读循环 + `pclose`，捕获标准输出；`output(cmd)` 便捷返回 stdout；`exec_combined(cmd)` 经 `2>&1` 合并 stderr。
  - 复用 `io::c_str` / `core` 顶层 `popen`/`pclose`/`fread`，退出码位运算 `>>`/`&` 由 binary.rs 支持。
- **D3 印证**：`process::exec("clang --version")` 返回 `status == 0`，证明 Rlyeh 侧已能驱动外部工具链（clang 随 toolchain 安装）；Rlyeh 版 driver `assemble()` 完整自举留待 0.3.0。

## 受影响组件
`rlyeh-std/rlyeh/process/module.rl`（新增）、`rlyeh-std/rlyeh/core.rl`（`module process;` + import）、`rlyeh-driver`（`assemble` 系列，Rust 侧未改动）。

## 验证
- 新增 run-pass：`tests/run-pass/process_exec.rl`（含 `.out`）：`exec("echo hello")`→status 0 / stdout "hello\n"；`output("echo rlyeh")`→"rlyeh\n"；`system("true")`→0 / `system("false")`→1；`exec("clang --version")`→status 0。
- 全量 `rlyeh test tests`：**253 用例全过**。

## 状态
🟢 D1/D2 落地 + D3 工具链 FFI 印证；Rlyeh 版 driver assemble 完整自举待 0.3.0。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P2-8 拆出为叶子 |
| 2026-09-02 | 进程调用 / 外部工具链 FFI 核心落地：process 模块（system/exec/output/exec_combined）+ clang FFI 印证；全量回归 253/253 |
