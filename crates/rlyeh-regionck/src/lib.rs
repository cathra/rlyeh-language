//! # zeta-regionck
//!
//! Zeta 语言区域检查器（L1 区域系统静态验证）。
//!
//! 对类型检查后的 HIR 进行区域相关验证：
//! - 区域块的嵌套与作用域；
//! - `in 'r` / `transfer ... out of 'r` 引用的区域可见性；
//! - transfer 对象必须分配在声明的源区域内（ADR-003：不拷贝、仅转移所有权簿记）；
//! - 同一对象禁止重复 transfer。
//!
//! ## 设计约定
//!
//! - HIR 节点不携带源码位置，错误中的 `line`/`col` 当前为占位 0，
//!   待 Span 传播后填充（见 P004 文档偏差说明）。
//! - 引用逃逸（RegionEscape）的完整分析需要类型信息，MVP 阶段仅保留错误
//!   类型与构造入口，供后续阶段启用。

#![warn(missing_docs)]
#![warn(unsafe_code)]

mod checker;
mod error;

pub use checker::RegionChecker;
pub use error::RegionError;
