//! `zeta test` 用例套件矩阵：把工作区根 `tests/` 目录纳入 `cargo test` 驱动，
//! 保证 CI 全量跑测试时 compile-pass / compile-fail / run-pass 用例都被执行。

use std::path::Path;

use zeta_driver::test_runner::{run_test_suite, TestKind};

#[test]
fn zeta_test_suite_all_pass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests");
    let summary = run_test_suite(&root);

    assert!(summary.total > 0, "tests/ 目录下没有找到任何用例");
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

#[test]
fn zeta_test_suite_covers_all_kinds() {
    // 三类用例都应有覆盖（防止误删整类目录）
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests");
    let summary = run_test_suite(&root);
    for kind in [
        TestKind::CompilePass,
        TestKind::CompileFail,
        TestKind::RunPass,
    ] {
        let n = summary.results.iter().filter(|r| r.kind == kind).count();
        assert!(n > 0, "{} 类别没有任何用例", kind.label());
    }
}
