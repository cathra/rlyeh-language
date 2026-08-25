//! 标准库预置（prelude）集成测试：文件入口 API 自动注入 `rlyeh-std/rlyeh/core.rl`，
//! 用户程序无需内联即可使用 `Option<T>` / `Result<T, E>`。
//!
//! 需要系统 clang（与 driver_test.rs / std_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-prelude-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 用户程序直接使用 Option/Result（不内联标准库源码，验证自动注入）。
#[test]
fn prelude_option_result_usable() {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(
        &file,
        r#"
fn main() {
    let a = Option::Some(42);
    println(a.is_some());
    println(a.unwrap());

    let b = Option::None;
    println(b.is_none());
    println(b.unwrap_or(-1));

    let ok = Result::Ok(7);
    println(ok.is_ok());
    println(ok.unwrap());

    let err = Result::Err(3);
    println(err.is_ok());
}
"#,
    )
    .expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("prelude 编译运行失败");
    assert_eq!(out, "1\n42\n1\n-1\n1\n7\n0\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 嵌套泛型：`Option<Result<i64, i64>>` 经由 prelude 注入后单态化。
#[test]
fn prelude_nested_generic() {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(
        &file,
        r#"
fn main() {
    let outer = Option::Some(Result::Ok(5));
    println(outer.is_some());
    let inner = outer.unwrap();
    println(inner.unwrap());
}
"#,
    )
    .expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("prelude 嵌套泛型失败");
    assert_eq!(out, "1\n5\n");
    let _ = std::fs::remove_dir_all(&dir);
}
