//! `Vec<T>` for-in 容器迭代 + 索引访问集成测试（文件入口 API，自动注入 core.zeta）。
//!
//! 覆盖：`for x in v` 求和/打印、break/continue 控制流、`v[i]` 索引读写、
//! 空 Vec 迭代 0 次、嵌套迭代、`v[i]` 步长与动态扩容后的正确性。
//!
//! 需要系统 clang（与 driver_test.rs / std_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-vec-for-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("Vec for 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// `for x in v` 求和：索引遍历覆盖全部元素。
#[test]
fn vec_for_sum() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(10);
    v.push(20);
    v.push(30);
    v.push(40);
    let mut sum = 0;
    for x in v {
        sum = sum + x;
    }
    println(sum);
}
"#,
    );
    assert_eq!(out, "100\n");
}

/// `for x in v` 逐元素打印（遍历顺序与 push 顺序一致）。
#[test]
fn vec_for_print() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::with_capacity(2);
    v.push(1);
    v.push(2);
    v.push(3);
    for x in v {
        println(x);
    }
}
"#,
    );
    assert_eq!(out, "1\n2\n3\n");
}

/// break / continue 控制流：continue 跳过 2，break 在 4 处终止。
#[test]
fn vec_for_break_continue() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    v.push(4);
    v.push(5);
    let mut sum = 0;
    for x in v {
        if x == 2 {
            continue;
        }
        if x == 4 {
            break;
        }
        sum = sum + x;
    }
    println(sum);
}
"#,
    );
    assert_eq!(out, "4\n"); // 1 + 3
}

/// `v[i]` 索引读取：动态扩容（cap 4 -> 8）后按原索引取元素。
#[test]
fn vec_index_read() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::new();
    v.push(7);
    v.push(8);
    v.push(9);
    v.push(10);
    v.push(11);
    println(v.len());
    println(v[0]);
    println(v[2]);
    println(v[4]);
}
"#,
    );
    assert_eq!(out, "5\n7\n9\n11\n");
}

/// `v[i] = x` 索引写入：赋值后读取验证。
#[test]
fn vec_index_write() {
    let out = run(
        r#"
fn main() {
    let mut v: Vec<i64> = Vec::with_capacity(4);
    v.push(1);
    v.push(2);
    v.push(3);
    v[1] = 99;
    println(v[0]);
    println(v[1]);
    println(v[2]);
    v[0] = v[1] + v[2];
    println(v[0]);
}
"#,
    );
    assert_eq!(out, "1\n99\n3\n102\n");
}

/// 空 Vec 迭代 0 次 + 嵌套 for（外层迭代同时内层求和）。
#[test]
fn vec_for_empty_and_nested() {
    let out = run(
        r#"
fn main() {
    let empty: Vec<i64> = Vec::new();
    let mut count = 0;
    for x in empty {
        count = count + 1;
    }
    println(count);

    let mut outer: Vec<i64> = Vec::new();
    outer.push(1);
    outer.push(2);
    let mut total = 0;
    for a in outer {
        let mut inner: Vec<i64> = Vec::new();
        inner.push(a);
        inner.push(a * 10);
        for b in inner {
            total = total + b;
        }
    }
    println(total);
}
"#,
    );
    assert_eq!(out, "0\n33\n"); // (1 + 10) + (2 + 20)
}
