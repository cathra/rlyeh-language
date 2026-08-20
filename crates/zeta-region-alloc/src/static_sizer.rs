//! 编译期静态大小推断：根据 AST 分析结果估算区域内的分配总量。
//!
//! P010 智能区域分配器用 [`StaticSizer`] 的结果作为初始块大小的静态依据，
//! 仅在静态信息缺失（`Unknown` / `Range`）时回退到 PGO 或自适应默认值。

/// 静态大小估计结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SizeEstimate {
    /// 无法静态推断。
    Unknown,
    /// 精确大小（字节）。
    Exact(usize),
    /// 上界（字节）：有界循环内分配，实际大小不超过该值。
    UpperBound(usize),
    /// 区间（字节）：分支导致大小不确定。
    Range {
        /// 最小路径大小。
        min: usize,
        /// 最大路径大小。
        max: usize,
    },
}

/// 分配点执行频率。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecFrequency {
    /// 执行恰好一次。
    Once,
    /// 有界循环内执行，最多 `max_iterations` 次。
    Loop {
        /// 循环最大迭代次数。
        max_iterations: usize,
    },
    /// 无法确定（循环边界未知等）。
    Unknown,
}

/// 一个静态分配点（由编译器在 HIR 分析阶段产出）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocSite {
    /// 分配点 ID（编译器内部标识，如 `fn::line:col`）。
    pub site_id: String,
    /// 每次分配的类型大小（字节）。
    pub type_size: usize,
    /// 执行频率。
    pub frequency: ExecFrequency,
}

/// 一个分支结构：多个互斥分支，每个分支包含一组分配点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchInfo {
    /// 各互斥分支的分配点 ID 列表。
    pub branches: Vec<Vec<String>>,
}

/// 静态大小推断器：维护分配点与分支结构，输出区域大小估计。
#[derive(Debug, Clone, Default)]
pub struct StaticSizer {
    sites: Vec<AllocSite>,
    branches: Vec<BranchInfo>,
}

impl StaticSizer {
    /// 新建空推断器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个分配点。
    pub fn add_site(&mut self, site: AllocSite) {
        self.sites.push(site);
    }

    /// 注册一个分支结构。
    pub fn add_branch(&mut self, branch: BranchInfo) {
        self.branches.push(branch);
    }

    /// 全部注册的分配点。
    pub fn sites(&self) -> &[AllocSite] {
        &self.sites
    }

    /// 全部注册的分支结构。
    pub fn branches(&self) -> &[BranchInfo] {
        &self.branches
    }

    /// 按 ID 查找分配点。
    pub fn find_site(&self, site_id: &str) -> Option<&AllocSite> {
        self.sites.iter().find(|s| s.site_id == site_id)
    }

    /// 估算区域大小：无分支时线性汇总；有分支时按路径展开取区间。
    pub fn estimate(&self) -> SizeEstimate {
        if self.sites.is_empty() && self.branches.is_empty() {
            return SizeEstimate::Unknown;
        }
        if !self.branches.is_empty() {
            return self.estimate_with_branches();
        }
        self.estimate_linear()
    }

    /// 静态精确大小：仅在 `Exact` 时返回。
    pub fn static_size(&self) -> Option<usize> {
        match self.estimate() {
            SizeEstimate::Exact(n) => Some(n),
            _ => None,
        }
    }

    /// 线性（顺序执行）场景：全部分配点求和。
    ///
    /// - 全部 `Once` 或 `Loop{max<=1}` → `Exact(sum)`；
    /// - 含 `Loop{max>1}` → `UpperBound(sum)`；
    /// - 含 `Unknown` → `Unknown`。
    fn estimate_linear(&self) -> SizeEstimate {
        let mut has_bounded_loop = false;
        let mut exact = true;
        let mut total_max = 0usize;

        for site in &self.sites {
            match site.frequency {
                ExecFrequency::Once => total_max += site.type_size,
                ExecFrequency::Loop { max_iterations } => {
                    if max_iterations > 1 {
                        has_bounded_loop = true;
                        exact = false;
                    }
                    total_max += site.type_size.saturating_mul(max_iterations);
                }
                ExecFrequency::Unknown => return SizeEstimate::Unknown,
            }
        }

        if exact {
            SizeEstimate::Exact(total_max)
        } else if has_bounded_loop {
            SizeEstimate::UpperBound(total_max)
        } else {
            SizeEstimate::Unknown
        }
    }

    /// 分支场景：为每个分支路径独立求和，取最小 / 最大路径为区间。
    fn estimate_with_branches(&self) -> SizeEstimate {
        let mut path_sums: Vec<usize> = vec![0];
        for branch in &self.branches {
            let mut new_sums = Vec::new();
            for current_sum in &path_sums {
                for branch_sites in &branch.branches {
                    let mut sum = *current_sum;
                    for site_id in branch_sites {
                        if let Some(site) = self.find_site(site_id) {
                            sum += self.max_size_for_site(site);
                        }
                    }
                    new_sums.push(sum);
                }
            }
            path_sums = new_sums;
        }

        let min = path_sums.iter().min().copied().unwrap_or(0);
        let max = path_sums.iter().max().copied().unwrap_or(0);
        if min == max {
            SizeEstimate::Exact(min)
        } else {
            SizeEstimate::Range { min, max }
        }
    }

    /// 单分配点的上界大小。
    fn max_size_for_site(&self, site: &AllocSite) -> usize {
        match site.frequency {
            ExecFrequency::Once => site.type_size,
            ExecFrequency::Loop { max_iterations } => {
                site.type_size.saturating_mul(max_iterations)
            }
            ExecFrequency::Unknown => site.type_size,
        }
    }
}
