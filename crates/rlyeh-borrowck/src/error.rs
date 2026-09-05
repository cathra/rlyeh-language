//! 借用检查错误。

use std::fmt;

use rlyeh_lexer::Span;

/// 借用检查错误。
///
/// HIR 子节点（表达式 / 语句 / 块）现已携带源 `Span`（由 typecheck 在生成 HIR
/// 时从 `AstExpr` / `AstStmt` 全量传播），borrowck 错误坐标取自查错节点自身的
/// `span`（表达式 / 语句 / 块级粒度，合并源码坐标），经 `render(prelude_lines)`
/// 减预置行数还原为用户坐标。
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
    ///
    /// `span` 取自查错节点自身的 `span`（表达式 / 语句 / 块级，合并源码坐标），
    /// 由 `check_expr` / `check_stmt` / `check_block` 在入口处写入 `cur_span`。
    pub(crate) fn use_after_transfer(name: impl Into<String>, span: Span) -> Self {
        BorrowError::UseAfterTransfer {
            name: name.into(),
            line: span.line,
            col: span.col,
        }
    }

    /// 不可变赋值错误构造辅助。
    pub(crate) fn assign_to_immutable(name: impl Into<String>, span: Span) -> Self {
        BorrowError::AssignToImmutable {
            name: name.into(),
            line: span.line,
            col: span.col,
        }
    }

    /// 借用冲突错误构造辅助。
    pub(crate) fn borrow_conflict(detail: impl Into<String>, span: Span) -> Self {
        BorrowError::BorrowConflict {
            detail: detail.into(),
            line: span.line,
            col: span.col,
        }
    }

    /// 对不可变绑定取可变引用错误构造辅助。
    pub(crate) fn borrow_mut_immutable(name: impl Into<String>, span: Span) -> Self {
        BorrowError::BorrowMutImmutable {
            name: name.into(),
            line: span.line,
            col: span.col,
        }
    }

    /// 悬垂引用错误构造辅助。
    pub(crate) fn dangling_reference(name: impl Into<String>, span: Span) -> Self {
        BorrowError::DanglingReference {
            name: name.into(),
            line: span.line,
            col: span.col,
        }
    }

    /// 错误正文（不含 `line:col:` 前缀）。
    fn message(&self) -> String {
        match self {
            BorrowError::UseAfterTransfer { name, .. } => {
                format!("use of moved value: `{name}` was transferred out of its region")
            }
            BorrowError::AssignToImmutable { name, .. } => {
                format!("cannot assign to immutable variable `{name}`")
            }
            BorrowError::BorrowConflict { detail, .. } => {
                format!("borrow conflict: {detail}")
            }
            BorrowError::BorrowMutImmutable { name, .. } => format!(
                "cannot borrow `{name}` as mutable, as it is not declared as mutable"
            ),
            BorrowError::DanglingReference { name, .. } => format!(
                "`{name}` does not live long enough: borrowed reference escapes its scope"
            ),
            BorrowError::MoveWhileBorrowed { detail, .. } => {
                format!("cannot move out of a borrowed value: {detail}")
            }
        }
    }

    /// 渲染诊断文本，并把合并源码坐标（含 std 预置偏移）还原为用户文件坐标。
    ///
    /// `prelude_lines` 为预置行数；用户行号 = 合并行号 - `prelude_lines`
    /// （SH-P2-6 L1 余量：与 typecheck 诊断对齐到同一坐标系）。
    pub fn render(&self, prelude_lines: usize) -> String {
        let line = match self {
            BorrowError::UseAfterTransfer { line, .. }
            | BorrowError::AssignToImmutable { line, .. }
            | BorrowError::BorrowConflict { line, .. }
            | BorrowError::BorrowMutImmutable { line, .. }
            | BorrowError::DanglingReference { line, .. }
            | BorrowError::MoveWhileBorrowed { line, .. } => *line,
        };
        let col = match self {
            BorrowError::UseAfterTransfer { col, .. }
            | BorrowError::AssignToImmutable { col, .. }
            | BorrowError::BorrowConflict { col, .. }
            | BorrowError::BorrowMutImmutable { col, .. }
            | BorrowError::DanglingReference { col, .. }
            | BorrowError::MoveWhileBorrowed { col, .. } => *col,
        };
        // 无真实位置（line == 0，仅测试/调试占位；生产路径恒 >= 1）：
        // 退化为纯消息，不输出误导性的 `0:0:` 前缀。
        if line == 0 {
            return self.message();
        }
        format!(
            "{}:{}: {}",
            line.saturating_sub(prelude_lines),
            col,
            self.message()
        )
    }
}

impl fmt::Display for BorrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render(0))
    }
}

impl std::error::Error for BorrowError {}
