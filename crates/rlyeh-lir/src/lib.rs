//! # rlyeh-lir
//!
//! Rlyeh 语言低级中间表示（LIR）：目标无关的三地址码。
//!
//! 由优化后的 MIR 降低而来（见 [`lower::lower_program`]），
//! 与 MIR 的核心区别：
//!
//! - **三地址码**：`Assign` 右值操作数限制为立即数或局部变量；
//!   嵌套二元 / 一元运算被拆平为带临时变量的独立指令；
//! - **类型标注**：每个局部变量携带标量类型（`LirType`，可直映射 LLVM），
//!   运算指令显式标注结果类型；MIR 的表达式树在 LIR 中失去递归结构；
//! - **区域标注**：`RegionEnter` / `RegionExit` / `AllocInRegion` / `Transfer`
//!   保留为标注指令，供后续后端（分配器 / GC）使用；LLVM 后端 MVP 忽略。
//!
//! LIR 是代码生成（`rlyeh-codegen`）的输入。

#![warn(missing_docs)]
#![warn(unsafe_code)]

pub mod error;
pub mod lower;

pub use rlyeh_hir::{FieldScalar, HirBinaryOp, HirUnaryOp};

use error::LirError;

/// 局部变量名（与 MIR 一致）。
pub type Local = String;

/// LIR 标量类型（可直接映射到 LLVM 标量类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LirType {
    /// 64 位有符号整数（MVP 默认整数宽度）
    I64,
    /// 64 位浮点（double）
    F64,
    /// 布尔
    Bool,
    /// 字符（i8）
    Char,
    /// 字符串（i8*）
    Str,
    /// &str 胖指针（data 指针 + 长度双槽；V2 子区间视图）
    StrFat,
    /// 切片胖指针（data 指针 + 长度双槽；`&[T]` / `&mut [T]`，与 StrFat 同布局 `{i8*, i64}`）
    SliceFat,
    /// 聚合对象指针（i8*；枚举 / 结构体等堆对象）
    Ptr,
    /// 单元类型（void）
    Unit,
}

impl LirType {
    /// 是否为数值类型（整数 / 浮点）。
    pub fn is_numeric(self) -> bool {
        matches!(self, LirType::I64 | LirType::F64)
    }

    /// 是否为整数类型。
    pub fn is_integer(self) -> bool {
        matches!(self, LirType::I64)
    }
}

impl std::fmt::Display for LirType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LirType::I64 => write!(f, "i64"),
            LirType::F64 => write!(f, "f64"),
            LirType::Bool => write!(f, "bool"),
            LirType::Char => write!(f, "char"),
            LirType::Str => write!(f, "string"),
            LirType::StrFat => write!(f, "strfat"),
            LirType::SliceFat => write!(f, "slicefat"),
            LirType::Ptr => write!(f, "ptr"),
            LirType::Unit => write!(f, "()"),
        }
    }
}

/// LIR 程序。
#[derive(Debug, Clone, PartialEq)]
pub struct LirProgram {
    /// 函数列表
    pub functions: Vec<LirFunction>,
}

/// LIR 函数（三地址码控制流图）。
#[derive(Debug, Clone, PartialEq)]
pub struct LirFunction {
    /// 函数名
    pub name: String,
    /// 参数（名称 + 类型）
    pub params: Vec<(Local, LirType)>,
    /// 返回类型（从 `Return` 操作数推断；`Unit` 表示 void）
    pub return_type: LirType,
    /// 全部局部变量类型表（含参数与临时变量）
    pub locals: Vec<(Local, LirType)>,
    /// 基本块列表（index 0 为入口）
    pub blocks: Vec<LirBlock>,
    /// 是否为 extern 声明（无函数体，codegen 生成 `declare` 而非 `define`）
    pub is_extern: bool,
    /// extern 声明返回类型是否为 32 位整数（`-> i32`，如 pthread 的
    /// `trylock`/`close` 等返回 `int` 的函数）。codegen 据此生成
    /// `declare i32` + 调用后 `sext` 存槽，规避 x86-64 上以 i64 声明
    /// 时 int 返回值高位未定义的问题。
    pub extern_ret32: bool,
}

/// LIR 基本块。
#[derive(Debug, Clone, PartialEq)]
pub struct LirBlock {
    /// 指令序列
    pub stmts: Vec<LirStmt>,
    /// 终止符
    pub terminator: LirTerminator,
}

/// LIR 指令（三地址码）。
#[derive(Debug, Clone, PartialEq)]
pub enum LirStmt {
    /// `target = value`（value 为立即数或变量复制）
    Assign {
        /// 目标变量
        target: Local,
        /// 右值
        value: LirOperand,
    },
    /// `target = lhs op rhs`（三地址二元运算）
    Binary {
        /// 目标变量
        target: Local,
        /// 运算符
        op: HirBinaryOp,
        /// 操作数类型（比较 / 逻辑的结果类型为 `bool`）
        ty: LirType,
        /// 左操作数
        lhs: LirOperand,
        /// 右操作数
        rhs: LirOperand,
    },
    /// `target = op operand`（一元运算）
    Unary {
        /// 目标变量
        target: Local,
        /// 运算符
        op: HirUnaryOp,
        /// 操作数类型
        ty: LirType,
        /// 操作数
        operand: LirOperand,
    },
    /// `[target =] callee(args)`
    Call {
        /// 返回值变量（无返回值为 `None`）
        target: Option<Local>,
        /// 被调函数名（含内建函数）
        callee: String,
        /// 实参（局部变量）
        args: Vec<Local>,
    },
    /// `[target =] *callee(args)`：通过函数指针（函数值）间接调用。
    CallIndirect {
        /// 返回值变量（无返回值为 `None`）
        target: Option<Local>,
        /// 函数指针变量（LIR 层统一为 `i8*` 槽）
        callee: Local,
        /// 实参（局部变量）
        args: Vec<Local>,
        /// 参数类型（按被调函数签名解析）
        param_tys: Vec<LirType>,
        /// 返回类型
        ret_ty: LirType,
    },
    /// 区域进入（L3 接线：`rlyeh_region_enter`）
    RegionEnter {
        /// 区域名（匿名区域为 `None`）
        name: Option<String>,
        /// 区域选项（初始大小 / 扩容 / 自适应 / 精确 / 策略）
        options: rlyeh_hir::HirRegionOptions,
    },
    /// 区域退出（L3 接线：`rlyeh_region_exit`）
    RegionExit {
        /// 区域名（与 `RegionEnter` 配对；匿名区域为 `None`）
        name: Option<String>,
    },
    /// 区域归属分配（L3 接线：`rlyeh_region_alloc` + 值镜像）
    AllocInRegion {
        /// 目标变量
        target: Local,
        /// 区域名
        region: String,
        /// 对象字节大小（`type_slot_count × 8`，0 = 标量 / 无需接线）
        size: usize,
    },
    /// 区域归属分配（**字面量直接构造**）：对象在区域指针上逐字段构造，
    /// 无中间堆临时、无值镜像 memcpy。
    ///
    /// 由 codegen 的 `inline_region_literal` 变换产生：把
    /// `Alloc(t)` + `FieldSet(t,..)*` + `AllocInRegion(t)` 的字面量构造
    /// 重写为 `AllocInRegionDirect(t)` + `FieldSet(t,..)*`——字段直接写入
    /// 区域内存，`t` 槽即区域指针。
    AllocInRegionDirect {
        /// 目标变量（对象指针，指向区域内内存）
        target: Local,
        /// 区域名
        region: String,
        /// 对象字节大小（`type_slot_count × 8`）
        size: usize,
    },
    /// 所有权转移（后端可忽略）
    Transfer {
        /// 被转移的变量
        place: Local,
        /// 源区域名
        region: String,
    },
    /// `target = alloc(slots)`：堆上分配聚合对象
    /// （`slots` 个 8 字节槽；槽 0 为枚举判别值 tag）。
    Alloc {
        /// 目标变量（对象指针）
        target: Local,
        /// 槽数
        slots: usize,
        /// 标量聚合（≤2 槽、字段全标量的 enum/struct）按值分配：
        /// codegen 落到栈上 `[2 x i64]` 槽（免 calloc），返回/传参按值。
        by_value: bool,
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
        ty: FieldScalar,
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
        ty: FieldScalar,
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
        ty: FieldScalar,
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
        ty: FieldScalar,
        /// `true` 表示字符串索引
        is_str: bool,
    },
    /// `target = addr_of(operand)`：取引用（`&x` / `&mut x`）。
    /// 引用值统一为指针（`Ptr` 类型）：聚合对象取其对象指针（拷贝），
    /// 标量取变量存储槽地址。
    AddrOf {
        /// 目标变量（引用值）
        target: Local,
        /// 被引用变量
        operand: Local,
        /// 被引用值的标量种类（`Ptr` 表示聚合对象）
        pointee: FieldScalar,
    },
    /// `target = deref_read(base)`：解引用读取（`*p`）。
    /// 聚合（`ty = Ptr`）为指针拷贝；标量为 load。
    DerefRead {
        /// 目标变量
        target: Local,
        /// 引用变量
        base: Local,
        /// 被指向值的标量种类
        ty: FieldScalar,
    },
    /// `deref_write(base, value)`：解引用写入（`*p = v`）。
    DerefWrite {
        /// 引用变量
        base: Local,
        /// 待写入的变量
        value: Local,
        /// 被指向值的标量种类
        ty: FieldScalar,
    },
    /// `target = &base.field`（V1）：取聚合对象字段槽地址（GEP 到字段槽，
    /// 结果存 target 指针槽）——真实字段地址，写回经 DerefWrite 生效。
    FieldAddr {
        /// 目标变量（指针值）
        target: Local,
        /// 对象指针变量
        base: Local,
        /// 槽位索引
        index: usize,
        /// 字段标量种类
        ty: FieldScalar,
    },
    /// `target = base + offset * elem_size`（V1）：裸指针算术
    /// （元素步长：is_str→1 字节，其余→8 字节，与 IndexGet 步长规则一致）。
    PtrAdd {
        /// 目标变量（指针值）
        target: Local,
        /// 指针变量
        base: Local,
        /// 偏移变量（i64）
        offset: Local,
        /// 元素标量种类
        elem: FieldScalar,
        /// 是否为字符串字符区（步长 1 字节）
        is_str: bool,
    },
    /// `target = cast(value as to)`（U6 Cast IR）：
    /// 数值→数值类型转换，codegen 按源操作数 LIR 存储类型
    /// 与目标语义位宽发射 `fptosi`/`sitofp`/`trunc`/`sext`/`zext`。
    Cast {
        /// 目标变量（转换结果）
        target: Local,
        /// 源值操作数
        value: LirOperand,
        /// 目标类型名（`Type::name()` 输出）
        to: String,
    },
}

/// LIR 操作数：立即数或局部变量引用。
#[derive(Debug, Clone, PartialEq)]
pub enum LirOperand {
    /// 整数字面量（i128 保留，LLVM 生成时按 `i64` 截断前已校验范围）
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
    /// 局部变量引用
    Local(Local),
    /// 函数地址（具名函数符号）
    FnPtr(String),
}

impl LirOperand {
    /// 是否为局部变量引用。
    pub fn as_local(&self) -> Option<&Local> {
        match self {
            LirOperand::Local(l) => Some(l),
            _ => None,
        }
    }

    /// 立即数操作数的类型（`Local` 返回 `None`）。
    pub fn literal_type(&self) -> Option<LirType> {
        match self {
            LirOperand::Int(_) => Some(LirType::I64),
            LirOperand::Float(_) => Some(LirType::F64),
            LirOperand::String(_) => Some(LirType::Str),
            LirOperand::Char(_) => Some(LirType::Char),
            LirOperand::Bool(_) => Some(LirType::Bool),
            LirOperand::Unit => Some(LirType::Unit),
            LirOperand::Local(_) | LirOperand::FnPtr(_) => None,
        }
    }

    /// 结合外部类型表解析操作数类型（变量查表）。
    pub fn resolve_type(&self, locals: &[(Local, LirType)]) -> Option<LirType> {
        self.literal_type().or_else(|| match self {
            LirOperand::Local(l) => locals.iter().find(|(name, _)| name == l).map(|(_, ty)| *ty),
            _ => None,
        })
    }
}

/// LIR 基本块终止符。
#[derive(Debug, Clone, PartialEq)]
pub enum LirTerminator {
    /// `return [value]`（`None` 表示返回单元值）
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

/// 便捷辅助：LIR 程序的验证性检查（供测试 / 后端调试使用）。
impl LirProgram {
    /// 查找指定名称的函数。
    pub fn find_function(&self, name: &str) -> Option<&LirFunction> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// 校验程序结构（跳转目标块索引合法）。
    pub fn validate(&self) -> Result<(), LirError> {
        for f in &self.functions {
            for (i, block) in f.blocks.iter().enumerate() {
                let targets = match &block.terminator {
                    LirTerminator::Jump(t) => vec![*t],
                    LirTerminator::CondJump {
                        then, otherwise, ..
                    } => vec![*then, *otherwise],
                    LirTerminator::Return(_) => Vec::new(),
                };
                for t in targets {
                    if t >= f.blocks.len() {
                        return Err(LirError::MissingTerminator {
                            function: f.name.clone(),
                            block: i,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
