//! PGO 顾问：根据运行时画像（`.zeta_profile`）推荐区域初始大小。

use crate::profile::PgoData;

/// PGO 决策参数。
#[derive(Debug, Clone, PartialEq)]
pub struct StrategyParams {
    /// EWMA 平滑系数。
    pub alpha: f64,
    /// 初始容量下限（字节）。
    pub initial_cap: usize,
    /// 安全系数（在分位数基础上放大）。
    pub safety_factor: f64,
    /// 使用的分位数（0..=1）。
    pub percentile: f64,
}

impl Default for StrategyParams {
    fn default() -> Self {
        Self {
            alpha: 0.3,
            initial_cap: 64 * 1024,
            safety_factor: 1.1,
            percentile: 0.95,
        }
    }
}

/// PGO 数据使用错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PgoError {
    /// PGO 数据为空。
    #[error("PGO data is empty")]
    EmptyData,
    /// 没有指定区域的数据。
    #[error("no PGO data for region `{0}`")]
    NoDataForRegion(String),
    /// 数据损坏。
    #[error("corrupted PGO data: {0}")]
    CorruptedData(String),
}

/// PGO 顾问：从 [`PgoData`] 中提取区域画像，按分位数 + 安全系数给出推荐大小。
#[derive(Debug, Clone)]
pub struct PgoAdvisor {
    data: PgoData,
    params: StrategyParams,
}

impl PgoAdvisor {
    /// 新建空顾问。
    pub fn new() -> Self {
        Self {
            data: PgoData::new(),
            params: StrategyParams::default(),
        }
    }

    /// 从 PGO 数据构建顾问。
    pub fn from_data(data: PgoData) -> Self {
        Self {
            data,
            params: StrategyParams::default(),
        }
    }

    /// 覆盖决策参数（链式）。
    pub fn with_params(mut self, params: StrategyParams) -> Self {
        self.params = params;
        self
    }

    /// 底层 PGO 数据。
    pub fn data(&self) -> &PgoData {
        &self.data
    }

    /// 推荐区域初始大小：`p95 × safety_factor`，下限为 `initial_cap`。
    ///
    /// 该区域无 PGO 数据时返回 `None`（由上层回退静态 / 自适应）。
    pub fn recommend_size(&self, region_id: &str) -> Option<usize> {
        let region = self.data.region_profile(region_id)?;
        let base = region.size_stats.p95;
        let suggested = (base as f64 * self.params.safety_factor) as usize;
        Some(suggested.max(self.params.initial_cap))
    }

    /// 获取指定区域适用的决策参数；数据为空或缺失时返回对应错误。
    pub fn params_for(&self, region_id: &str) -> Result<StrategyParams, PgoError> {
        if self.data.regions.is_empty() {
            return Err(PgoError::EmptyData);
        }
        if self.data.region_profile(region_id).is_none() {
            return Err(PgoError::NoDataForRegion(region_id.to_string()));
        }
        Ok(self.params.clone())
    }
}

impl Default for PgoAdvisor {
    fn default() -> Self {
        Self::new()
    }
}
