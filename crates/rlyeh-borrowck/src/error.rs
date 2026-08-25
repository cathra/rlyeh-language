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
    /// 借用冲突：同一时刻存在多个可变借用 / 可变与不可变借用并存，
    /// 或写入（赋值）被借用中的变量（Rust E0502 / E0499 对应）。
    BorrowConflict {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 对不可变绑定取可变引用（`let x = 1; let r = &mut x;`，Rust E0596 对应）。
    BorrowMutImmutable {
        /// 变量名。
        name: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
    },
    /// 悬垂引用：对局部变量的引用逃逸出其作用域
    /// （`fn f() -> &i64 { let x = 1; &x }`，Rust E0597 对应）。
    DanglingReference {
        /// 被引用（已消亡）的变量名。
        name: String,
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

    /// 借用冲突错误构造辅助。
    pub(crate) fn borrow_conflict(detail: impl Into<String>) -> Self {
        BorrowError::BorrowConflict {
            detail: detail.into(),
            line: 0,
            col: 0,
        }
    }

    /// 对不可变绑定取可变引用错误构造辅助。
    pub(crate) fn borrow_mut_immutable(name: impl Into<String>) -> Self {
        BorrowError::BorrowMutImmutable {
            name: name.into(),
            line: 0,
            col: 0,
        }
    }

    /// 悬垂引用错误构造辅助。
    pub(crate) fn dangling_reference(name: impl Into<String>) -> Self {
        BorrowError::DanglingReference {
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
            BorrowError::BorrowMutImmutable { name, .. } => {
                write!(
                    f,
                    "cannot borrow `{name}` as mutable, as it is not declared as mutable"
                )
            }
            BorrowError::DanglingReference { name, .. } => {
                write!(
                    f,
                    "`{name}` does not live long enough: borrowed reference escapes its scope"
                )
            }
            BorrowError::MoveWhileBorrowed { detail, .. } => {
                write!(f, "cannot move out of a borrowed value: {detail}")
            }
        }
    }
}

impl std::error::Error for BorrowError {}
