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
        /// 相关位置标注（SH-P2-6 L2）：`(位置, 标签)` 列表。
        related: Vec<(Span, String)>,
    },
    /// 赋值给不可变绑定（`let x = 1; x = 2;`，Rust E0384 对应）。
    AssignToImmutable {
        /// 不可变绑定的变量名。
        name: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
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
        /// 相关位置标注（SH-P2-6 L2）：冲突的另一处借用位置。
        related: Vec<(Span, String)>,
    },
    /// 对不可变绑定取可变引用（`let x = 1; let r = &mut x;`，Rust E0596 对应）。
    BorrowMutImmutable {
        /// 变量名。
        name: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
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
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
    },
    /// 不能移动（transfer / move）处于借用中的值（防御性预留）。
    MoveWhileBorrowed {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
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
            related: Vec::new(),
        }
    }

    /// 不可变赋值错误构造辅助。
    pub(crate) fn assign_to_immutable(name: impl Into<String>, span: Span) -> Self {
        BorrowError::AssignToImmutable {
            name: name.into(),
            line: span.line,
            col: span.col,
            related: Vec::new(),
        }
    }

    /// 借用冲突错误构造辅助。
    pub(crate) fn borrow_conflict(detail: impl Into<String>, span: Span) -> Self {
        BorrowError::BorrowConflict {
            detail: detail.into(),
            line: span.line,
            col: span.col,
            related: Vec::new(),
        }
    }

    /// 对不可变绑定取可变引用错误构造辅助。
    pub(crate) fn borrow_mut_immutable(name: impl Into<String>, span: Span) -> Self {
        BorrowError::BorrowMutImmutable {
            name: name.into(),
            line: span.line,
            col: span.col,
            related: Vec::new(),
        }
    }

    /// 悬垂引用错误构造辅助。
    pub(crate) fn dangling_reference(name: impl Into<String>, span: Span) -> Self {
        BorrowError::DanglingReference {
            name: name.into(),
            line: span.line,
            col: span.col,
            related: Vec::new(),
        }
    }

    /// 追加相关位置标注（SH-P2-6 L2）：`(位置, 标签)` 列表，渲染为次级 `= note:` 行。
    pub(crate) fn with_related(mut self, spans: Vec<(Span, String)>) -> Self {
        match &mut self {
            BorrowError::UseAfterTransfer { related, .. }
            | BorrowError::AssignToImmutable { related, .. }
            | BorrowError::BorrowConflict { related, .. }
            | BorrowError::BorrowMutImmutable { related, .. }
            | BorrowError::DanglingReference { related, .. }
            | BorrowError::MoveWhileBorrowed { related, .. } => *related = spans,
        }
        self
    }

    /// 稳定错误码（SH-P2-6 L2 结构化诊断），形如 `BC0xx`。
    pub fn code(&self) -> &'static str {
        match self {
            BorrowError::UseAfterTransfer { .. } => "BC001",
            BorrowError::AssignToImmutable { .. } => "BC002",
            BorrowError::BorrowConflict { .. } => "BC003",
            BorrowError::BorrowMutImmutable { .. } => "BC004",
            BorrowError::DanglingReference { .. } => "BC005",
            BorrowError::MoveWhileBorrowed { .. } => "BC006",
        }
    }

    /// 修复建议（SH-P2-6 L2 结构化诊断）；无可行建议时返回 `None`。
    pub fn help(&self) -> Option<&'static str> {
        match self {
            BorrowError::UseAfterTransfer { .. } => Some(
                "被 `transfer` 转移后的值不能再使用；如需复用请 `clone` 或重新创建",
            ),
            BorrowError::AssignToImmutable { .. } => {
                Some("用 `let mut` 声明该绑定以支持赋值")
            }
            BorrowError::BorrowConflict { .. } => Some(
                "缩短借用生命周期，确保新旧借用不重叠；或克隆数据以避免别名冲突",
            ),
            BorrowError::BorrowMutImmutable { .. } => {
                Some("用 `let mut` 声明该绑定以支持 `&mut`")
            }
            BorrowError::DanglingReference { .. } => {
                Some("返回的引用只能指向参数或全局，不能指向局部变量")
            }
            BorrowError::MoveWhileBorrowed { .. } => {
                Some("在借用结束后再转移所有权")
            }
        }
    }

    /// 结构化诊断文本（SH-P2-6 L2）：在用户文件坐标下渲染
    /// `行:列: [CODE] 消息`，并附 `= help:` 修复建议与 `= note:` 相关位置标注。
    ///
    /// 旧 [`BorrowError::render`] / [`std::fmt::Display`] 保持原 `行:列: 消息`
    /// 格式不变（既有单测依赖其精确输出）；本方法供 `rlyeh-driver` 渲染 richer 诊断。
    pub fn render_structured(&self, prelude_lines: usize) -> String {
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
        let related = match self {
            BorrowError::UseAfterTransfer { related, .. }
            | BorrowError::AssignToImmutable { related, .. }
            | BorrowError::BorrowConflict { related, .. }
            | BorrowError::BorrowMutImmutable { related, .. }
            | BorrowError::DanglingReference { related, .. }
            | BorrowError::MoveWhileBorrowed { related, .. } => related,
        };
        let mut out = format!(
            "{}:{}: [{}] {}",
            line.saturating_sub(prelude_lines),
            col,
            self.code(),
            self.message()
        );
        if let Some(h) = self.help() {
            out.push_str(&format!("\n  = help: {h}"));
        }
        for (sp, label) in related {
            let rl = sp.line.saturating_sub(prelude_lines);
            out.push_str(&format!("\n  = note: {label} ({rl}:{})", sp.col));
        }
        out
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
