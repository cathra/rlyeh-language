//! link：汇编链接与工具链探测子模块。
//! （由 lib.rs 拆分而来，保持语义等价）
//!
//! 覆盖：clang 汇编链接（native / wasm / 交叉 ELF）、wasi-libc 与运行时静态库
//! 路径探测、目标 triple 与架构解析、LLVM IR 符号表与字符串转义。

use super::*;

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
pub(crate) fn assemble_wasm(
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
pub(crate) fn wasi_sysroot() -> Option<PathBuf> {
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
pub(crate) fn wasm_ld_dir() -> Option<PathBuf> {
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
pub(crate) fn actor_runtime_lib_path() -> Option<PathBuf> {
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
pub(crate) fn gc_runtime_lib_path() -> Option<PathBuf> {
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
pub(crate) fn region_runtime_lib_path() -> Option<PathBuf> {
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
pub(crate) fn host_os_code() -> i32 {
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
pub(crate) fn rust_lld_path() -> Option<PathBuf> {
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
pub(crate) fn assemble_cross_elf(
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
pub(crate) fn rust_compiler_builtins_path(target: &str) -> Option<PathBuf> {
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
pub(crate) fn rust_sysroot() -> Option<PathBuf> {
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
pub(crate) fn rust_sysroot_target_selfcontained(target: &str) -> PathBuf {
    rust_sysroot()
        .unwrap_or_default()
        .join("lib/rustlib")
        .join(target)
        .join("lib/self-contained")
}

/// 本机架构（与 [`target_arch`] 同一归一化命名）。
pub(crate) fn host_arch() -> &'static str {
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
pub(crate) fn escape_llvm_c_string(s: &str) -> (String, usize) {
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
pub(crate) fn llvm_quoted_name(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\22"))
}

/// 从 LLVM IR 文本解析所有 Rlyeh actor 的 handle / factory 函数名
/// （`<actor>::__handle` / `<actor>::__state_new` 结尾的 `define`）。
pub(crate) fn actor_symbols_from_ir(llvm: &str) -> Vec<String> {
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
pub(crate) fn actor_resolve_ir(llvm: &str) -> String {
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
pub(crate) fn wasm_actor_runtime_lib_path() -> Option<std::path::PathBuf> {
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

