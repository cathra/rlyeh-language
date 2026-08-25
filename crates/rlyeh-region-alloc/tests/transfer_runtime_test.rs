//! P005 Transfer 运行时集成测试：
//! `execute_transfer` 所有权句柄、`is_transferred` 状态查询、
//! 区域销毁时对已 transfer 对象跳过析构（无 double free）。

use std::cell::Cell;
use std::ptr::NonNull;
use std::rc::Rc;

use zeta_region_alloc::Region;

/// 记录析构次数的类型。
struct Tracked(Rc<Cell<usize>>);

impl Drop for Tracked {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

/// 在区域中分配一个 `T`，返回其 `NonNull<T>`（结束借用后供 transfer 使用）。
fn alloc_ptr<T>(region: &mut Region, value: T) -> NonNull<T> {
    let ptr = {
        let obj = region.allocate(value).unwrap();
        NonNull::from(obj)
    };
    ptr
}

#[test]
fn test_execute_transfer_returns_owned_handle() {
    let mut region = Region::with_initial_size(1024);
    let ptr = alloc_ptr(&mut region, 42u64);

    assert!(!region.is_transferred(ptr.cast::<u8>()));

    // transfer：所有权移出区域，返回 'static 句柄
    let owned: &'static mut u64 = region.execute_transfer(ptr);
    assert_eq!(*owned, 42);

    // 状态查询：已标记为 transferred
    assert!(region.is_transferred(ptr.cast::<u8>()));
    assert_eq!(region.transferred_count(), 1);

    // 句柄可继续读写
    *owned = 100;
    assert_eq!(*owned, 100);

    // 接收方在区域销毁前把值带出区域（transfer 的真实用途：所有权移交）
    let taken = *owned;

    // SAFETY: 测试中 region 不再被引用
    unsafe {
        region.destroy();
    }
    // transferred 对象不参与区域析构（bump 区域整体释放，句柄随之失效）
    assert_eq!(region.stats.destructor_calls, 0);
    assert_eq!(taken, 100);
}

#[test]
fn test_transferred_objects_skip_destructor() {
    let drop_count = Rc::new(Cell::new(0));
    let mut region = Region::with_initial_size(4096);

    let mut ptrs = Vec::new();
    for _ in 0..10 {
        ptrs.push(alloc_ptr(&mut region, Tracked(drop_count.clone())));
    }

    // transfer 前 5 个对象
    for p in ptrs.iter().take(5) {
        let _owned: &'static mut Tracked = region.execute_transfer(*p);
    }

    assert_eq!(drop_count.get(), 0);
    assert_eq!(region.transferred_count(), 5);

    // SAFETY: 测试中 region 不再被引用；transferred 对象由 _owned 持有
    unsafe {
        region.destroy();
    }
    // 只有未 transfer 的 5 个对象被析构，无 double free
    assert_eq!(drop_count.get(), 5);
    assert_eq!(region.stats.destructor_calls, 5);
}

#[test]
fn test_transfer_then_query_state() {
    let mut region = Region::with_initial_size(1024);
    let a = alloc_ptr(&mut region, 1u32);
    let b = alloc_ptr(&mut region, 2u32);

    region.mark_transferred(a.cast::<u8>());
    assert!(region.is_transferred(a.cast::<u8>()));
    assert!(!region.is_transferred(b.cast::<u8>()));
    assert_eq!(region.transferred_count(), 1);

    let _owned: &'static mut u32 = region.execute_transfer(b);
    assert!(region.is_transferred(b.cast::<u8>()));
    assert_eq!(region.transferred_count(), 2);

    // SAFETY: 测试中 region 不再被引用
    unsafe {
        region.destroy();
    }
    // 两个对象均无析构需求（u32），destroy 不 panic 即通过
    assert_eq!(region.stats.destructor_calls, 0);
}
