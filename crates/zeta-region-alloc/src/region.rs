//! 区域（Region）：bump 分配 + 析构管理 + 扩容策略 + 统计。

use std::mem::{align_of, size_of};
use std::ptr::{self, NonNull};

use crate::bump::BumpAllocator;
use crate::destructor::DestructorRegistry;
use crate::error::AllocError;
use crate::stats::RegionStats;
use crate::strategy::GrowthStrategy;

/// 默认初始块大小（字节）。
const DEFAULT_BLOCK_SIZE: usize = 8;

/// L1 区域：所有对象在同一块（组）内存中 bump 分配，随区域整体销毁。
///
/// 默认扩容策略为固定倍率（×2）；可通过 [`Region::strategy`] 调整。
/// 区域内的析构顺序为**后分配先析构**（LIFO）。
pub struct Region {
    /// 区域名（调试 / 统计用）。
    pub name: Option<String>,
    /// 扩容策略。
    pub strategy: GrowthStrategy,
    /// 分配统计。
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
        Self {
            name: None,
            strategy: GrowthStrategy::Multiply { factor: 2.0 },
            stats: RegionStats::default(),
            allocator: BumpAllocator::new(size).expect("initial region block allocation"),
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

        let (ptr, wasted) = match self.allocator.try_allocate(size, align) {
            Some(ok) => ok,
            None => {
                if !self.allows_growth() {
                    return Err(AllocError::OutOfMemory { requested: size });
                }
                let next = self
                    .strategy
                    .calculate_next_size(self.capacity(), size, &self.stats);
                self.allocator.grow(next)?;
                self.stats.growth_count += 1;
                self.allocator
                    .try_allocate(size, align)
                    .ok_or(AllocError::OutOfMemory { requested: size })?
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
    pub fn allocate_bytes(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        if self.destroyed || size == 0 {
            return None;
        }
        let align = align.max(1);
        let (ptr, wasted) = match self.allocator.try_allocate(size, align) {
            Some(ok) => ok,
            None => {
                if !self.allows_growth() {
                    return None;
                }
                let next = self
                    .strategy
                    .calculate_next_size(self.capacity(), size, &self.stats);
                self.allocator.grow(next).ok()?;
                self.stats.growth_count += 1;
                self.allocator.try_allocate(size, align)?
            }
        };
        self.stats.allocation_count += 1;
        self.stats.total_allocated += size;
        self.stats.total_wasted += wasted;
        Some(ptr)
    }

    /// 当前总容量（所有块之和）。
    pub fn capacity(&self) -> usize {
        self.allocator.capacity()
    }

    /// 已使用字节数。
    pub fn used(&self) -> usize {
        self.allocator.used()
    }

    /// 剩余字节数。
    pub fn remaining(&self) -> usize {
        self.capacity() - self.used()
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
