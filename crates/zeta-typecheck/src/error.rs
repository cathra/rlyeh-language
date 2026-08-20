//! 类型检查错误定义。

use std::fmt;

use zeta_lexer::Span;

/// 类型检查错误。
///
/// 所有错误均携带源码位置（行 / 列），通过 [`std::fmt::Display`]
/// 输出人类可读的错误信息。
#[derive(Debug, Clone, PartialEq)]
pub enum TypeError {
    /// 未定义变量
    UndefinedVariable {
        /// 变量名
        name: String,
        /// 源码位置
        span: Span,
    },
    /// 未定义类型
    UndefinedType {
        /// 类型名
        name: String,
        /// 源码位置
        span: Span,
    },
    /// 未定义函数
    UndefinedFunction {
        /// 函数名
        name: String,
        /// 源码位置
        span: Span,
    },
    /// 函数缺少函数体
    MissingFunctionBody {
        /// 函数名
        name: String,
        /// 源码位置
        span: Span,
    },
    /// 函数体嵌套深度超限（递归展开保护）
    FunctionBodyOverflow {
        /// 函数名
        name: String,
        /// 源码位置
        span: Span,
    },
    /// 类型不匹配
    WrongType {
        /// 期望的类型
        expected: String,
        /// 实际类型
        found: String,
        /// 源码位置
        span: Span,
    },
    /// 期望整数类型
    ExpectedInt {
        /// 实际类型
        found: String,
        /// 源码位置
        span: Span,
    },
    /// 期望布尔类型
    ExpectedBool {
        /// 实际类型
        found: String,
        /// 源码位置
        span: Span,
    },
    /// 期望数值类型
    ExpectedNumeric {
        /// 实际类型
        found: String,
        /// 源码位置
        span: Span,
    },
    /// 期望可迭代类型
    ExpectedIterable {
        /// 实际类型
        found: String,
        /// 源码位置
        span: Span,
    },
    /// 期望结构体类型
    ExpectedStruct {
        /// 实际类型
        found: String,
        /// 源码位置
        span: Span,
    },
    /// 结构体字段未定义
    UnknownField {
        /// 结构体名
        struct_name: String,
        /// 字段名
        field: String,
        /// 源码位置
        span: Span,
    },
    /// 结构体构造缺少字段
    MissingField {
        /// 结构体名
        struct_name: String,
        /// 字段名
        field: String,
        /// 源码位置
        span: Span,
    },
    /// 期望可变绑定
    ExpectedMutable {
        /// 实际类型
        found: String,
        /// 源码位置
        span: Span,
    },
    /// 函数未找到
    FunctionNotFound {
        /// 函数名
        name: String,
        /// 源码位置
        span: Span,
    },
    /// 实参数量不匹配
    UnexpectedArgumentCount {
        /// 函数名
        name: String,
        /// 期望数量
        expected: usize,
        /// 实际数量
        found: usize,
        /// 源码位置
        span: Span,
    },
    /// 实参类型不匹配
    ArgumentTypeMismatch {
        /// 函数名
        name: String,
        /// 实参序号
        index: usize,
        /// 期望类型
        expected: String,
        /// 实际类型
        found: String,
        /// 源码位置
        span: Span,
    },
    /// 缺少 `PartialEq` 支持
    MissingPartialEq {
        /// 类型
        type_: String,
        /// 源码位置
        span: Span,
    },
    /// 缺少 `PartialOrd` 支持
    MissingPartialOrd {
        /// 类型
        type_: String,
        /// 源码位置
        span: Span,
    },
    /// 比较链方向不一致（`0 < x > 10`）
    InconsistentComparison {
        /// 源码位置
        span: Span,
    },
    /// 比较链元素类型不兼容
    ChainTypeMismatch {
        /// 源码位置
        span: Span,
    },
    /// 集合内范围元素类型与值不兼容
    InSetTypeMismatch {
        /// 值类型
        value_type: String,
        /// 元素类型
        element_type: String,
        /// 源码位置
        span: Span,
    },
    /// 集合内范围端点不是编译期整数常量
    NonConstantBound {
        /// 源码位置
        span: Span,
    },
    /// 尚未支持的语法
    Unsupported {
        /// 描述
        what: String,
        /// 源码位置
        span: Span,
    },
}

impl TypeError {
    /// 错误位置的行号
    pub fn line(&self) -> usize {
        self.span().line
    }

    /// 错误位置的列号
    pub fn col(&self) -> usize {
        self.span().col
    }

    fn span(&self) -> Span {
        match self {
            TypeError::UndefinedVariable { span, .. }
            | TypeError::UndefinedType { span, .. }
            | TypeError::UndefinedFunction { span, .. }
            | TypeError::MissingFunctionBody { span, .. }
            | TypeError::FunctionBodyOverflow { span, .. }
            | TypeError::WrongType { span, .. }
            | TypeError::ExpectedInt { span, .. }
            | TypeError::ExpectedBool { span, .. }
            | TypeError::ExpectedNumeric { span, .. }
            | TypeError::ExpectedIterable { span, .. }
            | TypeError::ExpectedStruct { span, .. }
            | TypeError::UnknownField { span, .. }
            | TypeError::MissingField { span, .. }
            | TypeError::ExpectedMutable { span, .. }
            | TypeError::FunctionNotFound { span, .. }
            | TypeError::UnexpectedArgumentCount { span, .. }
            | TypeError::ArgumentTypeMismatch { span, .. }
            | TypeError::MissingPartialEq { span, .. }
            | TypeError::MissingPartialOrd { span, .. }
            | TypeError::InconsistentComparison { span }
            | TypeError::ChainTypeMismatch { span }
            | TypeError::InSetTypeMismatch { span, .. }
            | TypeError::NonConstantBound { span }
            | TypeError::Unsupported { span, .. } => *span,
        }
    }
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (line, col) = (self.line(), self.col());
        let loc = format!("{line}:{col}");
        match self {
            TypeError::UndefinedVariable { name, .. } => {
                write!(f, "{loc}: error: undefined variable `{name}`")
            }
            TypeError::UndefinedType { name, .. } => {
                write!(f, "{loc}: error: undefined type `{name}`")
            }
            TypeError::UndefinedFunction { name, .. } => {
                write!(f, "{loc}: error: undefined function `{name}`")
            }
            TypeError::MissingFunctionBody { name, .. } => {
                write!(f, "{loc}: error: function `{name}` is missing a body")
            }
            TypeError::FunctionBodyOverflow { name, .. } => {
                write!(f, "{loc}: error: function `{name}` body exceeds recursion limit")
            }
            TypeError::WrongType {
                expected, found, ..
            } => write!(f, "{loc}: error: expected `{expected}`, found `{found}`"),
            TypeError::ExpectedInt { found, .. } => {
                write!(f, "{loc}: error: expected an integer, found `{found}`")
            }
            TypeError::ExpectedBool { found, .. } => {
                write!(f, "{loc}: error: expected `bool`, found `{found}`")
            }
            TypeError::ExpectedNumeric { found, .. } => {
                write!(f, "{loc}: error: expected a numeric type, found `{found}`")
            }
            TypeError::ExpectedIterable { found, .. } => {
                write!(f, "{loc}: error: expected an iterable, found `{found}`")
            }
            TypeError::ExpectedStruct { found, .. } => {
                write!(f, "{loc}: error: expected a struct, found `{found}`")
            }
            TypeError::UnknownField {
                struct_name,
                field,
                ..
            } => write!(
                f,
                "{loc}: error: struct `{struct_name}` has no field `{field}`"
            ),
            TypeError::MissingField {
                struct_name,
                field,
                ..
            } => write!(
                f,
                "{loc}: error: missing field `{field}` in struct `{struct_name}` construction"
            ),
            TypeError::ExpectedMutable { found, .. } => {
                write!(f, "{loc}: error: expected a mutable binding, found `{found}`")
            }
            TypeError::FunctionNotFound { name, .. } => {
                write!(f, "{loc}: error: function `{name}` not found")
            }
            TypeError::UnexpectedArgumentCount {
                name,
                expected,
                found,
                ..
            } => write!(
                f,
                "{loc}: error: function `{name}` expects {expected} arguments, found {found}"
            ),
            TypeError::ArgumentTypeMismatch {
                name,
                index,
                expected,
                found,
                ..
            } => write!(
                f,
                "{loc}: error: argument {index} of `{name}` expects `{expected}`, found `{found}`"
            ),
            TypeError::MissingPartialEq { type_, .. } => {
                write!(f, "{loc}: error: type `{type_}` does not support `==`")
            }
            TypeError::MissingPartialOrd { type_, .. } => {
                write!(f, "{loc}: error: type `{type_}` does not support ordering")
            }
            TypeError::InconsistentComparison { .. } => {
                write!(f, "{loc}: error: comparison chain has inconsistent direction")
            }
            TypeError::ChainTypeMismatch { .. } => {
                write!(f, "{loc}: error: comparison chain element types are incompatible")
            }
            TypeError::InSetTypeMismatch {
                value_type,
                element_type,
                ..
            } => write!(
                f,
                "{loc}: error: in-set value type `{value_type}` is incompatible with element type `{element_type}`"
            ),
            TypeError::NonConstantBound { .. } => {
                write!(f, "{loc}: error: set range bounds must be compile-time integer constants")
            }
            TypeError::Unsupported { what, .. } => {
                write!(f, "{loc}: error: unsupported syntax: {what}")
            }
        }
    }
}

impl std::error::Error for TypeError {}
