//! Zep：Zeta 包管理器（MVP）。
//!
//! 能力：
//! - 项目脚手架：`zep new` / `zep init`
//! - 清单管理：`Zeta.toml` 解析、依赖增删
//! - 依赖解析：PubGrub 版本求解，产出 `Zeta.lock`
//! - 注册表：本地目录 + HTTP（`publish` / `search` / `download`）
//! - 构建集成：调用 `zeta` 编译器（受限环境沙箱 + 超时）

pub mod build;
pub mod commands;
pub mod error;
pub mod manifest;
pub mod registry;
pub mod resolve;
pub mod sandbox;
pub mod version;

pub use error::{Result, ZepError};
