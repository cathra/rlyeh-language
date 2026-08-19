//! # zeta-region-alloc
//!
//! Zeta 语言 L1 区域内存分配器（Rust 绑定层实现）。
//!
//! 区域（Region）将一批对象分配在同一段（组）连续内存中，随区域整体销毁，
//! 实现零开销的批量释放。设计遵循：
//!
//! - **ADR-002**：bump 分配 + 链表扩容（扩容新块，不拷贝旧数据）；
//! - **ADR-003**：`transfer` 是编译期操作——对象所有权移出区域，
//!   区域销毁时不再调用其析构（[`Region::mark_transferred`]）；
//! - **析构顺序**：后分配先析构（LIFO）。
//!
//! ## 使用示例
//!
//! ```no_run
//! use zeta_region_alloc::{GrowthStrategy, Region};
//!
//! let mut region = Region::with_initial_size(64).named("session");
//! region.strategy = GrowthStrategy::Multiply { factor: 2.0 };
//!
//! let a = region.allocate(42u64)?;
//! let b = region.allocate(String::from("hello"))?;
//! # Ok::<(), zeta_region_alloc::AllocError>(())
//! ```
//!
//! 区域销毁时自动逆序调用各对象的析构（`String` 的堆缓冲区被释放）。

#![warn(missing_docs)]
#![allow(unsafe_code)] // 绑定层：手动内存管理本质需要 unsafe

mod block;
mod bump;
mod destructor;
mod error;
mod region;
mod stats;
mod strategy;

pub use error::AllocError;
pub use region::Region;
pub use stats::RegionStats;
pub use strategy::GrowthStrategy;
