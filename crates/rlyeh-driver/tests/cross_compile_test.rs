//! E1 交叉编译测试：`--target` 构建可执行文件（本机目标 / 双架构交叉目标）。

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rlyeh-xc-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn file_cmd(path: &Path) -> String {
    let out = Command::new("file").arg(path).output().unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// 本机架构（与 `rlyeh_driver::target_arch` 同一归一化命名）。
fn host_arch_norm() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        a => a,
    }
}

#[test]
fn target_arch_normalization() {
    assert_eq!(
        rlyeh_driver::target_arch("arm64-apple-macosx"),
        Some("aarch64")
    );
    assert_eq!(
        rlyeh_driver::target_arch("aarch64-unknown-linux-gnu"),
        Some("aarch64")
    );
    assert_eq!(
        rlyeh_driver::target_arch("x86_64-apple-macosx"),
        Some("x86_64")
    );
    assert_eq!(
        rlyeh_driver::target_arch("wasm32-unknown-unknown"),
        Some("wasm32")
    );
    assert_eq!(rlyeh_driver::target_arch("noarch"), Some("noarch"));
    assert_eq!(rlyeh_driver::target_arch(""), None);

    // 主机目标与本机架构一致；跨架构才是交叉编译
    let host = rlyeh_driver::host_triple();
    assert_eq!(rlyeh_driver::target_arch(&host), Some(host_arch_norm()));
    assert!(!rlyeh_driver::is_cross_target(None));
    assert!(!rlyeh_driver::is_cross_target(Some(&host)));
    assert_eq!(
        rlyeh_driver::is_cross_target(Some("x86_64-apple-macosx")),
        host_arch_norm() != "x86_64"
    );
    assert_eq!(
        rlyeh_driver::is_cross_target(Some("arm64-apple-macosx")),
        host_arch_norm() != "aarch64"
    );
}

#[test]
fn build_with_host_target_runs() {
    let root = workspace_root();
    let hello = root.join("tests/run-pass/hello.rl");
    assert!(hello.exists(), "缺测试用例: {}", hello.display());
    let dir = tmp_dir("host");
    let exe = dir.join("hello-host");
    let target = rlyeh_driver::host_triple();
    rlyeh_driver::build_executable_file_with_target(&hello, &exe, Some(&target)).unwrap();
    let out = Command::new(&exe).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "Hello, Rlyeh!\n");
    assert!(out.status.success());
}

#[test]
fn build_without_target_still_works() {
    // 兼容性：无 --target 时行为不变
    let root = workspace_root();
    let hello = root.join("tests/run-pass/hello.rl");
    let dir = tmp_dir("notarget");
    let exe = dir.join("hello-plain");
    rlyeh_driver::build_executable_file(&hello, &exe).unwrap();
    let out = Command::new(&exe).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "Hello, Rlyeh!\n");
}

#[test]
fn build_invalid_target_fails() {
    let root = workspace_root();
    let hello = root.join("tests/run-pass/hello.rl");
    let dir = tmp_dir("bad");
    let exe = dir.join("hello-bad");
    let res = rlyeh_driver::build_executable_file_with_target(&hello, &exe, Some("not-a-real-triple"));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(res.is_err(), "无效 target 应编译失败");
}

/// macOS 上 clang 自带交叉能力：编译到与主机不同的架构，校验产物架构。
#[cfg(target_os = "macos")]
#[test]
fn cross_compile_alternate_arch_macos() {
    let root = workspace_root();
    let hello = root.join("tests/run-pass/hello.rl");
    let dir = tmp_dir("alt");
    let exe = dir.join("hello-alt");
    let (alt_triple, alt_marker) = if host_arch_norm() == "aarch64" {
        ("x86_64-apple-macosx", "x86_64")
    } else {
        ("arm64-apple-macosx", "arm64")
    };
    match rlyeh_driver::build_executable_file_with_target(&hello, &exe, Some(alt_triple)) {
        Ok(()) => {
            let info = file_cmd(&exe);
            let _ = std::fs::remove_dir_all(&dir);
            assert!(
                info.contains(alt_marker),
                "期望 {alt_marker} 架构产物，file 输出: {info}"
            );
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            eprintln!("跳过: 交叉编译不可用（缺少 SDK/工具链）: {e}");
        }
    }
}

/// 使用标准库特性（Vec 等）的程序交叉编译后仍可运行（Rosetta），
/// 验证 std 编译结果的目标无关性。
#[cfg(target_os = "macos")]
#[test]
fn cross_compile_std_features_and_run() {
    let dir = tmp_dir("std");
    let f = dir.join("std-demo.rl");
    std::fs::write(
        &f,
        "fn main() {\n    let mut v = Vec::new();\n    v.push(10);\n    v.push(20);\n    v.push(30);\n    let total = v[0] + v[1] + v[2];\n    print(total);\n}\n",
    )
    .unwrap();
    let exe = dir.join("std-demo");
    let alt = if host_arch_norm() == "aarch64" {
        "x86_64-apple-macosx"
    } else {
        "arm64-apple-macosx"
    };
    match rlyeh_driver::build_executable_file_with_target(&f, &exe, Some(alt)) {
        Ok(()) => {
            let out = Command::new(&exe).output().unwrap();
            let _ = std::fs::remove_dir_all(&dir);
            assert_eq!(String::from_utf8_lossy(&out.stdout), "60");
            assert!(out.status.success());
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            eprintln!("跳过: 交叉编译不可用（缺少 SDK/工具链）: {e}");
        }
    }
}
