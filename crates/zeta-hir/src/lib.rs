//! # zeta-hir
//!
//! Zeta 语言高级中间表示（HIR）定义。
//!
//! 语法分析器（zeta-parser）产生 AST 后，由语义分析阶段
//! （zeta-typecheck）类型检查并展开为类型标注前的 HIR。
//!
//! ## 设计约定
//!
//! - HIR 是**结构化**中间表示：比较链、`in` 集合/裸范围等高层语法
//!   在此被展开为低级运算：
//!   - 比较链 `0 < x < 10` → `(0 < x) && (x < 10)`
//!   - 集合成员判断 `x in (0..<10)` → 离散成员 `==`/`!=` 链，
//!     大集合保留为 [`HirExpr::SetLookup`]
//!   - 裸范围区间判断 `x in 0..<10` → [`HirExpr::RangeCheck`]
//! - 节点不携带源码位置与类型标注（类型由 typecheck 侧返回）。

#![warn(missing_docs)]
#![warn(unsafe_code)]

/// 类型检查完成后的程序。
#[derive(Debug, Clone, PartialEq)]
pub struct HirProgram {
    /// 顶层项列表
    pub items: Vec<HirItem>,
}

/// 顶层项。
#[derive(Debug, Clone, PartialEq)]
pub struct HirItem {
    /// 项名称
    pub name: String,
    /// 项内容
    pub kind: HirItemKind,
}

/// 顶层项内容。
#[derive(Debug, Clone, PartialEq)]
pub enum HirItemKind {
    /// 函数声明
    Fn(HirFnDecl),
    /// const 声明
    Const(HirConstDecl),
}

/// 函数声明（类型检查后的函数体）。
#[derive(Debug, Clone, PartialEq)]
pub struct HirFnDecl {
    /// 参数列表（仅保留名称，类型在类型签名表）
    pub params: Vec<HirParam>,
    /// 函数体（抽象方法为 `None`）
    pub body: Option<HirBlock>,
}

/// 函数参数。
#[derive(Debug, Clone, PartialEq)]
pub struct HirParam {
    /// 参数名
    pub name: String,
}

/// const 声明。
#[derive(Debug, Clone, PartialEq)]
pub struct HirConstDecl {
    /// 初始值（展开后的表达式）
    pub value: HirExpr,
}

/// 代码块。
#[derive(Debug, Clone, PartialEq)]
pub struct HirBlock {
    /// 语句列表
    pub stmts: Vec<HirStmt>,
    /// 末尾表达式（块的值）
    pub final_expr: Option<HirExpr>,
}

/// 语句。
#[derive(Debug, Clone, PartialEq)]
pub enum HirStmt {
    /// `let [mut] name = expr;`
    Let {
        /// 绑定名
        name: String,
        /// 初始化表达式
        init: HirExpr,
        /// 是否可变
        mutable: bool,
    },
    /// 表达式语句（无分号）
    Expr(HirExpr),
    /// 带分号的表达式语句
    Semi(HirExpr),
}

/// 表达式（展开后的低层形式）。
#[derive(Debug, Clone, PartialEq)]
pub enum HirExpr {
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
    /// 变量引用
    Variable(String),
    /// 赋值表达式（`target = value`；MVP 目标仅限变量）
    Assign {
        /// 赋值目标（变量名）
        target: String,
        /// 赋值运算符
        op: HirAssignOp,
        /// 被赋的值
        value: Box<HirExpr>,
    },
    /// 二元运算
    Binary(HirBinaryOp, Box<HirExpr>, Box<HirExpr>),
    /// 一元运算
    Unary(HirUnaryOp, Box<HirExpr>),
    /// 集合成员查找（`x in (0..<10)` 展开结果，
    /// 元素较多时编译器生成查找表 / 二分，HIR 层面保留成员列表）
    SetLookup {
        /// 被判断的值
        value: Box<HirExpr>,
        /// 离散成员列表
        members: Vec<HirExpr>,
        /// 是否为取反（`not in`）
        negated: bool,
    },
    /// 区间判断（`x in 0..<10` 展开结果）
    RangeCheck {
        /// 被判断的值
        value: Box<HirExpr>,
        /// 下界（可为 `None`，表示无界）
        lower: Option<Box<HirExpr>>,
        /// 上界（可为 `None`，表示无界）
        upper: Option<Box<HirExpr>>,
        /// 下界是否包含（`>=` / `>`）
        lower_inclusive: bool,
        /// 上界是否包含（`<=` / `<`）
        upper_inclusive: bool,
        /// 是否为取反（`not in`）
        negated: bool,
    },
    /// if 表达式
    If {
        /// 条件
        cond: Box<HirExpr>,
        /// then 块
        then_block: Box<HirBlock>,
        /// else 块
        else_block: Option<Box<HirBlock>>,
    },
    /// 块表达式
    Block(Box<HirBlock>),
    /// 函数 / 宏调用
    Call {
        /// 被调用的名称
        callee: String,
        /// 实参
        args: Vec<HirExpr>,
    },
    /// while 循环（`while cond { body }`）
    While {
        /// 条件表达式
        cond: Box<HirExpr>,
        /// 循环体
        body: Box<HirBlock>,
    },
    /// loop 循环（`loop { body }`）
    Loop {
        /// 循环体
        body: Box<HirBlock>,
    },
    /// return 语句
    Return(Option<Box<HirExpr>>),
    /// break 语句
    Break(Option<Box<HirExpr>>),
    /// continue 语句
    Continue,
    /// 区域表达式（`region 'r { ... }`）
    Region {
        /// 区域名（匿名区域为 `None`）
        name: Option<String>,
        /// 区域选项
        options: HirRegionOptions,
        /// 区域体
        body: Box<HirBlock>,
    },
    /// 区域归属（`expr in 'r`）
    InRegion {
        /// 被归属的表达式
        expr: Box<HirExpr>,
        /// 区域名（不含 `'`）
        region: String,
    },
    /// 转移（`transfer expr out of 'r`）
    Transfer {
        /// 被转移的表达式
        expr: Box<HirExpr>,
        /// 源区域名（不含 `'`）
        region: String,
    },
    /// 单元值 `()`
    Unit,
}

/// 区域选项（编译期已知，由 AST `RegionOptions` 复制而来）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HirRegionOptions {
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
}

/// HIR 赋值运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirAssignOp {
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

/// HIR 二元运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirBinaryOp {
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
    And,
    /// `||`
    Or,
}

/// HIR 一元运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirUnaryOp {
    /// `-`
    Neg,
    /// `!`
    Not,
}
