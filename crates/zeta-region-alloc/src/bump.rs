//! Bump 分配器：由多个内存块组成的连续分配器（块管理）。
//!
//! 遵循 ADR-002：扩容时分配新块（大小翻倍或按策略），**不拷贝**旧数据。
//!
//! 本模块只负责**块生命周期管理**（增删/容量/当前块查询）；bump 光标状态
//! （`base`/`cursor`/`limit`）由 [`crate::Region`] 作为权威维护——LLVM 后端
//! 对 `in 'r` 分配内联生成 bump 快路径，直接读写 `Region` 首部的固定偏移字段，
//! 仅越界时才经 C ABI 调用回本 crate 扩容。

use crate::block::MemoryBlock;
use crate::error::AllocError;

/// 由多个内存块组成的 bump 分配器（仅块管理）。
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

    /// 追加一个 `size` 字节的新块作为当前块。
    pub(crate) fn grow(&mut self, size: usize) -> Result<(), AllocError> {
        let block = MemoryBlock::new(size)?;
        self.blocks.push(block);
        self.current = self.blocks.len() - 1;
        Ok(())
    }

    /// 当前块的 `(基址, 容量)`。
    pub(crate) fn current_block(&self) -> (*mut u8, usize) {
        let block = &self.blocks[self.current];
        (block.as_ptr(), block.capacity())
    }

    /// 所有块总容量。
    pub(crate) fn capacity(&self) -> usize {
        self.blocks.iter().map(MemoryBlock::capacity).sum()
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
