//! 区域分配错误。

use std::fmt;

/// 区域分配错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocError {
    /// 计算溢出（对齐 / 大小计算超出地址空间）。
    Overflow,
    /// 区域已销毁后仍尝试分配。
    Destroyed,
    /// 内存不足：区域内无可扩容空间或底层分配失败。
    OutOfMemory {
        /// 本次请求的字节数。
        requested: usize,
    },
    /// 请求大小超过区域最大容量上限（`SmartRegion`）。
    TooLarge {
        /// 本次请求的字节数。
        requested: usize,
        /// 区域允许的最大块大小。
        max: usize,
    },
    /// 区域已冻结（销毁 / `transfer` 冻结后）仍尝试分配。
    Frozen,
}

impl fmt::Display for AllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AllocError::Overflow => write!(f, "region allocation size overflow"),
            AllocError::Destroyed => write!(f, "region has been destroyed"),
            AllocError::OutOfMemory { requested } => {
                write!(f, "region out of memory (requested {requested} bytes)")
            }
            AllocError::TooLarge { requested, max } => {
                write!(f, "region allocation too large (requested {requested} bytes, max {max})")
            }
            AllocError::Frozen => write!(f, "region is frozen"),
        }
    }
}

impl std::error::Error for AllocError {}
