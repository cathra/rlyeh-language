//! 编译器集成接口：向编译器（`rlyeh-regionck`）暴露区域决策结果与 PGO 数据。

use crate::profile::PgoData;

/// 编译器为单个区域记录的编译期信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionCompileInfo {
    /// 区域 ID（编译器内部标识）。
    pub region_id: String,
    /// 静态估计大小（字节）。
    pub estimated_size: usize,
    /// 决定采用的初始块大小（字节）。
    pub initial_size: usize,
    /// 区域最大容量（字节）。
    pub max_size: usize,
    /// 决策来源描述。
    pub decision: String,
}

/// 编译器集成入口：汇总区域决策并生成可读报告。
#[derive(Debug, Clone, Default)]
pub struct CompilerInterface {
    /// 已注册的区域。
    regions: Vec<RegionCompileInfo>,
    /// 可选 PGO 数据（编译时从 `.rl_profile` 加载）。
    pgo_data: Option<PgoData>,
}

impl CompilerInterface {
    /// 新建编译器集成入口。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注入 PGO 数据（链式）。
    pub fn with_pgo_data(mut self, data: PgoData) -> Self {
        self.pgo_data = Some(data);
        self
    }

    /// 注册一个区域的编译期信息（链式）。
    pub fn register_region(mut self, info: RegionCompileInfo) -> Self {
        self.regions.push(info);
        self
    }

    /// 已注册区域数。
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// 是否有 PGO 数据。
    pub fn has_pgo_data(&self) -> bool {
        self.pgo_data.is_some()
    }

    /// 生成区域分配决策报告（人类可读）。
    pub fn generate_report(&self) -> String {
        let mut report = String::from("Region Allocation Report\n========================\n");
        if let Some(pgo) = &self.pgo_data {
            report.push_str(&format!("PGO data version: {}\n", pgo.version));
            report.push_str(&format!("PGO regions: {}\n", pgo.regions.len()));
        } else {
            report.push_str("PGO data: none\n");
        }
        report.push('\n');
        report.push_str("Regions:\n");
        for r in &self.regions {
            report.push_str(&format!(
                "  - {}: estimated {} B, initial {} B, max {} B, decision: {}\n",
                r.region_id, r.estimated_size, r.initial_size, r.max_size, r.decision
            ));
        }
        if self.regions.is_empty() {
            report.push_str("  (none registered)\n");
        }
        report
    }
}
