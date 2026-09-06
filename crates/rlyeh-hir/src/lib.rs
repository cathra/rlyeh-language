//! # rlyeh-hir
//!
//! Rlyeh 语言高级中间表示（HIR）定义。
//!
//! 语法分析器（rlyeh-parser）产生 AST 后，由语义分析阶段
//! （rlyeh-typecheck）类型检查并展开为类型标注前的 HIR。
//!
//! ## 设计约定
//!
//! - HIR 是**结构化**中间表示：比较链、`in` 集合/裸范围等高层语法
//!   在此被展开为低级运算：
//!   - 比较链 `0 < x < 10` → `(0 < x) && (x < 10)`
//!   - 集合成员判断 `x in (0..<10)` → 离散成员 `==`/`!=` 链，
//!     大集合保留为 [`HirExpr::SetLookup`]
//!   - 裸范围区间判断 `x in 0..<10` → [`HirExpr::RangeCheck`]
//! - 节点携带源码位置（`HirExpr` / `HirStmt` / `HirBlock` 均含 `span` 字段，
//!   由 typecheck 在生成 HIR 时从 `AstExpr` / `AstStmt` 全量传播）；类型标注
//!   不在此层，由 typecheck 侧随表达式返回。

#![warn(missing_docs)]

use rlyeh_lexer::Span;

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
    /// 源码位置（合并源码中的字节偏移；来自 std 预置的项其偏移落在
    /// `prelude_len` 之前，可用于过滤 std 前缀——SH-P2-5 快照基线用）。
    pub span: Span,
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
    /// 函数体（抽象方法 / extern 声明为 `None`）
    pub body: Option<HirBlock>,
    /// 是否为 extern 外部函数声明（无函数体，符号由链接器解析）
    pub is_extern: bool,
    /// extern 函数签名（参数类型名 + 返回类型名，LIR 侧解析为 `LirType`）；
    /// 非 extern 为 `None`。
    pub extern_sig: Option<(Vec<String>, String)>,
}

/// 函数参数。
#[derive(Debug, Clone, PartialEq)]
pub struct HirParam {
    /// 参数名
    pub name: String,
    /// 源码位置
    pub span: Span,
    /// 是否为引用参数（`&T` / `&mut T` / `&self` / `&mut self`）。
    ///
    /// 借用检查据此区分「按引用参数」（指向调用方内存，返回 `&param.field`
    /// 合法）与「按值参数」（`self` 按值等，返回 `&param.field` 悬垂，
    /// 见 lang-defects #9）。由 typecheck 在 HIR 构建时按 AST 参数类型
    /// `AstType::Ref` 判定填充；合成参数（`actor`/`closure`/`thread`/extern）
    /// 一律记为 `true`（调用方/运行时所有，沿用「参数永不被标记悬垂」现状）。
    pub is_ref: bool,
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
    /// 源码位置
    pub span: Span,
}

/// 语句种类。
///
/// 注意：带源码位置的语句见 [`HirStmt`]（本枚举为其 `kind` 字段）。
#[derive(Debug, Clone, PartialEq)]
pub enum HirStmtKind {
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

/// 语句（携带源码位置）。
#[derive(Debug, Clone, PartialEq)]
pub struct HirStmt {
    /// 语句种类
    pub kind: HirStmtKind,
    /// 源码位置
    pub span: Span,
}

impl HirStmt {
    /// 构造带源码位置的语句节点。
    pub fn new(kind: HirStmtKind, span: Span) -> Self {
        HirStmt { kind, span }
    }
}

/// 表达式种类（展开后的低层形式）。
///
/// 注意：带源码位置的表达式见 [`HirExpr`]（本枚举为其 `kind` 字段）。
#[derive(Debug, Clone, PartialEq)]
pub enum HirExprKind {
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
    /// 引用表达式（`&x` / `&mut x`）：求值为被引用对象的指针
    /// （聚合对象 = 对象指针本身；标量 = 其存储槽地址）。
    Ref {
        /// 被引用的表达式（MVP 仅限变量）
        expr: Box<HirExpr>,
        /// 是否可变引用（`&mut`）
        is_mut: bool,
        /// 被引用值的标量种类（`Ptr` 表示聚合对象，取址即对象指针）
        pointee: FieldScalar,
    },
    /// 解引用表达式（`*p`，读取）：对标量引用为 load，对聚合引用为指针拷贝
    Deref {
        /// 被解引用的引用表达式
        expr: Box<HirExpr>,
        /// 被指向值的标量种类
        ty: FieldScalar,
    },
    /// 解引用赋值（`*p = v` / `*p += v`）
    DerefSet {
        /// 被解引用的引用表达式
        base: Box<HirExpr>,
        /// 待写入的值
        value: Box<HirExpr>,
        /// 被指向值的标量种类
        ty: FieldScalar,
    },
    /// 裸指针算术（`ptr + n`，V1 迭代器瘦指针推进）：
    /// 结果 = `base + n * elem_size`（元素步长由 `elem` 标量种类决定：
    /// U8→1 字节，其余→8 字节，与数组 IndexGet 步长规则一致）
    PtrAdd {
        /// 指针表达式（`*const T` / `*mut T`）
        base: Box<HirExpr>,
        /// 偏移量（i64）
        offset: Box<HirExpr>,
        /// 元素标量种类（决定步长）
        elem: FieldScalar,
    },
    /// 数值转换（U6 Cast IR）：`expr as target`。
    /// typecheck 仅对「数值→数值」转换产出本节点（其余保持擦除，
    /// 如指针 / 引用转换）；`to` 为目标类型名（`Type::name()` 输出，
    /// 如 `i8`/`u8`/`i32`/`i64`/`f32`/`f64`/`bool`/`char`）。
    Cast {
        /// 被转换的表达式
        expr: Box<HirExpr>,
        /// 目标类型名
        to: String,
    },
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
    /// `unsafe` 块表达式（SH-P0-1：受控手动内存管理作用域）
    UnsafeBlock(Box<HirBlock>),
    /// 函数 / 宏调用
    Call {
        /// 被调用的名称
        callee: String,
        /// 实参
        args: Vec<HirExpr>,
    },
    /// 函数地址值：引用具名函数（解析后的完整符号名），
    /// 用于 `let f = my_func;` 等函数一等值场景。
    FnPtr(String),
    /// 间接调用：通过函数指针值调用（`f(args)`，f 为函数值）。
    /// 签名以类型名字符串透传（与 extern 签名序列化同一约定，
    /// LIR 层用 `parse_extern_type` 解析为 `LirType`）。
    CallIndirect {
        /// 被调用的函数值表达式
        callee: Box<HirExpr>,
        /// 实参
        args: Vec<HirExpr>,
        /// 参数类型名（`type_to_extern_name` 渲染）
        param_names: Vec<String>,
        /// 返回类型名
        ret_name: String,
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
        /// 被归属对象的大小（字节，`type_slot_count(ty) * 8`；L3 接线用，
        /// 0 表示标量 / 无需接线）
        size: usize,
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
    /// 聚合对象分配：在堆上分配 `slots` 个 8 字节槽，返回对象指针。
    ///
    /// 由 typecheck 在展开枚举 / 结构体构造时生成。聚合类型的值
    /// 一律表示为指向该对象的指针（MVP 布局：每槽 8 字节）。
    Alloc {
        /// 槽数
        slots: usize,
        /// 标量聚合（≤2 槽、字段全标量的 enum/struct）按值分配：
        /// codegen 落到栈上 `[2 x i64]` 槽（免 calloc），返回/传参按值。
        by_value: bool,
        /// 是否为 `&str`（StrFat `{data,len}`）双槽值——仅 `as_str`/`as_str_range`
        /// 构造时置 `true`。供 LIR 精确推断 StrFat（区别于普通双槽 by_value 结构体，
        /// 避免 `Point{x,y}` 被误判），见 rlyeh-lir `infer_function_types`。
        is_strfat: bool,
    },
    /// 读取聚合对象槽位 `index`（槽 0 为枚举判别值 tag）。
    FieldGet {
        /// 对象指针表达式
        base: Box<HirExpr>,
        /// 槽位索引
        index: usize,
        /// 槽值标量种类
        ty: FieldScalar,
    },
    /// 写入聚合对象槽位 `index`（求值为单元值）。
    FieldSet {
        /// 对象指针表达式
        base: Box<HirExpr>,
        /// 槽位索引
        index: usize,
        /// 待写入的值
        value: Box<HirExpr>,
        /// 槽值标量种类
        ty: FieldScalar,
    },
    /// 索引读取：数组 `arr[i]` / 字符串 `s[i]`。
    ///
    /// `base` 求值为对象指针（数组槽区 / 字符串字符区），`index` 为运行时整数。
    Index {
        /// 被索引对象（数组 / 字符串）
        base: Box<HirExpr>,
        /// 索引表达式（运行时 i64）
        index: Box<HirExpr>,
        /// 元素标量种类
        elem: FieldScalar,
        /// `true` 表示字符串索引（字符步长 1 字节）；数组元素步长 8 字节
        is_str: bool,
    },
    /// 索引写入 `arr[i] = v`（求值为单元值）。
    IndexSet {
        /// 被索引对象（数组 / 字符串）
        base: Box<HirExpr>,
        /// 索引表达式（运行时 i64）
        index: Box<HirExpr>,
        /// 待写入的值
        value: Box<HirExpr>,
        /// 元素标量种类
        elem: FieldScalar,
        /// `true` 表示字符串索引（字符步长 1 字节）；数组元素步长 8 字节
        is_str: bool,
    },
}

/// 表达式（展开后的低层形式，携带源码位置）。
///
/// 由 rlyeh-typecheck 在类型检查阶段构造；`kind` 为展开后的低层运算，
/// `span` 为对应源 `AstExpr` 的源码位置（合成节点取最近源位置兜底）。
/// borrowck / regionck 据此给出精确到表达式的诊断坐标。
#[derive(Debug, Clone, PartialEq)]
pub struct HirExpr {
    /// 表达式种类
    pub kind: HirExprKind,
    /// 源码位置
    pub span: Span,
}

impl HirExpr {
    /// 构造带源码位置的表达式节点。
    pub fn new(kind: HirExprKind, span: Span) -> Self {
        HirExpr { kind, span }
    }
}

/// 聚合对象字段的标量存储种类（typecheck 展开时确定）。
///
/// MVP 布局约定：对象内存按 8 字节槽对齐，槽 0 为枚举判别值（`Int`），
/// 标量字段直接内联，聚合字段（枚举 / 结构体 / 元组 / 数组）存指针。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldScalar {
    /// 整数（i64）
    Int,
    /// 浮点（f64）
    Float,
    /// 布尔
    Bool,
    /// 字符
    Char,
    /// 字符串指针
    Str,
    /// &str 胖指针（data 指针 + 长度双槽；V2 子区间视图，对齐 Rust fat pointer）
    StrFat,
    /// 切片胖指针（data 指针 + 长度双槽；`&[T]` / `&mut [T]`，与 StrFat 同布局 `{i8*, i64}`）
    SliceFat,
    /// 聚合对象 / 引用指针
    Ptr,
    /// repr(C) 结构体字段：C 真布局下的内存字节偏移与标量类型（详见 `ReprConv`）。
    /// 读取时按 `field_ty` 从 `offset` 处 load 窄值再经 `conv` 提升为 Rlyeh 宽值；
    /// 写入时经逆转换（`trunc`/`fptrunc`）降为 `field_ty` 后落内存。
    ReprCField {
        /// C 布局字节偏移
        offset: u32,
        /// 字段在内存中的 LLVM 类型（如 `"i8"`/`"i16"`/`"i32"`/`"i32"`(char)/`"i1"`/`"i8*"`）
        field_ty: &'static str,
        /// 窄字段值 → Rlyeh 宽值的提升方式
        conv: ReprConv,
    },
    /// repr(C) 嵌套聚合子对象（嵌套结构体）：字段本身在内存中内联于父对象，
    /// `FieldGet` 返回指向 `base + offset` 的子指针（i8*，指向内联的子对象），
    /// 后续 `.inner` 访问复用内层结构体的 C 布局（offset 累加）。`FieldSet`
    /// 经 `llvm.memcpy` 把整个子对象（size 字节）拷入 `base + offset`，实现
    /// 嵌套聚合按 C 规则内联打包。
    ReprCSubPtr {
        /// C 布局字节偏移（父对象内）
        offset: u32,
        /// 子对象字节大小（memcpy 长度；仅 `FieldSet` 用）
        size: u32,
    },
}

/// repr(C) 字段窄化→宽值的提升方式（读取时应用到 load 结果）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReprConv {
    /// 无转换（i64 / f64 / bool(i1) / char(i32) / 指针(i8*) 与 Rlyeh 宽值同构）
    None,
    /// 无符号窄整数（u8/u16/u32）→ Rlyeh i64（`zext`）
    Zext,
    /// 有符号窄整数（i8/i16/i32）→ Rlyeh i64（`sext`）
    Sext,
    /// f32 → Rlyeh f64（`fpext`）
    Fpext,
}

/// 区域分配策略（`strategy (bump)`，MVP 仅 bump）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirRegionStrategy {
    /// 显式 bump 分配（默认策略，等价倍率扩容）
    Bump,
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
    /// 显式分配策略（`strategy (bump)`）
    pub strategy: Option<HirRegionStrategy>,
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
    /// `&`（位与）
    BitAnd,
    /// `|`（位或）
    BitOr,
    /// `^`（位异或）
    BitXor,
    /// `<<`（左移）
    Shl,
    /// `>>`（算术右移，有符号语义）
    Shr,
}

/// HIR 一元运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirUnaryOp {
    /// `-`
    Neg,
    /// `!`
    Not,
}
