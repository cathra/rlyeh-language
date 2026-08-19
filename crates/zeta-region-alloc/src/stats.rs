//! 区域统计信息。

/// 区域分配统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RegionStats {
    /// 累计分配对象个数。
    pub allocation_count: usize,
    /// 累计分配字节数（不含对齐损失）。
    pub total_allocated: usize,
    /// 累计对齐损失字节数（块内向上对齐的间隙）。
    pub total_wasted: usize,
    /// 扩容次数。
    pub growth_count: usize,
    /// 区域销毁时实际执行的析构函数次数。
    pub destructor_calls: usize,
}
