//! 智能区域基准：分配吞吐与扩容开销。

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use zeta_region_alloc::{SizeAdvisor, SmartRegion};

/// 1000 次 u64 分配的吞吐（含一次扩容）。
fn bench_smart_region(c: &mut Criterion) {
    let mut group = c.benchmark_group("smart_region");
    group.bench_function("allocate_1000_u64", |b| {
        let advisor = SizeAdvisor::new("bench".into());
        let mut region = SmartRegion::new("bench".into(), advisor).unwrap();
        b.iter(|| {
            for i in 0..1000u64 {
                black_box(region.allocate(i).unwrap());
            }
        });
        // SAFETY: 基准中不再引用区域对象
        unsafe {
            region.destroy();
        }
    });
    group.finish();
}

criterion_group!(benches, bench_smart_region);
criterion_main!(benches);
