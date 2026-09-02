// ===== 进程调用 / 外部工具链 FFI 模块（D，SH-P2-2，0.2.0-D） =====
//
// 提供对外部命令（/bin/sh -c）的调用与输出捕获，使 Rlyeh 侧可驱动 clang /
// rust-lld / wasm-ld 等工具链，解锁后端 assemble 自举（D3）。
//
// 复用既有 libc 原语（core.rl 顶层 extern）：`popen` / `pclose` / `fread`，
// 不引入新的 extern 符号；退出码经 `(waitpid_status >> 8) & 0xFF` 归一化。
//
// 已知限制（MVP）：
//  - `system` 经 popen+pclose 实现，不继承父进程 stdout（仅取退出码）；对产生
//    大量输出的命令可能阻塞（子进程管道写满），`exec`/`exec_combined` 已读取。
//  - 依赖 /bin/sh；WASI / Windows 不支持。

// 外部命令执行结果。
struct Output {
    status: i32,    // 归一化退出码（0 = 成功）
    stdout: String, // 捕获的标准输出（exec_combined 含 stderr）
}

// 从已打开的 FILE* 句柄读取全部内容（复用 C fread 块读）。
fn read_all(f: i64) -> String {
    let mut buf = String::new();
    loop {
        let mut tmp = String::with_capacity(256);
        let n = fread(tmp, 1, 256, f);
        if n == 0 {
            break;   // EOF
        }
        let mut i = 0;
        while i < n {
            buf.push_byte(tmp.data[i]);
            i = i + 1;
        }
    }
    buf
}

// D1：以 /bin/sh -c 执行命令，返回退出码（0 = 成功）。
fn system(command: String) -> i32 {
    let f = popen(c_str(command), String::from("r"));
    if f == 0 {
        return -1;
    }
    let s = pclose(f);
    (s >> 8) & 0xFF
}

// D2：执行命令并捕获标准输出，返回 Output{ status, stdout }。
fn exec(command: String) -> process::Output {
    let f = popen(c_str(command), String::from("r"));
    if f == 0 {
        return process::Output { status: -1, stdout: String::from("") };
    }
    let captured = process::read_all(f);
    let s = pclose(f);
    process::Output { status: (s >> 8) & 0xFF, stdout: captured }
}

// D2 便捷：仅返回捕获的标准输出文本。
fn output(command: String) -> String {
    process::exec(command).stdout
}

// D2 合并：捕获 stdout + stderr（2>&1），用于编译器 / 工具链诊断输出。
fn exec_combined(command: String) -> process::Output {
    let f = popen(c_str(command + String::from(" 2>&1")), String::from("r"));
    if f == 0 {
        return process::Output { status: -1, stdout: String::from("") };
    }
    let captured = process::read_all(f);
    let s = pclose(f);
    process::Output { status: (s >> 8) & 0xFF, stdout: captured }
}
