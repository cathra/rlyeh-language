//! 代码生成错误。

use std::fmt;

use rlyeh_lir::LirType;

/// 代码生成错误。
#[derive(Debug, Clone, PartialEq)]
pub enum CodegenError {
    /// 遇到当前后端不支持的 LIR 类型。
    UnsupportedType {
        /// 类型
        ty: LirType,
        /// 上下文说明
        context: String,
    },
    /// 调用未定义的函数。
    UndefinedFunction {
        /// 函数名
        name: String,
    },
    /// `main` 函数定义不合法（MVP 要求无参数）。
    InvalidMain,
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodegenError::UnsupportedType { ty, context } => {
                write!(f, "类型 {ty} 不支持：{context}")
            }
            CodegenError::UndefinedFunction { name } => {
                write!(f, "调用未定义的函数 `{name}`")
            }
            CodegenError::InvalidMain => {
                write!(f, "`main` 函数必须无参数（MVP 约束）")
            }
        }
    }
}

impl std::error::Error for CodegenError {}
