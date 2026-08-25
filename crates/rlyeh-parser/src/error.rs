//! 语法错误类型。

use thiserror::Error;
use zeta_lexer::{LexError, Span};

/// 语法分析错误
#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    /// 意外的 token
    #[error("unexpected token: expected {expected}, found {found} at {line}:{col}")]
    UnexpectedToken {
        /// 期望内容的描述
        expected: String,
        /// 实际遇到的 token
        found: String,
        /// 行号
        line: usize,
        /// 列号
        col: usize,
    },

    /// 意外的文件结尾
    #[error("unexpected end of file")]
    UnexpectedEof,

    /// 非法的比较链（链上方向不一致，如 `0 < x > 10`）。
    ///
    /// 语义分析阶段用于报告区间外写法冲突；解析器允许收集
    /// 混合方向链，由后续阶段校验。
    #[error("invalid comparison chain at {line}:{col}")]
    InvalidComparisonChain {
        /// 行号
        line: usize,
        /// 列号
        col: usize,
    },

    /// 缺少表达式
    #[error("expected {expected} at {line}:{col}")]
    MissingExpr {
        /// 期望内容的描述
        expected: String,
        /// 行号
        line: usize,
        /// 列号
        col: usize,
    },

    /// 宏错误（`macro_rules!` 定义 / 调用 / 展开失败）。
    #[error("macro error: {msg} at {line}:{col}")]
    Macro {
        /// 错误信息
        msg: String,
        /// 行号
        line: usize,
        /// 列号
        col: usize,
    },

    /// 词法错误
    #[error(transparent)]
    LexError(#[from] LexError),
}

impl ParseError {
    /// 错误位置的 Span（供语义分析阶段传播位置信息）
    pub fn span(&self) -> Span {
        match self {
            ParseError::UnexpectedToken { line, col, .. }
            | ParseError::InvalidComparisonChain { line, col }
            | ParseError::MissingExpr { line, col, .. }
            | ParseError::Macro { line, col, .. } => Span {
                start: 0,
                end: 0,
                line: *line,
                col: *col,
            },
            ParseError::UnexpectedEof => Span {
                start: 0,
                end: 0,
                line: 0,
                col: 0,
            },
            ParseError::LexError(e) => e.span(),
        }
    }
}
