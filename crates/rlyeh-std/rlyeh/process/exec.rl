// process/exec.rl：外部命令执行自由函数 —— 2026-09-18 由 process/module.rl 拆出。
//
// 归属子模块 `process::exec`。对外函数名由 process/module.rl 的 `pub import` 登记
// 重导出别名保持（`process::exec` / `process::system` / `process::output` /
// `process::exec_combined`）。
//
// 复用既有 libc 原语（core 的 externs 单元）：`popen` / `pclose` / `fread`，
// 不引入新的 extern 符号；退出码经 `(waitpid_status >> 8) & 0xFF` 归一化。
//
// 已知限制（MVP）：依赖 /bin/sh；WASI / Windows 不支持。

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
fn exec(command: String) -> process::output::Output {
    let f = popen(c_str(command), String::from("r"));
    if f == 0 {
        return process::output::Output { status: -1, stdout: String::from("") };
    }
    let captured = read_all(f);
    let s = pclose(f);
    process::output::Output { status: (s >> 8) & 0xFF, stdout: captured }
}

// D2 便捷：仅返回捕获的标准输出文本。
fn output(command: String) -> String {
    exec(command).stdout
}

// D2 合并：捕获 stdout + stderr（2>&1），用于编译器 / 工具链诊断输出。
fn exec_combined(command: String) -> process::output::Output {
    let f = popen(c_str(command + String::from(" 2>&1")), String::from("r"));
    if f == 0 {
        return process::output::Output { status: -1, stdout: String::from("") };
    }
    let captured = read_all(f);
    let s = pclose(f);
    process::output::Output { status: (s >> 8) & 0xFF, stdout: captured }
}
