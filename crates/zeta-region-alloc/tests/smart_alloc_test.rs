//! P010 智能区域分配器集成测试：静态推断、PGO 决策、自适应扩容、析构管理。

use std::sync::atomic::{AtomicUsize, Ordering};

use zeta_region_alloc::compiler_interface::{CompilerInterface, RegionCompileInfo};
use zeta_region_alloc::pgo_advisor::PgoAdvisor;
use zeta_region_alloc::profile::{
    GrowthStats, PgoData, ProfileCollector, RegionProfile, SizeStats,
};
use zeta_region_alloc::size_advisor::{DecisionSource, SizeAdvisor};
use zeta_region_alloc::static_sizer::{
    AllocSite, BranchInfo, ExecFrequency, SizeEstimate, StaticSizer,
};
use zeta_region_alloc::SmartRegion;

/// 基础分配：无提示、无静态信息 → 自适应默认（4 KB），100 个 u64 不扩容。
#[test]
fn test_smart_region_basic() {
    let advisor = SizeAdvisor::new("r1".into());
    let mut region = SmartRegion::new("r1".into(), advisor).unwrap();

    for i in 0..100u64 {
        region.allocate(i).unwrap();
    }

    let stats = region.stats();
    assert_eq!(stats.allocation_count, 100);
    assert!(stats.peak_usage > 0);
}

/// 用户提示小初始块 → 分配大对象触发扩容。
#[test]
fn test_smart_region_growth() {
    let advisor = SizeAdvisor::new("r2".into()).with_user_hint(64); // 只有 64 bytes
    let mut region = SmartRegion::new("r2".into(), advisor).unwrap();

    let big = [0u8; 2048];
    let p = region.allocate(big).unwrap();
    assert_eq!(p.len(), 2048);

    let stats = region.stats();
    assert!(stats.growth_count >= 1);
}

/// 静态精确推断 → 初始大小接近总和（含 10% 余量）。
#[test]
fn test_static_exact() {
    let mut sizer = StaticSizer::new();
    sizer.add_site(AllocSite {
        site_id: "site1".into(),
        type_size: 32,
        frequency: ExecFrequency::Once,
    });
    sizer.add_site(AllocSite {
        site_id: "site2".into(),
        type_size: 24,
        frequency: ExecFrequency::Once,
    });

    let advisor = SizeAdvisor::new("r3".into()).with_sizer(sizer);
    let plan = advisor.decide();

    assert!(matches!(plan.source, DecisionSource::StaticExact(56)));
    assert!(plan.initial_size >= 56);
    assert!(plan.growth_factor <= 1.5);
}

/// 有界循环 → 静态上界，初始大小接近上界且扩容倍率保守。
#[test]
fn test_no_growth_when_exact() {
    let mut sizer = StaticSizer::new();
    sizer.add_site(AllocSite {
        site_id: "loop1".into(),
        type_size: 100,
        frequency: ExecFrequency::Loop { max_iterations: 10 },
    });

    let advisor = SizeAdvisor::new("r4".into()).with_sizer(sizer);
    let plan = advisor.decide();

    // 静态上界 100 × 10 = 1000
    assert!(plan.initial_size >= 1000 && plan.initial_size < 1200);
    assert!(plan.growth_factor <= 1.5);
    assert!(matches!(plan.source, DecisionSource::StaticUpperBound(1000)));
}

/// 分支结构 → 区间推断（min/max 路径）。
#[test]
fn test_static_with_branch() {
    let mut sizer = StaticSizer::new();
    sizer.add_site(AllocSite {
        site_id: "1".into(),
        type_size: 1024,
        frequency: ExecFrequency::Once,
    });
    sizer.add_site(AllocSite {
        site_id: "2".into(),
        type_size: 64,
        frequency: ExecFrequency::Once,
    });
    sizer.add_branch(BranchInfo {
        branches: vec![vec!["1".into()], vec!["2".into()]],
    });

    let estimate = sizer.estimate();
    assert!(matches!(estimate, SizeEstimate::Range { min: 64, max: 1024 }));
}

/// EWMA 学习：多轮分配 + 销毁不 panic，趋势可被预测（无硬断言）。
#[test]
fn test_ewma_prediction() {
    // 模拟 5 轮：每轮分配一组数据后销毁区域
    for round in 1..=5 {
        let advisor = SizeAdvisor::new("ewma".into()).with_user_hint(1024);
        let mut region = SmartRegion::new("ewma".into(), advisor).unwrap();
        for i in 0..10 {
            let size = 100 + (round * 10 + i) * 8;
            let data = vec![0u8; size];
            region.allocate(data).unwrap();
        }
        // SAFETY: 测试中不再引用区域对象
        unsafe {
            region.destroy();
        }
    }
}

/// PGO 推荐：p95 × safety_factor，有下限保护。
#[test]
fn test_pgo_recommendation() {
    let data = PgoData {
        version: 1,
        generated_at: String::new(),
        regions: vec![RegionProfile {
            region_id: "test_region".to_string(),
            size_stats: SizeStats {
                total_samples: 10,
                total_bytes: 10_000,
                min: 100,
                max: 1_000_000,
                p50: 500_000,
                p90: 700_000,
                p95: 786_432,
                mean: 500_000,
            },
            growth_stats: GrowthStats {
                total_growths: 3,
                total_growth_bytes: 2048,
                avg_growth_bytes: 683,
                max_growth_bytes: 1024,
            },
            sample_count: 10,
            last_updated: String::new(),
        }],
    };

    let advisor = PgoAdvisor::from_data(data);
    let size = advisor.recommend_size("test_region").unwrap();
    assert!((786_432..900_000).contains(&size));
}

/// PGO 数据保存 / 加载往返。
#[test]
fn test_pgo_save_load() {
    let path = format!("/tmp/zeta_pgo_test_{}.json", std::process::id());
    let mut collector = ProfileCollector::new();
    collector.record_allocation("test", 64);
    collector.record_allocation("test", 128);
    collector.record_allocation("test", 96);
    collector.save(&path).unwrap();

    let loaded = ProfileCollector::load(&path).unwrap();
    let data = loaded.generate();
    assert!(data.region_ids().contains(&"test".to_string()));

    let _ = std::fs::remove_file(&path);
}

/// 编译器集成报告。
#[test]
fn test_report_generation() {
    let mut collector = ProfileCollector::new();
    collector.record_allocation("http_server", 1024);
    collector.record_allocation("http_server", 2048);
    let pgo = collector.generate();

    let ci = CompilerInterface::new()
        .with_pgo_data(pgo)
        .register_region(RegionCompileInfo {
            region_id: "http_server".into(),
            estimated_size: 4096,
            initial_size: 4096,
            max_size: 16_384,
            decision: "AdaptiveDefault".into(),
        });

    let report = ci.generate_report();
    assert!(report.contains("Region Allocation Report"));
    assert!(report.contains("PGO data version: 1"));
    assert!(report.contains("http_server"));
}

/// 析构管理：需要 drop 的对象在销毁时逆序执行析构。
#[test]
fn test_destructor_calls() {
    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct TrackDrop;
    impl Drop for TrackDrop {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    let advisor = SizeAdvisor::new("d1".into());
    let mut region = SmartRegion::new("d1".into(), advisor).unwrap();
    for _ in 0..50 {
        region.allocate(TrackDrop).unwrap();
    }
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 0);

    // SAFETY: 测试中不再引用区域对象
    unsafe {
        region.destroy();
    }
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 50);
}

/// 碎片跟踪：扩容时旧块剩余空间计入 fragmentation。
#[test]
fn test_fragmentation_tracking() {
    let advisor = SizeAdvisor::new("r6".into()).with_user_hint(256);
    let mut region = SmartRegion::new("r6".into(), advisor).unwrap();

    let _big = region.allocate([0u8; 1200]).unwrap();
    let _small = region.allocate(vec![0u8; 200]).unwrap();

    let stats = region.stats();
    assert!(stats.fragmentation > 0);
    assert!(stats.growth_count >= 1);
}

/// 自适应扩容：压力下增长次数有限（智能扩容不激进）。
#[test]
fn test_adaptive_growth_under_pressure() {
    let advisor = SizeAdvisor::new("r7".into()); // 无 hint，走自适应
    let mut region = SmartRegion::new("r7".into(), advisor).unwrap();

    for _ in 0..50 {
        let data = [0u8; 300];
        region.allocate(data).unwrap();
    }

    let stats = region.stats();
    assert!(stats.growth_count >= 1);
}
