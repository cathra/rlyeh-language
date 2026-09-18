//! 动态切片集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/`）。
//!
//! 覆盖：
//! - `Vec<T>` 动态切片 `v[lo..<hi]` / `v[lo...hi]` / `v[lo<..hi]`
//!   （desugar 为 std 泛型方法 `Vec::slice`，越界 clamp，返回全新缓冲）
//! - 数组 `[T; N]` 动态切片（typecheck 展开为 Vec 拷贝循环 + push 实例化）
//! - `String::from(s)` 支持绑定字面量的变量（local_inits 追踪）
//!
//! 需要系统 clang（与 vec_more_ops_test.rs / string_slice_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-dynslice-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("动态切片测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// Vec 基本切片：`v[lo..<hi]` 半开区间，返回全新缓冲（值拷贝），
/// 原 Vec 不受影响；len / get 读取验证。
#[test]
fn vec_slice_basic() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(10); v.push(20); v.push(30); v.push(40); v.push(50);
    let sub = v[1..<3];            // [20, 30]
    println(sub.len());            // 2
    println(sub.get(0));           // 20
    println(sub.get(1));           // 30
    println(v.len());              // 5（原 Vec 不受影响）
    println(v.get(1));             // 20（原值仍在）
    // 空区间
    let empty = v[3..<3];
    println(empty.len());          // 0
}
"#,
    );
    assert_eq!(out, "2\n20\n30\n5\n20\n0\n");
}

/// Vec 切片区间变体与边界 clamp：`...` 双闭、`<..` 不含下界、
/// 负 start / 超长 end 收敛、start >= end 空。
#[test]
fn vec_slice_ranges_clamp() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(1); v.push(2); v.push(3); v.push(4); v.push(5);
    let a = v[0...2];              // [0, 3) → 1, 2, 3
    println(a.len());              // 3
    println(a.get(2));             // 3
    let b = v[1<..3];              // (1, 3] → 下标 2,3 → 3, 4
    println(b.len());              // 2
    println(b.get(0));             // 3
    let c = v[-3..<2];             // clamp: [0, 2) → 1, 2
    println(c.len());              // 2
    println(c.get(0));             // 1
    let d = v[3..<99];             // clamp: [3, 5) → 4, 5
    println(d.len());              // 2
    println(d.get(1));             // 5
    let e = v[4..<1];              // start >= end → 空
    println(e.len());              // 0
}
"#,
    );
    assert_eq!(out, "3\n3\n2\n3\n2\n1\n2\n5\n0\n");
}

/// Vec 切片结果可继续操作（push / 再切片），元素按值拷贝互不影响。
#[test]
fn vec_slice_combo() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(1); v.push(2); v.push(3); v.push(4); v.push(5); v.push(6);
    let mut a = v[1..<5];          // [2, 3, 4, 5]
    a.push(99);
    println(a.len());              // 5
    println(a.get(4));             // 99
    println(v.len());              // 6（原 Vec 未变）
    let b = a[1..<3];              // [3, 4]
    println(b.len());              // 2
    println(b.get(0));             // 3
    println(b.get(1));             // 4
    // String 元素切片
    let mut vs: Vec<String> = Vec::new();
    vs.push(String::from("a"));
    vs.push(String::from("bb"));
    vs.push(String::from("ccc"));
    let ss = vs[0...1];            // ["a", "bb"]
    println(ss.len());             // 2
    println(ss.get(1) == String::from("bb"));
}
"#,
    );
    assert_eq!(out, "5\n99\n6\n2\n3\n4\n2\ntrue\n");
}

/// 数组动态切片：`[T; N]` 切片展开为 Vec 拷贝，边界 clamp 到 [0, N]。
#[test]
fn array_slice_basic() {
    let out = run(
        r#"
fn main() {
    let arr = [10, 20, 30, 40, 50];
    let a = arr[1..<3];            // [20, 30]
    println(a.len());              // 2
    println(a.get(0));             // 20
    println(a.get(1));             // 30
    // 动态边界（运行时变量）
    let lo = 1;
    let hi = 4;
    let b = arr[lo..<hi];          // [20, 30, 40]
    println(b.len());              // 3
    println(b.get(2));             // 40
    let c = arr[0...2];            // 双闭 → [10, 20, 30]
    println(c.len());              // 3
    println(c.get(0));             // 10
}
"#,
    );
    assert_eq!(out, "2\n20\n30\n3\n40\n3\n10\n");
}

/// 数组切片边界 clamp 与空切片（负 start / 超长 end / start >= end）。
#[test]
fn array_slice_clamp_empty() {
    let out = run(
        r#"
fn main() {
    let arr = [1, 2, 3];
    let a = arr[-5..<2];           // clamp [0, 2) → 1, 2
    println(a.len());              // 2
    println(a.get(0));             // 1
    let b = arr[1..<99];           // clamp [1, 3) → 2, 3
    println(b.len());              // 2
    println(b.get(1));             // 3
    let c = arr[2..<1];            // start >= end → 空
    println(c.len());              // 0
    let d = arr[3..<3];            // 越界后收敛为空
    println(d.len());              // 0
}
"#,
    );
    assert_eq!(out, "2\n1\n2\n3\n0\n0\n");
}

/// 数组切片结果可继续操作（push / 再切片）；bool 元素数组切片。
#[test]
fn array_slice_combo() {
    let out = run(
        r#"
fn main() {
    let arr = [1, 2, 3, 4, 5, 6];
    let mut a = arr[0..<4];        // [1, 2, 3, 4]
    a.push(9);
    println(a.len());              // 5
    println(a.get(4));             // 9
    let b = a[2..<5];              // [3, 4, 9]
    println(b.len());              // 3
    println(b.get(2));             // 9
    // bool 元素
    let flags = [true, false, true];
    let fb = flags[0..<2];         // [true, false]
    println(fb.len());             // 2
    println(fb.get(1));            // false
}
"#,
    );
    assert_eq!(out, "5\n9\n3\n9\n2\nfalse\n");
}

/// `String::from(s)`：字面量直用 + 绑定字面量的变量（local_inits 追踪）。
#[test]
fn string_from_variable() {
    let out = run(
        r#"
fn main() {
    let s = "hello rlyeh";
    let t = String::from(s);
    println(t);                    // hello rlyeh
    println(t.len());              // 11
    println(t == String::from("hello rlyeh"));
    // 字面量直用仍可用
    let u = String::from("direct");
    println(u == String::from("direct"));
    // 变量内容与字面量一致，可参与拼接
    let w = t + String::from("!");
    println(w);                    // hello rlyeh!
}
"#,
    );
    assert_eq!(out, "hello rlyeh\n11\ntrue\ntrue\nhello rlyeh!\n");
}

/// P8（2026-08-29）：Vec 切片省略边界——`v[..]`（全量）/ `v[..<2]`（省略下界）/
/// `v[1..<]`（省略上界，依赖 `Vec::slice` 的 clamp：`e > len → len`）。
#[test]
fn vec_slice_omitted_bounds() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(10); v.push(20); v.push(30);
    // 全量切片（两侧省略）
    let a = v[..];
    println(a.len());            // 3
    let a2 = v[..<];
    println(a2.len());           // 3
    // 省略下界
    let b = v[..<2];
    println(b.len());            // 2
    println(b.get(0));           // 10
    // 省略上界
    let c = v[1..<];
    println(c.len());            // 2
    println(c.get(0));           // 20
}
"#,
    );
    assert_eq!(out, "3\n3\n2\n10\n2\n20\n");
}

/// P8（2026-08-29）：String 切片省略边界——`s[1..<]`（省略上界）/
/// `s[..<2]`（省略下界），依赖 `String::substring` 的 clamp。
#[test]
fn string_slice_omitted_bounds() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello");
    let t = s[1..<];             // "ello"
    println(t);
    let u = s[..<2];             // "he"
    println(u);
    let w = s[..];               // 全量 "hello"
    println(w);
}
"#,
    );
    assert_eq!(out, "ello\nhe\nhello\n");
}
