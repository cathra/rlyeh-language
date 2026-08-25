//! # 标准库预置（prelude）加载
//!
//! 编译器自动定位并注入 `zeta-std/zeta/core.zeta`（含其子模块
//! `time.zeta` / `io.zeta` / `net.zeta` / `sync.zeta`，经模块展开合入），
//! 使用户程序无需内联即可使用 `Option<T>` / `Result<T, E>` / `Duration` /
//! `read_file` 等核心类型与函数。
//!
//! 定位策略（MVP）：
//! - 环境变量 `ZETA_STD_PATH` 优先（可指向 `core.zeta` 文件或所在目录）；
//! - 否则按源码仓库布局：`<zeta-driver>/../zeta-std/zeta/core.zeta`。

use std::path::PathBuf;

use crate::error::DriverError;
use crate::module::load_combined_source;

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

/// 加载标准库预置源码（`core.zeta` + 子模块展开为单文件）。
///
/// 入口文件不存在时返回 `Ok(None)`（无标准库环境下静默降级，用户程序仍可编译）；
/// 其余 IO 错误（含 std 子模块缺失等配置错误）原样返回。
pub(crate) fn load_std_prelude() -> Result<Option<String>, DriverError> {
    let path = std_core_path();
    if !path.is_file() {
        return Ok(None);
    }
    load_combined_source(&path).map(Some)
}
