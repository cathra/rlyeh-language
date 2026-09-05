//! 区域检查错误。

use std::fmt;

use rlyeh_lexer::Span;

/// 区域检查错误。
///
/// HIR 子节点（表达式 / 语句 / 块）现已携带源 `Span`（由 typecheck 在生成 HIR
/// 时从 `AstExpr` / `AstStmt` 全量传播），regionck 错误坐标取自查错节点自身的
/// `span`（表达式 / 语句 / 块级粒度，合并源码坐标），经 `render(prelude_lines)`
/// 减预置行数还原为用户文件坐标。
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
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
    },
    /// 非法 transfer：对象不在所声明的源区域内。
    InvalidTransfer {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
    },
    /// 引用了不存在的区域（区域名未在作用域内）。
    RegionNotFound {
        /// 区域名（不含 `'`）。
        name: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
    },
    /// 对无法静态判定归属的对象执行 transfer（MVP 阶段保留）。
    PartialTransfer {
        /// 错误描述。
        detail: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
    },
    /// 同一对象被重复 transfer。
    DoubleTransfer {
        /// 对象名。
        name: String,
        /// 行号（预留）。
        line: usize,
        /// 列号（预留）。
        col: usize,
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
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
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
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
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
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
        /// 相关位置标注（SH-P2-6 L2）。
        related: Vec<(Span, String)>,
    },
}

impl RegionError {
    /// 非法 transfer 错误构造辅助。
    pub(crate) fn invalid_transfer(detail: impl Into<String>, span: Span) -> Self {
        RegionError::InvalidTransfer {
            detail: detail.into(),
            line: span.line,
            col: span.col,
            related: Vec::new(),
        }
    }

    /// 区域不存在错误构造辅助。
    pub(crate) fn not_found(name: impl Into<String>, span: Span) -> Self {
        RegionError::RegionNotFound {
            name: name.into(),
            line: span.line,
            col: span.col,
            related: Vec::new(),
        }
    }

    /// 重复 transfer 错误构造辅助。
    pub(crate) fn double_transfer(name: impl Into<String>, span: Span) -> Self {
        RegionError::DoubleTransfer {
            name: name.into(),
            line: span.line,
            col: span.col,
            related: Vec::new(),
        }
    }

    /// 无法静态判定归属的 transfer 错误构造辅助（P005）。
    pub(crate) fn partial_transfer(detail: impl Into<String>, span: Span) -> Self {
        RegionError::PartialTransfer {
            detail: detail.into(),
            line: span.line,
            col: span.col,
            related: Vec::new(),
        }
    }

    /// 嵌套方向错误构造辅助（P005）。
    pub(crate) fn outer_region_transfer(detail: impl Into<String>, span: Span) -> Self {
        RegionError::OuterRegionTransfer {
            detail: detail.into(),
            line: span.line,
            col: span.col,
            related: Vec::new(),
        }
    }

    /// 追加相关位置标注（SH-P2-6 L2）：`(位置, 标签)` 列表，渲染为次级 `= note:` 行。
    ///
    /// 检查器在重复 transfer / 嵌套方向错误 / 对象归属其它区域错误中回指
    /// 首次 transfer 处或区域声明处。
    pub(crate) fn with_related(mut self, spans: Vec<(Span, String)>) -> Self {
        match &mut self {
            RegionError::RegionEscape { related, .. }
            | RegionError::InvalidTransfer { related, .. }
            | RegionError::RegionNotFound { related, .. }
            | RegionError::PartialTransfer { related, .. }
            | RegionError::DoubleTransfer { related, .. }
            | RegionError::CannotTransferReference { related, .. }
            | RegionError::OuterRegionTransfer { related, .. }
            | RegionError::UnsizedTransfer { related, .. } => *related = spans,
        }
        self
    }

    /// 稳定错误码（SH-P2-6 L2 结构化诊断），形如 `RC0xx`。
    pub fn code(&self) -> &'static str {
        match self {
            RegionError::RegionEscape { .. } => "RC001",
            RegionError::InvalidTransfer { .. } => "RC002",
            RegionError::RegionNotFound { .. } => "RC003",
            RegionError::PartialTransfer { .. } => "RC004",
            RegionError::DoubleTransfer { .. } => "RC005",
            RegionError::CannotTransferReference { .. } => "RC006",
            RegionError::OuterRegionTransfer { .. } => "RC007",
            RegionError::UnsizedTransfer { .. } => "RC008",
        }
    }

    /// 修复建议（SH-P2-6 L2 结构化诊断）；无可行建议时返回 `None`。
    pub fn help(&self) -> Option<&'static str> {
        match self {
            RegionError::RegionEscape { .. } => Some(
                "在区域结束前 `transfer` 该对象出区域，或延长区域作用域",
            ),
            RegionError::InvalidTransfer { .. } => {
                Some("仅可 `transfer` 位于声明源区域内的对象")
            }
            RegionError::RegionNotFound { .. } => {
                Some("先 `region 'name { ... }` 声明该区域，再使用 `in 'name`")
            }
            RegionError::PartialTransfer { .. } => Some(
                "确保 transfer 目标可静态判定归属（为整体变量，非表达式结果）",
            ),
            RegionError::DoubleTransfer { .. } => {
                Some("同一对象只能 `transfer` 一次；重复转移前确认是否已转出")
            }
            RegionError::CannotTransferReference { .. } => {
                Some("不能 transfer 引用；改为 transfer 其指向的值")
            }
            RegionError::OuterRegionTransfer { .. } => {
                Some("`transfer ... out of 'r` 须写在 `'r` 直接作用域内")
            }
            RegionError::UnsizedTransfer { .. } => {
                Some("仅可 transfer `Sized` 类型对象")
            }
        }
    }

    /// 结构化诊断文本（SH-P2-6 L2）：在用户文件坐标下渲染
    /// `行:列: [CODE] 消息`，并附 `= help:` 修复建议与 `= note:` 相关位置标注。
    ///
    /// 旧 [`RegionError::render`] / [`std::fmt::Display`] 保持原 `行:列: 消息`
    /// 格式不变（既有单测依赖其精确输出）；本方法供 `rlyeh-driver` 渲染 richer 诊断。
    pub fn render_structured(&self, prelude_lines: usize) -> String {
        let line = match self {
            RegionError::RegionEscape { line, .. }
            | RegionError::InvalidTransfer { line, .. }
            | RegionError::RegionNotFound { line, .. }
            | RegionError::PartialTransfer { line, .. }
            | RegionError::DoubleTransfer { line, .. }
            | RegionError::CannotTransferReference { line, .. }
            | RegionError::OuterRegionTransfer { line, .. }
            | RegionError::UnsizedTransfer { line, .. } => *line,
        };
        let col = match self {
            RegionError::RegionEscape { col, .. }
            | RegionError::InvalidTransfer { col, .. }
            | RegionError::RegionNotFound { col, .. }
            | RegionError::PartialTransfer { col, .. }
            | RegionError::DoubleTransfer { col, .. }
            | RegionError::CannotTransferReference { col, .. }
            | RegionError::OuterRegionTransfer { col, .. }
            | RegionError::UnsizedTransfer { col, .. } => *col,
        };
        let related = match self {
            RegionError::RegionEscape { related, .. }
            | RegionError::InvalidTransfer { related, .. }
            | RegionError::RegionNotFound { related, .. }
            | RegionError::PartialTransfer { related, .. }
            | RegionError::DoubleTransfer { related, .. }
            | RegionError::CannotTransferReference { related, .. }
            | RegionError::OuterRegionTransfer { related, .. }
            | RegionError::UnsizedTransfer { related, .. } => related,
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
            RegionError::RegionEscape { detail, .. } => format!("region escape: {detail}"),
            RegionError::InvalidTransfer { detail, .. } => {
                format!("invalid transfer: {detail}")
            }
            RegionError::RegionNotFound { name, .. } => {
                format!("region `'{name}` not found in current scope")
            }
            RegionError::PartialTransfer { detail, .. } => {
                format!("partial transfer: {detail}")
            }
            RegionError::DoubleTransfer { name, .. } => {
                format!("object `{name}` is transferred more than once")
            }
            RegionError::CannotTransferReference { detail, .. } => {
                format!("cannot transfer a reference: {detail}")
            }
            RegionError::OuterRegionTransfer { detail, .. } => {
                format!("cannot transfer from an inner region: {detail}")
            }
            RegionError::UnsizedTransfer { detail, .. } => {
                format!("cannot transfer an unsized value: {detail}")
            }
        }
    }

    /// 渲染诊断文本，并把合并源码坐标（含 std 预置偏移）还原为用户文件坐标。
    ///
    /// `prelude_lines` 为预置行数；用户行号 = 合并行号 - `prelude_lines`
    /// （SH-P2-6 L1 余量：与 typecheck 诊断对齐到同一坐标系）。
    pub fn render(&self, prelude_lines: usize) -> String {
        let line = match self {
            RegionError::RegionEscape { line, .. }
            | RegionError::InvalidTransfer { line, .. }
            | RegionError::RegionNotFound { line, .. }
            | RegionError::PartialTransfer { line, .. }
            | RegionError::DoubleTransfer { line, .. }
            | RegionError::CannotTransferReference { line, .. }
            | RegionError::OuterRegionTransfer { line, .. }
            | RegionError::UnsizedTransfer { line, .. } => *line,
        };
        let col = match self {
            RegionError::RegionEscape { col, .. }
            | RegionError::InvalidTransfer { col, .. }
            | RegionError::RegionNotFound { col, .. }
            | RegionError::PartialTransfer { col, .. }
            | RegionError::DoubleTransfer { col, .. }
            | RegionError::CannotTransferReference { col, .. }
            | RegionError::OuterRegionTransfer { col, .. }
            | RegionError::UnsizedTransfer { col, .. } => *col,
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

impl fmt::Display for RegionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render(0))
    }
}

impl std::error::Error for RegionError {}
