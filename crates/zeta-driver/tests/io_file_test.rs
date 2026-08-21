//! 测试标准库 io 模块（阶段 B2）与 net 基础绑定（阶段 B3）：
//! libc 文件 IO —— `read_file` / `write_file` / `append_file` / `c_str` / `read_line`；
//! 主机名查询 —— `hostname()`。
//!
//! 实现基于通用 FFI（`extern fn`，阶段 A4）：String 的 LIR 表示即 data 指针（i8*），
//! 可直接作为 C 字符串/缓冲传入 libc；路径参数经 `c_str` 附加 NUL 终止。
//!
//! 需要系统 clang（与 std_test.rs / ffi_extern_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时目录，避免并行测试互相覆盖。
fn temp_dir() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-io-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_dir();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("io 模块测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 写文件 + 读文件往返：内容与字节数一致。
#[test]
fn file_write_read_roundtrip() {
    let dir = temp_dir();
    let path = dir.join("a.txt");
    let p = path.to_str().unwrap();
    let out = run(&format!(
        r#"
fn main() {{
    let path = String::from("{p}");
    let w = write_file(path, String::from("Hello Zeta!"));
    println(w);                  // 11 字节
    let content = read_file(path);
    println(content);            // 内容一致
}}
"#,
    ));
    assert_eq!(out, "11\nHello Zeta!\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 覆盖写（O_TRUNC）语义 + 读取。
#[test]
fn file_overwrite_truncates() {
    let dir = temp_dir();
    let path = dir.join("b.txt");
    let p = path.to_str().unwrap();
    let out = run(&format!(
        r#"
fn main() {{
    let path = String::from("{p}");
    let _ = write_file(path, String::from("first"));
    let _ = write_file(path, String::from("second"));
    let content = read_file(path);
    println(content);            // second（截断覆盖）
}}
"#,
    ));
    assert_eq!(out, "second\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 追加写（O_APPEND）语义 + 读取。
#[test]
fn file_append() {
    let dir = temp_dir();
    let path = dir.join("c.txt");
    let p = path.to_str().unwrap();
    let out = run(&format!(
        r#"
fn main() {{
    let path = String::from("{p}");
    let _ = write_file(path, String::from("AB"));
    let a = append_file(path, String::from("CD"));
    println(a);                  // 2 字节
    let content = read_file(path);
    println(content);            // ABCD（追加）
}}
"#,
    ));
    assert_eq!(out, "2\nABCD\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// UTF-8 多字节内容往返（中文标点，字节数与内容一致）。
#[test]
fn file_utf8_content() {
    let dir = temp_dir();
    let path = dir.join("utf8.txt");
    let p = path.to_str().unwrap();
    // "你好，Zeta！" UTF-8 编码 = 3*3 + 1*4 + 3 = 16 字节
    let out = run(&format!(
        r#"
fn main() {{
    let path = String::from("{p}");
    let text = String::from("你好，Zeta！");
    let w = write_file(path, text);
    println(w);                  // 16 字节
    let content = read_file(path);
    println(content);            // 内容一致
}}
"#,
    ));
    assert_eq!(out, "16\n你好，Zeta！\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 大文件往返（4096 字节）：lseek 取 size + 单次 read 读满。
#[test]
fn file_large_content() {
    let dir = temp_dir();
    let path = dir.join("large.txt");
    let p = path.to_str().unwrap();
    let out = run(&format!(
        r#"
fn main() {{
    let path = String::from("{p}");
    let mut big = String::new();
    let mut i = 0;
    while i < 4096 {{
        big.push_byte(120);      // 'x'
        i = i + 1;
    }}
    let w = write_file(path, big);
    println(w);                  // 4096
    let content = read_file(path);
    println(content.len);        // 4096
    println(content.data[0] == 120);    // true
    println(content.data[4095] == 120); // true
}}
"#,
    ));
    assert_eq!(out, "4096\n4096\ntrue\ntrue\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 文件不存在：read_file 返回空串（打开失败路径）。
#[test]
fn file_missing_returns_empty() {
    let dir = temp_dir();
    let missing = dir.join("no-such-file.txt");
    let p = missing.to_str().unwrap();
    let out = run(&format!(
        r#"
fn main() {{
    let content = read_file(String::from("{p}"));
    println(content.len);        // 0（打开失败返回空串）
}}
"#,
    ));
    assert_eq!(out, "0\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 空文件：lseek size = 0，读取返回空串。
#[test]
fn file_empty_read() {
    let dir = temp_dir();
    let path = dir.join("empty.txt");
    std::fs::write(&path, "").expect("写入空文件失败");
    let p = path.to_str().unwrap();
    let out = run(&format!(
        r#"
fn main() {{
    let content = read_file(String::from("{p}"));
    println(content.len);        // 0
    println(content == String::from(""));  // true
}}
"#,
    ));
    assert_eq!(out, "0\ntrue\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// net 基础绑定：hostname() 返回非空主机名（长度 < 256）。
#[test]
fn net_hostname() {
    let out = run(
        r#"
fn main() {
    let h = hostname();
    println(h.len > 0);      // true（gethostname 成功）
    println(h.len < 256);    // true
}
"#,
    );
    assert_eq!(out, "true\ntrue\n");
}

/// 组合链路：写文件 → 读回 → 拼接字符串 → 再次写出 → 读回验证。
#[test]
fn file_compose_chain() {
    let dir = temp_dir();
    let path = dir.join("chain.txt");
    let p = path.to_str().unwrap();
    let out = run(&format!(
        r#"
fn main() {{
    let path = String::from("{p}");
    let _ = write_file(path, String::from("zeta"));
    let content = read_file(path);
    let greeting = content + String::from("-lang");
    let _ = write_file(path, greeting);
    let again = read_file(path);
    println(again);            // zeta-lang
    println(again.len);        // 9
}}
"#,
    ));
    assert_eq!(out, "zeta-lang\n9\n");
    let _ = std::fs::remove_dir_all(&dir);
}
