//! 区域（Region）：bump 分配 + 析构管理 + 扩容策略 + 统计。
//!
//! # 内联快路径（LLVM 后端接线）
//!
//! [`Region`] 以 `#[repr(C)]` 声明，首部为 **6 个固定偏移的快路径字段**
//! （`base`/`cursor`/`limit` + 3 个快路径统计）。LLVM 后端
//! （`zeta-codegen`）对 `in 'r` 聚合分配**内联生成 bump 快路径**：
//!
//! ```text
//! aligned = (cursor + align - 1) & !(align - 1)
//! new     = aligned + size
//! ok      = new <= limit            ; 越界才调用运行时（慢路径）
//! ```
//!
//! 与 [`Region::try_bump`] 的公式逐位一致；块基址由 `std::alloc` 保证
//! 8 字节对齐，且 `align` 恒为 2 的幂，因此相对偏移位运算与
//! `checked_next_multiple_of` 等价。**修改快路径字段顺序或语义时，
//! 必须同步更新 `zeta-codegen/src/llvm.rs` 的 `AllocInRegion` 内联序列。**

use std::mem::{align_of, size_of};
use std::ptr::{self, NonNull};

use crate::block::BASE_ALIGN;
use crate::bump::BumpAllocator;
use crate::destructor::DestructorRegistry;
use crate::error::AllocError;
use crate::stats::RegionStats;
use crate::strategy::GrowthStrategy;

/// 默认初始块大小（字节）。
const DEFAULT_BLOCK_SIZE: usize = 8;

/// 内联快路径字段的固定字节偏移（LLVM 后端按此 GEP 访问）。
///
/// - `+0`  `base`：当前块基址；
/// - `+8`  `cursor`：当前块已用偏移（相对 `base`）；
/// - `+16` `limit`：当前块容量；
/// - `+24` `alloc_count`：慢路径（扩容后分配）与 Rust API 累计次数——
///   内联快路径为换取热路径性能**不计数**（codegen 仅读写 +0/+8/+16）；
/// - `+32` `alloc_bytes`：慢路径 / Rust API 累计分配字节（热路径不维护）；
/// - `+40` `waste_bytes`：慢路径 / Rust API 累计对齐损失（热路径不维护）。
#[allow(dead_code)]
const FAST_BASE_OFF: usize = 0;
#[allow(dead_code)]
const FAST_CURSOR_OFF: usize = 8;
#[allow(dead_code)]
const FAST_LIMIT_OFF: usize = 16;
#[allow(dead_code)]
const FAST_ALLOC_COUNT_OFF: usize = 24;
#[allow(dead_code)]
const FAST_ALLOC_BYTES_OFF: usize = 32;
#[allow(dead_code)]
const FAST_WASTE_BYTES_OFF: usize = 40;

/// L1 区域：所有对象在同一块（组）内存中 bump 分配，随区域整体销毁。
///
/// 默认扩容策略为固定倍率（×2）；可通过 [`Region::strategy`] 调整。
/// 区域内的析构顺序为**后分配先析构**（LIFO）。
///
/// # 布局契约
///
/// `#[repr(C)]` 保证首部 6 个快路径字段顺序与偏移固定（见 [`FAST_BASE_OFF`]
/// 系列常量）。LLVM 后端内联 bump 依赖该布局；新增/调整首部字段前须评估
/// `zeta-codegen/src/llvm.rs` 的对应 GEP 偏移。
#[repr(C)]
pub struct Region {
    // ── 内联快路径字段（codegen 按固定偏移访问；勿改顺序）──
    base: *mut u8,
    cursor: usize,
    limit: usize,
    alloc_count: usize,
    alloc_bytes: usize,
    waste_bytes: usize,
    // ── 慢路径字段 ──
    /// 区域名（调试 / 统计用）。
    pub name: Option<String>,
    /// 扩容策略。
    pub strategy: GrowthStrategy,
    /// 分配统计（Rust API `allocate<T>` 路径更新；C ABI / 内联快路径
    /// 更新首部快路径字段，读取时经 [`Region::total_allocs`] 合并）。
    pub stats: RegionStats,
    allocator: BumpAllocator,
    destructors: DestructorRegistry,
    transferred: Vec<NonNull<u8>>,
    destroyed: bool,
}

impl Region {
    /// 以默认初始大小（8 字节）创建区域。
    pub fn new() -> Self {
        Self::with_initial_size(DEFAULT_BLOCK_SIZE)
    }

    /// 指定初始块大小创建区域。
    pub fn with_initial_size(size: usize) -> Self {
        let allocator = BumpAllocator::new(size).expect("initial region block allocation");
        let (base, limit) = allocator.current_block();
        Self {
            base,
            cursor: 0,
            limit,
            alloc_count: 0,
            alloc_bytes: 0,
            waste_bytes: 0,
            name: None,
            strategy: GrowthStrategy::Multiply { factor: 2.0 },
            stats: RegionStats::default(),
            allocator,
            destructors: DestructorRegistry::new(),
            transferred: Vec::new(),
            destroyed: false,
        }
    }

    /// 设置区域名并返回自身（链式）。
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 在区域内 bump 分配 `size` 字节、对齐 `align`（内联快路径同款公式）。
    ///
    /// 成功返回 `(指针, 对齐损失)`；当前块剩余空间不足返回 `None`。
    /// `align` 须为 2 的幂（codegen 恒传 8；Rust 对齐均为 1/2/4/8/16 幂）。
    #[inline(always)]
    fn try_bump(&mut self, size: usize, align: usize) -> Option<(NonNull<u8>, usize)> {
        let a = align.max(BASE_ALIGN);
        let (aligned, wasted) = if a.is_power_of_two() {
            let off = self.cursor.checked_add(a - 1)?;
            let aligned = off & !(a - 1);
            (aligned, aligned - self.cursor)
        } else {
            let aligned = self.cursor.checked_next_multiple_of(a)?;
            (aligned, aligned - self.cursor)
        };
        let end = aligned.checked_add(size)?;
        if end > self.limit {
            return None;
        }
        self.cursor = end;
        // SAFETY: end <= limit，指针位于当前块内（base 由 std::alloc 返回非空）。
        let ptr = unsafe { NonNull::new_unchecked((self.base as usize + aligned) as *mut u8) };
        Some((ptr, wasted))
    }

    /// 扩容：按策略分配新块并切换到当前块，同步快路径字段。
    fn grow(&mut self, requested: usize) -> Result<(), AllocError> {
        let next = self
            .strategy
            .calculate_next_size(self.capacity(), requested, &self.stats);
        self.allocator.grow(next)?;
        self.stats.growth_count += 1;
        let (base, limit) = self.allocator.current_block();
        self.base = base;
        self.limit = limit;
        self.cursor = 0;
        Ok(())
    }

    /// 在区域内分配一个 `T` 值，返回其可变引用。
    ///
    /// 空间不足时按 [`Region::strategy`] 扩容；`Exact` 模式或扩容失败
    /// 返回 [`AllocError::OutOfMemory`]。
    pub fn allocate<T>(&mut self, value: T) -> Result<&mut T, AllocError> {
        if self.destroyed {
            return Err(AllocError::Destroyed);
        }
        let size = size_of::<T>();
        let align = align_of::<T>();

        let (ptr, wasted) = match self.try_bump(size, align) {
            Some(ok) => ok,
            None => {
                if !self.allows_growth() {
                    return Err(AllocError::OutOfMemory { requested: size });
                }
                self.grow(size)?;
                self.try_bump(size, align).ok_or(AllocError::OutOfMemory {
                    requested: size,
                })?
            }
        };

        // SAFETY: ptr 指向区域内空闲内存，写入后由析构注册表接管。
        unsafe {
            ptr::write(ptr.as_ptr().cast::<T>(), value);
        }
        self.destructors.register::<T>(ptr);
        self.stats.allocation_count += 1;
        self.stats.total_allocated += size;
        self.stats.total_wasted += wasted;

        // SAFETY: ptr 已写入合法 T，借用期与 &mut self 一致。
        Ok(unsafe { &mut *ptr.as_ptr().cast::<T>() })
    }

    /// 在区域内分配一块裸内存（不写入值、不注册析构），供 C ABI 接线使用。
    ///
    /// 空间不足时按 [`Region::strategy`] 扩容；`Exact` 模式或扩容失败返回 `None`。
    /// 返回的指针指向区域内内存，随区域销毁整体释放。
    ///
    /// 这是 LLVM 内联 bump 快路径的**慢路径**（越界分支调用）：调用时
    /// `cursor` 尚未被内联代码修改，此处重新尝试分配必然因越界进入扩容。
    pub fn allocate_bytes(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        if self.destroyed || size == 0 {
            return None;
        }
        let (ptr, wasted) = match self.try_bump(size, align.max(1)) {
            Some(ok) => ok,
            None => {
                if !self.allows_growth() {
                    return None;
                }
                self.grow(size).ok()?;
                self.try_bump(size, align.max(1))?
            }
        };
        self.alloc_count += 1;
        self.alloc_bytes += size;
        self.waste_bytes += wasted;
        Some(ptr)
    }

    /// 合并分配次数（Rust API 统计 + C ABI 慢路径统计）。
    ///
    /// 注意：LLVM 内联 bump 快路径（`in 'r` 字面量直接构造）分配**不计数**
    /// ——这是换取热路径性能的有意取舍，`total_allocs` 可能低于实际分配数。
    pub fn total_allocs(&self) -> usize {
        self.stats.allocation_count + self.alloc_count
    }

    /// 慢路径 / Rust API 累计分配字节（内联快路径不维护，见 `FAST_*_OFF` 注释）。
    pub fn fast_alloc_bytes(&self) -> usize {
        self.alloc_bytes
    }

    /// 慢路径 / Rust API 累计对齐损失（内联快路径不维护）。
    pub fn fast_waste_bytes(&self) -> usize {
        self.waste_bytes
    }

    /// 当前总容量（所有块之和）。
    pub fn capacity(&self) -> usize {
        self.allocator.capacity()
    }

    /// 已使用字节数（旧块全满 + 当前块已用）。
    pub fn used(&self) -> usize {
        // 旧块必然已满（bump 只在当前块耗尽时扩容），因此
        // 已用 = 总容量 - 当前块剩余 + 当前块已用。
        self.capacity() - self.limit + self.cursor
    }

    /// 剩余字节数。
    pub fn remaining(&self) -> usize {
        self.limit - self.cursor
    }

    /// 内存块数量（含扩容产生的块）。
    pub fn block_count(&self) -> usize {
        self.allocator.block_count()
    }

    /// 已转移对象数量。
    pub fn transferred_count(&self) -> usize {
        self.transferred.len()
    }

    /// 标记 `ptr` 指向的对象已 `transfer` 出区域：
    /// 从析构列表中移除（区域销毁时不再调用其析构），所有权交由外部管理。
    pub fn mark_transferred(&mut self, ptr: NonNull<u8>) {
        self.destructors.remove(ptr);
        self.transferred.push(ptr);
    }

    /// 查询 `ptr` 指向的对象是否已 `transfer` 出区域。
    pub fn is_transferred(&self, ptr: NonNull<u8>) -> bool {
        self.transferred.contains(&ptr)
    }

    /// 执行一次完整的 `transfer`（ADR-003）：
    ///
    /// 1. 从区域的析构列表中移除 `ptr` 指向的对象；
    /// 2. 标记该位置为"已迁出"（区域销毁时跳过其析构）；
    /// 3. 返回所有权句柄 `&'static mut T`，由接收方负责其生命周期与析构。
    ///
    /// 注意：对象仍位于区域的内存块中（零拷贝，ADR-003）。bump 区域销毁时
    /// 整体释放全部块，因此**接收方必须在 [`Region::destroy`] 之前**读取、
    /// 修改或把值 move 出区域，销毁后句柄失效。
    ///
    /// # Safety
    /// 调用者必须保证：
    /// - `ptr` 指向区域内一个已初始化的 `T`（由 [`Region::allocate`] 返回）；
    /// - transfer 后，区域代码不再通过区域内引用访问该对象（所有权已移交）；
    /// - 接收方在区域销毁前处理完对象（区域不再调用其析构，也失去其内存）。
    pub fn execute_transfer<T>(&mut self, ptr: NonNull<T>) -> &'static mut T {
        let ptr_u8 = ptr.cast::<u8>();
        self.mark_transferred(ptr_u8);
        // SAFETY: 调用者按上述约定保证 ptr 指向合法 T；`&'static` 表示所有权
        // 已从区域移交，生命周期由接收方决定。
        unsafe { &mut *ptr.as_ptr() }
    }

    /// 销毁区域：逆序执行析构（跳过已 `transfer` 的对象）并释放全部内存块。
    ///
    /// # Safety
    /// 调用者必须保证不存在指向区域内对象的外部引用（借用检查器无法静态验证）；
    /// 已 `transfer` 的对象由外部持有者负责，须在销毁前处理完毕。
    pub unsafe fn destroy(&mut self) {
        if self.destroyed {
            return;
        }
        self.stats.destructor_calls += self.destructors.run_all();
        self.allocator.deallocate_all();
        self.transferred.clear();
        self.destroyed = true;
    }

    fn allows_growth(&self) -> bool {
        !matches!(self.strategy, GrowthStrategy::Exact)
    }
}

impl Default for Region {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Region {
    fn drop(&mut self) {
        // SAFETY: Drop 时对象引用必然已失效，可安全销毁。
        unsafe {
            self.destroy();
        }
    }
}
