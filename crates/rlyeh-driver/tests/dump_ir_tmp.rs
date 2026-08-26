// 临时：dump main 中 _t30 相关 + printf 参数
use std::path::PathBuf;
use rlyeh_driver::compile_file_to_llvm;

#[test]
fn dump_ir() {
    let entry = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run-pass/v2_probe.rl");
    let ir = compile_file_to_llvm(&entry).unwrap();
    let mut in_main = false;
    let mut buf: Vec<String> = Vec::new();
    for line in ir.lines() {
        if line.starts_with("define") && line.contains("main") {
            in_main = true;
        }
        if in_main {
            if line.trim_start().starts_with("ret") && line.contains("i32 0") {
                break;
            }
            buf.push(line.to_string());
        }
    }
    for (i, l) in buf.iter().enumerate() {
        if l.contains("_t30") || l.contains("@printf") {
            for j in (i.saturating_sub(2))..(i + 2).min(buf.len()) {
                println!("{}", buf[j]);
            }
            println!("  ---");
        }
    }
}
