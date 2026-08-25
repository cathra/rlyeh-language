// N4：eprintln!/eprint! 宏与 eprint/eprintln 内建（输出到 stderr，.out 仅断言 stdout）
fn main() {
    println(0);                   // stdout: 0
    eprintln("err-1");            // stderr: err-1\n
    eprint("err-2");              // stderr: err-2（无换行）
    eprintln("!");                // stderr: err-2!\n
    eprintln!("v={}", 42);        // stderr: v=42\n（占位符格式化）
    eprintln();                   // stderr: 空行
    let s = String::from("var-msg");
    eprintln(s);                  // stderr: var-msg\n（String 变量）
    eprintln(7);                  // stderr: 7\n（非 String 参数）
    eprint("direct");             // 内建直接调用 → stderr
    eprintln("");                 // stderr: 换行
    println(1);                   // stdout: 1
}
