//! 驱动通用工具：运行产物 / 临时目录 / 错误拼接 / 编译器定位。
//!
//! 由 lib.rs 拆分而来。

use super::*;

/// 执行可执行文件并捕获标准输出。
pub(crate) fn run_exe(exe: &Path) -> Result<String, DriverError> {
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

pub(crate) fn temp_dir() -> PathBuf {
    let seq = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "rlyeh-mvp-{}-{}-{seq}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ))
}

/// 拼接错误列表为单行文本。
pub(crate) fn join_errors<T: std::fmt::Display>(errs: Vec<T>) -> String {
    errs.iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

/// 查找可用的 C 编译器（优先 clang，回退 cc）。
pub(crate) fn clang_path() -> String {
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
