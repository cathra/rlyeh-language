//! 引用与借用（G1）集成测试：`&x` / `&mut x` / `*p` 解引用。
//!
//! 文件入口 API，自动注入 `zeta-std/zeta/core.zeta`。
//!
//! 覆盖：
//! - 标量引用：`&i64` 参数 + `*p` 读取 / `*p = v` 写入 / `*p += v` 复合赋值
//! - 解引用表达式：`let y = *p;` / `println(*p)`
//! - 聚合引用：`&Struct` 字段访问 / `&mut Struct` 字段写入（peel_ref 自动）
//! - 容器引用：`&Vec<i64>` 方法调用
//! - 引用逐级下传（`&x` → `&T` 参数 → `&T` 参数）
//!
//! 需要系统 clang（与 dynamic_slice_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-ref-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("引用测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 标量引用读取：`&x` 传 `fn f(p: &i64)`，`*p` 解引用读取。
#[test]
fn scalar_ref_read() {
    let out = run(
        r#"
fn double(p: &i64) -> i64 {
    *p * 2
}

fn main() {
    let x = 21;
    println(double(&x));       // 42
    let p = &x;
    println(*p);               // 21（解引用表达式读取）
    let y = *p;                // 解引用绑定
    println(y + 1);            // 22
}
"#,
    );
    assert_eq!(out, "42\n21\n22\n");
}

/// 标量引用写入：`&mut x` → `*p = v`，写回外部变量可见。
#[test]
fn scalar_ref_write() {
    let out = run(
        r#"
fn bump(p: &mut i64) {
    *p = *p + 1;
}

fn main() {
    let mut x = 10;
    bump(&mut x);
    println(x);                // 11
    let p = &mut x;
    *p = 99;
    println(x);                // 99
}
"#,
    );
    assert_eq!(out, "11\n99\n");
}

/// 复合赋值：`*p += v` 展开为 `*p = *p + v`。
#[test]
fn scalar_ref_compound_assign() {
    let out = run(
        r#"
fn add(p: &mut i64, v: i64) {
    *p += v;
}

fn main() {
    let mut x = 5;
    add(&mut x, 7);
    println(x);                // 12
    let p = &mut x;
    *p *= 3;
    println(x);                // 36
}
"#,
    );
    assert_eq!(out, "12\n36\n");
}

/// 聚合引用字段读取：`&Struct` → 字段访问（引用自动剥皮）。
#[test]
fn struct_ref_field_read() {
    let out = run(
        r#"
struct Point { x: i64, y: i64 }

fn norm1(p: &Point) -> i64 {
    p.x + p.y
}

fn main() {
    let pt = Point { x: 3, y: 4 };
    println(norm1(&pt));       // 7
    let r = &pt;
    println(r.x);              // 3（引用变量直接字段访问）
}
"#,
    );
    assert_eq!(out, "7\n3\n");
}

/// 聚合引用字段写入：`&mut Struct` → 字段赋值写回。
#[test]
fn struct_ref_field_write() {
    let out = run(
        r#"
struct Point { x: i64, y: i64 }

fn setx(p: &mut Point, v: i64) {
    p.x = v;
}

fn main() {
    let mut pt = Point { x: 1, y: 2 };
    setx(&mut pt, 99);
    println(pt.x);             // 99
    let r = &mut pt;
    r.y = 100;
    println(pt.y);             // 100
}
"#,
    );
    assert_eq!(out, "99\n100\n");
}

/// 容器引用：`&Vec<i64>` 方法调用（len / get）。
#[test]
fn vec_ref_methods() {
    let out = run(
        r#"
fn total(v: &Vec<i64>) -> i64 {
    let mut s: i64 = 0;
    let mut i: i64 = 0;
    while i < v.len() {
        s = s + v.get(i);
        i = i + 1;
    }
    s
}

fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(1); v.push(2); v.push(3); v.push(4);
    println(total(&v));        // 10
    let r = &v;
    println(r.len());          // 4（引用变量直接方法调用）
}
"#,
    );
    assert_eq!(out, "10\n4\n");
}

/// 引用逐级下传：`&x` 经多个 `&T` 参数链式传递。
#[test]
fn ref_chain() {
    let out = run(
        r#"
fn inner(p: &i64) -> i64 {
    *p + 1
}

fn outer(p: &i64) -> i64 {
    inner(p)
}

fn main() {
    let x = 41;
    println(outer(&x));        // 42
    println(inner(&x));        // 42
}
"#,
    );
    assert_eq!(out, "42\n42\n");
}

/// 引用变量重新赋值（引用的别名拷贝）与混用。
#[test]
fn ref_alias_copy() {
    let out = run(
        r#"
fn main() {
    let a = 10;
    let b = 20;
    let p = &a;
    let q = p;                 // 引用拷贝（别名）
    println(*q);               // 10
    let p2 = &b;
    println(*p2);              // 20
    println(*p);               // 10（原引用不受影响）
}
"#,
    );
    assert_eq!(out, "10\n20\n10\n");
}

/// 返回引用：`fn id(p: &T) -> &T { p }` 引用逐级返回。
#[test]
fn return_reference() {
    let out = run(
        r#"
fn id(p: &i64) -> &i64 {
    p
}

fn deref(p: &i64) -> i64 {
    *id(p)
}

fn main() {
    let x = 7;
    let r = id(&x);
    println(*r);               // 7（返回引用解引用）
    println(deref(&x));        // 7（返回引用再传参）
}
"#,
    );
    assert_eq!(out, "7\n7\n");
}

/// `&mut T` 传给 `&T` 参数（宽松可变性兼容；严格互斥留给 borrowck）。
#[test]
fn mut_to_immut_ref() {
    let out = run(
        r#"
fn read(p: &i64) -> i64 {
    *p
}

fn main() {
    let mut x = 5;
    println(read(&mut x));     // 5（&mut 传 &）
    let y = &mut x;
    *y = 10;
    println(read(y));          // 10（引用变量复用）
}
"#,
    );
    assert_eq!(out, "5\n10\n");
}
