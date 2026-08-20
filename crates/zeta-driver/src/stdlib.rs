//! # 标准库预置（prelude）加载
//!
//! 编译器自动定位并注入 `zeta-std/zeta/core.zeta`，使用户程序无需内联即可
//! 使用 `Option<T>` / `Result<T, E>` 等核心类型。
//!
//! 定位策略（MVP）：
//! - 环境变量 `ZETA_STD_PATH` 优先（可指向 `core.zeta` 文件或所在目录）；
//! - 否则按源码仓库布局：`<zeta-driver>/../zeta-std/zeta/core.zeta`。

use std::path::PathBuf;

use crate::error::DriverError;

/// 标准库 `core.zeta` 路径：`ZETA_STD_PATH` 环境变量优先，否则按仓库布局定位。
fn std_core_path() -> PathBuf {
    if let Ok(p) = std::env::var("ZETA_STD_PATH") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            return p.join("core.zeta");
        }
        return p;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../zeta-std/zeta/core.zeta")
}

/// 加载标准库预置源码。
///
/// 文件不存在时返回 `Ok(None)`（无标准库环境下静默降级，用户程序仍可编译），
/// 其余 IO 错误原样返回。
pub(crate) fn load_std_prelude() -> Result<Option<String>, DriverError> {
    let path = std_core_path();
    match std::fs::read_to_string(&path) {
        Ok(source) => Ok(Some(source)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(DriverError::Io(e)),
    }
}
