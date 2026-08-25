//! 综合大小决策器：综合静态推断、PGO 画像与用户提示，输出分配计划。

use crate::pgo_advisor::PgoAdvisor;
use crate::static_sizer::{SizeEstimate, StaticSizer};

/// 自适应默认初始块大小（字节）。
pub const DEFAULT_INITIAL_SIZE: usize = 4096;

/// 决策来源（记录在统计中，便于诊断）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionSource {
    /// 用户显式指定的大小。
    UserSpecified(usize),
    /// 静态精确推断。
    StaticExact(usize),
    /// 静态上界推断。
    StaticUpperBound(usize),
    /// PGO 画像推荐。
    PgoRecommended(usize),
    /// 自适应默认。
    AdaptiveDefault,
}

/// 区域分配计划：`SmartRegion` 按此初始化。
#[derive(Debug, Clone)]
pub struct AllocatorPlan {
    /// 初始块大小（字节）。
    pub initial_size: usize,
    /// 最小块大小（字节）。
    pub min_size: usize,
    /// 最大块大小（字节）。
    pub max_size: usize,
    /// 扩容倍率。
    pub growth_factor: f64,
    /// 决策来源。
    pub source: DecisionSource,
}

/// 综合决策器。
///
/// 决策优先级（P010）：
/// 1. 用户提示（`with_user_hint`）；
/// 2. 静态精确 / 上界 / 区间；
/// 3. PGO 推荐（仅当静态信息缺失）；
/// 4. 自适应默认（4 KB）。
#[derive(Debug, Clone)]
pub struct SizeAdvisor {
    region_name: String,
    sizer: StaticSizer,
    user_hint: Option<usize>,
    pgo: Option<PgoAdvisor>,
    min_size: usize,
    max_size: Option<usize>,
    growth_factor: f64,
}

impl SizeAdvisor {
    /// 为指定区域新建决策器。
    pub fn new(region_name: String) -> Self {
        Self {
            region_name,
            sizer: StaticSizer::new(),
            user_hint: None,
            pgo: None,
            min_size: 0,
            max_size: None,
            growth_factor: 2.0,
        }
    }

    /// 注入静态推断器（链式）。
    pub fn with_sizer(mut self, sizer: StaticSizer) -> Self {
        self.sizer = sizer;
        self
    }

    /// 注入用户提示大小（链式，最高优先级）。
    pub fn with_user_hint(mut self, hint: usize) -> Self {
        self.user_hint = Some(hint);
        self
    }

    /// 注入 PGO 顾问（链式，静态信息缺失时使用）。
    pub fn with_pgo(mut self, pgo: PgoAdvisor) -> Self {
        self.pgo = Some(pgo);
        self
    }

    /// 覆盖扩容倍率（链式）。
    pub fn with_growth_factor(mut self, factor: f64) -> Self {
        self.growth_factor = factor;
        self
    }

    /// 覆盖大小边界（链式）：`(min, max)`。
    pub fn with_size_bounds(mut self, min: usize, max: usize) -> Self {
        self.min_size = min;
        self.max_size = Some(max);
        self
    }

    /// 静态推断器引用。
    pub fn sizer(&self) -> &StaticSizer {
        &self.sizer
    }

    /// 做出决策，返回分配计划。
    pub fn decide(&self) -> AllocatorPlan {
        if let Some(hint) = self.user_hint {
            return AllocatorPlan {
                initial_size: hint,
                min_size: self.min_size.max(hint),
                max_size: self.max_size.unwrap_or(hint * 4).max(hint),
                growth_factor: 1.0,
                source: DecisionSource::UserSpecified(hint),
            };
        }

        match self.sizer.estimate() {
            SizeEstimate::Exact(n) => {
                let with_margin = (n as f64 * 1.1) as usize;
                AllocatorPlan {
                    initial_size: with_margin,
                    min_size: with_margin,
                    max_size: self.max_size.unwrap_or(n * 4).max(with_margin),
                    growth_factor: 1.5,
                    source: DecisionSource::StaticExact(n),
                }
            }
            SizeEstimate::UpperBound(n) => AllocatorPlan {
                initial_size: n,
                min_size: n,
                max_size: self.max_size.unwrap_or(n * 4).max(n),
                // 上界已接近实际需求，小幅扩容即可。
                growth_factor: 1.5,
                source: DecisionSource::StaticUpperBound(n),
            },
            SizeEstimate::Range { max, .. } => {
                let with_margin = (max as f64 * 1.2) as usize;
                AllocatorPlan {
                    initial_size: with_margin,
                    min_size: with_margin,
                    max_size: self.max_size.unwrap_or(max * 4).max(with_margin),
                    growth_factor: 2.0,
                    source: DecisionSource::StaticUpperBound(max),
                }
            }
            SizeEstimate::Unknown => {
                if let Some(pgo) = &self.pgo {
                    if let Some(recommended) = pgo.recommend_size(&self.region_name) {
                        return AllocatorPlan {
                            initial_size: recommended,
                            min_size: recommended / 2,
                            max_size: self
                                .max_size
                                .unwrap_or(recommended * 4)
                                .max(recommended),
                            growth_factor: 1.5,
                            source: DecisionSource::PgoRecommended(recommended),
                        };
                    }
                }
                AllocatorPlan {
                    initial_size: DEFAULT_INITIAL_SIZE,
                    min_size: self.min_size,
                    max_size: self.max_size.unwrap_or(DEFAULT_INITIAL_SIZE * 4),
                    growth_factor: self.growth_factor,
                    source: DecisionSource::AdaptiveDefault,
                }
            }
        }
    }
}
