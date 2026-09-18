//! # 标准库预置（prelude）加载
//!
//! 编译器自动定位并注入 `rlyeh-std/rlyeh/`（标准库根目录），使用户程序无需内联
//! 即可使用 `Option<T>` / `Result<T, E>` / `Duration` / `read_file` 等核心类型与函数。
//!
//! ## 目录布局（每个单元 = 一个目录 + `module.rl` 入口）
//!
//! ```text
//! rlyeh-std/rlyeh/
//! ├── module.rl              # ③ 声明与导出：`module time;` … 子模块声明 + 根命名空间 import 重导出
//! ├── core/module.rl         # ① 根命名空间类型定义（String / Vec / Option / Result …）
//! ├── str_ext/module.rl      # ② String / Chars / Lines 方法扩展           ┐
//! ├── convert/module.rl      # ② 数值 / JSON ←→ 字符串转换自由函数          │ 平铺单元
//! ├── collections/module.rl  # ② HashMap / HashSet / VecDeque / BTreeMap   │ （与 core 平级）
//! ├── externs/module.rl      # ② libc FFI extern 声明                      ┘
//! ├── future/module.rl       #    `module future;` → 真子模块（包 `module` 壳）
//! └── fmt/ io/ net/ fs/ time/ sync/ thread/ serde/ process/
//! ```
//!
//! 加载顺序 = **`core` 入口 → 平铺单元（固定顺序）→ `module.rl`（声明与导出）**，
//! 与原单文件 `core.rl` 的行序一致，故**符号顺序与全名不变**（零语义差异）。
//!
//! ## 为何平铺单元不包 `module` 壳
//!
//! 编译器对 `String` / `Vec` / `HashMap` 等按**全名**特判（构造器展开 / 比较 / 切片 /
//! JSON / TOML 等），`module` 前缀会使其失配；`extern` 符号名亦须与 libc 一致。
//! 故这些单元承载的是**根命名空间**内容，由本模块按固定顺序**平铺拼接**而非经
//! `module x;` 展开（`module.rl` 中的声明仅供真子模块使用）。
//!
//! 单元内 `module xxx;` 仍相对标准库根目录解析（见 [`crate::module::load_combined_source_in`]）。
//!
//! 定位策略（MVP）：
//! - 环境变量 `RLYEH_STD_PATH` 优先（可指向标准库根目录，或 `core/module.rl` 入口文件）；
//! - 否则按源码仓库布局：`<rlyeh-driver>/../rlyeh-std/rlyeh/`。

use std::path::{Path, PathBuf};

use crate::error::DriverError;
use crate::module::load_combined_source_in;

/// 与 `core` **平级**的平铺单元（顺序 = 原 `core.rl` 行序）。
///
/// 由本模块在 `core` 入口之后**平铺拼接**（不包 `module` 壳，保持全名与符号顺序）；
/// 与 `rlyeh-std/rlyeh/core/module.rl` 尾部注释、`rlyeh-std/rlyeh/module.rl` 头部注释
/// 保持一致，增删单元时同步。
const FLAT_UNITS: &[&str] = &["str_ext", "convert", "collections", "externs"];

/// 标准库根目录：`RLYEH_STD_PATH` 优先（目录或入口文件路径），否则按仓库布局定位。
fn std_root_dir() -> PathBuf {
    if let Ok(p) = std::env::var("RLYEH_STD_PATH") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            return p;
        }
        // 指向 `core/module.rl`：标准库根目录 = 其上两级；指向其它文件：取父目录。
        if let Some(dir) = p.parent() {
            if dir.file_name().is_some_and(|n| n == "core") {
                return dir
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| dir.to_path_buf());
            }
            return dir.to_path_buf();
        }
        return p;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rlyeh-std/rlyeh")
}

/// 加载标准库预置源码（`core` 入口 + 平铺单元 + 声明与导出，合并为单文件）。
///
/// 入口文件不存在时返回 `Ok(None)`（无标准库环境下静默降级，用户程序仍可编译）；
/// 其余 IO 错误（含 std 子模块缺失等配置错误）原样返回。
pub(crate) fn load_std_prelude() -> Result<Option<String>, DriverError> {
    let root = std_root_dir();
    let core_entry = root.join("core").join("module.rl");
    if !core_entry.is_file() {
        return Ok(None);
    }
    let std_root: &Path = &root;

    // ① `core`：根命名空间类型定义（String / Vec / Option / Result / 编译器特判项）。
    let mut combined = load_combined_source_in(&core_entry, std_root)?;

    // ② 与 `core` 平级的平铺单元：按固定顺序拼接，保持全名与符号顺序不变。
    for unit in FLAT_UNITS {
        let f = root.join(unit).join("module.rl");
        if f.is_file() {
            combined.push('\n');
            combined.push_str(&load_combined_source_in(&f, std_root)?);
        }
    }

    // ③ `module.rl`：子模块声明（`module time;` …）与根命名空间重导出（`import …`），
    //    须位于全部类型之后（符号顺序解析）。
    let decls = root.join("module.rl");
    if decls.is_file() {
        combined.push('\n');
        combined.push_str(&load_combined_source_in(&decls, std_root)?);
    }

    Ok(Some(combined))
}
