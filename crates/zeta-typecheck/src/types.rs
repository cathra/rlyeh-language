//! Zeta 类型系统定义。

use std::fmt;

/// 类型可变性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    /// `&T`
    Immutable,
    /// `&mut T`
    Mutable,
}

/// Zeta 类型。
///
/// 语义分析阶段使用的类型表示。字面量默认推断为 [`Type::I64`]
/// （整数）/ [`Type::F64`]（浮点）；时间字面量归一化为分钟值后
/// 按 [`Type::I64`] 处理，以便与整数集合 / 范围统一比较。
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// 8 位有符号整数
    I8,
    /// 16 位有符号整数
    I16,
    /// 32 位有符号整数
    I32,
    /// 64 位有符号整数
    I64,
    /// 128 位有符号整数
    I128,
    /// 平台有符号整数
    ISize,
    /// 8 位无符号整数
    U8,
    /// 16 位无符号整数
    U16,
    /// 32 位无符号整数
    U32,
    /// 64 位无符号整数
    U64,
    /// 128 位无符号整数
    U128,
    /// 平台无符号整数
    USize,
    /// 32 位浮点
    F32,
    /// 64 位浮点
    F64,
    /// 布尔
    Bool,
    /// 字符
    Char,
    /// 字符串
    Str,
    /// 单元类型 `()`
    Unit,
    /// 永不返回类型 `!`
    Never,
    /// 未推断类型
    Infer,
    /// 引用类型
    Ref(Box<Type>, Mutability),
    /// 数组类型
    Array(Box<Type>, usize),
    /// 元组类型
    Tuple(Vec<Type>),
    /// 具名类型（结构体 / 枚举 / trait 等）
    Named(String, Vec<Type>),
    /// 泛型占位
    Generic(String),
}

impl Type {
    /// 是否为数值类型（整数 / 浮点）。
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::I128
                | Type::ISize
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::U128
                | Type::USize
                | Type::F32
                | Type::F64
        )
    }

    /// 是否为整数类型。
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::I128
                | Type::ISize
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::U128
                | Type::USize
        )
    }

    /// 是否为布尔类型。
    pub fn is_bool(&self) -> bool {
        matches!(self, Type::Bool)
    }

    /// 是否为浮点类型。
    pub fn is_float(&self) -> bool {
        matches!(self, Type::F32 | Type::F64)
    }

    /// 与另一类型是否兼容（可参与同一比较 / 集合）。
    ///
    /// 数值类型互相兼容；其余要求类型完全相同。
    pub fn compatible_with(&self, other: &Type) -> bool {
        (self.is_numeric() && other.is_numeric()) || self == other
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I8 => write!(f, "i8"),
            Type::I16 => write!(f, "i16"),
            Type::I32 => write!(f, "i32"),
            Type::I64 => write!(f, "i64"),
            Type::I128 => write!(f, "i128"),
            Type::ISize => write!(f, "isize"),
            Type::U8 => write!(f, "u8"),
            Type::U16 => write!(f, "u16"),
            Type::U32 => write!(f, "u32"),
            Type::U64 => write!(f, "u64"),
            Type::U128 => write!(f, "u128"),
            Type::USize => write!(f, "usize"),
            Type::F32 => write!(f, "f32"),
            Type::F64 => write!(f, "f64"),
            Type::Bool => write!(f, "bool"),
            Type::Char => write!(f, "char"),
            Type::Str => write!(f, "string"),
            Type::Unit => write!(f, "()"),
            Type::Never => write!(f, "!"),
            Type::Infer => write!(f, "_"),
            Type::Ref(t, m) => match m {
                Mutability::Immutable => write!(f, "&{t}"),
                Mutability::Mutable => write!(f, "&mut {t}"),
            },
            Type::Array(t, n) => write!(f, "[{t}; {n}]"),
            Type::Tuple(ts) => {
                let inner = ts
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "({inner})")
            }
            Type::Named(name, args) => {
                if args.is_empty() {
                    write!(f, "{name}")
                } else {
                    let inner = args
                        .iter()
                        .map(|t| t.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    write!(f, "{name}<{inner}>")
                }
            }
            Type::Generic(name) => write!(f, "{name}"),
        }
    }
}

/// 函数类型签名。
#[derive(Debug, Clone, PartialEq)]
pub struct FnSignature {
    /// 参数类型
    pub params: Vec<Type>,
    /// 返回类型
    pub return_type: Type,
}

/// 结构体定义。
#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    /// 字段名与类型
    pub fields: Vec<(String, Type)>,
}
