//! 内存块：bump 分配的基础单元。
//!
//! 每个块是一段连续堆内存；块整体随 `Drop` 释放。
//! bump 光标状态（`cursor`）由 [`crate::Region`] 作为权威维护（内联快路径
//! 需要首部固定偏移），内存块本身只记录 `ptr`/`size`。

use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;

use crate::error::AllocError;

/// 内存块的基础对齐（8 字节，覆盖常见标量类型）。
///
/// 同时也是 [`crate::Region`] 内联 bump 快路径的对齐下限：
/// LLVM 后端生成的 `aligned = (cursor + align - 1) & !(align - 1)`
/// 依赖 `align` 为 2 的幂且不小于此值。
pub(crate) const BASE_ALIGN: usize = 8;

/// 一段连续的内存块。
pub(crate) struct MemoryBlock {
    ptr: NonNull<u8>,
    size: usize,
}

impl MemoryBlock {
    /// 分配一个 `size` 字节的内存块（基址按 [`BASE_ALIGN`] 对齐）。
    pub(crate) fn new(size: usize) -> Result<Self, AllocError> {
        let size = size.max(1);
        let layout =
            Layout::from_size_align(size, BASE_ALIGN).map_err(|_| AllocError::Overflow)?;
        // SAFETY: layout 已校验（size>0、align=8 合法）。
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return Err(AllocError::OutOfMemory { requested: size });
        }
        // SAFETY: alloc 成功返回非空指针。
        Ok(Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            size,
        })
    }

    /// 块总容量（字节）。
    pub(crate) fn capacity(&self) -> usize {
        self.size
    }

    /// 块基址。
    pub(crate) fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for MemoryBlock {
    fn drop(&mut self) {
        let layout =
            Layout::from_size_align(self.size, BASE_ALIGN).expect("block layout is valid");
        // SAFETY: ptr 由同 layout 的 alloc 返回，且仅释放一次。
        unsafe { dealloc(self.ptr.as_ptr(), layout) };
    }
}
