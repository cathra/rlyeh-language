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
pub mod test_runner;

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
pub fn run_source_file(entry: &Path) -> Result<String, DriverError> {
    let dir = temp_dir();
    let exe = dir.join("zeta-run");
    build_executable_file(entry, &exe)?;
    let out = run_exe(&exe)?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(out)
}

/// 读取源码文件并执行静态检查（`zeta check`）。
///
/// 解析失败返回 `parse-error` 诊断；否则返回 lint 规则诊断。
/// 仅负责分析，不涉及完整编译流水线。
pub fn check_source_file(path: &Path) -> Result<Vec<zeta_check::Diagnostic>, DriverError> {
    let source = std::fs::read_to_string(path).map_err(DriverError::Io)?;
    Ok(zeta_check::check_source(&source))
}

/// 读取源码文件并生成 Markdown 文档（`zeta doc`）。
pub fn doc_source_file(path: &Path, options: &zeta_doc::DocOptions) -> Result<String, DriverError> {
    let source = std::fs::read_to_string(path).map_err(DriverError::Io)?;
    zeta_doc::doc_source(&source, options).map_err(DriverError::Doc)
}

/// 读取 PGO 画像文件并生成区域大小预测报告（`zeta profile`，阶段 F2 数据回灌消费侧）。
///
/// `.zeta_profile` 为 JSON 格式（见 `zeta_region_alloc::profile`）：
/// 运行时按区域记录的分配量统计（p50/p90/p95/mean/max 等）。
/// 本函数加载画像 → 以 p95×安全系数预测初始区域大小 → 输出编译决策报告
/// （复用 `zeta_region_alloc::PgoAdvisor` / `CompilerInterface`）。
/// 语言级 region 接线后，该预测可直接回灌 `region 'r adaptive` 的初始容量。
pub fn region_profile_report(path: &Path) -> Result<String, DriverError> {
    let text = std::fs::read_to_string(path).map_err(DriverError::Io)?;
    let data: zeta_region_alloc::PgoData = serde_json::from_str(&text)
        .map_err(|e| DriverError::Profile(format!("解析 `.zeta_profile` 失败: {e}")))?;
    Ok(build_region_report(&data))
}

/// 依据 PGO 数据生成区域分配决策报告（纯函数，便于单元测试）。
///
/// 每个区域：`estimated_size` = 历史平均分配量（静态基线）、
/// `initial_size` = PGO 推荐（p95×安全系数，下限 64KiB）、
/// `max_size` = 推荐×4 与历史峰值取大、`decision` 描述依据。
pub fn build_region_report(data: &zeta_region_alloc::PgoData) -> String {
    let advisor = zeta_region_alloc::PgoAdvisor::from_data(data.clone());
    let mut interface =
        zeta_region_alloc::CompilerInterface::new().with_pgo_data(data.clone());
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
        interface = interface.register_region(zeta_region_alloc::RegionCompileInfo {
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
/// 缓存目录为 `<cache_dir>/.zeta_cache`；`--force` 语义下强制全量重编译。
pub struct IncrementalDriver {
    /// 缓存根（`.zeta_cache` 的父目录）
    cache_dir: PathBuf,
    /// 强制全量重编译（忽略缓存）
    force: bool,
    /// 禁用标准库预置注入（`--no-std`）
    no_std: bool,
    /// LLVM 目标 triple（`--target` 交叉编译；`None` = 主机目标）
    target: Option<String>,
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
            target: None,
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
        assemble(&outcome.llvm, out_path, self.target.as_deref())?;
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
    // 注入平台内建（`__zeta_target_os`）后写盘——codegen 对 `__zeta_` 前缀 extern 不生成 declare。
    let llvm_with_builtins = format!("{llvm}\n{}", platform_builtin_ir(target));
    std::fs::write(&ll_path, llvm_with_builtins).map_err(DriverError::Io)?;

    // WebAssembly 目标走独立汇编链路（wasi-libc sysroot + wasm-ld），其余走系统链接器。
    if let Some(t) = target {
        if is_wasm_triple(t) {
            return assemble_wasm(&ll_path, out_path, t, &dir);
        }
    }

    let clang = clang_path();
    // 运行时 C ABI（staticlib）：Zeta 程序经 `extern fn zeta_actor_*` /
    // `extern fn zeta_gc_*` 调用。链接器按需提取对象——不含相应特性的程序不受影响
    // （库可缺失则跳过）。交叉编译到其他架构时本机 staticlib 无法链接，跳过并提示。
    let runtime_libs: Vec<PathBuf> = if is_cross_target(target) {
        eprintln!(
            "zeta: 提示: 交叉编译目标 `{}` 与主机架构不同，跳过 Actor / GC 运行时库（相关特性暂不支持交叉编译）",
            target.unwrap_or_default()
        );
        Vec::new()
    } else {
        [actor_runtime_lib_path(), gc_runtime_lib_path()]
            .into_iter()
            .flatten()
            .collect()
    };
    let mut cmd = Command::new(&clang);
    if let Some(t) = target {
        cmd.arg(format!("--target={t}"));
    }
    cmd.arg(&ll_path);
    for lib in &runtime_libs {
        if let Some(parent) = lib.parent() {
            cmd.arg("-L").arg(parent);
        }
        cmd.arg("-l").arg(
            lib.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.trim_start_matches("lib").trim_end_matches(".a"))
                .unwrap_or("zeta_runtime"),
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
    let _ = std::fs::remove_dir_all(&dir);
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
/// 入口语义：WASI 的 `_start` 由 wasi-libc crt1.o 提供并调用 Zeta 生成的 `main()`；
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
    // `__main_argc_argv(int argc, char **argv)`，而 Zeta 生成的是无参 `@main`。
    // 将 IR 中的 main 定义重命名为 `__main_argc_argv` 并补齐 ABI 参数，
    // 使 crt1.o 能解析入口；同时避免 clang 为无参 main 生成额外包装符号。
    let mut ll = std::fs::read_to_string(ll_path).map_err(DriverError::Io)?;
    // wasm32 指针/尺寸位宽适配：wasi-libc 的 malloc(size_t)/memcmp(size_t)
    // 为 32 位参数，而 Zeta 的 IR 按 64 位（isize）声明调用；将声明与调用的
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
                    let tmp = format!("%zeta.wasm32.{counter}");
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
    ll = ll.replace("declare i8* @malloc(i64)", "declare i8* @malloc(i32)");
    ll = ll.replace(
        "declare i32 @memcmp(i8*, i8*, i64)",
        "declare i32 @memcmp(i8*, i8*, i32)",
    );
    // 注意：marker 止于 `(`，参数区从 `i64 ` 开始解析。
    ll = adapt_wide_int_args(&ll, "call i8* @malloc(");
    ll = adapt_wide_int_args(&ll, "call i32 @memcmp(i8*, i8*, ");
    // WASI 入口适配：wasi-libc 的 `__main_void`（crt1 链）调用
    // `__main_argc_argv(int argc, char **argv)`，而 Zeta 生成的是无参 `@main`。
    // 将 IR 中的 main 定义重命名为 `__main_argc_argv` 并补齐 ABI 参数，
    // 使 crt1.o 能解析入口；同时避免 clang 为无参 main 生成额外包装符号。
    if ll.contains("define i32 @main()") {
        ll = ll.replace(
            "define i32 @main() {",
            "define i32 @__main_argc_argv(i32 %argc, i8** %argv) {",
        );
    }
    std::fs::write(ll_path, ll).map_err(DriverError::Io)?;
    let mut cmd = Command::new(&clang);
    cmd.arg(format!("--target={target}"));
    cmd.arg(format!("--sysroot={}", sysroot.display()));
    // 关闭 clang 默认的 compiler-rt builtins 链接（wasm32 的 libclang_rt.builtins.a
    // 不在 Xcode CLT / brew llvm 内），改由 wasi-libc 的 crt1.o + libc.a 提供入口与库函数。
    // Zeta 的 i64/f64 运算在 wasm32 均为原生指令，不依赖 software-intrinsic。
    cmd.arg("-nostdlib");
    cmd.arg(ll_path);
    cmd.arg(lib_dir.join("crt1.o"));
    cmd.arg(lib_dir.join("libc.a"));
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
    let _ = std::fs::remove_dir_all(dir);
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
        .join("libzeta_actor_runtime.a");
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
        .join("libzeta_gc_runtime.a");
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
        Some(t) => target_arch(t) != Some(host_arch()),
    }
}

/// 本机架构（与 [`target_arch`] 同一归一化命名）。
fn host_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        a => a,
    }
}

/// 平台内建 `__zeta_target_os()` 的返回码（0=未知 1=linux 2=macos 3=windows 4=freebsd）。
///
/// 从目标 triple 提取 OS 段（`None` = 主机）；标准库据此做平台分支
/// （如 `sockaddr_in4` 的 `sin_len` 布局：macOS 有、Linux 无）。
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
    } else {
        0
    }
}

/// 注入平台内建的 LLVM IR 定义文本（`__zeta_target_os` 返回当前目标 OS 码）。
fn platform_builtin_ir(target: Option<&str>) -> String {
    format!(
        "\n; --- 平台内建（driver 按目标注入）---\ndefine internal i32 @__zeta_target_os() {{\nentry:\n  ret i32 {}\n}}\n",
        target_os_code(target)
    )
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
    eprint!("{}", String::from_utf8_lossy(&out.stderr));
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
