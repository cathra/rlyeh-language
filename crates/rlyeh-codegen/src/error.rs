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
    /// 内部不变量被破坏——LLVM 发射按 `LirStmt` 变体分派到子模块时，
    /// 收到的变体与调用点的 or-pattern 约定不符。
    ///
    /// 调用点已保证只有约定变体会进入对应子模块，故本变体**不可达**；
    /// 保留它是为了在分派表被误改时给出明确诊断而非静默跳过发射。
    Internal(String),
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
            CodegenError::Internal(msg) => {
                write!(f, "编译器内部错误：{msg}")
            }
        }
    }
}

impl std::error::Error for CodegenError {}
