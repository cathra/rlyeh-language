//! 测试标准库 io 模块（阶段 B2 / M3a）与 net 基础绑定（阶段 B3 / M3b）：
//! libc 文件 IO —— `read_file` / `write_file` / `append_file` / `c_str` / `read_line`；
//! 主机名查询 —— `hostname()`。
//!
//! M3a（2026-08）后 io 自由函数 Result 化：`read_file`/`write_file`/`append_file`
//! 返回 `Result<T, IoError>`，失败不再用"空串 / -1"哨兵值；本文件用例均按
//! `match { Ok(..) / Err(..) }` 解包验证。
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
    match write_file(path, String::from("Hello Zeta!")) {{
        Ok(w) => println(w),             // 11 字节
        Err(e) => println(-1),
    }}
    match read_file(path) {{
        Ok(v) => println(v),             // 内容一致
        Err(e) => println(-1),
    }}
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
    match read_file(path) {{
        Ok(v) => println(v),             // second（截断覆盖）
        Err(e) => println(-1),
    }}
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
    match append_file(path, String::from("CD")) {{
        Ok(a) => println(a),             // 2 字节
        Err(e) => println(-1),
    }}
    match read_file(path) {{
        Ok(v) => println(v),             // ABCD（追加）
        Err(e) => println(-1),
    }}
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
    match write_file(path, text) {{
        Ok(w) => println(w),             // 16 字节
        Err(e) => println(-1),
    }}
    match read_file(path) {{
        Ok(v) => println(v),             // 内容一致
        Err(e) => println(-1),
    }}
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
    match write_file(path, big) {{
        Ok(w) => println(w),             // 4096
        Err(e) => println(-1),
    }}
    match read_file(path) {{
        Ok(content) => {{
            println(content.len);        // 4096
            println(content.data[0] == 120);    // true
            println(content.data[4095] == 120); // true
        }}
        Err(e) => println(-1),
    }}
}}
"#,
    ));
    assert_eq!(out, "4096\n4096\ntrue\ntrue\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 文件不存在：read_file 返回 Err(NotFound)（M3a Result 化，不再是空串）。
#[test]
fn file_missing_returns_empty() {
    let dir = temp_dir();
    let missing = dir.join("no-such-file.txt");
    let p = missing.to_str().unwrap();
    let out = run(&format!(
        r#"
fn main() {{
    let content = read_file(String::from("{p}"));
    println(content.is_err());           // 1（打开失败 → Err(NotFound)）
}}
"#,
    ));
    assert_eq!(out, "1\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 空文件：lseek size = 0，读取成功返回空串。
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
    match content {{
        Ok(v) => {{
            println(v.len);              // 0
            println(v == String::from(""));  // true
        }}
        Err(e) => println(-1),
    }}
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
    match hostname() {
        Ok(h) => {
            println(h.len > 0);      // true（gethostname 成功）
            println(h.len < 256);    // true
        }
        Err(e) => println(-1),
    }
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
    match read_file(path) {{
        Ok(v) => {{
            let greeting = v + String::from("-lang");
            let _ = write_file(path, greeting);
            match read_file(path) {{
                Ok(again) => {{
                    println(again);            // zeta-lang
                    println(again.len);        // 9
                }}
                Err(e) => println(-1),
            }}
        }}
        Err(e) => println(-1),
    }}
}}
"#,
    ));
    assert_eq!(out, "zeta-lang\n9\n");
    let _ = std::fs::remove_dir_all(&dir);
}
