//! `rlyeh test` 用例套件矩阵：把工作区根 `tests/` 目录纳入 `cargo test` 驱动，
//! 保证 CI 全量跑测试时 compile-pass / compile-fail / run-pass 用例都被执行。

use std::path::Path;

use rlyeh_driver::test_runner::{run_test_suite, TestKind};

#[test]
fn rlyeh_test_suite_all_pass() {
    // 注意：套件执行必须保持**单个测试函数**——多个测试函数会被 cargo test
    // 并行执行，同一套 run-pass 用例（如 fs_dir 的 `/tmp` 路径）被两个进程
    // 同时操作会互相竞争导致偶发失败。
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests");
    let summary = run_test_suite(&root);

    assert!(summary.total > 0, "tests/ 目录下没有找到任何用例");
    // 三类用例都应有覆盖（防止误删整类目录）
    for kind in [
        TestKind::CompilePass,
        TestKind::CompileFail,
        TestKind::RunPass,
    ] {
        let n = summary.results.iter().filter(|r| r.kind == kind).count();
        assert!(n > 0, "{} 类别没有任何用例", kind.label());
    }
    assert_eq!(
        summary.failed, 0,
        "测试套件有 {} 个失败用例:{}",
        summary.failed,
        summary
            .results
            .iter()
            .filter(|r| !r.passed)
            .map(|r| format!("\n[失败] {}: {}", r.name, r.detail))
            .collect::<String>()
    );
}
