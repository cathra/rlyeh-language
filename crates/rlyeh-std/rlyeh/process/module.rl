// process/module.rl：进程调用 / 外部工具链 FFI 模块——只做「子模块声明 + 暴露内容导出」。
//
// 组成：
//   process/output.rl   struct Output                                        → 子模块 process::output
//   process/exec.rl     read_all / system / exec / output / exec_combined    → 子模块 process::exec
//
// `pub import` 登记 `process::Xxx → process::<mod>::Xxx` 重导出别名（typecheck
// `register_use` 的 `is_pub` 分支），使既有引用（用户代码 `process::exec(...)` /
// `process::Output`、std 内部）无需改动。裸名导出见标准库根 module.rl。
//
// 背景（D，SH-P2-2）：使 Rlyeh 侧可驱动 clang / rust-lld / wasm-ld 等工具链，
// 解锁后端 assemble 自举（D3）。复用既有 libc 原语（core 的 externs 单元）。
//
// module 声明顺序 = 收集期注册顺序：output 须先于 exec（exec 签名引用
// process::output::Output）。

module output;
pub import output::Output;
module exec;
pub import exec::system;
pub import exec::exec;
pub import exec::output;
pub import exec::exec_combined;
