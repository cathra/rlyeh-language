//! 智能区域（`SmartRegion`）：自适应 bump 分配器（P010 最终形态）。
//!
//! 与基础 [`crate::Region`] 相比，`SmartRegion`：
//!
//! - 初始块大小由 [`SizeAdvisor`] 决策（静态推断 / PGO / 用户提示 / 自适应）；
//! - 扩容倍率由 EWMA 预测驱动，受 `max_size` 上限约束；
//! - 需要 drop 的对象按 LIFO 注册，区域销毁时逆序析构；
//! - 记录分配 / 扩容事件与碎片统计，供 PGO 收集与报告使用。

use std::alloc::{alloc, dealloc, Layout};
use std::mem::{align_of, needs_drop, size_of};
use std::ptr::{self, NonNull};

use crate::error::AllocError;
use crate::profile::now_iso;
use crate::size_advisor::SizeAdvisor;
use crate::stats::{GrowthEvent, RegionStats};

/// 智能块基础对齐（16 字节，覆盖常见 SIMD 类型）。
const BLOCK_ALIGN: usize = 16;
/// 初始块大小下限（字节）：用户提示 / 静态估计过小时仍保证可用空间。
const MIN_INITIAL_SIZE: usize = 1024;
/// EWMA 默认平滑系数。
const DEFAULT_EWMA_ALPHA: f64 = 0.3;

/// 一段连续内存块（16 字节对齐），整体随 `Drop` 释放。
struct SmartBlock {
    ptr: NonNull<u8>,
    size: usize,
    current: usize,
    end: usize,
    allocated: bool,
    layout: Layout,
}

impl SmartBlock {
    /// 分配一个 `size` 字节的内存块。
    fn new(size: usize) -> Result<Self, AllocError> {
        let size = size.max(1);
        let layout =
            Layout::from_size_align(size, BLOCK_ALIGN).map_err(|_| AllocError::Overflow)?;
        // SAFETY: layout 已校验（size > 0、align = 16 合法）。
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return Err(AllocError::OutOfMemory { requested: size });
        }
        // SAFETY: alloc 成功返回非空指针。
        Ok(Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            size,
            current: 0,
            end: size,
            allocated: true,
            layout,
        })
    }

    /// 块内 bump 分配 `size` 字节、对齐 `align`。
    ///
    /// 成功返回 `(指针, 对齐损失)`；剩余空间不足返回 `None`。
    fn allocate(&mut self, size: usize, align: usize) -> Option<(NonNull<u8>, usize)> {
        let base = self.ptr.as_ptr() as usize;
        let addr = base + self.current;
        let aligned = addr.checked_next_multiple_of(align.max(BLOCK_ALIGN))?;
        let wasted = aligned - addr;
        let new_current = aligned.checked_add(size)?;
        if new_current > base + self.end {
            return None;
        }
        self.current = new_current - base;
        // SAFETY: aligned 位于块内（aligned + size <= base + end）。
        Some((
            unsafe { NonNull::new_unchecked(aligned as *mut u8) },
            wasted,
        ))
    }
}

impl Drop for SmartBlock {
    fn drop(&mut self) {
        if self.allocated {
            // SAFETY: ptr 由同 layout 的 alloc 返回，仅释放一次。
            unsafe { dealloc(self.ptr.as_ptr(), self.layout) };
        }
    }
}

/// 析构条目：记录对象指针与其析构函数。
struct DestructorEntry {
    ptr: NonNull<u8>,
    drop_fn: unsafe fn(NonNull<u8>),
}

/// 通用 `drop_in_place` 适配：把裸指针还原为 `*mut T` 后原地析构。
///
/// # Safety
/// `ptr` 必须指向由 [`SmartRegion::allocate::<T>`] 写入的合法 `T`。
unsafe fn drop_in_place<T>(ptr: NonNull<u8>) {
    // SAFETY: 调用方保证 ptr 指向有效的 T。
    unsafe { ptr::drop_in_place(ptr.as_ptr() as *mut T) };
}

/// EWMA 平滑状态：跟踪历史分配大小，驱动扩容预测。
struct EwmaState {
    alpha: f64,
    current_estimate: Option<f64>,
}

impl EwmaState {
    fn new(alpha: f64) -> Self {
        Self {
            alpha,
            current_estimate: None,
        }
    }

    fn update(&mut self, sample: usize) -> f64 {
        let estimate = match self.current_estimate {
            Some(prev) => self.alpha * sample as f64 + (1.0 - self.alpha) * prev,
            None => sample as f64,
        };
        self.current_estimate = Some(estimate);
        estimate
    }
}

/// 智能区域：自适应 bump 分配器。
///
/// - 初始大小来自 [`SizeAdvisor`]（静态 / PGO / 用户提示 / 自适应默认）；
/// - 扩容时新块大小取 `max(EWMA 预测, 请求大小, 当前块 ×2)`，受 `max_size` 上限约束；
/// - 需要 drop 的对象按 LIFO 注册，区域销毁时逆序析构；
/// - 记录分配 / 扩容事件与碎片统计，供 PGO 收集与报告使用。
pub struct SmartRegion {
    name: String,
    blocks: Vec<SmartBlock>,
    current: usize,
    destructors: Vec<DestructorEntry>,
    stats: RegionStats,
    frozen: bool,
    max_size: usize,
    growth_factor: f64,
    ewma_state: EwmaState,
    allocation_samples: Vec<usize>,
    growth_events: Vec<GrowthEvent>,
}

impl SmartRegion {
    /// 按决策计划创建智能区域。
    ///
    /// `advisor.decide()` 的结果决定初始 / 最大块大小；初始大小有 1 KB 下限。
    pub fn new(name: String, advisor: SizeAdvisor) -> Result<Self, AllocError> {
        let plan = advisor.decide();
        let initial_size = plan.initial_size.max(MIN_INITIAL_SIZE);
        let max_size = plan.max_size.max(initial_size * 4);
        let block = SmartBlock::new(initial_size)?;
        let stats = RegionStats {
            region_name: name.clone(),
            created_at: now_iso(),
            decision_source: format!("{:?}", plan.source),
            ..RegionStats::default()
        };
        Ok(Self {
            name,
            blocks: vec![block],
            current: 0,
            destructors: Vec::new(),
            stats,
            frozen: false,
            max_size,
            growth_factor: plan.growth_factor,
            ewma_state: EwmaState::new(DEFAULT_EWMA_ALPHA),
            allocation_samples: Vec::new(),
            growth_events: Vec::new(),
        })
    }

    /// 区域名。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 区域统计快照引用。
    pub fn stats(&self) -> &RegionStats {
        &self.stats
    }

    /// 总容量（所有块之和，字节）。
    pub fn capacity(&self) -> usize {
        self.blocks.iter().map(|b| b.size).sum()
    }

    /// 已使用字节数（所有块当前偏移之和）。
    pub fn used(&self) -> usize {
        self.blocks.iter().map(|b| b.current).sum()
    }

    /// 内存块数量（含扩容产生的块）。
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// 扩容事件记录。
    pub fn growth_events(&self) -> &[GrowthEvent] {
        &self.growth_events
    }

    /// 历史分配大小样本。
    pub fn allocation_samples(&self) -> &[usize] {
        &self.allocation_samples
    }

    /// 区域最大容量上限（字节）。
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// 在区域内分配一个 `T` 值，返回其可变引用。
    ///
    /// 空间不足时按 EWMA 预测扩容；超过 `max_size` 上限返回
    /// [`AllocError::TooLarge`]，区域冻结后返回 [`AllocError::Frozen`]。
    pub fn allocate<T>(&mut self, value: T) -> Result<&mut T, AllocError> {
        if self.frozen {
            return Err(AllocError::Frozen);
        }
        let type_size = size_of::<T>();
        let type_align = align_of::<T>();

        // 在当前块内尝试分配；失败时记录旧块大小与碎片字节。
        let (ptr, old_size, frag) = {
            let current_block = self
                .blocks
                .get_mut(self.current)
                .ok_or(AllocError::Destroyed)?;
            match current_block.allocate(type_size, type_align) {
                Some(ok) => (Some(ok), current_block.size, 0),
                None => (
                    None,
                    current_block.size,
                    current_block.end - current_block.current,
                ),
            }
        };

        let (ptr, wasted) = match ptr {
            Some(pair) => pair,
            None => {
                let new_size = self.next_block_size(type_size)?;
                let old_capacity = self.capacity();
                self.ensure_capacity(new_size)?;
                let current_block = self
                    .blocks
                    .get_mut(self.current)
                    .expect("new block exists after growth");
                self.stats.fragmentation += frag;
                self.stats.growth_count += 1;
                self.growth_events.push(GrowthEvent {
                    timestamp: now_iso(),
                    old_size,
                    new_size,
                    reason: format!(
                        "requested {type_size} B; used {old_capacity} B of capacity"
                    ),
                });
                current_block
                    .allocate(type_size, type_align)
                    .ok_or(AllocError::OutOfMemory { requested: type_size })?
            }
        };

        // SAFETY: ptr 指向区域内空闲内存，写入后由析构注册表接管。
        unsafe {
            ptr::write(ptr.as_ptr().cast::<T>(), value);
        }
        if needs_drop::<T>() {
            self.destructors.push(DestructorEntry {
                ptr,
                drop_fn: drop_in_place::<T>,
            });
        }
        self.stats.allocation_count += 1;
        self.stats.total_allocated += type_size;
        self.stats.total_wasted += wasted;
        let used_now = self.used();
        if used_now > self.stats.peak_usage {
            self.stats.peak_usage = used_now;
        }
        self.allocation_samples.push(type_size);
        self.ewma_state.update(type_size);

        // SAFETY: ptr 已写入合法 T，借用期与 &mut self 一致。
        Ok(unsafe { &mut *ptr.as_ptr().cast::<T>() })
    }

    /// 销毁区域：逆序执行析构（LIFO）并释放全部内存块。
    ///
    /// # Safety
    /// 调用者必须保证不存在指向区域内对象的外部引用（借用检查器无法静态验证）。
    pub unsafe fn destroy(&mut self) {
        if self.frozen {
            return;
        }
        self.frozen = true;
        for entry in self.destructors.drain(..).rev() {
            // SAFETY: 条目由 allocate 注册，ptr 指向有效 T。
            unsafe { (entry.drop_fn)(entry.ptr) };
            self.stats.destructor_calls += 1;
        }
        self.stats.total_deallocated += self.capacity();
        self.blocks.clear();
        self.current = 0;
    }

    /// 计算扩容后的新块大小：
    /// `max(EWMA 预测, 请求大小, 当前块 × growth_factor)`。
    fn next_block_size(&self, requested: usize) -> Result<usize, AllocError> {
        let current_size = self.blocks[self.current].size;
        let ewma_suggested = self.ewma_state.current_estimate.unwrap_or(0.0) as usize;
        let grown = (current_size as f64 * self.growth_factor).ceil() as usize;
        let new_size = ewma_suggested.max(requested).max(grown).max(current_size + 1);
        if new_size > self.max_size {
            return Err(AllocError::TooLarge {
                requested,
                max: self.max_size,
            });
        }
        Ok(new_size)
    }

    /// 追加一个新块并设为当前块。
    fn ensure_capacity(&mut self, size: usize) -> Result<(), AllocError> {
        let block = SmartBlock::new(size)?;
        self.blocks.push(block);
        self.current = self.blocks.len() - 1;
        Ok(())
    }
}

impl Drop for SmartRegion {
    fn drop(&mut self) {
        // SAFETY: Drop 时对象引用必然已失效，可安全销毁。
        unsafe {
            self.destroy();
        }
    }
}
