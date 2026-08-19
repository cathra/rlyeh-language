//! 区域检查错误。

use std::fmt;

/// 区域检查错误。
///
/// 注意：HIR 节点不携带源码位置（见 zeta-hir 设计约定），
/// `line` / `col` 当前恒为 0，位置信息留待引入 Span 传播后填充。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionError {
    /// 区域逃逸：区域内对象在未 `transfer` 的情况下离开区域。
    RegionEscape {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 非法 transfer：对象不在所声明的源区域内。
    InvalidTransfer {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 引用了不存在的区域（区域名未在作用域内）。
    RegionNotFound {
        /// 区域名（不含 `'`）。
        name: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 对无法静态判定归属的对象执行 transfer（MVP 阶段保留）。
    PartialTransfer {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 同一对象被重复 transfer。
    DoubleTransfer {
        /// 对象名。
        name: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 不能 transfer 引用（P005：`transfer &x out of 'r`）。
    ///
    /// 当前 MVP 阶段 typecheck 拒绝 `&` 取址表达式（返回 `Unsupported`），
    /// 该变体为防御性预留：待 HIR 引入引用节点（`AddrOf`）后在此处拒绝。
    CannotTransferReference {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 不能从内层区域转移外层区域的对象（P005 嵌套方向检查）。
    ///
    /// `transfer x out of 'r` 必须在 `'r` 的直接作用域内书写；
    /// 从更内层区域转移外层区域对象会破坏区域的生命周期层级。
    OuterRegionTransfer {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 不能 transfer 非 `Sized` 对象（P005）。
    ///
    /// 依赖类型信息，MVP 阶段保留变体与构造入口，待类型标注进入 HIR 后启用。
    UnsizedTransfer {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
}

impl RegionError {
    /// 非法 transfer 错误构造辅助。
    pub(crate) fn invalid_transfer(detail: impl Into<String>) -> Self {
        RegionError::InvalidTransfer {
            detail: detail.into(),
            line: 0,
            col: 0,
        }
    }

    /// 区域不存在错误构造辅助。
    pub(crate) fn not_found(name: impl Into<String>) -> Self {
        RegionError::RegionNotFound {
            name: name.into(),
            line: 0,
            col: 0,
        }
    }

    /// 重复 transfer 错误构造辅助。
    pub(crate) fn double_transfer(name: impl Into<String>) -> Self {
        RegionError::DoubleTransfer {
            name: name.into(),
            line: 0,
            col: 0,
        }
    }

    /// 无法静态判定归属的 transfer 错误构造辅助（P005）。
    pub(crate) fn partial_transfer(detail: impl Into<String>) -> Self {
        RegionError::PartialTransfer {
            detail: detail.into(),
            line: 0,
            col: 0,
        }
    }

    /// 嵌套方向错误构造辅助（P005）。
    pub(crate) fn outer_region_transfer(detail: impl Into<String>) -> Self {
        RegionError::OuterRegionTransfer {
            detail: detail.into(),
            line: 0,
            col: 0,
        }
    }
}

impl fmt::Display for RegionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegionError::RegionEscape { detail, .. } => {
                write!(f, "region escape: {detail}")
            }
            RegionError::InvalidTransfer { detail, .. } => {
                write!(f, "invalid transfer: {detail}")
            }
            RegionError::RegionNotFound { name, .. } => {
                write!(f, "region `'{name}` not found in current scope")
            }
            RegionError::PartialTransfer { detail, .. } => {
                write!(f, "partial transfer: {detail}")
            }
            RegionError::DoubleTransfer { name, .. } => {
                write!(f, "object `{name}` is transferred more than once")
            }
            RegionError::CannotTransferReference { detail, .. } => {
                write!(f, "cannot transfer a reference: {detail}")
            }
            RegionError::OuterRegionTransfer { detail, .. } => {
                write!(f, "cannot transfer from an inner region: {detail}")
            }
            RegionError::UnsizedTransfer { detail, .. } => {
                write!(f, "cannot transfer an unsized value: {detail}")
            }
        }
    }
}

impl std::error::Error for RegionError {}
