//! 词法错误类型。

use thiserror::Error;

use crate::Span;

/// 词法错误
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LexError {
    /// 非法字符
    #[error("invalid character '{ch}' at {line}:{col}")]
    InvalidChar {
        /// 非法字符
        ch: char,
        /// 行号
        line: usize,
        /// 列号
        col: usize,
    },

    /// 未闭合的字符串字面量
    #[error("unterminated string literal starting at {line}:{col}")]
    UnterminatedString {
        /// 起始行号
        line: usize,
        /// 起始列号
        col: usize,
    },

    /// 未闭合的字符字面量
    #[error("unterminated character literal at {line}:{col}")]
    UnterminatedChar {
        /// 起始行号
        line: usize,
        /// 起始列号
        col: usize,
    },

    /// 非法转义序列
    #[error("invalid escape sequence '\\{ch}' at {line}:{col}")]
    InvalidEscape {
        /// 转义字符
        ch: char,
        /// 行号
        line: usize,
        /// 列号
        col: usize,
    },

    /// 整数溢出（超出 i128 范围）
    #[error("integer literal too large at {line}:{col}")]
    IntTooLarge {
        /// 行号
        line: usize,
        /// 列号
        col: usize,
    },

    /// 非法时间字面量
    #[error("invalid time literal at {line}:{col}")]
    InvalidTime {
        /// 行号
        line: usize,
        /// 列号
        col: usize,
    },

    /// 未闭合的块注释
    #[error("unterminated block comment starting at {line}:{col}")]
    UnterminatedBlockComment {
        /// 起始行号
        line: usize,
        /// 起始列号
        col: usize,
    },
}

impl LexError {
    /// 错误位置的 Span（供后续阶段传播位置信息）
    pub fn span(&self) -> Span {
        let (line, col) = match self {
            LexError::InvalidChar { line, col, .. }
            | LexError::UnterminatedString { line, col }
            | LexError::UnterminatedChar { line, col }
            | LexError::InvalidEscape { line, col, .. }
            | LexError::IntTooLarge { line, col }
            | LexError::InvalidTime { line, col }
            | LexError::UnterminatedBlockComment { line, col } => (*line, *col),
        };
        Span {
            start: 0,
            end: 0,
            line,
            col,
        }
    }
}
