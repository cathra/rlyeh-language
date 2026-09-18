//! `Option<T>` / `Result<T, E>` 补充方法集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/`）。
//!
//! 覆盖：`Option::expect`（Some 返回 / String 实例化）、`Option::unwrap_or`
//! （Some 返回自身 / None 返回默认，i64 + String 双实例化）、`Result::is_err`
//! （Ok→0 / Err→1 / 与 is_ok 互补）、`Result::unwrap_or`（Ok 返回 / Err 默认）、
//! `Result::expect`（Ok 路径）、组合链路、String 聚合载荷实例化
//! （`unwrap_or_string_aggregate`，验证 codegen「返回非载荷来源聚合」路径已修复）。
//!
//! 注意：`expect` 在 None/Err 上是 `loop {}` 死循环（崩溃替代），测试只走
//! Some/Ok 路径；未命中语义由 `unwrap_or` 的默认值断言覆盖。
//!
//! 需要系统 clang（与 std_test.rs / agg_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-optres-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
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

/// Option/Result 的 String（聚合载荷）实例化：unwrap_or 返回 default / 载荷均可
/// 正常使用（此前 codegen「返回非载荷来源聚合」路径损坏，现已修复）。
#[test]
fn unwrap_or_string_aggregate() {
    let out = run(
        r#"
fn main() {
    // None.unwrap_or(default) → default（String 聚合载荷）
    let none: Option<String> = Option::None;
    let r1 = none.unwrap_or(String::from("dft"));
    println(r1);                                 // dft
    // Some.unwrap_or(default) → 载荷
    let some = Option::Some(String::from("abc"));
    let r2 = some.unwrap_or(String::from("zz"));
    println(r2);                                 // abc
    // 返回的 String 可正常走方法 / 比较
    println(r2.to_upper());                      // ABC
    println(r1.len());                           // 3
    println(r1 == String::from("dft"));          // true
    // 拼接（+ 原地追加到左操作数缓冲）
    let r3 = r1 + String::from("!");
    println(r3);                                 // dft!
    // Result<String, i64>：Err 返回 default / Ok 返回载荷
    let err: Result<String, i64> = Result::Err(1);
    let r4 = err.unwrap_or(String::from("default"));
    println(r4);                                 // default
    let ok: Result<String, i64> = Result::Ok(String::from("ok"));
    println(ok.unwrap_or(String::from("x")));    // ok
    // 循环内重复调用（每次独立 default）
    let mut i = 0;
    while i < 2 {
        let n: Option<String> = Option::None;
        println(n.unwrap_or(String::from("loop")));
        i = i + 1;
    }
}
"#,
    );
    assert_eq!(out, "dft\nabc\nABC\n3\ntrue\ndft!\ndefault\nok\nloop\nloop\n");
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

/// 用户级 match 解构具体实例化枚举的聚合载荷：此前泛型替换只下沉 std 泛型
/// 方法体，用户代码 `match (o: Option<String>)` 解构 `Some(v)` 时报
/// `expected String, found T`。修复后 `T` 由 `pat_ty` 类型参数推导。
#[test]
fn user_level_match_string_payload() {
    let out = run(
        r#"
fn main() {
    // Some 分支返回载荷（String 聚合）
    let o1: Option<String> = Option::Some(String::from("abc"));
    let r1 = match o1 {
        Option::Some(v) => v,
        Option::None => String::from("dft"),
    };
    println(r1);                                  // abc
    // None 分支返回 default
    let o2: Option<String> = Option::None;
    let r2 = match o2 {
        Option::Some(v) => v,
        Option::None => String::from("dft"),
    };
    println(r2);                                  // dft
    // 载荷参与 String 方法链
    let o3: Option<String> = Option::Some(String::from("hi"));
    let r3 = match o3 {
        Option::Some(v) => v + String::from("!"),
        Option::None => String::from("x"),
    };
    println(r3);                                  // hi!
    println(r3.len());                            // 3
    // Result 聚合载荷解构（Err 分支返回 String 载荷）
    let e: Result<String, i64> = Result::Err(1);
    let r4 = match e {
        Result::Ok(v) => v,
        Result::Err(_) => String::from("err"),
    };
    println(r4);                                  // err
    let ok: Result<String, i64> = Result::Ok(String::from("ok"));
    let r5 = match ok {
        Result::Ok(v) => v,
        Result::Err(_) => String::from("err"),
    };
    println(r5);                                  // ok
    // 嵌套：Result<Option<String>, i64> 双层聚合载荷解构
    let inner: Option<String> = Option::Some(String::from("deep"));
    let outer: Result<Option<String>, i64> = Result::Ok(inner);
    let r6 = match outer {
        Result::Ok(opt) => match opt {
            Option::Some(s) => s + String::from("!"),
            Option::None => String::from("none"),
        },
        Result::Err(_) => String::from("err"),
    };
    println(r6);                                  // deep!
    println(r6.to_upper());                       // DEEP!
}
"#,
    );
    assert_eq!(
        out,
        "abc\ndft\nhi!\n3\nerr\nok\ndeep!\nDEEP!\n"
    );
}

/// 裸构造（无显式类型注解）的 Option/Result：类型参数由实参推断，
/// 泛型方法实例化不再泄漏 `_`（Infer）——此前 `Result::Err(7).unwrap_or(100)`
/// 返回类型为 `_`，参与运算报 `expected a numeric type, found _`。
#[test]
fn bare_enum_infer() {
    let out = run(
        r#"
fn main() {
    // Result::Err 裸构造：T 无信息 → Infer，由 default 实参定型
    let err = Result::Err(7);
    let z = err.unwrap_or(100) * 2;
    println(z);                                  // 200
    // Result::Ok 裸构造 + match 解构
    let ok = Result::Ok(10);
    let r = match ok {
        Result::Ok(v) => v + 1,
        Result::Err(e) => e,
    };
    println(r);                                  // 11
    // Option::Some 裸构造 + unwrap_or + 比较
    let some = Option::Some(5);
    let x = some.unwrap_or(0);
    println(x > 3);                              // true
    // 聚合载荷裸构造（String 由载荷定型）
    let s = Option::Some(String::from("hi"));
    println(s.unwrap_or(String::from("d")));     // hi
    // 链式：unwrap_or 结果参与算术
    let e2 = Result::Err(3);
    println(e2.unwrap_or(1) + 4);                // 5
}
"#,
    );
    assert_eq!(out, "200\n11\ntrue\nhi\n5\n");
}
