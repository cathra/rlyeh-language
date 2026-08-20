//! LIR lowering 错误。

use std::fmt;

use crate::LirType;

/// LIR lowering 错误。
#[derive(Debug, Clone, PartialEq)]
pub enum LirError {
    /// 基本块缺少终止符。
    MissingTerminator {
        /// 函数名
        function: String,
        /// 块索引
        block: usize,
    },
    /// 同一变量被赋以不一致的类型。
    TypeConflict {
        /// 变量名
        variable: String,
        /// 已有类型
        expected: LirType,
        /// 新类型
        found: LirType,
    },
    /// 整数字面量超出 i64 范围（LLVM 后端 MVP 仅支持 i64）。
    IntOverflow {
        /// 原始 i128 值
        value: i128,
    },
}

impl fmt::Display for LirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LirError::MissingTerminator { function, block } => {
                write!(f, "函数 `{function}` 的块 {block} 缺少终止符")
            }
            LirError::TypeConflict {
                variable,
                expected,
                found,
            } => {
                write!(
                    f,
                    "变量 `{variable}` 类型冲突：期望 {expected}，实际 {found}"
                )
            }
            LirError::IntOverflow { value } => {
                write!(f, "整数字面量 {value} 超出 i64 范围")
            }
        }
    }
}

impl std::error::Error for LirError {}
