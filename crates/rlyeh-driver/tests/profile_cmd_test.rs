//! F2「PGO 数据回灌」集成测试：`.rl_profile` → 区域大小预测 → 编译报告。

use std::path::PathBuf;

use rlyeh_driver::{build_region_report, region_profile_report};
use rlyeh_region_alloc::{PgoAdvisor, ProfileCollector};

/// 构造含两个区域的 PGO 画像（可控分布）。
fn sample_data() -> rlyeh_region_alloc::PgoData {
    let mut c = ProfileCollector::new();
    // http_pool：小分配量样本（p95×1.1 低于 64KiB 下限）
    for i in 0..100 {
        c.record_allocation("http_pool", 1024 + i * 8);
        c.record_growth("http_pool", 256);
    }
    // big_buf：大分配量样本（p95×1.1 超过 64KiB 下限，验证 PGO 建议生效）
    for i in 0..100 {
        c.record_allocation("big_buf", 200_000 + i * 100);
        c.record_growth("big_buf", 40_000);
    }
    // session：单次分配
    c.record_allocation("session", 64);
    c.generate()
}

/// 临时文件（进程内唯一，测试后清理）。
fn temp_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rlyeh-profile-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn report_contains_all_regions_and_pgo_metadata() {
    let report = build_region_report(&sample_data());
    assert!(report.contains("Region Allocation Report"));
    assert!(report.contains("PGO data version:"));
    assert!(report.contains("PGO regions: 3"));
    for id in ["http_pool", "big_buf", "session"] {
        assert!(
            report.contains(&format!("- {id}: estimated")),
            "report 应包含区域 {id}\n{report}"
        );
    }
}

#[test]
fn recommendation_matches_advisor_and_obeys_floor() {
    let data = sample_data();
    let advisor = PgoAdvisor::from_data(data.clone());
    let report = build_region_report(&data);

    // 大样本：p95×1.1 > 64KiB → 采纳 PGO 建议
    let big = advisor.recommend_size("big_buf").unwrap();
    assert!(big > 64 * 1024);
    assert!(
        report.contains(&format!("- big_buf: estimated 204950 B, initial {big} B")),
        "报告应采用 advisor 推荐值 {big}\n{report}"
    );

    // 小样本：p95×1.1 低于下限 → 下限 64KiB
    let small = advisor.recommend_size("http_pool").unwrap();
    assert_eq!(small, 64 * 1024);
    assert!(
        report.contains(&format!(
            "- http_pool: estimated 1420 B, initial {} B",
            64 * 1024
        )),
        "小样本应回落下限 64KiB\n{report}"
    );

    // 单次分配区域：样本数不足也给出建议（>= 下限）
    let session = advisor.recommend_size("session").unwrap();
    assert!(session >= 64 * 1024);
}

#[test]
fn report_file_roundtrip() {
    let path = temp_path("roundtrip.rl_profile");
    let json = serde_json::to_string(&sample_data()).unwrap();
    std::fs::write(&path, &json).unwrap();

    let report = region_profile_report(&path).unwrap();
    assert!(report.contains("big_buf"));
    assert!(report.contains("PGO data version:"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn invalid_json_is_profile_error() {
    let path = temp_path("invalid.rl_profile");
    std::fs::write(&path, "not valid json {").unwrap();

    let err = region_profile_report(&path).unwrap_err();
    assert!(
        err.to_string().contains("[profile]"),
        "应报 [profile] 错误: {err}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn missing_file_is_io_error() {
    let path = temp_path("nope.rl_profile");
    let err = region_profile_report(&path).unwrap_err();
    assert!(err.to_string().contains("I/O"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn empty_data_reports_no_regions() {
    let data = ProfileCollector::new().generate();
    let report = build_region_report(&data);
    assert!(report.contains("PGO regions: 0"));
    assert!(report.contains("(none registered)"));
}
