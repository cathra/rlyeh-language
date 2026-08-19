//! Bump 分配器：由多个内存块组成的连续分配器。
//!
//! 遵循 ADR-002：扩容时分配新块（大小翻倍或按策略），**不拷贝**旧数据。

use std::ptr::NonNull;

use crate::block::MemoryBlock;
use crate::error::AllocError;

/// 由多个内存块组成的 bump 分配器。
pub(crate) struct BumpAllocator {
    blocks: Vec<MemoryBlock>,
    current: usize,
}

impl BumpAllocator {
    /// 以 `initial_size` 字节的初始块创建分配器。
    pub(crate) fn new(initial_size: usize) -> Result<Self, AllocError> {
        let block = MemoryBlock::new(initial_size)?;
        Ok(Self {
            blocks: vec![block],
            current: 0,
        })
    }

    /// 在当前块内 bump 分配；空间不足返回 `None`（不自动扩容）。
    pub(crate) fn try_allocate(
        &mut self,
        size: usize,
        align: usize,
    ) -> Option<(NonNull<u8>, usize)> {
        self.blocks[self.current].allocate(size, align)
    }

    /// 追加一个 `size` 字节的新块作为当前块。
    pub(crate) fn grow(&mut self, size: usize) -> Result<(), AllocError> {
        let block = MemoryBlock::new(size)?;
        self.blocks.push(block);
        self.current = self.blocks.len() - 1;
        Ok(())
    }

    /// 所有块总容量。
    pub(crate) fn capacity(&self) -> usize {
        self.blocks.iter().map(MemoryBlock::capacity).sum()
    }

    /// 所有块已使用字节数。
    pub(crate) fn used(&self) -> usize {
        self.blocks.iter().map(MemoryBlock::used).sum()
    }

    /// 块数量。
    pub(crate) fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// 释放所有块（各块 `Drop` 归还内存）。
    pub(crate) fn deallocate_all(&mut self) {
        self.blocks.clear();
        self.current = 0;
    }
}
