//! Token 定义。

/// Token 类型
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `let`
    Let,
    /// `mut`
    Mut,
    /// `const`
    Const,
    /// `static`
    Static,
    /// `fn`
    Fn,
    /// `return`
    Return,
    /// `pub`
    Pub,
    /// `priv`
    Priv,
    /// `if`
    If,
    /// `else`
    Else,
    /// `match`
    Match,
    /// `for`
    For,
    /// `while`
    While,
    /// `loop`
    Loop,
    /// `break`
    Break,
    /// `continue`
    Continue,
    /// `true`
    True,
    /// `false`
    False,
    /// `and`
    And,
    /// `or`
    Or,
    /// `not`
    Not,
    /// `struct`
    Struct,
    /// `enum`
    Enum,
    /// `impl`（扩展块关键字：`impl T: P` / `impl T`）
    Impl,
    /// `protocol`（协议声明关键字；`protocol` / `extension` 已从语法中移除）
    Protocol,
    /// `type`
    Type,
    /// `where`
    Where,
    /// `Self`
    SelfKw,
    /// `region`
    Region,
    /// `gc_region`
    GcRegion,
    /// `in`
    In,
    /// `transfer`
    Transfer,
    /// `out`
    Out,
    /// `of`
    Of,
    /// `unsafe`
    Unsafe,
    /// `actor`
    Actor,
    /// `async`
    Async,
    /// `await`
    Await,
    /// `spawn`
    Spawn,
    /// `send`
    Send,
    /// `recv`
    Recv,
    /// `mod`
    Mod,
    /// `use`
    Use,
    /// `as`
    As,
    /// `extern`
    Extern,
    /// `dyn`
    Dyn,

    // 标识符和字面量
    /// 标识符
    Ident(String),
    /// 整数字面量
    IntLiteral(i128),
    /// 浮点字面量
    FloatLiteral(f64),
    /// 字符串字面量
    StringLiteral(String),
    /// 字符字面量
    CharLiteral(char),
    /// 布尔字面量
    BoolLiteral(bool),

    /// 生命周期/区域标签，如 `'r`
    Lifetime(String),

    /// 时间字面量（9am / 22:00）
    TimeLiteral {
        /// 小时（已按 12/24 小时制归一化）
        hour: u8,
        /// 分钟
        minute: u8,
        /// 原始写法是否为 pm
        is_pm: bool,
    },

    // 运算符
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`
    AndAnd,
    /// `||`
    OrOr,
    /// `!`
    NotNot,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `=`
    Assign,
    /// `+=`
    PlusEq,
    /// `-=`
    MinusEq,
    /// `*=`
    StarEq,
    /// `/=`
    SlashEq,
    /// `%=`
    PercentEq,
    /// `..`（旧范围语法，已废弃；仅用于错误提示）
    Range,
    /// `..<`（左闭右开区间 `[a, b)`）
    DotDotLt,
    /// `...`（闭区间 `[a, b]`）
    DotDotDot,
    /// `<..`（左开右闭区间 `(a, b]`）
    LtDotDot,
    /// `->`（函数返回）
    Arrow,
    /// `=>`（match 分支）
    FatArrow,

    // 分隔符
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `;`
    Semicolon,
    /// `.`
    Dot,

    // 特殊
    /// `@`（属性/宏）
    At,
    /// `$`（声明式宏元变量前缀，如 `$x:expr`）
    Dollar,
    /// `#`（attribute 前缀，如 `#[derive(Serialize)]`；`r#ident`/`r#"..."#` 由专用分支消费）
    Pound,
    /// `?`（声明式宏重复操作符 `?`；`?` 错误传播运算符仍规划中）
    Question,
    /// `not in` 组合
    NotIn,

    /// 文件结束
    Eof,
}

/// 带位置信息的 Token
#[derive(Debug, Clone, PartialEq)]
pub struct LocatedToken {
    /// 具体的 Token
    pub token: Token,
    /// 源码位置
    pub span: Span,
}

/// 源码位置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// 字节偏移（闭区间起点）
    pub start: usize,
    /// 字节偏移（开区间终点）
    pub end: usize,
    /// 行号（从 1 开始）
    pub line: usize,
    /// 列号（从 1 开始，按字符计）
    pub col: usize,
}

impl Span {
    /// 零值占位位置（测试 / 合成节点兜底用，不代表任何真实源码坐标）。
    pub fn dummy() -> Self {
        Span {
            start: 0,
            end: 0,
            line: 0,
            col: 0,
        }
    }
}

impl LocatedToken {
    /// 构造带位置的 Token
    pub fn new(token: Token, span: Span) -> Self {
        Self { token, span }
    }
}
