//! # rlyeh-driver
//!
//! Rlyeh 编译器驱动：串联完整编译流水线
//!
//! ```text
//! 源码 → typecheck（含 parse）→ borrowck → regionck
//!      → MIR lowering + 优化 → LIR lowering → LLVM IR 文本 → clang → 可执行文件
//! ```
//!
//! 提供库级 API（[`compile_to_llvm`] / [`build_executable`] / [`run_source`]）
//! 与增量编译入口（[`IncrementalDriver`]），以及 CLI 二进制
//! （`rlyeh run` / `rlyeh build`，见 `main.rs`）。

#![warn(missing_docs)]
#![warn(unsafe_code)]

pub mod error;
pub mod incremental;
mod module;
mod stdlib;
pub mod test_runner;

use std::path::{Path, PathBuf};
use std::process::Command;

use error::DriverError;
pub use incremental::cache::CacheStats;
use incremental::cache::IncrementalCache;
use incremental::hash::{compute_interface_hash, compute_source_hash, extract_interface};
use rlyeh_borrowck::BorrowChecker;
use rlyeh_regionck::RegionChecker;

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
    build_executable_with_target(source, out_path, None)
}

/// 指定 LLVM 目标 triple 构建可执行文件（`None` 为主机目标）。
pub fn build_executable_with_target(
    source: &str,
    out_path: &Path,
    target: Option<&str>,
) -> Result<(), DriverError> {
    let llvm = full_pipeline(source)?;
    assemble(&llvm, out_path, target)
}

/// 编译并运行源码，返回程序标准输出（UTF-8），无缓存。
///
/// 编译深递归程序（async/HTTP 状态机）时 Rlyeh 编译器（Rust 代码）递归较深，
/// macOS 上默认线程栈仅 ~2MB 会溢出；与 run-pass worker 线程一致（见
/// `test_runner.rs`），在 64MB 栈线程中执行编译+运行。Linux 默认 8MB 栈不受影响。
pub fn run_source(source: &str) -> Result<String, DriverError> {
    let source = source.to_string();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = temp_dir();
            let exe = dir.join("rlyeh-run");
            build_executable(&source, &exe)?;
            let out = run_exe(&exe)?;
            let _ = std::fs::remove_dir_all(&dir);
            Ok(out)
        })
        .map_err(|e| DriverError::Run(format!("无法启动编译线程: {e}")))?
        .join()
        .map_err(|_| DriverError::Run("编译线程 panic".to_string()))?
}

/// 编译入口文件（含 `module foo;` 外部模块）为 LLVM IR 文本，无缓存。
///
/// 入口文件所在目录下的 `<name>.rl` 或 `<name>/module.rl` 会被自动加载，
/// 所有模块文件合并为单一符号空间（扁平符号名 `模块名::item`）。
/// 标准库预置（`rlyeh-std/rlyeh/core.rl`，若存在）自动注入为前缀。
pub fn compile_file_to_llvm(entry: &Path) -> Result<String, DriverError> {
    let source = module::load_combined_source(entry)?;
    full_pipeline(&source_with_std(source, false)?)
}

/// 编译入口文件（含外部模块）为可执行文件，无缓存。
pub fn build_executable_file(entry: &Path, out_path: &Path) -> Result<(), DriverError> {
    build_executable_file_with_target(entry, out_path, None)
}

/// 指定 LLVM 目标 triple 编译入口文件（含外部模块）并构建可执行文件。
pub fn build_executable_file_with_target(
    entry: &Path,
    out_path: &Path,
    target: Option<&str>,
) -> Result<(), DriverError> {
    let llvm = compile_file_to_llvm(entry)?;
    assemble(&llvm, out_path, target)
}

/// 编译并运行入口文件（含外部模块），返回程序标准输出（UTF-8），无缓存。
///
/// 编译深递归程序（async/HTTP 状态机）时 Rlyeh 编译器（Rust 代码）递归较深，
/// macOS 上默认线程栈仅 ~2MB 会溢出；与 run-pass worker 线程一致（见
/// `test_runner.rs`），在 64MB 栈线程中执行编译+运行。Linux 默认 8MB 栈不受影响。
pub fn run_source_file(entry: &Path) -> Result<String, DriverError> {
    let entry = entry.to_path_buf();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = temp_dir();
            let exe = dir.join("rlyeh-run");
            build_executable_file(&entry, &exe)?;
            let out = run_exe(&exe)?;
            let _ = std::fs::remove_dir_all(&dir);
            Ok(out)
        })
        .map_err(|e| DriverError::Run(format!("无法启动编译线程: {e}")))?
        .join()
        .map_err(|_| DriverError::Run("编译线程 panic".to_string()))?
}

/// 读取源码文件并执行静态检查（`rlyeh check`）。
///
/// 解析失败返回 `parse-error` 诊断；否则返回 lint 规则诊断。
/// 仅负责分析，不涉及完整编译流水线。
pub fn check_source_file(path: &Path) -> Result<Vec<rlyeh_check::Diagnostic>, DriverError> {
    let source = std::fs::read_to_string(path).map_err(DriverError::Io)?;
    Ok(rlyeh_check::check_source(&source))
}

/// 读取源码文件并生成 Markdown 文档（`rlyeh doc`）。
pub fn doc_source_file(path: &Path, options: &rlyeh_doc::DocOptions) -> Result<String, DriverError> {
    let source = std::fs::read_to_string(path).map_err(DriverError::Io)?;
    rlyeh_doc::doc_source(&source, options).map_err(DriverError::Doc)
}

/// 读取 PGO 画像文件并生成区域大小预测报告（`rlyeh profile`，阶段 F2 数据回灌消费侧）。
///
/// `.rl_profile` 为 JSON 格式（见 `rlyeh_region_alloc::profile`）：
/// 运行时按区域记录的分配量统计（p50/p90/p95/mean/max 等）。
/// 本函数加载画像 → 以 p95×安全系数预测初始区域大小 → 输出编译决策报告
/// （复用 `rlyeh_region_alloc::PgoAdvisor` / `CompilerInterface`）。
/// 语言级 region 接线后，该预测可直接回灌 `region 'r adaptive` 的初始容量。
pub fn region_profile_report(path: &Path) -> Result<String, DriverError> {
    let text = std::fs::read_to_string(path).map_err(DriverError::Io)?;
    let data: rlyeh_region_alloc::PgoData = serde_json::from_str(&text)
        .map_err(|e| DriverError::Profile(format!("解析 `.rl_profile` 失败: {e}")))?;
    Ok(build_region_report(&data))
}

/// 读取 `.rl_profile`，为每个区域生成 L3 PGO 回灌提示（区域名 → 推荐初始容量）。
///
/// 推荐容量 = `PgoAdvisor::recommend_size`（p95 × 安全系数，下限 64KiB）。
/// 注入到 `adaptive` 区域后，`rlyeh_region_enter` 以该容量创建初始块。
pub fn region_hints_from_profile(
    path: &Path,
) -> Result<std::collections::HashMap<String, usize>, DriverError> {
    let text = std::fs::read_to_string(path).map_err(DriverError::Io)?;
    let data: rlyeh_region_alloc::PgoData = serde_json::from_str(&text)
        .map_err(|e| DriverError::Profile(format!("解析 `.rl_profile` 失败: {e}")))?;
    let advisor = rlyeh_region_alloc::PgoAdvisor::from_data(data.clone());
    let mut hints = std::collections::HashMap::new();
    for id in data.region_ids() {
        if let Some(size) = advisor.recommend_size(&id) {
            hints.insert(id, size);
        }
    }
    Ok(hints)
}

/// 依据 PGO 数据生成区域分配决策报告（纯函数，便于单元测试）。
///
/// 每个区域：`estimated_size` = 历史平均分配量（静态基线）、
/// `initial_size` = PGO 推荐（p95×安全系数，下限 64KiB）、
/// `max_size` = 推荐×4 与历史峰值取大、`decision` 描述依据。
pub fn build_region_report(data: &rlyeh_region_alloc::PgoData) -> String {
    let advisor = rlyeh_region_alloc::PgoAdvisor::from_data(data.clone());
    let mut interface =
        rlyeh_region_alloc::CompilerInterface::new().with_pgo_data(data.clone());
    let mut ids = data.region_ids();
    ids.sort_unstable();
    for id in &ids {
        let region = data
            .region_profile(id)
            .expect("region id from region_ids() must exist");
        let initial = advisor
            .recommend_size(id)
            .unwrap_or(region.size_stats.mean);
        let max = (initial * 4).max(region.size_stats.max);
        let decision = format!(
            "PGO p95 {}B × safety 1.1（下限 64KiB）；p50 {}B / mean {}B / max {}B",
            region.size_stats.p95, region.size_stats.p50, region.size_stats.mean,
            region.size_stats.max
        );
        interface = interface.register_region(rlyeh_region_alloc::RegionCompileInfo {
            region_id: id.to_string(),
            estimated_size: region.size_stats.mean,
            initial_size: initial,
            max_size: max,
            decision,
        });
    }
    interface.generate_report()
}

/// 增量编译驱动：源码哈希命中时跳过完整流水线，直接复用缓存 LLVM IR。
///
/// 缓存目录为 `<cache_dir>/.rlyeh_cache`；`--force` 语义下强制全量重编译。
pub struct IncrementalDriver {
    /// 缓存根（`.rlyeh_cache` 的父目录）
    cache_dir: PathBuf,
    /// 强制全量重编译（忽略缓存）
    force: bool,
    /// 禁用标准库预置注入（`--no-std`）
    no_std: bool,
    /// LLVM 目标 triple（`--target` 交叉编译；`None` = 主机目标）
    target: Option<String>,
    /// 会话内缓存统计
    stats: CacheStats,
    /// L3 PGO 回灌：区域名 → 推荐初始容量（`rlyeh build --profile` 注入）
    region_hints: std::collections::HashMap<String, usize>,
}

impl IncrementalDriver {
    /// 创建增量编译驱动。
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            force: false,
            no_std: false,
            target: None,
            stats: CacheStats::default(),
            region_hints: std::collections::HashMap::new(),
        }
    }

    /// 注入 L3 PGO 回灌提示（区域名 → 推荐初始容量），
    /// 供 `adaptive` 区域在编译期采用推荐容量。
    pub fn with_region_hints(mut self, hints: std::collections::HashMap<String, usize>) -> Self {
        self.region_hints = hints;
        self
    }

    /// 强制全量重编译（忽略缓存）。
    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    /// 禁用标准库预置注入（文件入口 API 默认注入 `core.rl`）。
    pub fn with_no_std(mut self, no_std: bool) -> Self {
        self.no_std = no_std;
        self
    }

    /// 指定 LLVM 目标 triple（交叉编译；`None` 为主机目标）。
    pub fn with_target(mut self, target: Option<String>) -> Self {
        self.target = target;
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
        let llvm = full_pipeline_with_hints(source, &self.region_hints)?;
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
        assemble(&outcome.llvm, out_path, self.target.as_deref())?;
        Ok(outcome)
    }

    /// 增量编译、构建并运行，返回程序标准输出。
    pub fn run_source(
        &mut self,
        file: &str,
        source: &str,
    ) -> Result<(String, BuildOutcome), DriverError> {
        let dir = temp_dir();
        let exe = dir.join("rlyeh-run");
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
        assemble(&outcome.llvm, out_path, self.target.as_deref())?;
        Ok(outcome)
    }

    /// 增量编译入口文件（含外部模块）、构建并运行，返回程序标准输出。
    pub fn run_source_file(
        &mut self,
        entry: &Path,
    ) -> Result<(String, BuildOutcome), DriverError> {
        let dir = temp_dir();
        let exe = dir.join("rlyeh-run");
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
    full_pipeline_with_hints(source, &std::collections::HashMap::new())
}

/// 完整流水线，注入 L3 PGO 回灌提示（区域名 → 推荐初始容量）。
fn full_pipeline_with_hints(
    source: &str,
    region_hints: &std::collections::HashMap<String, usize>,
) -> Result<String, DriverError> {
    // 1. 类型检查（内部完成 lex + parse → HIR）
    let hir = rlyeh_typecheck::typecheck_source_with_region_hints(source, region_hints)
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
    let mut mir = rlyeh_mir::lower::lower_program(&hir);
    rlyeh_mir::passes::optimize(&mut mir);

    // 5. LIR lowering
    let lir = rlyeh_lir::lower::lower_program(&mir).map_err(|e| DriverError::Lir(e.to_string()))?;

    // 6. LLVM IR 文本生成
    rlyeh_codegen::generate_llvm(&lir).map_err(|e| DriverError::Codegen(e.to_string()))
}

/// LLVM IR 文本 → clang 汇编 / 链接 → 可执行文件。
///
/// `target` 为 LLVM 目标 triple（`--target` 交叉编译）；`None` 表示主机目标。
fn assemble(llvm: &str, out_path: &Path, target: Option<&str>) -> Result<(), DriverError> {
    // 输出路径的父目录必须存在（链接器无法创建）
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(DriverError::Io)?;
    }
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).map_err(DriverError::Io)?;
    let ll_path = dir.join("main.ll");
    // 注入平台内建（`__rlyeh_target_os`）后写盘——codegen 对 `__rlyeh_` 前缀 extern 不生成 declare。
    let llvm_with_builtins = format!("{llvm}\n{}", platform_builtin_ir(target, &llvm));
    // L4b: wasm 目标注入 actor 符号解析表（静态查表替代 dlsym，见 actor_resolve_ir）。
    let llvm_final = match target {
        Some(t) if is_wasm_triple(t) => {
            let actor_ir = actor_resolve_ir(&llvm_with_builtins);
            if actor_ir.is_empty() {
                llvm_with_builtins
            } else {
                format!("{llvm_with_builtins}\n{actor_ir}")
            }
        }
        _ => llvm_with_builtins,
    };
    std::fs::write(&ll_path, llvm_final).map_err(DriverError::Io)?;

    // WebAssembly 目标走独立汇编链路（wasi-libc sysroot + wasm-ld），其余走系统链接器。
    if let Some(t) = target {
        if is_wasm_triple(t) {
            return assemble_wasm(&ll_path, out_path, t, &dir);
        }
        // Y（2026-08-28）：跨 OS ELF 目标（Linux/RISC-V/LoongArch musl）——宿主
        // clang 的 ld 无法链接跨 OS 目标（macOS ld 不认识 `--hash-style` 等），
        // 改用 rust-lld（多目标链接器）+ Rust musl sysroot 的 crt/libc 链接。
        if is_elf_musl_target(t) && is_cross_target(target) {
            return assemble_cross_elf(&ll_path, out_path, t, &dir);
        }
    }

    let clang = clang_path();
    // 运行时 C ABI（staticlib）：Rlyeh 程序经 `extern fn rlyeh_actor_*` /
    // `extern fn rlyeh_gc_*` 调用。链接器按需提取对象——不含相应特性的程序不受影响
    // （库可缺失则跳过）。交叉编译到其他架构时本机 staticlib 无法链接，跳过并提示。
    let runtime_libs: Vec<PathBuf> = if is_cross_target(target) {
        eprintln!(
            "rlyeh: 提示: 交叉编译目标 `{}` 与主机架构不同，跳过 Actor / GC 运行时库（相关特性暂不支持交叉编译）",
            target.unwrap_or_default()
        );
        Vec::new()
    } else {
        [actor_runtime_lib_path(), gc_runtime_lib_path(), region_runtime_lib_path()]
            .into_iter()
            .flatten()
            .collect()
    };
    let mut cmd = Command::new(&clang);
    if let Some(t) = target {
        cmd.arg(format!("--target={t}"));
    }
    // 发布级优化：clang 汇编 LLVM IR 时启用 -O3 优化管线
    // （-O2 全部优化 + 更强的循环向量化 / 函数内联 / 循环变换）。
    // 生成的 IR 不含 noalias/nsw/nuw 标注，LLVM 的优化保守安全
    // （-O3 不依赖这些标注，无错误别名假设）。
    cmd.arg("-O3");
    cmd.arg(&ll_path);
    for lib in &runtime_libs {
        if let Some(parent) = lib.parent() {
            cmd.arg("-L").arg(parent);
        }
        cmd.arg("-l").arg(
            lib.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.trim_start_matches("lib").trim_end_matches(".a"))
                .unwrap_or("rlyeh_runtime"),
        );
    }
    cmd.arg("-o").arg(out_path);
    let result = cmd.output().map_err(|e| {
        DriverError::Clang(format!("无法启动 `{clang}`: {e}"))
    })?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        return Err(DriverError::Clang(stderr));
    }
    // 调试：设置 RLYEH_KEEP_TMP=1 时保留临时目录（含 main.ll 中间 IR）
    if std::env::var("RLYEH_KEEP_TMP").is_err() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    Ok(())
}

/// 指定目标 triple 是否为 WebAssembly（`wasm32` / `wasm64`）。
pub fn is_wasm_triple(triple: &str) -> bool {
    triple.starts_with("wasm32") || triple.starts_with("wasm64")
}

/// WebAssembly 汇编：clang 交叉编译 LLVM IR 为 wasm 目标，链接 wasi-libc 生成 `.wasm`。
///
/// 依赖（缺一即报错并提示安装）：
/// - wasi-libc（`brew install wasi-libc`，或 `WASI_SYSROOT` 环境变量指向 sysroot）
/// - wasm-ld（`brew install lld`；已在 PATH 则直接使用）
///
/// 入口语义：WASI 的 `_start` 由 wasi-libc crt1.o 提供并调用 Rlyeh 生成的 `main()`；
/// 标准库中 WASI 不存在的 extern 符号（socket/pthread 等）仅在被引用时才会链接，
/// 未使用的模块不会引入未定义符号。
fn assemble_wasm(
    ll_path: &Path,
    out_path: &Path,
    target: &str,
    dir: &Path,
) -> Result<(), DriverError> {
    let sysroot = wasi_sysroot().ok_or_else(|| {
        DriverError::Clang(
            "wasm 目标需要 wasi-libc sysroot：brew install wasi-libc，或设置 WASI_SYSROOT 环境变量"
                .to_string(),
        )
    })?;
    // wasi-libc 33 起为多目标布局 `lib/wasm32-wasi/`（含 crt1.o/libc.a）；旧版直接位于 `lib/`。
    let lib_dir = {
        let multi = sysroot.join("lib").join("wasm32-wasi");
        if multi.join("crt1.o").exists() {
            multi
        } else {
            sysroot.join("lib")
        }
    };
    let clang = clang_path();
    // WASI 入口适配：wasi-libc 的 `__main_void`（crt1 链）调用
    // `__main_argc_argv(int argc, char **argv)`，而 Rlyeh 生成的是无参 `@main`。
    // 将 IR 中的 main 定义重命名为 `__main_argc_argv` 并补齐 ABI 参数，
    // 使 crt1.o 能解析入口；同时避免 clang 为无参 main 生成额外包装符号。
    let mut ll = std::fs::read_to_string(ll_path).map_err(DriverError::Io)?;
    // wasm32 指针/尺寸位宽适配：wasi-libc 的 malloc(size_t)/memcmp(size_t)
    // 为 32 位参数，而 Rlyeh 的 IR 按 64 位（isize）声明调用；将声明与调用的
    // 参数位宽降为 i32（值用 trunc 包装），避免 import 签名不匹配 trap。
    fn adapt_wide_int_args(src: &str, marker: &str) -> String {
        let mut out = String::with_capacity(src.len());
        let mut rest = src;
        let mut counter = 0;
        while let Some(idx) = rest.find(marker) {
            out.push_str(&rest[..idx + marker.len()]);
            rest = &rest[idx + marker.len()..];
            let end = rest.find(')').unwrap_or(rest.len());
            let arg = &rest[..end];
            if let Some(inner) = arg.strip_prefix("i64 ") {
                let inner = inner.trim();
                if inner.starts_with('%') {
                    // trunc 是 instruction 不能内联在 call 参数位：在 call 行前
                    // 插入 trunc 定义，参数改用新寄存器。
                    let tmp = format!("%rlyeh.wasm32.{counter}");
                    counter += 1;
                    if let Some(nl) = out.rfind('\n') {
                        out.insert_str(
                            nl + 1,
                            &format!("  {tmp} = trunc i64 {inner} to i32\n"),
                        );
                    }
                    out.push_str(&format!("i32 {tmp}"));
                } else {
                    out.push_str(&format!("i32 {inner}"));
                }
            } else {
                out.push_str(arg);
            }
            rest = &rest[end..];
        }
        out.push_str(rest);
        out
    }
    // calloc 调用点适配：两个参数均降位宽（i64→i32，寄存器实参经 trunc 包装），
    // 并收集返回寄存器，随后把 `inttoptr i64 %r64 to i8*` 降宽为 i32（wasm32
    // 指针 32 位，i32 值直转指针）。
    fn adapt_calloc_wasm32(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        let mut rest = src;
        let marker = "call i64 @calloc(";
        let mut counter = 0;
        let mut ret_regs: Vec<String> = Vec::new();
        while let Some(idx) = rest.find(marker) {
            // 提取返回寄存器（`%r64 = call i64 @calloc(` 模式，行首缩进）。
            let head = &rest[..idx];
            if let Some(eq) = head.rfind('=') {
                if let Some(reg) = head[..eq].trim().rsplit(' ').next() {
                    if reg.starts_with('%') {
                        ret_regs.push(reg.to_string());
                    }
                }
            }
            // 调用点结果类型同步降为 i32（declare 已为 i32；若保持 i64，
            // LLVM 会为「i64 结果 ← i32 返回」插入 bitcast 并生成非法陷阱）。
            out.push_str(&rest[..idx]);
            out.push_str("call i32 @calloc(");
            rest = &rest[idx + marker.len()..];
            let end = rest.find(')').unwrap_or(rest.len());
            let args = &rest[..end];
            let mut new_args = String::new();
            let mut first = true;
            for p in args.split(',') {
                if !first {
                    new_args.push_str(", ");
                }
                first = false;
                let p = p.trim();
                if let Some(inner) = p.strip_prefix("i64 ") {
                    let inner = inner.trim();
                    if inner.starts_with('%') {
                        let tmp = format!("%rlyeh.wasm32.c{counter}");
                        counter += 1;
                        if let Some(nl) = out.rfind('\n') {
                            out.insert_str(
                                nl + 1,
                                &format!("  {tmp} = trunc i64 {inner} to i32\n"),
                            );
                        }
                        new_args.push_str(&format!("i32 {tmp}"));
                    } else {
                        new_args.push_str(&format!("i32 {inner}"));
                    }
                } else {
                    new_args.push_str(p);
                }
            }
            out.push_str(&new_args);
            out.push_str(")");
            // 参数区结束的 `)` 已手动补上，rest 需跳过该字符
            // （区别于 adapt_wide_int_args：它不补 `)`、靠 rest 自然推进）。
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        for reg in &ret_regs {
            // reg 值形如 `%r1`（已含 % 前缀），format 内直接插值。
            let from = format!("inttoptr i64 {reg} to i8*");
            let to = format!("inttoptr i32 {reg} to i8*");
            if out.contains(&from) {
                out = out.replace(&from, &to);
            }
            // 标准库直调（sync 模块 `let p = calloc(1, N)`）：返回值按 i64 存槽。
            // calloc 在 wasm 返回 i32，插入 `zext i32 → i64` 并让 store 用其结果。
            // reg 值已含 % 前缀（如 `%r4355`），format 内不要重复写 `%`。
            let call_pat = format!("{reg} = call i32 @calloc(");
            if let Some(idx) = out.find(&call_pat) {
                if let Some(nl) = out[idx..].find('\n') {
                    let insert_at = idx + nl + 1;
                    out.insert_str(insert_at, &format!("  {reg}.w = zext i32 {reg} to i64\n"));
                }
            }
            let store_from = format!("store i64 {reg}, i64*");
            let store_to = format!("store i64 {reg}.w, i64*");
            out = out.replace(&store_from, &store_to);
        }
        out
    }
    ll = ll.replace("declare i8* @malloc(i64)", "declare i8* @malloc(i32)");
    ll = ll.replace(
        "declare i32 @memcmp(i8*, i8*, i64)",
        "declare i32 @memcmp(i8*, i8*, i32)",
    );
    // 注意：marker 止于 `(`，参数区从 `i64 ` 开始解析。
    ll = adapt_wide_int_args(&ll, "call i8* @malloc(");
    ll = adapt_wide_int_args(&ll, "call i32 @memcmp(i8*, i8*, ");
    // calloc 适配：wasi-libc 的 calloc(size_t, size_t) 为 32 位签名，而 Rlyeh IR
    // 声明/调用按 i64（返回值经 inttoptr 转 i8*）。将声明与调用点参数降为 i32，
    // 并把紧跟的 `inttoptr i64 %r64 to i8*` 同步降宽（i32 值直转 32 位指针）。
    ll = ll.replace("declare i64 @calloc(i64, i64)", "declare i32 @calloc(i32, i32)");
    ll = adapt_calloc_wasm32(&ll);
    // WASI 入口适配：wasi-libc 的 `__main_void`（crt1 链）调用
    // `__main_argc_argv(int argc, char **argv)`，而 Rlyeh 生成的是无参 `@main`。
    // 将 IR 中的 main 定义重命名为 `__main_argc_argv` 并补齐 ABI 参数，
    // 使 crt1.o 能解析入口；同时避免 clang 为无参 main 生成额外包装符号。
    if ll.contains("define i32 @main()") {
        ll = ll.replace(
            "define i32 @main() {",
            "define i32 @__main_argc_argv(i32 %argc, i8** %argv) {",
        );
    }
    std::fs::write(ll_path, &ll).map_err(DriverError::Io)?;
    let mut cmd = Command::new(&clang);
    cmd.arg(format!("--target={target}"));
    cmd.arg(format!("--sysroot={}", sysroot.display()));
    // 发布级优化：LLVM IR 汇编启用 -O3 优化管线（与 native 对齐；
    // 更强内联/循环变换，生成 IR 不含 noalias/nsw/nuw 标注，优化保守安全）。
    cmd.arg("-O3");
    // 关闭 clang 默认的 compiler-rt builtins 链接（wasm32 的 libclang_rt.builtins.a
    // 不在 Xcode CLT / brew llvm 内），改由 wasi-libc 的 crt1.o + libc.a 提供入口与库函数。
    // Rlyeh 的 i64/f64 运算在 wasm32 均为原生指令，不依赖 software-intrinsic。
    cmd.arg("-nostdlib");
    cmd.arg(ll_path);
    cmd.arg(lib_dir.join("crt1.o"));
    cmd.arg(lib_dir.join("libc.a"));
    // L4b: 链接 wasm 版 Actor 运行时（单线程同步模式，替代 dlsym）。
    // 程序引用 rlyeh_actor_* 而库缺失时给出明确诊断（避免链接器 undefined symbol）。
    let uses_actor = ll.contains("@rlyeh_actor_");
    match wasm_actor_runtime_lib_path() {
        Some(lib) => {
            cmd.arg(lib);
        }
        None if uses_actor => {
            let _ = std::fs::remove_dir_all(dir);
            return Err(DriverError::Clang(
                "wasm 目标下 actor 程序需要 wasm 版运行时: 请先执行 \
                 `cargo build --target wasm32-wasip1 -p rlyeh-actor-runtime`"
                    .to_string(),
            ));
        }
        None => {}
    }
    cmd.arg("-o").arg(out_path);
    // wasm-ld 不在 PATH（Homebrew lld keg-only 时）则注入其 bin 目录，
    // clang 链接 wasm 目标时按名查找 `wasm-ld`。
    if let Some(ld_dir) = wasm_ld_dir() {
        let path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{}:{path}", ld_dir.display()));
    }
    let result = cmd
        .output()
        .map_err(|e| DriverError::Clang(format!("无法启动 `{clang}`: {e}")))?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        return Err(DriverError::Clang(stderr));
    }
    // 调试：设置 RLYEH_KEEP_TMP=1 时保留临时目录（含 main.ll 中间 IR）。
    if std::env::var("RLYEH_KEEP_TMP").is_err() {
        let _ = std::fs::remove_dir_all(dir);
    }
    Ok(())
}

/// 定位 WASI sysroot（wasi-libc 的头文件与 libc 库根目录）。
///
/// 顺序：`WASI_SYSROOT` 环境变量 → Homebrew wasi-libc 的
/// `share/wasi-sysroot`（Apple Silicon / Intel 两种前缀）。
/// 目录需含 `lib/`（wasi-libc 产物）才算有效。
fn wasi_sysroot() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("WASI_SYSROOT") {
        let p = PathBuf::from(p);
        if p.join("lib").exists() {
            return Some(p);
        }
    }
    for base in [
        "/opt/homebrew/opt/wasi-libc/share/wasi-sysroot",
        "/usr/local/opt/wasi-libc/share/wasi-sysroot",
    ] {
        let p = Path::new(base);
        if p.join("lib").exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}

/// 定位 wasm-ld 所在目录（PATH 中已存在则返回 `None`）。
///
/// Homebrew lld 可能 keg-only（不软链进 PATH），需显式注入其 bin 目录。
fn wasm_ld_dir() -> Option<PathBuf> {
    let probe = Command::new("wasm-ld")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if probe.is_ok_and(|s| s.success()) {
        return None;
    }
    for base in ["/opt/homebrew/opt/lld/bin", "/usr/local/opt/lld/bin"] {
        if Path::new(base).join("wasm-ld").exists() {
            return Some(PathBuf::from(base));
        }
    }
    None
}

/// 定位 Actor 运行时静态库（staticlib 产物）；缺失返回 `None`（无 actor 的程序不受影响）。
fn actor_runtime_lib_path() -> Option<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let lib = manifest
        .join("../../target")
        .join(profile)
        .join("librlyeh_actor_runtime.a");
    lib.exists().then_some(lib)
}

/// 定位 GC 运行时静态库（K4 追踪 GC，staticlib 产物）；缺失返回 `None`
/// （无 `gc_region` / `Gc::new` 的程序不受影响）。
fn gc_runtime_lib_path() -> Option<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let lib = manifest
        .join("../../target")
        .join(profile)
        .join("librlyeh_gc_runtime.a");
    lib.exists().then_some(lib)
}

/// 定位区域运行时静态库（L3 region 接线，staticlib 产物）；缺失返回 `None`
/// （不含 `region` 块的程序不受影响）。
fn region_runtime_lib_path() -> Option<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let lib = manifest
        .join("../../target")
        .join(profile)
        .join("librlyeh_region_alloc.a");
    lib.exists().then_some(lib)
}

/// 本机 LLVM 目标 triple（如 `arm64-apple-macosx` / `x86_64-unknown-linux-gnu`）。
pub fn host_triple() -> String {
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        a => a,
    };
    let os = match std::env::consts::OS {
        "macos" => "apple-macosx",
        "linux" => "unknown-linux-gnu",
        "windows" => "pc-windows-msvc",
        o => o,
    };
    format!("{arch}-{os}")
}

/// 从 LLVM target triple 提取架构名（`arm64`/`aarch64` 归一为 `aarch64`）。
pub fn target_arch(triple: &str) -> Option<&str> {
    let arch = triple.split('-').next()?;
    if arch.is_empty() {
        return None;
    }
    Some(match arch {
        "arm64" | "aarch64" => "aarch64",
        other => other,
    })
}

/// 指定目标是否与本机架构不同（交叉编译）。
pub fn is_cross_target(target: Option<&str>) -> bool {
    match target {
        None => false,
        // Y（2026-08-28 完善）：交叉 = 架构不同 **或** OS 不同。此前仅比架构，
        // 导致同架构跨 OS（macOS arm64 → Linux arm64）误判为同平台（宿主 ld 无法
        // 链接跨 OS 目标）。OS 码见 target_os_code（1=linux 2=macos 3=windows 4=bsd 5=wasi）。
        Some(t) => {
            target_arch(t) != Some(host_arch()) || target_os_code(Some(t)) != host_os_code()
        }
    }
}

/// 主机 OS 码（与 [`target_os_code`] 同一命名：1=linux 2=macos 3=windows 4=freebsd 5=wasi）。
fn host_os_code() -> i32 {
    match std::env::consts::OS {
        "linux" => 1,
        "macos" => 2,
        "windows" => 3,
        "freebsd" => 4,
        _ => 0,
    }
}

/// 指定目标是否为 Linux/ELF musl 目标（`*-linux-musl`）——交叉编译走 rust-lld +
/// Rust musl sysroot 链接链路（Y，2026-08-28）。
pub fn is_elf_musl_target(target: &str) -> bool {
    target.ends_with("-linux-musl")
}

/// rust-lld 路径探测：Rust 自带多目标链接器（`<rustup>/lib/rustlib/<host>/bin/rust-lld`），
/// 支持 ELF/RISC-V/LoongArch 等目标，替代宿主 clang 的 ld 做跨 OS 链接。
fn rust_lld_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let toolchains = Path::new(&home).join(".rustup/toolchains");
    let entries = std::fs::read_dir(&toolchains).ok()?;
    for e in entries.flatten() {
        // rust-lld 在 <host>/bin/ 下，遍历各 rustlib/<host>/bin
        let rustlib = e.path().join("lib/rustlib");
        if let Ok(rd) = std::fs::read_dir(&rustlib) {
            for host in rd.flatten() {
                let lld = host.path().join("bin/rust-lld");
                if lld.is_file() {
                    return Some(lld);
                }
            }
        }
    }
    None
}

/// 跨 OS ELF（Linux/RISC-V/LoongArch musl）汇编 + 链接：clang 汇编 LLVM IR → object，
/// rust-lld + Rust musl sysroot crt/libc 链接。解锁 macOS → Linux/RISC-V/LoongArch 交叉编译。
fn assemble_cross_elf(
    ll_path: &Path,
    out_path: &Path,
    target: &str,
    dir: &Path,
) -> Result<(), DriverError> {
    // 1. clang 汇编 LLVM IR → 目标平台 object
    let clang = clang_path();
    let obj = dir.join("main.o");
    // 注入 `rust_eh_personality` stub（链接 Rust compiler_builtins rlib 时，它引用
    // 此 panic/unwind 符号；Rlyeh 程序不用 unwind，stub 返回 0 即可）。
    let ll_content = std::fs::read_to_string(ll_path).map_err(DriverError::Io)?;
    let ll_with_stub = format!(
        "{ll_content}\n; rust_eh_personality stub（Rlyeh 不用 unwind，返回 0）\ndefine i32 @rust_eh_personality(i32, i8*, i8*) {{\nentry:\n  ret i32 0\n}}\n"
    );
    let ll_stub_path = dir.join("main_stub.ll");
    std::fs::write(&ll_stub_path, ll_with_stub).map_err(DriverError::Io)?;
    let mut asm_cmd = Command::new(&clang);
    asm_cmd
        .arg(format!("--target={target}"))
        .arg("-c")
        .arg(&ll_stub_path)
        .arg("-o")
        .arg(&obj);
    let asm_out = asm_cmd.output().map_err(|e| {
        DriverError::Clang(format!("无法启动 `{clang}`: {e}"))
    })?;
    if !asm_out.status.success() {
        return Err(DriverError::Clang(String::from_utf8_lossy(&asm_out.stderr).to_string()));
    }

    // 2. rust-lld + Rust musl sysroot crt/libc 链接
    let lld = rust_lld_path().ok_or_else(|| DriverError::Clang(
        "交叉编译 ELF 目标需要 rust-lld（未找到，Rust 工具链应自带）".to_string(),
    ))?;
    // Rust musl sysroot 的 self-contained：crt1.o/crti.o/crtbeginS.o + libc.a（从 rustc 查询）
    let sysroot = rust_sysroot_target_selfcontained(target);

    let scrt1 = sysroot.join("Scrt1.o");
    let crti = sysroot.join("crti.o");
    let crtbegin = sysroot.join("crtbeginS.o");
    let crtend = sysroot.join("crtendS.o");
    let crtn = sysroot.join("crtn.o");

    let mut link_cmd = Command::new(&lld);
    link_cmd
        .arg("-flavor").arg("gnu")
        .arg(&scrt1)
        .arg(&crti)
        .arg(&crtbegin)
        .arg(&obj)
        // 静态链接 musl libc.a（含 __*tf3 等 f128 内建；-Bstatic 避免动态搜索遗漏）
        .arg("-Bstatic")
        .arg("-lc")
        .arg("-Bdynamic")
        .arg("-L").arg(&sysroot)
        // arm64/RISC-V/LoongArch 的 long double = f128：musl printf 无条件引用
        // __*tf3 等 f128 内建（libc.a 仅 U 引用不提供定义）。Rust 的 compiler_builtins
        // 提供这些 soft-float 内建——链接其 rlib 解析符号。
        .arg("-o").arg(out_path)
        .arg(&crtend)
        .arg(&crtn)
        .arg("-O1")
        .arg("--strip-debug");
    // f128 内建（__*tf3 等）：arm64/RISC-V/LoongArch 的 long double 需要，Rust
    // compiler_builtins rlib 提供。条件添加（有则链接，无则跳过）。
    if let Some(cb) = rust_compiler_builtins_path(target) {
        link_cmd.arg(&cb);
        // compiler_builtins 引用 `rust_eh_personality`（panic/unwind）——musl 的
        // libunwind.a 提供（self-contained）。
        let libunwind = sysroot.join("libunwind.a");
        if libunwind.is_file() {
            link_cmd.arg(&libunwind);
        }
    }
    let link_out = link_cmd.output().map_err(|e| {
        DriverError::Clang(format!("无法启动 `{}`: {e}", lld.display()))
    })?;
    if !link_out.status.success() {
        return Err(DriverError::Clang(String::from_utf8_lossy(&link_out.stderr).to_string()));
    }
    Ok(())
}

/// 目标架构的 Rust `libcompiler_builtins` rlib 路径（提供 f128 等 soft-float 内建）。
/// arm64/RISC-V/LoongArch 的 long double = f128，musl printf 引用 `__*tf3`，需此库解析。
fn rust_compiler_builtins_path(target: &str) -> Option<PathBuf> {
    let sysroot = rust_sysroot()?;
    let dir = sysroot.join("lib/rustlib").join(target).join("lib");
    let entries = std::fs::read_dir(&dir).ok()?;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("libcompiler_builtins-") && name.ends_with(".rlib") {
            return Some(e.path());
        }
    }
    None
}

/// rustc 工具链 sysroot（`rustc --print sysroot`）。
fn rust_sysroot() -> Option<PathBuf> {
    let out = Command::new("rustc")
        .arg("--print").arg("sysroot")
        .output()
        .ok()?;
    if out.status.success() {
        Some(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string()))
    } else {
        None
    }
}

/// 从 rustc 查询目标 sysroot 的 self-contained 目录（`<rustc-sysroot>/lib/rustlib/<target>/lib/self-contained`）。
fn rust_sysroot_target_selfcontained(target: &str) -> PathBuf {
    rust_sysroot()
        .unwrap_or_default()
        .join("lib/rustlib")
        .join(target)
        .join("lib/self-contained")
}

/// 本机架构（与 [`target_arch`] 同一归一化命名）。
fn host_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        a => a,
    }
}

/// 平台内建 `__rlyeh_target_os()` 的返回码（0=未知 1=linux 2=macos 3=windows 4=freebsd 5=wasi）。
///
/// 从目标 triple 提取 OS 段（`None` = 主机）；标准库据此做平台分支
/// （如 `sockaddr_in4` 的 `sin_len` 布局：macOS 有、Linux 无；WASI 下无 socket
/// API，`net` 模块短路返回错误码——L4a 明确禁用文档化）。
pub fn target_os_code(target: Option<&str>) -> i32 {
    let os = match target {
        None => std::env::consts::OS,
        Some(t) => t,
    };
    if os.contains("linux") {
        1
    } else if os.contains("macosx") || os.contains("darwin") || os == "macos" {
        2
    } else if os.contains("windows") || os.contains("win32") {
        3
    } else if os.contains("freebsd") {
        4
    } else if os.contains("wasi") || os.contains("wasm") {
        5
    } else {
        0
    }
}


/// 将 Rust 字符串转义为 LLVM `c"..."` 常量体；返回（转义文本, 原始字节数）。
fn escape_llvm_c_string(s: &str) -> (String, usize) {
    let mut out = String::new();
    let mut bytes = 0;
    for b in s.bytes() {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\22"),
            b'\n' => out.push_str("\\0A"),
            b'\r' => out.push_str("\\0D"),
            0x20..=0x7E => out.push(b as char),
            _ => out.push_str(&format!("\\{b:02X}")),
        }
        bytes += 1;
    }
    (out, bytes)
}

/// LLVM 引号包裹的符号名（`::` 等非字母数字字符须用 `@"..."` 形式）。
fn llvm_quoted_name(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\22"))
}

/// 从 LLVM IR 文本解析所有 Rlyeh actor 的 handle / factory 函数名
/// （`<actor>::__handle` / `<actor>::__state_new` 结尾的 `define`）。
fn actor_symbols_from_ir(llvm: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in llvm.lines() {
        let line = line.trim_start();
        if !line.starts_with("define ") {
            continue;
        }
        let Some(at) = line.find('@') else { continue };
        let rest = &line[at + 1..];
        let name = if let Some(stripped) = rest.strip_prefix('"') {
            match stripped.find('"') {
                Some(end) => &stripped[..end],
                None => continue,
            }
        } else {
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '(')
                .unwrap_or(rest.len());
            &rest[..end]
        };
        if name.ends_with("::__handle") || name.ends_with("::__state_new") {
            out.push(name.to_string());
        }
    }
    out
}

/// 生成 wasm 目标的 actor 符号解析表（L4b）：替代 `dlsym` 的静态查表。
///
/// 编译期已知全部 handle / factory 函数，生成 `rlyeh_actor_resolve(name)`
/// （`strcmp` 字符串比较 → `ptrtoint` 函数地址），wasm 版运行时
/// （`rlyeh-actor-runtime` 同步模式）据此解析符号。返回空串表示无 actor。
fn actor_resolve_ir(llvm: &str) -> String {
    let symbols = actor_symbols_from_ir(llvm);
    if symbols.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    out.push_str("\n; --- L4b: actor 符号解析表（wasm 目标替代 dlsym）---\n");
    out.push_str("declare i32 @strcmp(i8*, i8*)\n");
    for (i, s) in symbols.iter().enumerate() {
        let (escaped, bytes) = escape_llvm_c_string(s);
        let len = bytes + 1;
        out.push_str(&format!(
            "@.zr.{i} = private unnamed_addr constant [{len} x i8] c\"{escaped}\\00\"\n"
        ));
    }
    // 外部链接：wasm 版运行时（静态库）需解析该符号；internal 对链接器不可见。
    out.push_str("define i64 @rlyeh_actor_resolve(i8* %name) {\n");
    out.push_str("entry:\n");
    let mut blocks = Vec::new();
    for (i, s) in symbols.iter().enumerate() {
        let (_, bytes) = escape_llvm_c_string(s);
        let len = bytes + 1;
        // handle 回调签名 `i64 (i64, i64, i64, i64, i64)`；factory
        // （`__state_new`）实际返回 `i8*`（状态对象指针），须与真实签名一致，
        // 否则 wasm 间接调用报 `indirect call type mismatch`。
        let fn_ty = if s.ends_with("::__state_new") {
            "i8* ()"
        } else {
            "i64 (i64, i64, i64, i64, i64)"
        };
        let quoted = llvm_quoted_name(s);
        blocks.push(format!(
            "  %s{i} = bitcast [{len} x i8]* @.zr.{i} to i8*\n  %c{i} = call i32 @strcmp(i8* %name, i8* %s{i})\n  %eq{i} = icmp eq i32 %c{i}, 0\n  br i1 %eq{i}, label %hit{i}, label %miss{i}\nhit{i}:\n  ret i64 ptrtoint ({fn_ty}* @{quoted} to i64)\nmiss{i}:"
        ));
    }
    blocks.push("  ret i64 0".to_string());
    out.push_str(&blocks.join("\n"));
    out.push_str("\n}\n");
    out
}

/// 探测 wasm 版 Actor 运行时静态库路径（`cargo build --target wasm32-wasip1
/// -p rlyeh-actor-runtime` 产物）；无则返回 `None`。
fn wasm_actor_runtime_lib_path() -> Option<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .parent()?;
    for target in ["wasm32-wasip1", "wasm32-wasi"] {
        for profile in ["release", "debug"] {
            let p = root
                .join("target")
                .join(target)
                .join(profile)
                .join("librlyeh_actor_runtime.a");
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

mod platform_ir;
mod util;

use platform_ir::*;
use util::*;