//! E2 WebAssembly 目标测试：`--target wasm32-wasi` 编译 Zeta 源码为 `.wasm`，
//! 用 wasmtime 运行验证（WASI preview1）。
//!
//! 依赖（缺一即优雅跳过）：
//! - wasi-libc sysroot（`brew install wasi-libc`，或 `WASI_SYSROOT` 环境变量）
//! - wasm-ld（`brew install lld`）
//! - wasmtime（`brew install wasmtime`）
//!
//! 覆盖：
//! - `is_wasm_triple` 目标判定（纯函数，无工具依赖）
//! - hello world 编译为 `.wasm` 并用 wasmtime 运行
//! - 标准库类型（数组 / String + 算术 + 位运算）在 wasm 下编译运行
//!   （验证 malloc 分配器、printf 接线、`_start` → `main` 入口链）

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
    let dir = std::env::temp_dir().join(format!("zeta-wasm-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn tool_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// WASI sysroot 是否就位（与 driver 的探测路径保持一致）。
fn wasi_sysroot_ready() -> bool {
    if let Ok(p) = std::env::var("WASI_SYSROOT") {
        if Path::new(&p).join("lib").exists() {
            return true;
        }
    }
    [
        "/opt/homebrew/opt/wasi-libc/share/wasi-sysroot",
        "/usr/local/opt/wasi-libc/share/wasi-sysroot",
    ]
    .iter()
    .any(|b| Path::new(b).join("lib").exists())
}

fn wasm_ld_ready() -> bool {
    tool_exists("wasm-ld")
        || Path::new("/opt/homebrew/opt/lld/bin/wasm-ld").exists()
        || Path::new("/usr/local/opt/lld/bin/wasm-ld").exists()
}

/// wasm 目标是否可编译运行（sysroot + wasm-ld + wasmtime 三者齐备）。
fn wasm_toolchain_ready() -> bool {
    wasi_sysroot_ready() && wasm_ld_ready() && tool_exists("wasmtime")
}

/// wasm 版 Actor 运行时静态库是否就位（L4b：`cargo build --target
/// wasm32-wasip1 -p zeta-actor-runtime` 产物；driver 探测路径之一）。
fn wasm_actor_runtime_ready() -> bool {
    let root = workspace_root();
    for target in ["wasm32-wasip1", "wasm32-wasi"] {
        for profile in ["release", "debug"] {
            let p = root
                .join("target")
                .join(target)
                .join(profile)
                .join("libzeta_actor_runtime.a");
            if p.exists() {
                return true;
            }
        }
    }
    false
}

/// 编译入口文件为 `.wasm` 并用 wasmtime 运行，返回 stdout。
fn build_and_run_wasm(entry: &Path, dir: &Path, tag: &str) -> String {
    let wasm = dir.join(format!("{tag}.wasm"));
    zeta_driver::build_executable_file_with_target(entry, &wasm, Some("wasm32-wasi")).unwrap();
    assert!(wasm.exists(), "应生成 .wasm 产物: {}", wasm.display());
    let out = Command::new("wasmtime").arg(&wasm).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "wasmtime 运行失败: {stderr}\nstdout: {stdout}"
    );
    stdout
}

#[test]
fn is_wasm_triple_detection() {
    assert!(zeta_driver::is_wasm_triple("wasm32-wasi"));
    assert!(zeta_driver::is_wasm_triple("wasm32-unknown-unknown"));
    assert!(zeta_driver::is_wasm_triple("wasm64-wasi"));
    assert!(!zeta_driver::is_wasm_triple("aarch64-apple-macosx"));
    assert!(!zeta_driver::is_wasm_triple("x86_64-unknown-linux-gnu"));
    assert!(!zeta_driver::is_wasm_triple(""));
}

/// hello world 编译为 .wasm 并在 wasmtime 下运行。
#[test]
fn wasm_hello_world_runs() {
    if !wasm_toolchain_ready() {
        eprintln!(
            "跳过: wasm 工具链不齐备（需要 wasi-libc / wasm-ld / wasmtime）"
        );
        return;
    }
    let root = workspace_root();
    let hello = root.join("tests/run-pass/hello.zeta");
    assert!(hello.exists(), "缺测试用例: {}", hello.display());
    let dir = tmp_dir("hello");
    let stdout = build_and_run_wasm(&hello, &dir, "hello");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(stdout, "Hello, Zeta!\n");
}

/// 标准库类型（Vec / String + 算术 + 位运算）在 wasm 下编译运行，
/// 验证分配器（malloc）、printf 接线与 `_start` → `main` 入口链。
#[test]
fn wasm_std_features_runs() {
    if !wasm_toolchain_ready() {
        eprintln!("跳过: wasm 工具链不齐备（需要 wasi-libc / wasm-ld / wasmtime）");
        return;
    }
    let dir = tmp_dir("std");
    let f = dir.join("std-demo.zeta");
    std::fs::write(
        &f,
        "fn main() {\n\
         \x20   let arr = [10, 20, 30];\n\
         \x20   let total = arr[0] + arr[1] + arr[2];\n\
         \x20   print(total);\n\
         \x20   print(\" \");\n\
         \x20   let s = String::from(\"wasm-ok\");\n\
         \x20   print(s);\n\
         \x20   print(\" \");\n\
         \x20   print(0x80 | 0x0F);\n\
         \x20   print(\"\\n\");\n\
         }\n",
    )
    .unwrap();
    let stdout = build_and_run_wasm(&f, &dir, "std-demo");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(stdout, "60 wasm-ok 143\n");
}

/// L4b: Actor 交叉编译 / WASM 支持——ask 往返 + send 异步 + FIFO 在
/// `wasm32-wasi` 目标下编译运行，输出与原生完全一致。
///
/// wasm 版 Actor 运行时（`zeta-actor-runtime` 同步模式）为单线程实现：
/// 无 dlsym / 线程池，符号经 driver 注入的静态表 `zeta_actor_resolve` 解析，
/// 消息同步派发（与 L1 `async` 的 MVP 同步语义一致）。
#[test]
fn wasm_actor_runs() {
    if !wasm_toolchain_ready() {
        eprintln!("跳过: wasm 工具链不齐备（需要 wasi-libc / wasm-ld / wasmtime）");
        return;
    }
    if !wasm_actor_runtime_ready() {
        eprintln!(
            "跳过: 缺 wasm 版 Actor 运行时（先执行 `cargo build --target wasm32-wasip1 -p zeta-actor-runtime`）"
        );
        return;
    }
    let root = workspace_root();
    let ping_pong = root.join("tests/run-pass/actor-ping-pong.zeta");
    assert!(ping_pong.exists(), "缺测试用例: {}", ping_pong.display());
    let dir = tmp_dir("actor");
    let stdout = build_and_run_wasm(&ping_pong, &dir, "actor");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(stdout, "11\n12\n2\n4\n");
}

/// L4b: Actor 交叉编译——受监督崩溃重启语义在 wasm 下与原生一致：
/// handle 返回 -1 触发崩溃协议，factory 重建初始状态，ask 崩溃时回复 0。
#[test]
fn wasm_actor_supervised_runs() {
    if !wasm_toolchain_ready() {
        eprintln!("跳过: wasm 工具链不齐备（需要 wasi-libc / wasm-ld / wasmtime）");
        return;
    }
    if !wasm_actor_runtime_ready() {
        eprintln!(
            "跳过: 缺 wasm 版 Actor 运行时（先执行 `cargo build --target wasm32-wasip1 -p zeta-actor-runtime`）"
        );
        return;
    }
    let dir = tmp_dir("actor-sup");
    let f = dir.join("actor-supervised.zeta");
    std::fs::write(
        &f,
        "// 受监督 actor：value 达 100 崩溃一次，supervisor 重建初始状态\n\
         actor Counter {\n\
         \x20   value: i64 = 0,\n\
         \x20   pub fn increment(amount: i64) -> i64 {\n\
         \x20       self.value += amount;\n\
         \x20       if self.value == 100 { return -1; }\n\
         \x20       self.value\n\
         \x20   }\n\
         }\n\
         fn main() {\n\
         \x20   let c = Counter::new_supervised(0);\n\
         \x20   let r1 = c.increment(50).await;   // 50\n\
         \x20   let r2 = c.increment(50).await;   // 100 → -1 崩溃 → 重建 → ask 回复 0\n\
         \x20   let r3 = c.increment(10).await;   // 重建后 0 + 10 = 10\n\
         \x20   println(r1);\n\
         \x20   println(r2);\n\
         \x20   println(r3);\n\
         }\n",
    )
    .unwrap();
    let stdout = build_and_run_wasm(&f, &dir, "actor-sup");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(stdout, "50\n0\n10\n");
}
