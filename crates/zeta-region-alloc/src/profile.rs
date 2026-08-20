//! PGO 数据格式：区域运行时画像的持久化模型与收集器。
//!
//! P010 智能区域分配器通过 Profile 文件（`.zeta_profile`）保存运行时统计
//! （分配大小分布、扩容记录），供下一次编译期选择初始块大小使用。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// PGO 数据文件版本。
pub const PGO_FILE_VERSION: u32 = 1;

/// 一套完整的 PGO 数据：若干区域的运行时统计画像。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PgoData {
    /// 文件格式版本。
    pub version: u32,
    /// 生成时间（RFC 3339）。
    pub generated_at: String,
    /// 各区域的画像。
    pub regions: Vec<RegionProfile>,
}

impl PgoData {
    /// 新建空 PGO 数据。
    pub fn new() -> Self {
        Self {
            version: PGO_FILE_VERSION,
            generated_at: now_iso(),
            regions: Vec::new(),
        }
    }

    /// 全部区域 ID。
    pub fn region_ids(&self) -> Vec<String> {
        self.regions.iter().map(|r| r.region_id.clone()).collect()
    }

    /// 按区域 ID 查询画像。
    pub fn region_profile(&self, region_id: &str) -> Option<&RegionProfile> {
        self.regions.iter().find(|r| r.region_id == region_id)
    }
}

impl Default for PgoData {
    fn default() -> Self {
        Self::new()
    }
}

/// 单个区域的运行时画像。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegionProfile {
    /// 区域唯一 ID（对应编译器内部区域名）。
    pub region_id: String,
    /// 分配大小分布统计。
    pub size_stats: SizeStats,
    /// 扩容记录统计。
    pub growth_stats: GrowthStats,
    /// 采样次数。
    pub sample_count: usize,
    /// 最近一次采样时间（RFC 3339）。
    pub last_updated: String,
}

/// 分配大小分布统计。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SizeStats {
    /// 采样总数。
    pub total_samples: usize,
    /// 采样字节总量。
    pub total_bytes: usize,
    /// 最小分配。
    pub min: usize,
    /// 最大分配。
    pub max: usize,
    /// P50 分位数。
    pub p50: usize,
    /// P90 分位数。
    pub p90: usize,
    /// P95 分位数。
    pub p95: usize,
    /// 平均值。
    pub mean: usize,
}

/// 扩容记录统计。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct GrowthStats {
    /// 扩容总次数。
    pub total_growths: usize,
    /// 扩容累计字节数。
    pub total_growth_bytes: usize,
    /// 平均单次扩容字节数。
    pub avg_growth_bytes: usize,
    /// 单次最大扩容字节数。
    pub max_growth_bytes: usize,
}

/// 配置文件读写错误。
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    /// 文件系统错误。
    #[error("profile io error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON 序列化 / 反序列化错误。
    #[error("profile json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// PGO 数据收集器：内存中累积各区域的分配 / 扩容样本，可生成、保存、加载。
#[derive(Debug, Clone, Default)]
pub struct ProfileCollector {
    /// 各区域分配大小样本。
    samples: HashMap<String, Vec<usize>>,
    /// 各区域扩容字节样本。
    growths: HashMap<String, Vec<usize>>,
    /// 生成画像时计算的分位数集合。
    percentiles: Vec<f64>,
}

impl ProfileCollector {
    /// 新建收集器，默认计算 P50 / P90 / P95。
    pub fn new() -> Self {
        Self {
            samples: HashMap::new(),
            growths: HashMap::new(),
            percentiles: vec![0.50, 0.90, 0.95],
        }
    }

    /// 记录一次分配大小。
    pub fn record_allocation(&mut self, region_id: &str, size: usize) {
        self.samples.entry(region_id.to_string()).or_default().push(size);
    }

    /// 记录一次扩容字节数。
    pub fn record_growth(&mut self, region_id: &str, growth_bytes: usize) {
        self.growths.entry(region_id.to_string()).or_default().push(growth_bytes);
    }

    /// 全部采样次数（所有区域之和）。
    pub fn total_samples(&self) -> usize {
        self.samples.values().map(Vec::len).sum()
    }

    /// 已收集数据的区域数。
    pub fn region_count(&self) -> usize {
        self.samples.len()
    }

    /// 设置生成画像时计算的分位数集合（链式）。
    pub fn with_percentiles(mut self, percentiles: &[f64]) -> Self {
        self.percentiles = percentiles.to_vec();
        self
    }

    /// 依据当前样本生成 PGO 数据（排序后的稳定分位数）。
    pub fn generate(&self) -> PgoData {
        let mut regions = Vec::new();
        let mut ids: Vec<&String> = self.samples.keys().collect();
        ids.sort();
        for id in ids {
            let mut s = self.samples.get(id).cloned().unwrap_or_default();
            s.sort_unstable();
            let total: usize = s.iter().sum();
            let size_stats = SizeStats {
                total_samples: s.len(),
                total_bytes: total,
                min: s.first().copied().unwrap_or(0),
                max: s.last().copied().unwrap_or(0),
                p50: percentile(&s, 0.50),
                p90: percentile(&s, 0.90),
                p95: percentile(&s, 0.95),
                mean: if s.is_empty() { 0 } else { total / s.len() },
            };
            let g = self.growths.get(id).cloned().unwrap_or_default();
            let growth_stats = if g.is_empty() {
                GrowthStats::default()
            } else {
                let gtotal: usize = g.iter().sum();
                GrowthStats {
                    total_growths: g.len(),
                    total_growth_bytes: gtotal,
                    avg_growth_bytes: gtotal / g.len(),
                    max_growth_bytes: *g.iter().max().unwrap_or(&0),
                }
            };
            regions.push(RegionProfile {
                region_id: id.clone(),
                size_stats,
                growth_stats,
                sample_count: s.len(),
                last_updated: now_iso(),
            });
        }
        PgoData {
            version: PGO_FILE_VERSION,
            generated_at: now_iso(),
            regions,
        }
    }

    /// 保存 PGO 数据到 JSON 文件。
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ProfileError> {
        let data = self.generate();
        let json = serde_json::to_string_pretty(&data)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// 从 JSON 文件加载 PGO 数据（近似重建样本：按均值重复采样次数）。
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let text = fs::read_to_string(path)?;
        let data: PgoData = serde_json::from_str(&text)?;
        let mut collector = Self::new();
        for region in data.regions {
            // 分位数画像无法还原原始样本，以均值近似重建。
            let mean = region.size_stats.mean;
            let count = region.size_stats.total_samples;
            collector
                .samples
                .insert(region.region_id.clone(), vec![mean; count]);
        }
        Ok(collector)
    }
}

/// 计算已排序样本的 `p`（0..=1）分位数（最近秩近似）。
pub(crate) fn percentile(sorted: &[usize], p: f64) -> usize {
    if sorted.is_empty() {
        return 0;
    }
    if p <= 0.0 {
        return sorted[0];
    }
    if p >= 1.0 {
        return *sorted.last().unwrap();
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

/// 当前 UTC 时间（RFC 3339，秒精度）。
pub(crate) fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// 天数（自 1970-01-01）转公历日期（Howard Hinnant 的 civil_from_days 算法）。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
