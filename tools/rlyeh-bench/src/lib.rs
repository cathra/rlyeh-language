//! # rlyeh-bench
//!
//! Rlyeh 语言基准测试框架：对编译产物多次运行计时，输出统计摘要
//! （平均 / 中位数 / 最小 / 最大 / 标准差 / 吞吐）。
//!
//! ## 使用方式
//!
//! - [`bench_executable`]：对已编译的可执行文件计时。
//! - [`bench_source`]：先通过编译回调生成产物，再计时。

#![warn(missing_docs)]
#![warn(unsafe_code)]

use std::fmt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

/// 基准选项。
#[derive(Debug, Clone)]
pub struct BenchOptions {
    /// 预热轮数（不计入统计，用于消除冷启动 / 懒初始化影响）
    pub warmup: usize,
    /// 测量轮数（计入统计）
    pub runs: usize,
    /// 静默模式：丢弃被测程序的标准输出与错误（常用于自动化）
    pub quiet: bool,
}

impl Default for BenchOptions {
    fn default() -> Self {
        Self {
            warmup: 2,
            runs: 10,
            quiet: false,
        }
    }
}

/// 基准统计报告。
#[derive(Debug, Clone)]
pub struct BenchReport {
    /// 每次运行的耗时（秒），长度为 `runs`
    pub measurements: Vec<f64>,
    /// 平均耗时（秒）
    pub mean: f64,
    /// 中位数耗时（秒）
    pub median: f64,
    /// 最小耗时（秒）
    pub min: f64,
    /// 最大耗时（秒）
    pub max: f64,
    /// 样本标准差（秒）
    pub stddev: f64,
    /// 吞吐（次/秒）
    pub ops_per_sec: f64,
}

impl BenchReport {
    /// 从一组耗时（秒）计算统计摘要。
    pub fn from_measurements(mut ms: Vec<f64>) -> Self {
        ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = ms.len() as f64;
        let sum: f64 = ms.iter().sum();
        let mean = if n > 0.0 { sum / n } else { 0.0 };
        let median = if ms.is_empty() {
            0.0
        } else if ms.len() % 2 == 1 {
            ms[ms.len() / 2]
        } else {
            (ms[ms.len() / 2 - 1] + ms[ms.len() / 2]) / 2.0
        };
        let min = ms.first().copied().unwrap_or(0.0);
        let max = ms.last().copied().unwrap_or(0.0);
        let variance = if ms.len() > 1 {
            ms.iter().map(|m| (m - mean).powi(2)).sum::<f64>() / (ms.len() - 1) as f64
        } else {
            0.0
        };
        let stddev = variance.sqrt();
        let ops_per_sec = if sum > 0.0 {
            ms.len() as f64 / sum
        } else {
            0.0
        };
        Self {
            measurements: ms,
            mean,
            median,
            min,
            max,
            stddev,
            ops_per_sec,
        }
    }
}

impl fmt::Display for BenchReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "测量 {} 次:", self.measurements.len())?;
        writeln!(f, "  平均: {}", fmt_duration(self.mean))?;
        writeln!(f, "  中位数: {}", fmt_duration(self.median))?;
        writeln!(
            f,
            "  最小: {} / 最大: {}",
            fmt_duration(self.min),
            fmt_duration(self.max)
        )?;
        writeln!(f, "  标准差: {}", fmt_duration(self.stddev))?;
        write!(f, "  吞吐: {:.2} ops/s", self.ops_per_sec)
    }
}

/// 将秒数格式化为可读单位。
fn fmt_duration(secs: f64) -> String {
    if secs >= 1.0 {
        format!("{:.3} s", secs)
    } else if secs >= 1e-3 {
        format!("{:.3} ms", secs * 1e3)
    } else if secs >= 1e-6 {
        format!("{:.3} µs", secs * 1e6)
    } else {
        format!("{:.3} ns", secs * 1e9)
    }
}

/// 对已编译产物执行基准测量。
///
/// 先运行 `warmup` 次预热，再运行 `runs` 次测量。被测程序任一非零退出码
/// 都会导致 `Err`。
pub fn bench_executable(path: &Path, opts: &BenchOptions) -> Result<BenchReport, String> {
    if !path.exists() {
        return Err(format!("产物不存在: {}", path.display()));
    }
    for _ in 0..opts.warmup {
        run_once(path, opts.quiet)?;
    }
    let mut ms = Vec::with_capacity(opts.runs);
    for _ in 0..opts.runs {
        ms.push(run_once(path, opts.quiet)?);
    }
    Ok(BenchReport::from_measurements(ms))
}

/// 单次运行并计时（秒）。
fn run_once(path: &Path, quiet: bool) -> Result<f64, String> {
    let start = Instant::now();
    let mut cmd = Command::new(path);
    if quiet {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    } else {
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    }
    let status = cmd
        .status()
        .map_err(|e| format!("无法运行 {}: {e}", path.display()))?;
    if !status.success() {
        return Err(format!(
            "程序退出码: {:?}（{}）",
            status.code(),
            path.display()
        ));
    }
    Ok(start.elapsed().as_secs_f64())
}

/// 编译源码后执行基准测量。
///
/// `compile` 负责把 `src` 编译到 `out`，返回 `Err` 时基准终止。
pub fn bench_source(
    src: &Path,
    out: &Path,
    opts: &BenchOptions,
    compile: impl FnOnce(&Path, &Path) -> Result<(), String>,
) -> Result<BenchReport, String> {
    compile(src, out)?;
    bench_executable(out, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_statistics() {
        let r = BenchReport::from_measurements(vec![0.010, 0.020, 0.030]);
        assert_eq!(r.measurements.len(), 3);
        assert!((r.mean - 0.020).abs() < 1e-12);
        assert!((r.median - 0.020).abs() < 1e-12);
        assert!((r.min - 0.010).abs() < 1e-12);
        assert!((r.max - 0.030).abs() < 1e-12);
        assert!(r.stddev > 0.0);
        assert!((r.ops_per_sec - 50.0).abs() < 1e-9);
    }

    #[test]
    fn report_handles_single_and_empty() {
        let single = BenchReport::from_measurements(vec![0.5]);
        assert!((single.median - 0.5).abs() < 1e-12);
        assert_eq!(single.stddev, 0.0);
        let empty = BenchReport::from_measurements(vec![]);
        assert_eq!(empty.mean, 0.0);
        assert_eq!(empty.median, 0.0);
        assert_eq!(empty.ops_per_sec, 0.0);
    }

    #[test]
    fn duration_formatting() {
        assert!(fmt_duration(1.5).contains("s"));
        assert!(fmt_duration(0.002).contains("ms"));
        assert!(fmt_duration(2e-7).contains("ns"));
    }
}
