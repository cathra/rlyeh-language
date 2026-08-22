//! # zeta-mir
//!
//! Zeta 语言中级中间表示（MIR）：控制流图（CFG）形式。
//!
//! 由类型检查后的 HIR 降低而来（见 [`lower::lower_program`]），
//! 再经基础优化 passes（常量折叠 / 死代码消除 / 基础内联，见 [`passes`]）处理，
//! 最终作为代码生成（LIR / 后端）的输入。
//!
//! ## 设计约定
//!
//! - **CFG 结构**：函数 = 基本块列表（index 0 为入口），块 = 指令序列 + 终止符；
//! - **SSA 风格（简化）**：表达式求值引入 `_tN` 临时变量；
//!   条件分支值通过"各分支写入同一结果变量、合并块读取"模拟 φ；
//! - **高层语法已在此展开**：`SetLookup` / `RangeCheck` 降低为比较链；
//! - **区域操作显式化**：`RegionEnter` / `RegionExit` / `AllocInRegion` / `Transfer`
//!   作为指令出现，供区域检查后的代码生成使用。

#![warn(missing_docs)]
#![warn(unsafe_code)]

pub mod lower;
pub mod passes;

use zeta_hir::{HirBinaryOp, HirRegionOptions, HirUnaryOp};

/// 局部变量名（MVP：仅具名变量，无字段投影）。
pub type Local = String;

/// MIR 程序。
#[derive(Debug, Clone, PartialEq)]
pub struct MirProgram {
    /// 函数列表
    pub functions: Vec<MirFunction>,
}

/// MIR 函数（控制流图）。
#[derive(Debug, Clone, PartialEq)]
pub struct MirFunction {
    /// 函数名
    pub name: String,
    /// 参数名（入口块中可直接引用）
    pub params: Vec<Local>,
    /// 基本块列表（index 0 为入口）
    pub blocks: Vec<BasicBlock>,
    /// 是否为 extern 外部函数声明（无函数体，`blocks` 为空）
    pub is_extern: bool,
    /// extern 函数签名：参数类型名列表 + 返回类型名（由 typecheck 序列化，
    /// LIR 侧解析为 `LirType`）
    pub extern_sig: Option<(Vec<String>, String)>,
}

/// 基本块：指令序列 + 终止符。
#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    /// 指令序列
    pub stmts: Vec<MirStmt>,
    /// 终止符（构建期可为 `None`，pass 期应有值）
    pub terminator: Option<MirTerminator>,
}

/// 指令。
#[derive(Debug, Clone, PartialEq)]
pub enum MirStmt {
    /// `target = value`
    Assign {
        /// 目标变量
        target: Local,
        /// 右值
        value: MirValue,
    },
    /// `[target =] callee(args)`
    Call {
        /// 返回值变量（无返回值为 `None`）
        target: Option<Local>,
        /// 被调函数名
        callee: String,
        /// 实参（局部变量）
        args: Vec<Local>,
    },
    /// 区域进入（携带区域选项，供分配器使用）
    RegionEnter {
        /// 区域名（匿名区域为 `None`）
        name: Option<String>,
        /// 区域选项
        options: HirRegionOptions,
    },
    /// 区域退出
    RegionExit,
    /// `target = alloc_in_region(target, 'region)`：对象在区域中登记
    AllocInRegion {
        /// 目标变量
        target: Local,
        /// 区域名
        region: String,
    },
    /// `transfer place out of 'region`
    Transfer {
        /// 被转移的变量
        place: Local,
        /// 源区域名
        region: String,
    },
    /// `target = alloc(slots)`：堆上分配聚合对象（`slots` 个 8 字节槽），
    /// 返回对象指针。由 typecheck 展开枚举 / 结构体构造时生成。
    Alloc {
        /// 目标变量（对象指针）
        target: Local,
        /// 槽数
        slots: usize,
    },
    /// `target = field_get(base, index)`：读取聚合对象槽位
    /// （槽 0 为枚举判别值 tag）。
    FieldGet {
        /// 目标变量
        target: Local,
        /// 对象指针变量
        base: Local,
        /// 槽位索引
        index: usize,
        /// 槽值标量种类
        ty: zeta_hir::FieldScalar,
    },
    /// `field_set(base, index, value)`：写入聚合对象槽位。
    FieldSet {
        /// 对象指针变量
        base: Local,
        /// 槽位索引
        index: usize,
        /// 待写入的变量
        value: Local,
        /// 槽值标量种类
        ty: zeta_hir::FieldScalar,
    },
    /// `target = index_get(base, index)`：运行时索引读取
    /// （数组元素步长 8 字节；字符串字符步长 1 字节）。
    IndexGet {
        /// 目标变量
        target: Local,
        /// 对象指针变量（数组槽区 / 字符串字符区）
        base: Local,
        /// 索引变量（i64）
        index: Local,
        /// 元素标量种类
        ty: zeta_hir::FieldScalar,
        /// `true` 表示字符串索引
        is_str: bool,
    },
    /// `index_set(base, index, value)`：运行时索引写入。
    IndexSet {
        /// 对象指针变量（数组槽区 / 字符串字符区）
        base: Local,
        /// 索引变量（i64）
        index: Local,
        /// 待写入的变量
        value: Local,
        /// 元素标量种类
        ty: zeta_hir::FieldScalar,
        /// `true` 表示字符串索引
        is_str: bool,
    },
    /// `target = addr_of(operand)`：取引用（`&x` / `&mut x`）。
    /// 聚合对象（`pointee = Ptr`）的"地址"即其对象指针（拷贝值）；
    /// 标量（`pointee = Int` 等）为变量存储槽的地址。
    AddrOf {
        /// 目标变量（引用值，LIR 层统一为指针）
        target: Local,
        /// 被引用变量
        operand: Local,
        /// 被引用值的标量种类（`Ptr` 表示聚合对象）
        pointee: zeta_hir::FieldScalar,
    },
    /// `target = deref_read(base)`：解引用读取（`*p`）。
    /// 聚合（`ty = Ptr`）为指针拷贝；标量为 load。
    DerefRead {
        /// 目标变量
        target: Local,
        /// 引用变量
        base: Local,
        /// 被指向值的标量种类
        ty: zeta_hir::FieldScalar,
    },
    /// `deref_write(base, value)`：解引用写入（`*p = v`）。
    DerefWrite {
        /// 引用变量
        base: Local,
        /// 待写入的变量
        value: Local,
        /// 被指向值的标量种类
        ty: zeta_hir::FieldScalar,
    },
}

/// 右值（可内联进 `Assign` 的运算树）。
#[derive(Debug, Clone, PartialEq)]
pub enum MirValue {
    /// 整数字面量
    Int(i128),
    /// 浮点字面量
    Float(f64),
    /// 字符串字面量
    String(String),
    /// 字符字面量
    Char(char),
    /// 布尔字面量
    Bool(bool),
    /// 单元值 `()`
    Unit,
    /// 复制局部变量
    Place(Local),
    /// 二元运算
    Binary {
        /// 运算符
        op: HirBinaryOp,
        /// 左操作数
        lhs: Box<MirValue>,
        /// 右操作数
        rhs: Box<MirValue>,
    },
    /// 一元运算
    Unary {
        /// 运算符
        op: HirUnaryOp,
        /// 操作数
        operand: Box<MirValue>,
    },
}

/// 基本块终止符。
#[derive(Debug, Clone, PartialEq)]
pub enum MirTerminator {
    /// `return [value]`（`None` 表示 `return;` / 返回单元值）
    Return(Option<Local>),
    /// 无条件跳转
    Jump(usize),
    /// 条件跳转（`cond` 为真跳 `then`，否则跳 `otherwise`）
    CondJump {
        /// 条件变量
        cond: Local,
        /// 条件为真时跳转的目标块
        then: usize,
        /// 条件为假时跳转的目标块
        otherwise: usize,
    },
}
