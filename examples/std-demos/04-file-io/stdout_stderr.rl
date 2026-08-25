// N2a：stdout/stderr 对象化（write/writeln/flush）
fn main() {
    // stdout 写入
    let o1 = stdout();
    println(o1.write(String::from("hello")).unwrap_or(-1));       // 5
    let o2 = stdout();
    println(o2.writeln(String::from("hi")).unwrap_or(-1));        // 3（"hi\n"）
    let o3 = stdout();
    println(o3.flush().unwrap_or(0));                             // 1
    // stderr 写入（内容经 fd 2 输出，不参与 stdout 断言）
    let e1 = stderr();
    println(e1.write(String::from("err")).unwrap_or(-1));         // 3
    let e2 = stderr();
    println(e2.writeln(String::from("e2")).unwrap_or(-1));        // 3（"e2\n"）
    let e3 = stderr();
    println(e3.flush().unwrap_or(0));                             // 1
}
