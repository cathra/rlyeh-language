//! 平台内建 `__zeta_target_os` 与 `sockaddr_in4` 双布局测试（E1 平台假设消除）。
//!
//! 覆盖：
//! - `target_os_code` 目标 triple → OS 码映射（macOS / Linux / Windows / wasm）
//! - 平台内建端到端接线：`extern fn __zeta_target_os()` 调用返回主机 OS 码
//!   （验证 driver 注入 `define internal` + codegen 跳过 `__zeta_` declare）
//! - `sockaddr_in4_with_layout` 双布局：macOS（sin_len 头）/ Linux（无 sin_len）
//! - `sockaddr_in4` 平台自适应：主机（macOS）下走 sin_len 布局
//!
//! 需要系统 clang（与 net_socket_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-plat-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("平台内建测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 本机 OS 码（与 `zeta_driver::target_os_code` 同一定义）。
fn host_os_code() -> i32 {
    match std::env::consts::OS {
        "linux" => 1,
        "macos" => 2,
        "windows" => 3,
        "freebsd" => 4,
        _ => 0,
    }
}

/// target triple → OS 码映射。
#[test]
fn target_os_code_mapping() {
    assert_eq!(zeta_driver::target_os_code(Some("arm64-apple-macosx")), 2);
    assert_eq!(zeta_driver::target_os_code(Some("x86_64-apple-macosx")), 2);
    assert_eq!(zeta_driver::target_os_code(Some("x86_64-unknown-linux-gnu")), 1);
    assert_eq!(zeta_driver::target_os_code(Some("aarch64-unknown-linux-gnu")), 1);
    assert_eq!(zeta_driver::target_os_code(Some("x86_64-pc-windows-msvc")), 3);
    assert_eq!(zeta_driver::target_os_code(Some("wasm32-unknown-unknown")), 0);
    // 无 target = 主机；主机 triple 也应得到同一码
    assert_eq!(zeta_driver::target_os_code(None), host_os_code());
    let host = zeta_driver::host_triple();
    assert_eq!(zeta_driver::target_os_code(Some(&host)), host_os_code());
}

/// 平台内建端到端接线：直接 extern 声明并调用 `__zeta_target_os()`。
#[test]
fn platform_builtin_host_runs() {
    let out = run("extern fn __zeta_target_os() -> i32;\nfn main() { print(__zeta_target_os()); }\n");
    assert_eq!(out, host_os_code().to_string());
}

/// `sockaddr_in4_with_layout` 双布局字节序列。
#[test]
fn sockaddr_in4_dual_layout() {
    let out = run(
        r#"
fn dump(s: String) -> i64 {
    let mut i = 0;
    while i < 16 {
        if i > 0 { print(","); }
        print(s.data[i]);
        i = i + 1;
    }
    print("\n");
    0
}
fn main() {
    dump(sockaddr_in4_with_layout(true, 8080, 127, 0, 0, 1));
    dump(sockaddr_in4_with_layout(false, 8080, 127, 0, 0, 1));
    dump(sockaddr_in4(8080, 127, 0, 0, 1));
}
"#,
    );
    let macos = "16,2,31,144,127,0,0,1,0,0,0,0,0,0,0,0\n";
    let linux = "2,0,31,144,127,0,0,1,0,0,0,0,0,0,0,0\n";
    assert_eq!(out, format!("{macos}{linux}{macos}"));
}

/// 端口/地址字节在不同布局下保持一致（仅头部差异）。
#[test]
fn sockaddr_in4_tail_bytes_consistent() {
    let out = run(
        r#"
fn head(s: String) -> i64 {
    s.data[0]
}
fn main() {
    let m = sockaddr_in4_with_layout(true, 8080, 127, 0, 0, 1);
    let l = sockaddr_in4_with_layout(false, 8080, 127, 0, 0, 1);
    println(m.data[2]);  // 端口高字节一致
    println(l.data[2]);
    println(m.data[5]);  // IP 第二字节一致
    println(l.data[5]);
}
"#,
    );
    assert_eq!(out, "31\n31\n0\n0\n");
}
