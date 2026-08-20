//! 借用检查错误。

use std::fmt;

/// 借用检查错误。
///
/// 注意：HIR 节点不携带源码位置（见 zeta-hir 设计约定），
/// `line` / `col` 当前恒为 0，位置信息留待引入 Span 传播后填充。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorrowError {
    /// use-after-move：变量被 `transfer` 转移后再次使用（Rust E0382 对应）。
    UseAfterTransfer {
        /// 被转移后仍被使用的变量名。
        name: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 赋值给不可变绑定（`let x = 1; x = 2;`，Rust E0384 对应）。
    AssignToImmutable {
        /// 不可变绑定的变量名。
        name: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 借用冲突：同一时刻存在多个可变借用 / 可变与不可变借用并存。
    ///
    /// 当前 MVP 阶段 typecheck 拒绝 `&` 取址表达式（返回 `Unsupported`），
    /// 该变体为防御性预留：待 HIR 引入引用节点（`AddrOf`）后在此处拒绝。
    BorrowConflict {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 不能移动（transfer / move）处于借用中的值（防御性预留）。
    MoveWhileBorrowed {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
}

impl BorrowError {
    /// use-after-move 错误构造辅助。
    pub(crate) fn use_after_transfer(name: impl Into<String>) -> Self {
        BorrowError::UseAfterTransfer {
            name: name.into(),
            line: 0,
            col: 0,
        }
    }

    /// 不可变赋值错误构造辅助。
    pub(crate) fn assign_to_immutable(name: impl Into<String>) -> Self {
        BorrowError::AssignToImmutable {
            name: name.into(),
            line: 0,
            col: 0,
        }
    }
}

impl fmt::Display for BorrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BorrowError::UseAfterTransfer { name, .. } => {
                write!(
                    f,
                    "use of moved value: `{name}` was transferred out of its region"
                )
            }
            BorrowError::AssignToImmutable { name, .. } => {
                write!(f, "cannot assign to immutable variable `{name}`")
            }
            BorrowError::BorrowConflict { detail, .. } => {
                write!(f, "borrow conflict: {detail}")
            }
            BorrowError::MoveWhileBorrowed { detail, .. } => {
                write!(f, "cannot move out of a borrowed value: {detail}")
            }
        }
    }
}

impl std::error::Error for BorrowError {}
