//! `Option<T>` / `Result<T, E>` 补充方法集成测试（文件入口 API，自动注入 `zeta-std/zeta/core.zeta`）。
//!
//! 覆盖：`Option::expect`（Some 返回 / String 实例化）、`Option::unwrap_or`
//! （Some 返回自身 / None 返回默认，i64 + String 双实例化）、`Result::is_err`
//! （Ok→0 / Err→1 / 与 is_ok 互补）、`Result::unwrap_or`（Ok 返回 / Err 默认）、
//! `Result::expect`（Ok 路径）、组合链路。
//!
//! 注意：`expect` 在 None/Err 上是 `loop {}` 死循环（崩溃替代），测试只走
//! Some/Ok 路径；未命中语义由 `unwrap_or` 的默认值断言覆盖。
//!
//! 需要系统 clang（与 std_test.rs / agg_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-optres-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("Option/Result 补充方法测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// Option::expect Some 路径：i64 / String 双实例化，与 unwrap 等价。
#[test]
fn option_expect_some() {
    let out = run(
        r#"
fn main() {
    let a = Option::Some(42);
    println(a.expect(String::from("boom")));     // 42（消息参数不打印）
    let s = Option::Some(String::from("hi"));
    println(s.expect(String::from("oops")));     // hi（String 实例化）
    let n = Option::Some(7);
    println(n.expect(String::from("x")) + 3);    // 10（expect 结果可参与算术）
    println(Option::Some(9).expect(String::from("m")) == 9); // true（与字面量相等）
}
"#,
    );
    assert_eq!(out, "42\nhi\n10\ntrue\n");
}

/// Option::unwrap_or：Some 返回自身 / None 返回默认；i64 + String 双实例化。
#[test]
fn option_unwrap_or_default() {
    let out = run(
        r#"
fn main() {
    let a = Option::Some(100);
    println(a.unwrap_or(-1));                    // 100（Some 返回自身）
    let b = Option::None;
    println(b.unwrap_or(-1));                    // -1（None 返回默认）
    println(Option::Some(5).unwrap_or(0) * 2);   // 10（默认值不参与计算）
    println(Option::Some(3).unwrap_or(9));       // 3（Some 优先）
}
"#,
    );
    assert_eq!(out, "100\n-1\n10\n3\n");
}

/// Result::is_err：Ok→0 / Err→1 / 与 is_ok 互补（总和恒 1）。
#[test]
fn result_is_err() {
    let out = run(
        r#"
fn main() {
    let ok = Result::Ok(5);
    let err = Result::Err(3);
    println(ok.is_err());                        // 0（Ok 非错误）
    println(err.is_err());                       // 1（Err 是错误）
    println(ok.is_ok());                         // 1（互补）
    println(err.is_ok());                        // 0（互补）
    println(ok.is_err() + ok.is_ok());           // 1（互补和恒 1）
    println(err.is_err() + err.is_ok());         // 1
}
"#,
    );
    assert_eq!(out, "0\n1\n1\n0\n1\n1\n");
}

/// Result::unwrap_or：Ok 返回 v / Err 返回 default；i64 + String 双实例化。
#[test]
fn result_unwrap_or_default() {
    let out = run(
        r#"
fn main() {
    let ok = Result::Ok(42);
    let err = Result::Err(-7);
    println(ok.unwrap_or(0));                    // 42（Ok 返回载荷）
    println(err.unwrap_or(0));                   // 0（Err 返回默认）
    println(err.unwrap_or(99));                  // 99（默认值可任意）
    println(Result::Ok(8).unwrap_or(1) - 8);     // 0（Ok 载荷直接参与运算）
}
"#,
    );
    assert_eq!(out, "42\n0\n99\n0\n");
}

/// Result::expect Ok 路径：返回载荷，与 unwrap 等价（Err 路径死循环不触发）。
#[test]
fn result_expect_ok() {
    let out = run(
        r#"
fn main() {
    let ok = Result::Ok(77);
    println(ok.expect(String::from("fail")));    // 77（Ok 返回载荷）
    let s = Result::Ok(String::from("ok!"));
    println(s.expect(String::from("e")));        // ok!（String 实例化）
    println(Result::Ok(21).expect(String::from("m")) * 2); // 42
    println(Result::Ok(5).expect(String::from("m")) == 5); // true
}
"#,
    );
    assert_eq!(out, "77\nok!\n42\ntrue\n");
}

/// 组合：Option 与 Result 混合链路（unwrap_or 链式 / is_err 决策 / match 兜底）。
#[test]
fn option_result_combine() {
    let out = run(
        r#"
fn main() {
    let ok: Result<i64, i64> = Result::Ok(10);
    let err: Result<i64, i64> = Result::Err(3);
    let opt: Option<i64> = Option::Some(4);
    let v = ok.unwrap_or(0) + err.unwrap_or(100) + opt.unwrap_or(1);
    println(v);                                  // 10 + 100 + 4 = 114
    let flag = err.is_err();
    if flag == 1 {
        println("has error");                    // has error（决策分支）
    }
    let s = Result::Ok(String::from("file.txt"));
    let base = s.expect(String::from("e")).strip_suffix(String::from(".txt")).unwrap();
    println(base.to_upper());                    // FILE（expect 载荷再走 String 方法）
    let m = Option::Some(String::from("x"));
    println(m.expect(String::from("e")) + String::from("y")); // xy（expect + 拼接）
}
"#,
    );
    assert_eq!(out, "114\nhas error\nFILE\nxy\n");
}
