//! 增量编译端到端测试：缓存命中 / 未命中 / 强制重编译 / 产物一致性。

use std::path::PathBuf;

use rlyeh_driver::IncrementalDriver;

const HELLO: &str = r#"fn main() { println("Hello, Rlyeh!"); }"#;
const HELLO_V2: &str = r#"fn main() { println("Hello, Rlyeh v2!"); }"#;

/// 独立临时目录。
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rlyeh-incr-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn first_compile_misses_second_hits() {
    let dir = temp_dir("hit");
    let mut driver = IncrementalDriver::new(dir.clone());

    let first = driver.compile_to_llvm("main.rl", HELLO).unwrap();
    assert!(!first.cache_hit, "首次编译应为 miss");
    assert_eq!(driver.stats().hits, 0);
    assert_eq!(driver.stats().misses, 1);

    let second = driver.compile_to_llvm("main.rl", HELLO).unwrap();
    assert!(second.cache_hit, "相同源码二次编译应为 hit");
    assert_eq!(driver.stats().hits, 1);
    assert_eq!(driver.stats().misses, 1);
    assert_eq!(first.llvm, second.llvm, "命中产物应与首次编译一致");
    cleanup(&dir);
}

#[test]
fn source_change_invalidates_cache() {
    let dir = temp_dir("invalidate");
    let mut driver = IncrementalDriver::new(dir.clone());

    driver.compile_to_llvm("main.rl", HELLO).unwrap();
    driver.compile_to_llvm("main.rl", HELLO).unwrap();
    assert_eq!(driver.stats().hits, 1);

    let third = driver.compile_to_llvm("main.rl", HELLO_V2).unwrap();
    assert!(!third.cache_hit, "源码变化应触发全量重编译");
    assert_eq!(driver.stats().hits, 1);
    assert_eq!(driver.stats().misses, 2);

    // 回到旧版本 → 缓存仍然有效（产物按哈希归档，互不覆盖）
    let back = driver.compile_to_llvm("main.rl", HELLO).unwrap();
    assert!(back.cache_hit);
    cleanup(&dir);
}

#[test]
fn force_flag_ignores_cache() {
    let dir = temp_dir("force");
    let mut driver = IncrementalDriver::new(dir.clone());
    driver.compile_to_llvm("main.rl", HELLO).unwrap();

    let mut forced = IncrementalDriver::new(dir.clone()).with_force(true);
    let outcome = forced.compile_to_llvm("main.rl", HELLO).unwrap();
    assert!(!outcome.cache_hit, "--force 应忽略缓存全量重编译");
    cleanup(&dir);
}

#[test]
fn cache_is_scoped_per_directory() {
    let dir_a = temp_dir("scoped-a");
    let dir_b = temp_dir("scoped-b");
    let mut a = IncrementalDriver::new(dir_a.clone());
    let mut b = IncrementalDriver::new(dir_b.clone());

    a.compile_to_llvm("main.rl", HELLO).unwrap();
    let outcome_b = b.compile_to_llvm("main.rl", HELLO).unwrap();
    assert!(!outcome_b.cache_hit, "不同缓存目录不应共享命中");
    cleanup(&dir_a);
    cleanup(&dir_b);
}

#[test]
fn cache_survives_driver_recreation() {
    let dir = temp_dir("recreate");
    {
        let mut driver = IncrementalDriver::new(dir.clone());
        driver.compile_to_llvm("main.rl", HELLO).unwrap();
    }
    {
        // 新驱动（模拟新进程）应命中磁盘缓存
        let mut driver = IncrementalDriver::new(dir.clone());
        let outcome = driver.compile_to_llvm("main.rl", HELLO).unwrap();
        assert!(outcome.cache_hit);
    }
    cleanup(&dir);
}

#[test]
fn incremental_run_source_end_to_end() {
    // 端到端：需要 clang 可执行（与 driver_test 的前提一致）
    let dir = temp_dir("run-e2e");
    let mut driver = IncrementalDriver::new(dir.clone());

    let (stdout, first) = driver.run_source("main.rl", HELLO).unwrap();
    assert_eq!(stdout, "Hello, Rlyeh!\n");
    assert!(!first.cache_hit);

    let (stdout, second) = driver.run_source("main.rl", HELLO).unwrap();
    assert_eq!(stdout, "Hello, Rlyeh!\n");
    assert!(second.cache_hit, "二次运行应命中 LLVM IR 缓存");

    let (stdout, third) = driver.run_source("main.rl", HELLO_V2).unwrap();
    assert_eq!(stdout, "Hello, Rlyeh v2!\n");
    assert!(!third.cache_hit);
    cleanup(&dir);
}
