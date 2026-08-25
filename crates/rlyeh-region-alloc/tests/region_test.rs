//! rlyeh-region-alloc 集成测试：分配、扩容策略、析构管理、transfer。

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;
use std::rc::Rc;

use rlyeh_region_alloc::{AllocError, GrowthStrategy, Region};

/// 记录析构次数的类型。
struct Tracked(Rc<Cell<usize>>);

impl Drop for Tracked {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn test_basic_allocation() {
    let mut region = Region::with_initial_size(32);
    region.allocate(10u64).unwrap();
    region.allocate(20u64).unwrap();

    assert_eq!(region.capacity(), 32);
    assert_eq!(region.used(), 16);
    assert_eq!(region.remaining(), 16);
    assert_eq!(region.block_count(), 1);
    assert_eq!(region.stats.allocation_count, 2);
    assert_eq!(region.stats.total_allocated, 16);
}

#[test]
fn test_bump_address_ordering() {
    // 同一块内 bump 分配地址递增
    let mut region = Region::with_initial_size(1024);
    let a = region.allocate(1u8).unwrap() as *const u8;
    let b = region.allocate(1u8).unwrap() as *const u8;
    let c = region.allocate(1u8).unwrap() as *const u8;
    assert!(a < b && b < c);
    assert_eq!(region.block_count(), 1);
}

#[test]
fn test_destructor_called_on_destroy() {
    let drop_count = Rc::new(Cell::new(0));
    let mut region = Region::with_initial_size(1024);
    for _ in 0..5 {
        region.allocate(Tracked(drop_count.clone())).unwrap();
    }
    // 未销毁前不执行析构
    assert_eq!(drop_count.get(), 0);

    // SAFETY: 测试中 region 不再被引用
    unsafe {
        region.destroy();
    }
    assert_eq!(drop_count.get(), 5);
    assert_eq!(region.stats.destructor_calls, 5);
}

#[test]
fn test_region_drop_auto_destroy() {
    // 作用域结束自动执行析构
    let drop_count = Rc::new(Cell::new(0));
    {
        let mut region = Region::with_initial_size(1024);
        region.allocate(Tracked(drop_count.clone())).unwrap();
        region.allocate(Tracked(drop_count.clone())).unwrap();
    }
    assert_eq!(drop_count.get(), 2);
}

#[test]
fn test_lifo_destructor_order() {
    // 后分配的先析构
    struct Labeled(Rc<RefCell<Vec<usize>>>, usize);
    impl Drop for Labeled {
        fn drop(&mut self) {
            self.0.borrow_mut().push(self.1);
        }
    }

    let order = Rc::new(RefCell::new(Vec::new()));
    let mut region = Region::with_initial_size(1024);
    region.allocate(Labeled(order.clone(), 1)).unwrap();
    region.allocate(Labeled(order.clone(), 2)).unwrap();
    region.allocate(Labeled(order.clone(), 3)).unwrap();

    // SAFETY: 测试中 region 不再被引用
    unsafe {
        region.destroy();
    }
    assert_eq!(order.borrow().as_slice(), &[3, 2, 1]);
}

#[test]
fn test_transfer_mark() {
    let drop_count = Rc::new(Cell::new(0));
    let mut region = Region::with_initial_size(1024);
    let obj = region.allocate(Tracked(drop_count.clone())).unwrap();
    let ptr = NonNull::from(obj).cast::<u8>();

    // transfer：对象所有权移出，区域销毁时不再调用其析构
    region.mark_transferred(ptr);
    assert_eq!(region.transferred_count(), 1);

    // SAFETY: 测试中 region 不再被引用；transferred 对象由外部持有
    unsafe {
        region.destroy();
    }
    assert_eq!(drop_count.get(), 0);
    assert_eq!(region.stats.destructor_calls, 0);
}

#[test]
fn test_growth_multiply() {
    let mut region = Region::new();
    region.strategy = GrowthStrategy::Multiply { factor: 2.0 };
    region.allocate(10u64).unwrap(); // 8B，填满初始 8B 块
    region.allocate(20u64).unwrap(); // 触发扩容：8 × 2 = 16B 新块

    assert_eq!(region.block_count(), 2);
    assert_eq!(region.capacity(), 8 + 16);
    assert_eq!(region.stats.growth_count, 1);
}

#[test]
fn test_growth_linear() {
    let mut region = Region::new();
    region.strategy = GrowthStrategy::Linear { chunk: 16 };
    region.allocate(10u64).unwrap();
    region.allocate(20u64).unwrap();

    assert_eq!(region.block_count(), 2);
    assert_eq!(region.capacity(), 8 + 24);
    assert_eq!(region.stats.growth_count, 1);
}

#[test]
fn test_growth_exact_no_growth() {
    let mut region = Region::with_initial_size(8);
    region.strategy = GrowthStrategy::Exact;
    region.allocate(10u64).unwrap(); // 填满

    let result = region.allocate(20u64);
    assert!(matches!(result, Err(AllocError::OutOfMemory { .. })));
    assert_eq!(region.capacity(), 8);
    assert_eq!(region.stats.growth_count, 0);
}

#[test]
fn test_growth_adaptive() {
    let mut region = Region::new();
    region.strategy = GrowthStrategy::Adaptive {
        adaptive_samples: Vec::new(),
        max_capacity: 1024,
    };
    if let GrowthStrategy::Adaptive {
        adaptive_samples, ..
    } = &mut region.strategy
    {
        for &s in &[100, 200, 150, 300] {
            adaptive_samples.push(s);
        }
    }

    let next = region
        .strategy
        .calculate_next_size(region.capacity(), 500, &region.stats);
    assert!(next >= 500, "next({next}) must satisfy the request");
    assert!(next <= 600, "next({next}) should track EWMA prediction");
}

#[test]
fn test_allocate_after_destroy() {
    let mut region = Region::with_initial_size(32);
    // SAFETY: 测试中 region 不再被引用
    unsafe {
        region.destroy();
    }
    let result = region.allocate(1u8);
    assert_eq!(result.unwrap_err(), AllocError::Destroyed);
}
