//! `HashMap<K, V>` for-in 元组模式迭代集成测试（文件入口 API，自动注入标准库）。
//!
//! 覆盖：`for (k, v) in m` 求和/计数、删除（墓碑）后仅遍历存活项、
//! 空 map 迭代 0 次、扩容 rehash 后遍历、非 i64 值类型。
//!
//! 注意：HashMap 为线性探测开放寻址，遍历顺序与插入顺序无关，
//! 所有断言均用求和 / 计数（不依赖具体顺序）。
//!
//! 需要系统 clang（与 vec_for_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-hashmap-for-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("HashMap for 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// `for (k, v) in m` 遍历求和：k + v 总和（不依赖遍历顺序）。
#[test]
fn hashmap_for_sum() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(10, 1);
    m.insert(20, 2);
    m.insert(30, 3);
    m.insert(40, 4);
    m.insert(50, 5);
    let mut sum = 0;
    for (k, v) in m {
        sum = sum + k + v;
    }
    println(sum);
}
"#,
    );
    assert_eq!(out, "165\n"); // (10+1)+(20+2)+(30+3)+(40+4)+(50+5)
}

/// 计数遍历：访问次数等于 len（3）。
#[test]
fn hashmap_for_count() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(7, 70);
    m.insert(8, 80);
    m.insert(9, 90);
    let mut count = 0;
    for (k, v) in m {
        count = count + 1;
        if k > v {
            count = count + 100;
        }
    }
    println(count);
}
"#,
    );
    assert_eq!(out, "3\n"); // 恰好 3 个存活项
}

/// 删除产生墓碑后迭代：只遍历存活项（跳槽逻辑正确）。
#[test]
fn hashmap_for_after_remove() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(1, 10);
    m.insert(2, 20);
    m.insert(3, 30);
    m.insert(4, 40);
    m.insert(5, 50);
    m.remove(2);
    m.remove(4);
    let mut sum = 0;
    for (k, v) in m {
        sum = sum + v;
    }
    println(sum);
    println(m.len());
}
"#,
    );
    assert_eq!(out, "90\n3\n"); // 10 + 30 + 50；len = 3
}

/// 空 map 迭代 0 次。
#[test]
fn hashmap_for_empty() {
    let out = run(
        r#"
fn main() {
    let m: HashMap<i64, i64> = HashMap::new();
    let mut count = 0;
    for (k, v) in m {
        count = count + 1;
    }
    println(count);
}
"#,
    );
    assert_eq!(out, "0\n");
}

/// 扩容 rehash（cap 8 -> 16 -> 32）后遍历：值总和 = 100 * (0+...+19) = 19000。
#[test]
fn hashmap_for_after_grow() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    let mut i = 0;
    while i < 20 {
        m.insert(i, i * 100);
        i = i + 1;
    }
    let mut sum = 0;
    for (k, v) in m {
        sum = sum + v;
    }
    println(sum);
    println(m.cap());
}
"#,
    );
    assert_eq!(out, "19000\n32\n"); // Robin Hood 负载 7/8：8→16（第 7 键）、16→32（第 14 键），20 键终态 cap=32
}

/// 非 i64 值类型（f64）：键值解构按对应标量种类读写。
#[test]
fn hashmap_for_float_value() {
    let out = run(
        r#"
fn main() {
    let mut m: HashMap<i64, f64> = HashMap::new();
    m.insert(1, 1.5);
    m.insert(2, 2.5);
    m.insert(3, 4.0);
    let mut sum = 0.0;
    for (k, v) in m {
        sum = sum + v;
    }
    println(sum);
}
"#,
    );
    assert_eq!(out, "8.000000\n"); // 1.5 + 2.5 + 4.0（f64 打印 %f 默认 6 位小数）
}
