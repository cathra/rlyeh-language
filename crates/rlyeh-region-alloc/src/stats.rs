//! 区域统计信息（基础区域与智能区域共用）。

/// 区域分配统计。
///
/// 既用于 [`crate::Region`]（基础区域，仅字段自增），也用于
/// [`crate::SmartRegion`]（智能区域，含决策来源与内存画像）。
#[derive(Debug, Clone, Default, PartialEq)]
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
    /// 区域名。
    pub region_name: String,
    /// 区域创建时间（RFC 3339）。
    pub created_at: String,
    /// 累计释放字节数（区域销毁时整体释放）。
    pub total_deallocated: usize,
    /// 峰值使用量（所有块已用字节数之和的最大值）。
    pub peak_usage: usize,
    /// 累计碎片字节数（扩容时旧块剩余空间）。
    pub fragmentation: usize,
    /// 初始大小决策来源（`DecisionSource` 的调试表示）。
    pub decision_source: String,
}

impl RegionStats {
    /// 分配效率：有效字节占（有效 + 对齐损失）的比例。
    pub fn allocation_efficiency(&self) -> f64 {
        let total = self.total_allocated + self.total_wasted;
        if total == 0 {
            0.0
        } else {
            self.total_allocated as f64 / total as f64
        }
    }

    /// 平均单次分配大小（字节）。
    pub fn avg_allocation_size(&self) -> usize {
        self.total_allocated
            .checked_div(self.allocation_count)
            .unwrap_or(0)
    }

    /// 扩容频率：每次分配平均触发的扩容次数。
    pub fn growth_frequency(&self) -> f64 {
        if self.allocation_count == 0 {
            0.0
        } else {
            self.growth_count as f64 / self.allocation_count as f64
        }
    }

    /// 生成人类可读的统计报告。
    pub fn report(&self) -> String {
        format!(
            "Region `{}` (created {})\n\
             \x20 allocations: {}, total: {} B, wasted: {} B\n\
             \x20 growths: {}, destructor calls: {}\n\
             \x20 peak usage: {} B, fragmentation: {} B\n\
             \x20 efficiency: {:.2}, avg size: {} B, growth frequency: {:.4}\n\
             \x20 decision: {}",
            self.region_name,
            self.created_at,
            self.allocation_count,
            self.total_allocated,
            self.total_wasted,
            self.growth_count,
            self.destructor_calls,
            self.peak_usage,
            self.fragmentation,
            self.allocation_efficiency(),
            self.avg_allocation_size(),
            self.growth_frequency(),
            self.decision_source,
        )
    }
}

/// 一次扩容事件（智能区域记录）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrowthEvent {
    /// 事件时间（RFC 3339）。
    pub timestamp: String,
    /// 扩容前块大小（字节）。
    pub old_size: usize,
    /// 扩容后块大小（字节）。
    pub new_size: usize,
    /// 触发原因。
    pub reason: String,
}
