//! 区域检查错误。

use std::fmt;

use rlyeh_lexer::Span;

/// 区域检查错误。
///
/// 注意：HIR 子节点（表达式 / 语句）不携带源码位置，regionck 错误坐标
/// 取自查错所在函数的 `HirItem.span`（函数级粒度，合并源码坐标）。
/// `line` / `col` 经 `render(prelude_lines)` 减预置行数还原为用户文件坐标
/// （SH-P2-6 L1 余量）；精确的语句级坐标需 HIR 子节点 Span 传播，属后续重构。
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
    pub(crate) fn invalid_transfer(detail: impl Into<String>, span: Span) -> Self {
        RegionError::InvalidTransfer {
            detail: detail.into(),
            line: span.line,
            col: span.col,
        }
    }

    /// 区域不存在错误构造辅助。
    pub(crate) fn not_found(name: impl Into<String>, span: Span) -> Self {
        RegionError::RegionNotFound {
            name: name.into(),
            line: span.line,
            col: span.col,
        }
    }

    /// 重复 transfer 错误构造辅助。
    pub(crate) fn double_transfer(name: impl Into<String>, span: Span) -> Self {
        RegionError::DoubleTransfer {
            name: name.into(),
            line: span.line,
            col: span.col,
        }
    }

    /// 无法静态判定归属的 transfer 错误构造辅助（P005）。
    pub(crate) fn partial_transfer(detail: impl Into<String>, span: Span) -> Self {
        RegionError::PartialTransfer {
            detail: detail.into(),
            line: span.line,
            col: span.col,
        }
    }

    /// 嵌套方向错误构造辅助（P005）。
    pub(crate) fn outer_region_transfer(detail: impl Into<String>, span: Span) -> Self {
        RegionError::OuterRegionTransfer {
            detail: detail.into(),
            line: span.line,
            col: span.col,
        }
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
