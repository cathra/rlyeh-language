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
    /// 数值类型互相兼容；`_`（Infer）与任意类型兼容（类型由上下文推断）；
    /// 数组要求长度一致且元素兼容；命名类型（泛型）要求同名且类型参数逐个兼容；
    /// 其余要求类型完全相同。
    pub fn compatible_with(&self, other: &Type) -> bool {
        matches!(self, Type::Infer)
            || matches!(other, Type::Infer)
            || (self.is_numeric() && other.is_numeric())
            || match (self, other) {
                (Type::Array(a, na), Type::Array(b, nb)) => {
                    na == nb && a.compatible_with(b)
                }
                (Type::Named(sa, aa), Type::Named(sb, ab)) => {
                    sa == sb
                        && aa.len() == ab.len()
                        && aa.iter().zip(ab).all(|(x, y)| x.compatible_with(y))
                }
                // 引用类型：内层兼容且可变性可接受
                // （`&mut T` 可传给 `&T`——宽松规则，严格互斥检查留给 borrowck）
                (Type::Ref(a, ma), Type::Ref(b, mb)) => {
                    // `&str` ↔ `&String`：同一只读借用视图（G2，MVP 中 &str 是
                    // String 对象的借用），内层类型可互视
                    let inner_ok = a.compatible_with(b)
                        || (matches!(**a, Type::Str) && is_named_string(b))
                        || (is_named_string(a) && matches!(**b, Type::Str));
                    inner_ok
                        && matches!(
                            (ma, mb),
                            (_, Mutability::Immutable) | (Mutability::Mutable, Mutability::Mutable)
                        )
                }
                // `&str` 视图与 String 值互用（G2：比较 `r == s`、`s == r`）
                (Type::Ref(a, _), Type::Named(n, _)) => matches!(**a, Type::Str) && n == "String",
                (Type::Named(n, _), Type::Ref(a, _)) => matches!(**a, Type::Str) && n == "String",
                _ => self == other,
            }
    }
}

/// 是否为 `String` 命名类型（`&str` ↔ `&String` 互视规则用）。
fn is_named_string(t: &Type) -> bool {
    matches!(t, Type::Named(n, _) if n == "String")
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

/// 枚举变体定义。
#[derive(Debug, Clone, PartialEq)]
pub struct VariantDef {
    /// 变体名
    pub name: String,
    /// 字段名与类型
    pub fields: Vec<(String, Type)>,
    /// 判别值（变体在枚举中的序号，槽 0 存储）
    pub tag: usize,
}

/// 枚举定义。
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    /// 枚举名
    pub name: String,
    /// 泛型参数名（如 `Option` 的 `["T"]`）
    pub type_params: Vec<String>,
    /// 变体列表（按声明顺序，`tag` 即下标）
    pub variants: Vec<VariantDef>,
    /// 对象槽数：`1（tag）+ max(变体字段数)`，MVP 布局下所有变体
    /// 的字段从槽 1 起连续排布。
    pub slot_count: usize,
}

/// 方法签名。
#[derive(Debug, Clone, PartialEq)]
pub struct MethodSig {
    /// 方法名
    pub name: String,
    /// 参数类型（含 `self`，位于 `params[0]`）
    pub params: Vec<Type>,
    /// 返回类型
    pub return_type: Type,
}

/// trait 定义。
#[derive(Debug, Clone, PartialEq)]
pub struct TraitDef {
    /// trait 名
    pub name: String,
    /// 泛型参数名
    pub type_params: Vec<String>,
    /// 抽象方法签名
    pub methods: Vec<MethodSig>,
}

/// impl 块中的方法（含原始 AST，供泛型实例化时克隆检查）。
#[derive(Debug, Clone)]
pub struct ImplMethod {
    /// 方法签名
    pub sig: MethodSig,
    /// 原始函数 AST（抽象方法 / 仅声明为 `None`）
    pub body: Option<zeta_ast::AstFnDecl>,
}

/// impl 块定义（inherent 或 trait impl）。
#[derive(Debug, Clone)]
pub struct ImplDef {
    /// 若为 trait impl，则为 trait 名；否则为 `None`（inherent impl）
    pub trait_name: Option<String>,
    /// impl 目标类型（如 `Named("Vec", [Generic("T")])`）
    pub self_type: Type,
    /// 泛型参数名
    pub type_params: Vec<String>,
    /// 方法（`self` 位于参数首位）
    pub methods: Vec<ImplMethod>,
}

/// 判定类型对应的对象槽标量种类（MVP 布局规则）。
pub fn field_scalar_of(ty: &Type) -> zeta_hir::FieldScalar {
    use zeta_hir::FieldScalar;
    match ty {
        Type::F32 | Type::F64 => FieldScalar::Float,
        Type::Bool => FieldScalar::Bool,
        Type::Char => FieldScalar::Char,
        Type::Str => FieldScalar::Str,
        // 聚合类型 / 引用 / 数组 / 元组均以指针形式存储
        Type::Ref(..) | Type::Array(..) | Type::Tuple(..) | Type::Named(..) => FieldScalar::Ptr,
        Type::Unit => FieldScalar::Int,
        _ => FieldScalar::Int,
    }
}

/// 将类型转换为泛型实例化的稳定键。
pub fn type_mono_key(ty: &Type) -> String {
    match ty {
        Type::Named(name, args) => {
            if args.is_empty() {
                name.clone()
            } else {
                let inner = args
                    .iter()
                    .map(type_mono_key)
                    .collect::<Vec<_>>()
                    .join("_");
                format!("{name}_{inner}")
            }
        }
        Type::Ref(t, m) => format!(
            "ref{}_{}",
            if *m == Mutability::Mutable { "mut" } else { "imm" },
            type_mono_key(t)
        ),
        Type::Tuple(ts) => {
            let inner = ts.iter().map(type_mono_key).collect::<Vec<_>>().join("_");
            format!("tup_{inner}")
        }
        _ => ty.to_string(),
    }
}
