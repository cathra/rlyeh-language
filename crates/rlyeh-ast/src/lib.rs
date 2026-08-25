//! # rlyeh-ast
//!
//! Rlyeh 语言抽象语法树（AST）定义。
//!
//! 语法分析器（rlyeh-parser）将 Token 流转换为本 crate 定义的 AST，
//! 供后续 HIR 生成、类型检查等阶段使用。
//!
//! ## 设计约定
//!
//! - 所有携带位置的节点均带有 [`Span`]（来自 rlyeh-lexer）。
//! - 表达式统一包装为 [`AstExpr`]（`kind` + `span`）。
//! - 比较链、`in` 集合判断、`region`、`transfer` 等 Rlyeh
//!   核心语法均以显式节点表达，语义检查（如比较链方向一致性）
//!   在后续阶段完成。

#![warn(missing_docs)]
#![warn(unsafe_code)]

use rlyeh_lexer::Span;

/// 完整程序（一个编译单元）。
#[derive(Debug, Clone, PartialEq)]
pub struct AstProgram {
    /// 顶层项列表
    pub items: Vec<AstItem>,
}

/// 顶层项。
#[derive(Debug, Clone, PartialEq)]
pub enum AstItem {
    /// 函数声明
    FnDecl(Box<AstFnDecl>),
    /// 结构体声明
    StructDecl(Box<AstStructDecl>),
    /// 枚举声明
    EnumDecl(Box<AstEnumDecl>),
    /// Trait 声明
    TraitDecl(Box<AstTraitDecl>),
    /// impl 块
    ImplBlock(Box<AstImplBlock>),
    /// 模块声明
    ModDecl(Box<AstModDecl>),
    /// use 导入
    UseDecl(Box<AstUseDecl>),
    /// const/static 声明
    ConstDecl(Box<AstConstDecl>),
    /// Actor 声明
    ActorDecl(Box<AstActorDecl>),
    /// 宏声明
    MacroDecl(Box<AstMacroDecl>),
    /// 顶层语句
    Statement(Box<AstStmt>),
}

/// 泛型参数（U3：携带 trait bound，`T: Bound1 + Bound2`）。
#[derive(Debug, Clone, PartialEq)]
pub struct AstTypeParam {
    /// 参数名
    pub name: String,
    /// 约束 trait 名列表（`T: A + B`；MVP 支持简单 trait 路径 ident）
    pub bounds: Vec<String>,
}

/// 函数声明。
#[derive(Debug, Clone, PartialEq)]
pub struct AstFnDecl {
    /// 函数名
    pub name: String,
    /// 泛型参数列表
    pub generics: Vec<AstTypeParam>,
    /// 参数列表
    pub params: Vec<AstParam>,
    /// 返回类型（缺省为 `()`）
    pub return_type: Option<AstType>,
    /// 函数体（trait 抽象方法为 `None`）
    pub body: Option<AstBlock>,
    /// 是否为 `pub`
    pub is_pub: bool,
    /// 是否为 `async`
    pub is_async: bool,
    /// 是否为 `extern`（外部函数声明，无函数体）
    pub is_extern: bool,
    /// 源码位置
    pub span: Span,
}

/// 函数参数。
#[derive(Debug, Clone, PartialEq)]
pub struct AstParam {
    /// 参数名
    pub name: String,
    /// 参数类型
    pub type_: AstType,
    /// 默认值（默认参数）
    pub default: Option<AstExpr>,
    /// 是否为 `mut`
    pub is_mut: bool,
    /// 源码位置
    pub span: Span,
}

/// Actor 声明。
#[derive(Debug, Clone, PartialEq)]
pub struct AstActorDecl {
    /// Actor 名
    pub name: String,
    /// 字段列表（带默认值）
    pub fields: Vec<AstActorField>,
    /// 方法列表（pub 或私有）
    pub methods: Vec<AstFnDecl>,
    /// 源码位置
    pub span: Span,
}

/// Actor 字段。
#[derive(Debug, Clone, PartialEq)]
pub struct AstActorField {
    /// 字段名
    pub name: String,
    /// 字段类型
    pub type_: AstType,
    /// 默认值
    pub default: Option<AstExpr>,
    /// 是否为 `pub`
    pub is_pub: bool,
    /// 源码位置
    pub span: Span,
}

/// 结构体声明。
#[derive(Debug, Clone, PartialEq)]
pub struct AstStructDecl {
    /// 结构体名
    pub name: String,
    /// 泛型参数名列表
    /// 泛型参数列表
    pub generics: Vec<AstTypeParam>,
    /// 命名字段
    pub fields: Vec<AstStructField>,
    /// 派生 trait 名列表（`#[derive(Serialize, Deserialize)]`，阶段 Q1b）
    pub derive: Vec<String>,
    /// 源码位置
    pub span: Span,
}

/// 结构体字段。
#[derive(Debug, Clone, PartialEq)]
pub struct AstStructField {
    /// 字段名
    pub name: String,
    /// 字段类型
    pub type_: AstType,
    /// 是否为 `pub`
    pub is_pub: bool,
    /// 源码位置
    pub span: Span,
}

/// 枚举声明。
#[derive(Debug, Clone, PartialEq)]
pub struct AstEnumDecl {
    /// 枚举名
    pub name: String,
    /// 泛型参数名列表
    /// 泛型参数列表
    pub generics: Vec<AstTypeParam>,
    /// 变体列表
    pub variants: Vec<AstEnumVariant>,
    /// 源码位置
    pub span: Span,
}

/// 枚举变体。
#[derive(Debug, Clone, PartialEq)]
pub struct AstEnumVariant {
    /// 变体名
    pub name: String,
    /// 元组负载字段（`Variant(u32, String)`）
    pub tuple_fields: Vec<AstType>,
    /// 命名负载字段（`Variant { x: u32 }`）
    pub struct_fields: Vec<AstStructField>,
    /// 源码位置
    pub span: Span,
}

/// Trait 声明。
#[derive(Debug, Clone, PartialEq)]
pub struct AstTraitDecl {
    /// Trait 名
    pub name: String,
    /// 泛型参数名列表
    /// 泛型参数列表
    pub generics: Vec<AstTypeParam>,
    /// 关联类型声明名列表（`type Item;`）
    pub types: Vec<String>,
    /// 抽象方法列表
    pub methods: Vec<AstFnDecl>,
    /// 源码位置
    pub span: Span,
}

/// impl 块。
#[derive(Debug, Clone, PartialEq)]
pub struct AstImplBlock {
    /// 实现的 Trait 名（`impl Trait for Type` 时为 `Some`）
    pub trait_name: Option<String>,
    /// 被实现的类型名
    pub type_name: String,
    /// 泛型参数名列表
    /// 泛型参数列表
    pub generics: Vec<AstTypeParam>,
    /// 关联类型定义列表（`type Item = Concrete;`）
    pub types: Vec<(String, AstType)>,
    /// 方法列表
    pub methods: Vec<AstFnDecl>,
    /// 源码位置
    pub span: Span,
}

/// 模块声明。
#[derive(Debug, Clone, PartialEq)]
pub struct AstModDecl {
    /// 模块名
    pub name: String,
    /// 模块内项（`mod foo;` 外部文件形式为空）
    pub items: Vec<AstItem>,
    /// 是否为外部文件形式（`module foo;` → 内容在 `foo.rl` 或 `foo/module.rl`）
    pub external: bool,
    /// 源码位置
    pub span: Span,
}

/// use 导入。
#[derive(Debug, Clone, PartialEq)]
pub struct AstUseDecl {
    /// 导入路径（`use a::b::c;` → `["a", "b", "c"]`）
    pub path: Vec<String>,
    /// 重命名别名（`as alias`）
    pub alias: Option<String>,
    /// 源码位置
    pub span: Span,
}

/// const/static 声明。
#[derive(Debug, Clone, PartialEq)]
pub struct AstConstDecl {
    /// 名称
    pub name: String,
    /// 类型标注
    pub type_: Option<AstType>,
    /// 初始值表达式
    pub value: AstExpr,
    /// 是否为 `static`
    pub is_static: bool,
    /// 源码位置
    pub span: Span,
}

/// 宏声明。
#[derive(Debug, Clone, PartialEq)]
pub struct AstMacroDecl {
    /// 宏名
    pub name: String,
    /// 形参名列表
    pub params: Vec<String>,
    /// 宏体原始源码文本（`{ ... }` 内的内容）
    pub body: String,
    /// 源码位置
    pub span: Span,
}

/// 表达式（携带源码位置）。
#[derive(Debug, Clone, PartialEq)]
pub struct AstExpr {
    /// 表达式种类
    pub kind: Box<ExprKind>,
    /// 源码位置
    pub span: Span,
}

impl AstExpr {
    /// 构造一个表达式节点
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self {
            kind: Box::new(kind),
            span,
        }
    }
}

/// 表达式种类。
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
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
    /// 时间字面量（`9am` / `22:00`）
    TimeLiteral {
        /// 小时（已按 12/24 小时制归一化）
        hour: u8,
        /// 分钟
        minute: u8,
        /// 原始写法是否为 pm
        is_pm: bool,
    },

    /// 标识符
    Ident(String),
    /// 路径表达式（`a::b::c`）
    Path(Vec<String>),
    /// 集合字面量 `(a, b, c)`，元素可为范围或单值
    Set(Vec<AstExpr>),
    /// 范围表达式 `a..<b` / `a...b` / `a<..b`
    Range {
        /// 下界
        lower: AstExpr,
        /// 上界
        upper: AstExpr,
        /// 下界是否包含（`a..` / `a...` 含，`a<..` 不含）
        lower_inclusive: bool,
        /// 上界是否包含（`a...` 含，`a..<` 不含）
        upper_inclusive: bool,
    },

    /// 二元运算
    Binary {
        /// 运算符
        op: BinaryOp,
        /// 左操作数
        left: AstExpr,
        /// 右操作数
        right: AstExpr,
    },
    /// 一元运算
    Unary {
        /// 运算符
        op: UnaryOp,
        /// 操作数
        operand: AstExpr,
    },

    /// 比较链（`0 < x < 10`）
    ComparisonChain {
        /// 链上所有操作数（长度为 operators.len() + 1）
        elements: Vec<AstExpr>,
        /// 链上所有比较运算符
        operators: Vec<CompareOp>,
    },

    /// 集合判断（`x in (1, 3, 5)` / `x in (0..<10)` / `x not in (...)`)
    ///
    /// `in` 右侧为括号集合时是**成员判断**：集合内元素可为单值或范围，
    /// 范围元素语义为展开为离散成员（`x in (0..<10)` ≡
    /// `x == 0 || x == 1 || ... || x == 9`，展开要求上下界为编译期整数常量）。
    InSet {
        /// 被判断的值
        value: AstExpr,
        /// 集合元素（单值或范围表达式）
        set: Vec<AstExpr>,
        /// 是否为取反（`not in`）
        negated: bool,
    },

    /// 范围判断（`x in 0..<10` / `x not in 0...10`，`in` 右侧为裸范围）
    ///
    /// 与集合判断不同：`in` 右侧为裸范围时是**区间判断**
    /// （`x in 0..<10` ≡ `x >= 0 && x < 10`）。
    InRange {
        /// 被判断的值
        value: AstExpr,
        /// 范围表达式（`ExprKind::Range`）
        range: AstExpr,
        /// 是否为取反（`not in`）
        negated: bool,
    },

    /// 区域归属（`expr in 'r`）
    InRegion {
        /// 表达式
        expr: AstExpr,
        /// 区域名（不含 `'`）
        region: String,
    },

    /// 赋值
    Assign {
        /// 赋值目标
        target: AstExpr,
        /// 赋值运算符
        op: AssignOp,
        /// 赋值值
        value: AstExpr,
    },

    /// if 表达式
    If {
        /// 条件
        cond: AstExpr,
        /// then 块
        then_block: AstBlock,
        /// else 块
        else_block: Option<AstBlock>,
    },

    /// match 表达式
    Match {
        /// 被匹配的表达式
        expr: AstExpr,
        /// 匹配臂
        arms: Vec<MatchArm>,
    },

    /// for 循环
    For {
        /// 循环变量模式
        pattern: AstPattern,
        /// 迭代器表达式
        iterator: AstExpr,
        /// 循环体
        body: AstBlock,
    },

    /// while 循环
    While {
        /// 条件
        cond: AstExpr,
        /// 循环体
        body: AstBlock,
    },

    /// loop 循环
    Loop {
        /// 循环体
        body: AstBlock,
    },

    /// region 表达式
    Region {
        /// 区域名（可选，缺省匿名区域）
        name: Option<String>,
        /// 区域选项
        options: RegionOptions,
        /// 区域体
        body: AstBlock,
    },

    /// gc_region 表达式（`gc_region { ... }`，K4 追踪 GC 生命周期作用域）
    GcRegion {
        /// 块体（结束触发 GC 周期）
        body: AstBlock,
    },

    /// transfer 表达式（`transfer data out of 'r`）
    Transfer {
        /// 被转移的表达式
        expr: AstExpr,
        /// 目标区域名（不含 `'`）
        region: String,
    },

    /// 函数调用
    Call {
        /// 被调用的表达式（标识符或路径）
        callee: AstExpr,
        /// 实参
        args: Vec<AstExpr>,
        /// 泛型类型实参（turbofish `::<T1, T2>`，L2：`json.parse::<T>(s)`）
        type_args: Vec<AstType>,
    },

    /// 宏调用（`println!(...)` / `format!(...)` 等内置格式化宏）。
    ///
    /// 声明式宏（`macro_rules!`）在 parse 阶段展开为普通 AST，不产生本节点；
    /// 内置格式化宏由 typecheck 层 desugar（I2 格式化引擎）。
    MacroCall {
        /// 宏名（含 `!`，如 `"println!"`）
        name: String,
        /// 实参（表达式列表）
        args: Vec<AstExpr>,
    },

    /// 方法调用
    MethodCall {
        /// 接收者
        receiver: AstExpr,
        /// 方法名
        method: String,
        /// 实参
        args: Vec<AstExpr>,
    },

    /// 字段访问
    FieldAccess {
        /// 被访问的表达式
        expr: AstExpr,
        /// 字段名
        field: String,
    },

    /// 结构体字面量构造（`Point { x: 3, y: 4 }`）
    StructCtor {
        /// 结构体路径（`a::b::Point`）
        type_name: Vec<String>,
        /// 命名字段初始化列表
        fields: Vec<(String, AstExpr)>,
    },

    /// 索引访问
    Index {
        /// 被索引的表达式
        expr: AstExpr,
        /// 索引表达式
        index: AstExpr,
    },

    /// 数组字面量 `[a, b, c]`（元素类型统一，MVP 元素为标量或聚合对象指针）
    ArrayLit(Vec<AstExpr>),

    /// 闭包
    Closure {
        /// 形参模式
        params: Vec<AstPattern>,
        /// 形参类型注解（与 `params` 平行；`None` = 无注解）。
        /// 支持 `|x: i64| ..` 语法：全注解时可在无 fn 上下文处创建闭包值对象。
        param_types: Vec<Option<AstType>>,
        /// 闭包体
        body: AstExpr,
        /// 捕获模式
        capture: CaptureMode,
    },

    /// 类型转换（`expr as Type`）
    Cast {
        /// 被转换的表达式
        expr: AstExpr,
        /// 目标类型
        target_type: AstType,
    },

    /// await 表达式（`expr.await`）
    Await(Box<AstExpr>),

    /// 块表达式
    Block(AstBlock),

    /// return 语句
    Return(Option<AstExpr>),

    /// `?` 错误传播运算符（K1）：`expr?`，在 Option/Result 上下文中解包，
    /// 失败时从当前函数早返回失败值。typecheck 层 desugar 为
    /// `match expr { Some(v) => v, None => return None }`（Result 类似）。
    Question(Box<AstExpr>),

    /// break 语句
    Break(Option<AstExpr>),

    /// continue 语句
    Continue,

    /// Actor 消息发送（`send actor.method(args)`）
    Send {
        /// 目标 Actor 表达式
        actor: AstExpr,
        /// 方法名
        method: String,
        /// 实参
        args: Vec<AstExpr>,
    },
}

/// 比较运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `==`
    Eq,
    /// `!=`
    Ne,
}

/// 二元运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Mod,
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
    /// `&&`（逻辑与；`and` 关键字同义）
    And,
    /// `||`（逻辑或；`or` 关键字同义）
    Or,
}

/// 一元运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-`
    Neg,
    /// `!` / `not`
    Not,
    /// `*`（解引用）
    Deref,
    /// `&`
    AddrOf,
    /// `&mut`
    AddrOfMut,
}

/// 赋值运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    /// `=`
    Assign,
    /// `+=`
    AddAssign,
    /// `-=`
    SubAssign,
    /// `*=`
    MulAssign,
    /// `/=`
    DivAssign,
}

/// 闭包捕获模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// `move || { ... }`
    Move,
    /// `|| { ... }`（默认借用）
    Borrow,
}

/// 区域分配策略（`strategy (bump)`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionStrategy {
    /// 显式 bump 分配（默认策略，等价倍率扩容）
    Bump,
}

/// 区域选项（`region 'r with_size(...) allow_growth(...) exact adaptive strategy(...)`）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionOptions {
    /// 固定初始大小（`with_size(N)`）
    pub size: Option<usize>,
    /// 是否允许扩容
    pub allow_growth: bool,
    /// 扩容因子（`allow_growth(growth_factor=f)`）
    pub growth_factor: Option<f64>,
    /// 自适应分配（`adaptive`）
    pub adaptive: bool,
    /// 精确大小模式（`exact`）
    pub exact: bool,
    /// 显式分配策略（`strategy (bump)`）
    pub strategy: Option<RegionStrategy>,
}

impl Default for RegionOptions {
    /// 默认区域选项：允许扩容、无固定大小、非自适应、非精确、无显式策略
    fn default() -> Self {
        Self {
            size: None,
            allow_growth: true,
            growth_factor: None,
            adaptive: false,
            exact: false,
            strategy: None,
        }
    }
}

/// 代码块。
#[derive(Debug, Clone, PartialEq)]
pub struct AstBlock {
    /// 语句列表
    pub stmts: Vec<AstStmt>,
    /// 末尾表达式（块的值）
    pub final_expr: Option<AstExpr>,
    /// 源码位置
    pub span: Span,
}

/// 语句。
#[derive(Debug, Clone, PartialEq)]
pub enum AstStmt {
    /// `let [mut] pattern [: Type] = expr [in 'r];`
    Let {
        /// 绑定模式
        pattern: AstPattern,
        /// 类型标注
        type_anno: Option<AstType>,
        /// 初始化表达式
        init: AstExpr,
        /// 是否可变
        mutable: bool,
    },
    /// 表达式语句（无分号）
    Expr(AstExpr),
    /// 嵌套项声明
    Item(AstItem),
    /// 带分号的表达式语句
    Semi(AstExpr),
}

/// 模式。
#[derive(Debug, Clone, PartialEq)]
pub enum AstPattern {
    /// 标识符绑定
    Ident(String),
    /// 通配符 `_`
    Wildcard,
    /// 字面量模式
    Literal(LiteralValue),
    /// 元组模式 `(a, b, c)`
    Tuple(Vec<AstPattern>),
    /// 结构体模式 `Point { x, y }`
    Struct(String, Vec<(String, AstPattern)>),
    /// 枚举模式 `Some(x)`
    Enum(String, Vec<AstPattern>),
    /// 带模块路径的枚举模式 `mod::Enum::Variant(x)`（最后一段为变体名）
    EnumPath(Vec<String>, Vec<AstPattern>),
    /// 范围模式 `0..<10` / `0...10` / `0<..10`
    Range {
        /// 下界
        lower: AstExpr,
        /// 上界
        upper: AstExpr,
        /// 下界是否包含（`0..` / `0...` 含，`0<..` 不含）
        lower_inclusive: bool,
        /// 上界是否包含（`0...` 含，`0..<` 不含）
        upper_inclusive: bool,
    },
    /// 引用模式 `ref pat` / `ref mut pat`
    Ref(Box<AstPattern>, bool),
}

/// 字面量值（用于模式匹配）。
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    /// 整数字面量
    Int(i128),
    /// 浮点字面量
    Float(f64),
    /// 字符串字面量
    Str(String),
    /// 字符字面量
    Char(char),
    /// 布尔字面量
    Bool(bool),
    /// 时间字面量
    Time {
        /// 小时
        hour: u8,
        /// 分钟
        minute: u8,
        /// 是否为 pm
        is_pm: bool,
    },
}

/// match 匹配臂。
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// 匹配模式
    pub pattern: AstPattern,
    /// 守卫条件（`pattern if guard => body`）
    pub guard: Option<AstExpr>,
    /// 臂体表达式
    pub body: AstExpr,
    /// 源码位置
    pub span: Span,
}

/// 类型。
#[derive(Debug, Clone, PartialEq)]
pub enum AstType {
    /// 路径类型（`u32` / `Result<Response, Error>`）
    Path(String, Vec<AstType>),
    /// 引用类型（`&T` / `&mut T`）
    Ref(Box<AstType>, bool),
    /// 裸指针类型（`*const T` / `*mut T`）
    RawPtr(Box<AstType>, bool),
    /// trait 对象类型（`dyn Trait`：数据指针 + vtable 胖指针）
    Dyn(String),
    /// 元组类型 `(A, B)`
    Tuple(Vec<AstType>),
    /// 数组类型 `[T; N]`
    Array(Box<AstType>, Option<Box<AstExpr>>),
    /// 函数类型 `fn(A) -> B`
    Fn(Vec<AstType>, Box<AstType>),
    /// 推断类型 `_`
    Infer,
}
