// D（SH-P2-2，0.2.0-D）：进程调用 / 外部工具链 FFI。
// D1 system + D2 exec/output 退出码与 stdout 捕获。
fn main() {
    let o = process::exec(String::from("echo hello"));
    println(o.status);     // 0（echo 成功）
    print!("{}", o.stdout);      // hello\n（print! 不再追加换行）

    let s = process::output(String::from("echo rlyeh"));
    print!("{}", s);             // rlyeh\n

    println(process::system(String::from("true")));    // 0
    println(process::system(String::from("false")));   // 1

    // D3 印证：外部工具链 FFI（clang 随 toolchain 安装），仅取退出码。
    let c = process::exec(String::from("clang --version"));
    println(c.status);     // 0
}
