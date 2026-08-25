//! C ABI 导出层（L3 region 选项接线）。
//!
//! LLVM 后端生成的代码经 `rlyeh_region_*` 函数操作运行时区域：
//!
//! - [`rlyeh_region_enter`]：按编译器传入的选项（初始大小 / 扩容策略 / 自适应 / 精确）
//!   创建区域，返回不透明句柄；
//! - [`rlyeh_region_alloc`]：**慢路径**分配——LLVM 后端对 `in 'r` 分配内联生成
//!   bump 快路径（直接读写 `Region` 首部固定偏移字段），仅在当前块空间不足
//!   （越界）时才调用本函数扩容并分配；
//! - [`rlyeh_region_transfer`]：登记对象已 `transfer` 出区域（销毁时跳过其析构）；
//! - [`rlyeh_region_exit`]：批量释放区域全部内存块并销毁句柄。
//!
//! 内存策略：区域内部使用 `std::alloc`（`MemoryBlock` 为 `std::alloc::alloc` +
//! `dealloc`，与 rlyeh-gc-runtime 验证过的组合一致），本层仅做 FFI 边界转换，
//! 不在 C 堆上自行分配。

use std::ffi::c_void;
use std::ptr::{self, NonNull};

use crate::{GrowthStrategy, Region};

/// 策略编码：`strategy (bump)`（显式 bump，等价默认倍率扩容）。
#[allow(dead_code)] // MVP：bump 即默认策略，编码保留供未来策略区分
pub const REGION_STRATEGY_BUMP: u8 = 1;

/// 创建区域，返回不透明句柄（`Box::into_raw`）。
///
/// 参数：
/// - `name_ptr` / `name_len`：区域名（可为空指针）；
/// - `initial_size`：初始块大小（字节，至少 1）；
/// - `allow_growth`：是否允许扩容（0/1）；
/// - `growth_factor`：倍率扩容因子；
/// - `exact`：精确模式（1 = 固定初始大小、不允许扩容）；
/// - `adaptive`：自适应模式（1 = EWMA 预测扩容）；
/// - `strategy`：策略编码（MVP 仅 `REGION_STRATEGY_BUMP`，bump 即默认倍率策略）。
#[no_mangle]
pub extern "C" fn rlyeh_region_enter(
    name_ptr: *const u8,
    name_len: usize,
    initial_size: usize,
    allow_growth: u8,
    growth_factor: f64,
    exact: u8,
    adaptive: u8,
    strategy: u8,
) -> *mut c_void {
    let name = if name_ptr.is_null() {
        String::new()
    } else {
        // SAFETY: 编译器保证 name_ptr 指向 name_len 字节的有效内存。
        let bytes = unsafe { std::slice::from_raw_parts(name_ptr, name_len) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    let growth = if exact != 0 {
        GrowthStrategy::Exact
    } else if adaptive != 0 {
        GrowthStrategy::Adaptive {
            adaptive_samples: Vec::new(),
            max_capacity: 0,
        }
    } else if allow_growth == 0 {
        GrowthStrategy::Exact
    } else {
        GrowthStrategy::Multiply {
            factor: if growth_factor.is_finite() && growth_factor > 0.0 {
                growth_factor
            } else {
                2.0
            },
        }
    };
    let _ = strategy; // MVP：bump 即默认倍率策略，无需区分
    let mut region = Region::with_initial_size(initial_size.max(1)).named(name);
    region.strategy = growth;
    Box::into_raw(Box::new(region)) as *mut c_void
}

/// 在区域内 bump 分配 `size` 字节（`align` 对齐），返回区域内存指针；
/// 失败（精确模式容量不足 / 扩容超限 / 区域已销毁）返回空指针。
///
/// **慢路径**：被 LLVM 内联快路径的越界分支调用。调用时 `cursor` 尚未被
/// 内联代码修改，`allocate_bytes` 会重新尝试（必然越界）并扩容后分配。
#[no_mangle]
pub extern "C" fn rlyeh_region_alloc(handle: *mut c_void, size: usize, align: usize) -> *mut u8 {
    if handle.is_null() || size == 0 {
        return ptr::null_mut();
    }
    // SAFETY: handle 由 rlyeh_region_enter 返回，有效期至 rlyeh_region_exit。
    let region = unsafe { &mut *(handle as *mut Region) };
    match region.allocate_bytes(size, align.max(1)) {
        Some(p) => p.as_ptr(),
        None => ptr::null_mut(),
    }
}

/// 登记 `ptr` 指向的对象已 `transfer` 出区域：
/// 从析构列表中移除，区域销毁时不再处理（MVP 下镜像不注册析构，仅作登记）。
#[no_mangle]
pub extern "C" fn rlyeh_region_transfer(handle: *mut c_void, ptr: *mut u8) {
    if handle.is_null() || ptr.is_null() {
        return;
    }
    // SAFETY: handle 由 rlyeh_region_enter 返回；ptr 指向区域内对象（或编译器镜像）。
    let region = unsafe { &mut *(handle as *mut Region) };
    // SAFETY: ptr 非空。
    region.mark_transferred(unsafe { NonNull::new_unchecked(ptr) });
}

/// 销毁区域：逆序执行析构（跳过已 transfer 的对象）并释放全部内存块。
#[no_mangle]
pub extern "C" fn rlyeh_region_exit(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    // SAFETY: handle 由 rlyeh_region_enter 返回，仅销毁一次。
    unsafe {
        drop(Box::from_raw(handle as *mut Region));
    }
}

/// 返回区域统计快照的部分计数（调试 / 测试用）：
/// 低 32 位 = 分配次数，高 32 位 = 内存块数。
#[no_mangle]
pub extern "C" fn rlyeh_region_stats(handle: *mut c_void) -> u64 {
    if handle.is_null() {
        return 0;
    }
    // SAFETY: handle 由 rlyeh_region_enter 返回，区域存活期间有效。
    let region = unsafe { &*(handle as *mut Region) };
    // 合并 Rust API（stats）与 C ABI / 内联快路径（alloc_count）两条分配路径。
    let allocs = region.total_allocs() as u64 & 0xFFFF_FFFF;
    let blocks = region.block_count() as u64 & 0xFFFF_FFFF;
    (blocks << 32) | allocs
}
