//! # zeta-driver
//!
//! Zeta 编译器驱动：串联完整编译流水线
//!
//! ```text
//! 源码 → typecheck（含 parse）→ borrowck → regionck
//!      → MIR lowering + 优化 → LIR lowering → LLVM IR 文本 → clang → 可执行文件
//! ```
//!
//! 提供库级 API（[`compile_to_llvm`] / [`build_executable`] / [`run_source`]）
//! 与增量编译入口（[`IncrementalDriver`]），以及 CLI 二进制
//! （`zeta run` / `zeta build`，见 `main.rs`）。

#![warn(missing_docs)]
#![warn(unsafe_code)]

pub mod error;
pub mod incremental;
mod module;
mod stdlib;

use std::path::{Path, PathBuf};
use std::process::Command;

use error::DriverError;
pub use incremental::cache::CacheStats;
use incremental::cache::IncrementalCache;
use incremental::hash::{compute_interface_hash, compute_source_hash, extract_interface};
use zeta_borrowck::BorrowChecker;
use zeta_regionck::RegionChecker;

/// 一次编译的结果（含缓存状态，供 CLI 报告）。
#[derive(Debug, Clone)]
pub struct BuildOutcome {
    /// 生成的 LLVM IR 文本
    pub llvm: String,
    /// 是否命中增量缓存（跳过完整流水线）
    pub cache_hit: bool,
    /// 截至本次编译的缓存统计
    pub stats: CacheStats,
}

/// 执行完整流水线，返回 LLVM IR 文本（无缓存）。
pub fn compile_to_llvm(source: &str) -> Result<String, DriverError> {
    full_pipeline(source)
}

/// 编译源码为可执行文件（LLVM IR → clang 汇编 / 链接），无缓存。
///
/// `out_path` 指定产物路径（父目录需存在）。
pub fn build_executable(source: &str, out_path: &Path) -> Result<(), DriverError> {
    let llvm = full_pipeline(source)?;
    assemble(&llvm, out_path)
}

/// 编译并运行源码，返回程序标准输出（UTF-8），无缓存。
pub fn run_source(source: &str) -> Result<String, DriverError> {
    let dir = temp_dir();
    let exe = dir.join("zeta-run");
    build_executable(source, &exe)?;
    let out = run_exe(&exe)?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(out)
}

/// 编译入口文件（含 `mod foo;` 外部模块）为 LLVM IR 文本，无缓存。
///
/// 入口文件所在目录下的 `<name>.zeta` 或 `<name>/mod.zeta` 会被自动加载，
/// 所有模块文件合并为单一符号空间（扁平符号名 `mod::item`）。
/// 标准库预置（`zeta-std/zeta/core.zeta`，若存在）自动注入为前缀。
pub fn compile_file_to_llvm(entry: &Path) -> Result<String, DriverError> {
    let source = module::load_combined_source(entry)?;
    full_pipeline(&source_with_std(source, false)?)
}

/// 编译入口文件（含外部模块）为可执行文件，无缓存。
pub fn build_executable_file(entry: &Path, out_path: &Path) -> Result<(), DriverError> {
    let llvm = compile_file_to_llvm(entry)?;
    assemble(&llvm, out_path)
}

/// 编译并运行入口文件（含外部模块），返回程序标准输出（UTF-8），无缓存。
pub fn run_source_file(entry: &Path) -> Result<String, DriverError> {
    let dir = temp_dir();
    let exe = dir.join("zeta-run");
    build_executable_file(entry, &exe)?;
    let out = run_exe(&exe)?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(out)
}

/// 增量编译驱动：源码哈希命中时跳过完整流水线，直接复用缓存 LLVM IR。
///
/// 缓存目录为 `<cache_dir>/.zeta_cache`；`--force` 语义下强制全量重编译。
pub struct IncrementalDriver {
    /// 缓存根（`.zeta_cache` 的父目录）
    cache_dir: PathBuf,
    /// 强制全量重编译（忽略缓存）
    force: bool,
    /// 禁用标准库预置注入（`--no-std`）
    no_std: bool,
    /// 会话内缓存统计
    stats: CacheStats,
}

impl IncrementalDriver {
    /// 创建增量编译驱动。
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            force: false,
            no_std: false,
            stats: CacheStats::default(),
        }
    }

    /// 强制全量重编译（忽略缓存）。
    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    /// 禁用标准库预置注入（文件入口 API 默认注入 `core.zeta`）。
    pub fn with_no_std(mut self, no_std: bool) -> Self {
        self.no_std = no_std;
        self
    }

    /// 会话内缓存统计。
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// 增量编译源码，返回 LLVM IR（命中缓存或全量编译后写入缓存）。
    ///
    /// `file` 是缓存键（通常为源文件路径）。
    pub fn compile_to_llvm(
        &mut self,
        file: &str,
        source: &str,
    ) -> Result<BuildOutcome, DriverError> {
        let mut cache = IncrementalCache::open(&self.cache_dir)?;
        if cache.was_recovered() {
            self.stats.recovered += 1;
        }

        let source_hash = compute_source_hash(source);

        // 1. 缓存查找：源码哈希一致 + 产物存在 → 直接复用
        if !self.force {
            if let Some(llvm) = cache.lookup_llvm(file, &source_hash)? {
                self.stats.hits += 1;
                return Ok(BuildOutcome {
                    llvm,
                    cache_hit: true,
                    stats: self.stats.clone(),
                });
            }
        }

        // 2. 全量编译 + 写入缓存
        self.stats.misses += 1;
        let llvm = full_pipeline(source)?;
        let interface = extract_interface(source)?;
        let interface_hash = compute_interface_hash(&interface);
        cache.store_llvm(file, &source_hash, &interface_hash, &llvm)?;
        let _ = cache.clean_stale();

        Ok(BuildOutcome {
            llvm,
            cache_hit: false,
            stats: self.stats.clone(),
        })
    }

    /// 增量编译并构建可执行文件。
    pub fn build_executable(
        &mut self,
        file: &str,
        source: &str,
        out_path: &Path,
    ) -> Result<BuildOutcome, DriverError> {
        let outcome = self.compile_to_llvm(file, source)?;
        assemble(&outcome.llvm, out_path)?;
        Ok(outcome)
    }

    /// 增量编译、构建并运行，返回程序标准输出。
    pub fn run_source(
        &mut self,
        file: &str,
        source: &str,
    ) -> Result<(String, BuildOutcome), DriverError> {
        let dir = temp_dir();
        let exe = dir.join("zeta-run");
        let outcome = self.build_executable(file, source, &exe)?;
        let stdout = run_exe(&exe)?;
        let _ = std::fs::remove_dir_all(&dir);
        Ok((stdout, outcome))
    }

    /// 增量编译入口文件（含外部模块），返回 LLVM IR。
    ///
    /// 缓存键为入口文件路径；组合源码哈希覆盖全部模块文件与标准库预置，
    /// 任一模块/标准库变更都会触发重新编译。
    pub fn compile_file_to_llvm(&mut self, entry: &Path) -> Result<BuildOutcome, DriverError> {
        let combined = source_with_std(module::load_combined_source(entry)?, self.no_std)?;
        let key = entry.to_string_lossy().to_string();
        self.compile_to_llvm(&key, &combined)
    }

    /// 增量编译入口文件（含外部模块）并构建可执行文件。
    pub fn build_executable_file(
        &mut self,
        entry: &Path,
        out_path: &Path,
    ) -> Result<BuildOutcome, DriverError> {
        let outcome = self.compile_file_to_llvm(entry)?;
        assemble(&outcome.llvm, out_path)?;
        Ok(outcome)
    }

    /// 增量编译入口文件（含外部模块）、构建并运行，返回程序标准输出。
    pub fn run_source_file(
        &mut self,
        entry: &Path,
    ) -> Result<(String, BuildOutcome), DriverError> {
        let dir = temp_dir();
        let exe = dir.join("zeta-run");
        let outcome = self.build_executable_file(entry, &exe)?;
        let stdout = run_exe(&exe)?;
        let _ = std::fs::remove_dir_all(&dir);
        Ok((stdout, outcome))
    }
}

/// 组合入口源码与标准库预置（`--no-std` 时原样返回）。
fn source_with_std(source: String, no_std: bool) -> Result<String, DriverError> {
    if no_std {
        return Ok(source);
    }
    Ok(match stdlib::load_std_prelude()? {
        Some(prelude) => format!("{prelude}\n{source}"),
        None => source,
    })
}

/// 完整流水线：typecheck → borrowck → regionck → MIR(+优化) → LIR → LLVM IR。
fn full_pipeline(source: &str) -> Result<String, DriverError> {
    // 1. 类型检查（内部完成 lex + parse → HIR）
    let hir = zeta_typecheck::typecheck_source(source)
        .map_err(|e| DriverError::Typecheck(e.to_string()))?;

    // 2. 借用检查（L0 所有权）
    BorrowChecker::new()
        .check_program(&hir)
        .map_err(|errs| DriverError::Borrow(join_errors(errs)))?;

    // 3. 区域检查（L1 区域系统）
    RegionChecker::new()
        .check_program(&hir)
        .map_err(|errs| DriverError::Region(join_errors(errs)))?;

    // 4. MIR lowering + 优化
    let mut mir = zeta_mir::lower::lower_program(&hir);
    zeta_mir::passes::optimize(&mut mir);

    // 5. LIR lowering
    let lir = zeta_lir::lower::lower_program(&mir).map_err(|e| DriverError::Lir(e.to_string()))?;

    // 6. LLVM IR 文本生成
    zeta_codegen::generate_llvm(&lir).map_err(|e| DriverError::Codegen(e.to_string()))
}

/// LLVM IR 文本 → clang 汇编 / 链接 → 可执行文件。
fn assemble(llvm: &str, out_path: &Path) -> Result<(), DriverError> {
    // 输出路径的父目录必须存在（链接器无法创建）
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(DriverError::Io)?;
    }
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).map_err(DriverError::Io)?;
    let ll_path = dir.join("main.ll");
    std::fs::write(&ll_path, llvm).map_err(DriverError::Io)?;

    let clang = clang_path();
    let result = Command::new(&clang)
        .arg(&ll_path)
        .arg("-o")
        .arg(out_path)
        .output()
        .map_err(|e| DriverError::Clang(format!("无法启动 `{clang}`: {e}")))?;

    let _ = std::fs::remove_dir_all(&dir);
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        return Err(DriverError::Clang(stderr));
    }
    Ok(())
}

/// 执行可执行文件并捕获标准输出。
fn run_exe(exe: &Path) -> Result<String, DriverError> {
    let out = Command::new(exe)
        .output()
        .map_err(|e| DriverError::Run(format!("无法运行产物: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(DriverError::Run(format!(
            "程序退出码 {:?}: {stderr}",
            out.status.code()
        )));
    }
    String::from_utf8(out.stdout).map_err(|e| DriverError::Run(format!("输出非 UTF-8: {e}")))
}

/// 独立临时目录：系统临时目录 + 进程 id + 时间戳 + 进程内原子序号，
/// 保证并行测试（多线程并发调用）下路径唯一，避免产物互相覆盖。
static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn temp_dir() -> PathBuf {
    let seq = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "zeta-mvp-{}-{}-{seq}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ))
}

/// 拼接错误列表为单行文本。
fn join_errors<T: std::fmt::Display>(errs: Vec<T>) -> String {
    errs.iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

/// 查找可用的 C 编译器（优先 clang，回退 cc）。
fn clang_path() -> String {
    for cand in ["clang", "cc"] {
        let status = Command::new(cand)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if status.is_ok_and(|s| s.success()) {
            return cand.to_string();
        }
    }
    "clang".to_string()
}
