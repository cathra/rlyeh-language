// process/output.rl：`Output`（外部命令执行结果）——2026-09-18 由 process/module.rl 拆出。
//
// 归属子模块 `process::output`。对外 `process::Output` 由 process/module.rl 的
// `pub import process::output::Output;` 登记重导出别名保持，既有引用（用户代码与
// std 内部）无需改动。

// 外部命令执行结果。
struct Output {
    status: i32,    // 归一化退出码（0 = 成功）
    stdout: String, // 捕获的标准输出（exec_combined 含 stderr）
}
