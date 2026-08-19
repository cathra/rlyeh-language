//! 内存块：bump 分配的基础单元。
//!
//! 每个块是一段连续堆内存，内部按 bump 顺序分配，永不归还单个对象；
//! 块整体随 `Drop` 释放。

use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;

use crate::error::AllocError;

/// 内存块的基础对齐（8 字节，覆盖常见标量类型）。
const BLOCK_ALIGN: usize = 8;

/// 一段连续的内存块。
pub(crate) struct MemoryBlock {
    ptr: NonNull<u8>,
    size: usize,
    used: usize,
}

impl MemoryBlock {
    /// 分配一个 `size` 字节的内存块。
    pub(crate) fn new(size: usize) -> Result<Self, AllocError> {
        let size = size.max(1);
        let layout =
            Layout::from_size_align(size, BLOCK_ALIGN).map_err(|_| AllocError::Overflow)?;
        // SAFETY: layout 已校验（size>0、align=8 合法）。
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return Err(AllocError::OutOfMemory { requested: size });
        }
        // SAFETY: alloc 成功返回非空指针。
        Ok(Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            size,
            used: 0,
        })
    }

    /// 块总容量（字节）。
    pub(crate) fn capacity(&self) -> usize {
        self.size
    }

    /// 已使用字节数。
    pub(crate) fn used(&self) -> usize {
        self.used
    }

    /// 在块内 bump 分配 `size` 字节、对齐 `align`。
    ///
    /// 成功返回 `(指针, 对齐损失)`；剩余空间不足返回 `None`。
    pub(crate) fn allocate(&mut self, size: usize, align: usize) -> Option<(NonNull<u8>, usize)> {
        let base = self.ptr.as_ptr() as usize;
        let addr = base + self.used;
        let aligned = addr.checked_next_multiple_of(align.max(BLOCK_ALIGN))?;
        let wasted = aligned - addr;
        let end = aligned.checked_add(size)?;
        if end > base + self.size {
            return None;
        }
        self.used = end - base;
        // SAFETY: aligned >= base 且 aligned+size <= base+size，均在块内。
        Some((
            unsafe { NonNull::new_unchecked(aligned as *mut u8) },
            wasted,
        ))
    }
}

impl Drop for MemoryBlock {
    fn drop(&mut self) {
        let layout =
            Layout::from_size_align(self.size, BLOCK_ALIGN).expect("block layout is valid");
        // SAFETY: ptr 由同 layout 的 alloc 返回，且仅释放一次。
        unsafe { dealloc(self.ptr.as_ptr(), layout) };
    }
}
