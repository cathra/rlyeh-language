//! 类型检查错误定义。

use std::fmt;

use rlyeh_lexer::Span;

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
    /// 在 `unsafe` 块外调用 extern 函数（SH-P0-1 E3 门禁）
    UnsafeExternCall {
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
    /// 类型联合成员不互不相交（U1 受限制的类型联合）
    ///
    /// "受限制"的核心约束：联合成员必须两两互不相交，保证 tag 判别无歧义、
    /// 收窄安全。重复成员（`i64 | i64`）与可重叠成员（`&T | &mut T`、
    /// `i64 | isize`）均触发本错误。
    UnionMembersNotDisjoint {
        /// 冲突的第一个成员
        first: String,
        /// 冲突的第二个成员
        second: String,
        /// 描述（冲突原因）
        why: String,
        /// 源码位置
        span: Span,
    },
    /// 泛型实参不满足 trait bound（U3）
    GenericBoundMismatch {
        /// 泛型参数名
        param: String,
        /// 未满足的 bound trait 名
        bound: String,
        /// 实参具体类型
        ty: String,
        /// 源码位置
        span: Span,
    },
    /// 泛型实参数量与类型参数数量不符（U8 泛型结构体构造）
    GenericArityMismatch {
        /// 泛型结构体名
        name: String,
        /// 类型参数数量
        expected: usize,
        /// 提供的实参数量
        found: usize,
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
            | TypeError::Unsupported { span, .. }
            | TypeError::UnionMembersNotDisjoint { span, .. }
            | TypeError::GenericBoundMismatch { span, .. }
            | TypeError::GenericArityMismatch { span, .. }
            | TypeError::UnsafeExternCall { span, .. } => *span,
        }
    }
}

/// 按给定的位置前缀 `loc`（`line:col`）写出诊断正文。
///
/// 抽取为独立函数，使 [`fmt::Display`] 与 [`TypeError::to_string_with_offset`]
/// 复用同一套正文格式化（仅 `loc` 不同）。
fn write_message(f: &mut fmt::Formatter<'_>, loc: &str, err: &TypeError) -> fmt::Result {
    match err {
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
        TypeError::UnsafeExternCall { name, .. } => write!(
            f,
            "{loc}: error: call to extern function `{name}` must be inside an `unsafe` block"
        ),
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
        TypeError::UnionMembersNotDisjoint {
            first,
            second,
            why,
            ..
        } => write!(
            f,
            "{loc}: error: union members `{first}` and `{second}` are not disjoint ({why})"
        ),
        TypeError::GenericBoundMismatch {
            param,
            bound,
            ty,
            ..
        } => write!(
            f,
            "{loc}: error: type `{ty}` does not implement trait `{bound}` (bound on generic parameter `{param}`)"
        ),
        TypeError::GenericArityMismatch {
            name,
            expected,
            found,
            ..
        } => write!(
            f,
            "{loc}: error: generic type `{name}` expects {expected} type argument(s), but {found} were provided"
        ),
    }
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let loc = format!("{}:{}", self.line(), self.col());
        write_message(f, &loc, self)
    }
}

impl TypeError {
    /// 渲染诊断文本，并把**合并源码坐标（含 std 预置偏移）还原为用户源码坐标**。
    ///
    /// `rlyeh run/build` 会把 std 预置（`prelude`）拼接到用户源码之前，
    /// 因此 `TypeError` 携带的 `Span` 行号是合并源码行号。落在实际用户代码上的
    /// 错误（`span.start >= prelude_len`）需减去预置行数 `prelude_lines` 才能得到
    /// 用户文件行号（`prelude` 以换行结尾，用户源码从下一行第 1 列起，列号不变）。
    /// 落在预置范围内（编译器生成项 / 预置内部位置）的错误不做还原，保持原坐标。
    ///
    /// 该方法是 L1（SH-P2-6 诊断对齐）的核心：使诊断行号对应用户 `.rl` 文件，
    /// 而非合并源码（此前会报出 `~7155` 这类偏移行号）。
    pub fn to_string_with_offset(&self, prelude_len: usize, prelude_lines: usize) -> String {
        let span = self.span();
        if span.start < prelude_len {
            return self.to_string();
        }
        let loc = format!("{}:{}", span.line.saturating_sub(prelude_lines), span.col);
        // 复用同一套正文格式化，仅替换 loc
        struct Offset<'a> {
            err: &'a TypeError,
            loc: String,
        }
        impl fmt::Display for Offset<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write_message(f, &self.loc, self.err)
            }
        }
        Offset {
            err: self,
            loc: loc.clone(),
        }
        .to_string()
    }
}

impl TypeError {
    /// 稳定错误码（SH-P2-6 L2 结构化诊断）。
    ///
    /// 形如 `TC0xx`，跨编译器版本稳定，可作为工具 / CI 的机器可读锚点。
    pub fn code(&self) -> &'static str {
        match self {
            TypeError::UndefinedVariable { .. } => "TC001",
            TypeError::UndefinedType { .. } => "TC002",
            TypeError::UndefinedFunction { .. } => "TC003",
            TypeError::MissingFunctionBody { .. } => "TC004",
            TypeError::FunctionBodyOverflow { .. } => "TC005",
            TypeError::WrongType { .. } => "TC006",
            TypeError::ExpectedInt { .. } => "TC007",
            TypeError::ExpectedBool { .. } => "TC008",
            TypeError::ExpectedNumeric { .. } => "TC009",
            TypeError::ExpectedIterable { .. } => "TC010",
            TypeError::ExpectedStruct { .. } => "TC011",
            TypeError::UnknownField { .. } => "TC012",
            TypeError::MissingField { .. } => "TC013",
            TypeError::ExpectedMutable { .. } => "TC014",
            TypeError::FunctionNotFound { .. } => "TC015",
            TypeError::UnsafeExternCall { .. } => "TC016",
            TypeError::UnexpectedArgumentCount { .. } => "TC017",
            TypeError::ArgumentTypeMismatch { .. } => "TC018",
            TypeError::MissingPartialEq { .. } => "TC019",
            TypeError::MissingPartialOrd { .. } => "TC020",
            TypeError::InconsistentComparison { .. } => "TC021",
            TypeError::ChainTypeMismatch { .. } => "TC022",
            TypeError::InSetTypeMismatch { .. } => "TC023",
            TypeError::NonConstantBound { .. } => "TC024",
            TypeError::Unsupported { .. } => "TC025",
            TypeError::UnionMembersNotDisjoint { .. } => "TC026",
            TypeError::GenericBoundMismatch { .. } => "TC027",
            TypeError::GenericArityMismatch { .. } => "TC028",
        }
    }

    /// 修复建议（SH-P2-6 L2 结构化诊断）；无可行建议时返回 `None`。
    pub fn help(&self) -> Option<&'static str> {
        match self {
            TypeError::UndefinedVariable { .. } => {
                Some("检查变量名拼写，或先 `let` 声明后再使用")
            }
            TypeError::UndefinedType { .. } => {
                Some("确认类型已定义或已 `import`；基础类型首字母大写（如 `i64`）")
            }
            TypeError::UndefinedFunction { .. } => {
                Some("确认函数名拼写，或先定义 / `import` 该函数")
            }
            TypeError::MissingFunctionBody { .. } => Some("为函数补充 `{ ... }` 函数体"),
            TypeError::FunctionBodyOverflow { .. } => {
                Some("函数体嵌套过深，建议拆分为更小的函数")
            }
            TypeError::WrongType { .. } => {
                Some("调整表达式类型以匹配期望，或显式转换（`as`）")
            }
            TypeError::ExpectedInt { .. } => Some("传入整数类型（如 `i64`）"),
            TypeError::ExpectedBool { .. } => Some("条件表达式需为 `bool`"),
            TypeError::ExpectedNumeric { .. } => Some("传入数值类型（如 `i64` / `f64`）"),
            TypeError::ExpectedIterable { .. } => {
                Some("`for` / `in` 右侧需为可迭代对象（数组 / `Vec` / `Range`）")
            }
            TypeError::ExpectedStruct { .. } => Some("传入结构体类型"),
            TypeError::UnknownField { .. } => {
                Some("检查字段名拼写；可用 `{struct}.` 查看可用字段")
            }
            TypeError::MissingField { .. } => Some("补全结构体构造中的缺失字段"),
            TypeError::ExpectedMutable { .. } => {
                Some("将绑定声明为 `let mut` 以支持可变借用 / 赋值")
            }
            TypeError::FunctionNotFound { .. } => Some("确认函数名拼写，或先定义该函数"),
            TypeError::UnsafeExternCall { .. } => Some("将调用包裹在 `unsafe { }` 块内"),
            TypeError::UnexpectedArgumentCount { .. } => Some("按函数签名调整实参数量"),
            TypeError::ArgumentTypeMismatch { .. } => Some("调整该实参类型以匹配形参"),
            TypeError::MissingPartialEq { .. } => {
                Some("为类型实现 `PartialEq` 以支持 `==` / 比较链")
            }
            TypeError::MissingPartialOrd { .. } => {
                Some("为类型实现 `PartialOrd` 以支持大小比较")
            }
            TypeError::InconsistentComparison { .. } => {
                Some("比较链方向需一致（全部 `<` 或全部 `>`）")
            }
            TypeError::ChainTypeMismatch { .. } => Some("比较链各元素类型需一致"),
            TypeError::InSetTypeMismatch { .. } => {
                Some("`in` 集合元素类型需与值类型一致")
            }
            TypeError::NonConstantBound { .. } => {
                Some("集合范围端点需为编译期整数常量（如 `0..<10`）")
            }
            TypeError::Unsupported { .. } => {
                Some("该语法在当前版本尚未支持，详见文档规划章节")
            }
            TypeError::UnionMembersNotDisjoint { .. } => {
                Some("调整联合成员，使其两两互不相交（如避免 `&T | &mut T`）")
            }
            TypeError::GenericBoundMismatch { .. } => {
                Some("为类型实现所需 trait，或放宽 `where` 约束")
            }
            TypeError::GenericArityMismatch { .. } => {
                Some("调整类型实参数量以匹配泛型参数")
            }
        }
    }

    /// 结构化诊断文本（SH-P2-6 L2）：在用户文件坐标下渲染
    /// `行:列: error[CODE]: 消息`，并附 `= help:` 修复建议。
    ///
    /// 复用既有 [`write_message`] 的正文格式化，仅把 `loc: error:` 前缀替换为
    /// `loc: error[CODE]:`；坐标还原逻辑与 [`TypeError::to_string_with_offset`] 一致。
    pub fn to_string_structured(&self, prelude_len: usize, prelude_lines: usize) -> String {
        let span = self.span();
        let loc = if span.start < prelude_len {
            format!("{}:{}", span.line, span.col)
        } else {
            format!(
                "{}:{}",
                span.line.saturating_sub(prelude_lines),
                span.col
            )
        };
        let full = {
            struct W<'a> {
                e: &'a TypeError,
                loc: String,
            }
            impl fmt::Display for W<'_> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write_message(f, &self.loc, self.e)
                }
            }
            W {
                e: self,
                loc: loc.clone(),
            }
            .to_string()
        };
        let prefix = format!("{loc}: error: ");
        let body = full.strip_prefix(&prefix).unwrap_or(&full);
        let mut out = format!("{loc}: error[{}]: {body}", self.code());
        if let Some(h) = self.help() {
            out.push_str(&format!("\n  = help: {h}"));
        }
        out
    }
}

impl std::error::Error for TypeError {}
