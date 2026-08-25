//! 扩容策略：决定区域内存块下一次扩容的大小。

use crate::stats::RegionStats;

/// 自适应预测的 EWMA 平滑系数。
const EWMA_ALPHA: f64 = 0.5;
/// 自适应预测的放大系数（预测均值 × 1.2 作为下一块大小）。
const ADAPTIVE_FACTOR: f64 = 1.2;

/// 区域扩容策略。
#[derive(Debug, Clone, PartialEq)]
pub enum GrowthStrategy {
    /// 精确模式：固定初始大小，不允许扩容。
    Exact,
    /// 固定倍率扩容：`next = max(requested, current * factor)`。
    Multiply {
        /// 扩容倍率。
        factor: f64,
    },
    /// 线性扩容：`next = current + chunk`。
    Linear {
        /// 线性增量。
        chunk: usize,
    },
    /// 自适应：以历史分配量 EWMA 预测下一块大小，
    /// 上限受 `max_capacity` 约束（0 表示不限制）。
    Adaptive {
        /// 历史单次分配量样本。
        adaptive_samples: Vec<usize>,
        /// 最大容量上限（0 = 不限制）。
        max_capacity: usize,
    },
}

/// 对样本做 EWMA（指数加权移动平均）平滑。
fn ewma(samples: &[usize]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut avg = samples[0] as f64;
    for &s in &samples[1..] {
        avg = EWMA_ALPHA * s as f64 + (1.0 - EWMA_ALPHA) * avg;
    }
    avg
}

impl GrowthStrategy {
    /// 计算下一次扩容的大小（字节）。
    ///
    /// - `current_capacity`：当前总容量；
    /// - `requested`：本次请求字节数（结果至少满足它）；
    /// - `stats`：区域统计（供自适应策略参考）。
    pub fn calculate_next_size(
        &self,
        current_capacity: usize,
        requested: usize,
        _stats: &RegionStats,
    ) -> usize {
        match self {
            GrowthStrategy::Exact => requested.max(current_capacity.saturating_add(1)),
            GrowthStrategy::Multiply { factor } => {
                let scaled = (current_capacity as f64 * factor).ceil() as usize;
                requested.max(scaled.max(current_capacity.saturating_add(1)))
            }
            GrowthStrategy::Linear { chunk } => {
                requested.max(current_capacity.saturating_add(*chunk))
            }
            GrowthStrategy::Adaptive {
                adaptive_samples,
                max_capacity,
            } => {
                let predicted = (ewma(adaptive_samples) * ADAPTIVE_FACTOR).ceil() as usize;
                let mut next = requested.max(predicted);
                next = next.max(current_capacity.saturating_add(1));
                if *max_capacity > 0 {
                    next = next.min(*max_capacity);
                }
                next.max(requested)
            }
        }
    }
}
