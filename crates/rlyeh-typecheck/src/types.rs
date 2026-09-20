//! Rlyeh 类型系统定义。

use std::collections::HashMap;
use std::fmt;

use rlyeh_hir::ReprConv;
use rlyeh_lexer::Span;
use crate::context::TypeContext;
use crate::error::TypeError;

/// 类型可变性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    /// `&T`
    Immutable,
    /// `&mut T`
    Mutable,
}

/// Rlyeh 类型。
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
    /// 引用类型；第三字段为可选生命周期名（`None` = 省略，走默认 region 推断），
    /// 由 borrowck 生命周期检查专项（T-0）引入。
    Ref(Box<Type>, Mutability, Option<String>),
    /// 裸指针类型（`*const T` / `*mut T`）
    RawPtr(Box<Type>, bool),
    /// trait 对象类型（`dyn Trait`）：数据指针 + vtable 指针的胖指针，占 2 槽
    Dyn(String),
    /// 数组类型
    Array(Box<Type>, usize),
    /// 切片类型（运行时长度未知；`&[T]` / `&mut [T]` 的元素类型，见切片类型系统规划）
    Slice(Box<Type>),
    /// 类型联合 `A | B | ...`（U1 受限制的类型联合）
    ///
    /// 成员**两两互不相交**（disjoint，构建时校验，见 `resolve_ast_type`），
    /// 保证 tag 判别无歧义、收窄安全。构造由任一成员值直接赋值（成员 ⊆ 联合）；
    /// 使用须先 `match` 收窄（U2 落地），未收窄禁止直接运算 / 方法调用。
    Union(Vec<Type>),
    /// 元组类型
    Tuple(Vec<Type>),
    /// 具名类型（结构体 / 枚举 / trait 等）
    Named(String, Vec<Type>),
    /// 受限标量枚举（U3 核心项，2026-08-30）：全单元变体、无泛型参数的枚举，
    /// 紧凑为单标量存储，值即 tag（与 `Named` 区分以便 `field_scalar_of` 等无
    /// `TypeContext` 的纯函数判定布局）。Display 与同名 `Named` 相同。
    ScalarEnum(String),
    /// 函数类型（`fn(A, B) -> C`），即函数指针类型
    Fn(Box<FnSignature>),
    /// 闭包值对象：捕获字段 + 参数 + 返回类型 + 生成的匿名函数名。
    ///
    /// 由 `let f = |x: i64| ..;` 全参数注解闭包创建，desugar 为捕获聚合对象
    /// （`Alloc` + `FieldSet`）；调用点 `f(..)` desugar 为
    /// `__closure_N(FieldGet(f, i)..., 实参...)`。仅存在于局部变量环境，
    /// 不跨函数边界（MVP）。
    Closure {
        /// 捕获字段类型（顺序与聚合对象槽位对应）
        captures: Vec<Type>,
        /// 闭包参数类型（来自参数注解）
        params: Vec<Type>,
        /// 返回类型
        ret: Box<Type>,
        /// 生成的匿名函数名（`__closure_N`）
        fn_name: String,
        /// 是否 `move` 闭包（F-M2：捕获环境所有权转移，可跨线程 `'static`）。
        /// 无捕获的闭包 `move`/`borrow` 语义等价，故仅在有捕获时区分。
        is_move: bool,
    },
    /// 泛型占位
    Generic(String),
    /// 关联类型投影（`F::Output`，W4 补全）：`base` 为被投影的基础类型
    /// （实例化前为 `Generic("F")`，实例化后为具体类型），`assoc` 为关联类型名。
    ///
    /// 仅出现在泛型函数签名/body 中 `F: Trait` 约束下的 `F::Assoc` 引用；
    /// 实例化时 base 替换为具体类型后按该类型实现的 trait 求值其关联类型。
    AssocProjection {
        /// 被投影的基础类型（`F::Output` 中的 `F`）
        base: Box<Type>,
        /// 关联类型名（`F::Output` 中的 `Output`）
        assoc: String,
    },
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

    /// B-2（P0'）：类型是否为 Copy（可按值安全拷贝，允许 `&T → T` 自动解引用取值）。
    ///
    /// 仅标量为 Copy；用户定义结构体 / 枚举 / 引用 / 堆装箱 / `Str` 视图均非 Copy，
    /// 以避免隐式移动 / 克隆大对象（与 RFC P0'「仅对 Copy 类型生效」一致）。
    pub fn is_copy(&self) -> bool {
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
                | Type::Bool
                | Type::Char
                | Type::Unit
        )
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
                (Type::Ref(a, ma, _), Type::Ref(b, mb, _)) => {
                    // `&str` ↔ `&String`：同一只读借用视图（G2，MVP 中 &str 是
                    // String 对象的借用），内层类型可互视
                    let inner_ok = a.compatible_with(b)
                        || (matches!(**a, Type::Str) && is_named_string(b))
                        || (is_named_string(a) && matches!(**b, Type::Str))
                        // S2 unsize coercion：`&[T; N]` → `&[T]`（数组引用可降级为
                        // 切片胖指针，元素类型须兼容；codegen 侧在调用点构造 `{data, len}`）
                        || matches!(
                            (&**a, &**b),
                            (Type::Array(ae, _), Type::Slice(be)) if ae.compatible_with(be)
                        );
                    inner_ok
                        && matches!(
                            (ma, mb),
                            (_, Mutability::Immutable) | (Mutability::Mutable, Mutability::Mutable)
                        )
                }
                // `&str` 视图与 String 值互用（G2：比较 `r == s`、`s == r`）
                (Type::Ref(a, _, _), Type::Named(n, _)) => matches!(**a, Type::Str) && n == "String",
                (Type::Named(n, _), Type::Ref(a, _, _)) => matches!(**a, Type::Str) && n == "String",
                // 裸指针（G3）：`*mut T` 可降级为 `*const T`；反向不可
                (Type::RawPtr(a, ma), Type::RawPtr(b, mb)) => {
                    (*mb || !*ma) && a.compatible_with(b)
                }
                // 引用 ↔ 裸指针互视（G3 宽松规则，借用安全性留给 borrowck）：
                // `&T`/`&mut T` 与 `*const T`/`*mut T` 内层兼容即可互传——FFI 场景
                // （`let p: *const T = &x;`、`fn f(p: *const T)` 传 `&x`），
                // codegen 布局同为 i8* 槽，双向转换零成本
                (Type::RawPtr(a, _), Type::Ref(b, _, _))
                | (Type::Ref(a, _, _), Type::RawPtr(b, _)) => a.compatible_with(b),
                // 函数类型：参数逐个兼容且返回类型兼容
                (Type::Fn(a), Type::Fn(b)) => {
                    a.params.len() == b.params.len()
                        && a.params
                            .iter()
                            .zip(&b.params)
                            .all(|(x, y)| x.compatible_with(y))
                        && a.return_type.compatible_with(&b.return_type)
                }
                // U2：成员 → 联合的**向上转换**（构造；成员 ⊆ 联合，协变）。
                // 反向（联合 → 成员）不落此分支，须先 `match` 收窄才可用。
                (s, Type::Union(us)) if !matches!(s, Type::Union(_)) => {
                    us.iter().any(|u| s.compatible_with(u))
                }
                // U2：联合 → 联合——成员集合相同（顺序无关）即兼容
                (Type::Union(as_), Type::Union(bs)) => {
                    as_.len() == bs.len() && as_.iter().all(|a| bs.contains(a))
                }
                // U3 核心项（2026-08-30）：标量枚举 → 整数单向兼容——枚举值（即 tag）
                // 可用于「期望整数」的上下文（赋值 / 比较 / 索引 / 位运算），读取其 tag。
                // 反向（整数 → 枚举）禁止，避免构造出无对应判别式的非法值。
                // 覆盖方向：期望类型为整数、实参为标量枚举（`expected.compatible_with(found)`）。
                (Type::I8, Type::ScalarEnum(_))
                | (Type::I16, Type::ScalarEnum(_))
                | (Type::I32, Type::ScalarEnum(_))
                | (Type::I64, Type::ScalarEnum(_))
                | (Type::ISize, Type::ScalarEnum(_))
                | (Type::U8, Type::ScalarEnum(_))
                | (Type::U16, Type::ScalarEnum(_))
                | (Type::U32, Type::ScalarEnum(_))
                | (Type::U64, Type::ScalarEnum(_))
                | (Type::USize, Type::ScalarEnum(_)) => true,
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
            Type::Ref(t, m, _) => match m {
                Mutability::Immutable => write!(f, "&{t}"),
                Mutability::Mutable => write!(f, "&mut {t}"),
            },
            Type::RawPtr(t, m) => {
                if *m {
                    write!(f, "*mut {t}")
                } else {
                    write!(f, "*const {t}")
                }
            }
            Type::Dyn(name) => write!(f, "dyn {name}"),
            Type::Array(t, n) => write!(f, "[{t}; {n}]"),
            Type::Slice(t) => write!(f, "[{t}]"),
            Type::Union(ts) => {
                let inner = ts
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(" | ");
                write!(f, "{inner}")
            }
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
            Type::ScalarEnum(name) => write!(f, "{name}"),
            Type::Generic(name) => write!(f, "{name}"),
            Type::AssocProjection { base, assoc } => write!(f, "{base}::{assoc}"),
            Type::Fn(sig) => {
                let inner = sig
                    .params
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "fn({inner}) -> {}", sig.return_type)
            }
            Type::Closure { params, ret, .. } => {
                let inner = params
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "closure({inner}) -> {}", ret)
            }
        }
    }
}

/// 函数类型签名。
#[derive(Debug, Clone, PartialEq)]
pub struct FnSignature {
    /// 参数类型
    pub params: Vec<Type>,
    /// 参数声明位置（SH-P2-6 L2 多位置）：与 `params` 同序，用于实参类型不匹配
    /// 回指形参声明处；合成 / 无源码位置的签名填 `Span::dummy()`
    pub param_spans: Vec<Span>,
    /// 返回类型
    pub return_type: Type,
}

/// 结构体定义。
#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    /// 字段名与类型
    pub fields: Vec<(String, Type)>,
    /// 字段声明位置（SH-P2-6 L2 多位置）：与 `fields` 同序，用于类型不匹配回指
    pub field_spans: Vec<Span>,
    /// 泛型参数名（如 `Vec` 的 `["T"]`；V1 2026-08：字段访问时按接收者
    /// 实例类型参数替换，用户代码 `Vec<Infer>.data` 等场景）
    pub type_params: Vec<String>,
    /// 是否 `#[repr(C)]`（SH-P0-1 E2）：真 C 布局，sub-8 字节字段按 C 规则打包。
    pub repr_c: bool,
}

/// 枚举变体定义。
#[derive(Debug, Clone, PartialEq)]
pub struct VariantDef {
    /// 变体名
    pub name: String,
    /// 字段名与类型
    pub fields: Vec<(String, Type)>,
    /// 字段声明处的源码位置（SH-P2-6 L2 多位置：与 `fields` 同序；命名域为字段声明
    /// span，元组域为变体声明 span），供字段类型不匹配时把 `= note:` 回指声明处。
    pub field_spans: Vec<Span>,
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
    /// trait 方法的默认实现 body（V3，2026-08-26：仅 trait 方法带 body 时填充；
    /// 抽象方法 / impl 方法为 `None`）。`impl Trait for X` 未实现该方法时，
    /// 方法调用回退到该默认实现（见 `check_method_call` 的 trait 默认方法回退）。
    pub default_body: Option<rlyeh_ast::AstFnDecl>,
}

/// trait 定义。
#[derive(Debug, Clone, PartialEq)]
pub struct TraitDef {
    /// trait 名
    pub name: String,
    /// 泛型参数名
    pub type_params: Vec<String>,
    /// 关联类型声明名（`type Item;`，U2）
    pub assoc_types: Vec<String>,
    /// 父协议（supertrait）名列表（PC-4：`protocol A: B` 的 `B`；裸名，使用时解析）。
    pub supertraits: Vec<String>,
    /// 抽象方法签名
    pub methods: Vec<MethodSig>,
}

/// impl 块中的方法（含原始 AST，供泛型实例化时克隆检查）。
#[derive(Debug, Clone)]
pub struct ImplMethod {
    /// 方法签名
    pub sig: MethodSig,
    /// 原始函数 AST（抽象方法 / 仅声明为 `None`）
    pub body: Option<rlyeh_ast::AstFnDecl>,
}

/// impl 块定义（inherent 或 trait impl）。
#[derive(Debug, Clone)]
pub struct ImplDef {
/// 若为 trait impl，则为 trait 名；否则为 `None`（inherent impl）
pub trait_name: Option<String>,
/// impl 目标类型（如 `Named("Vec", [Generic("T")])`）
pub self_type: Type,
/// trait 泛型实参（`impl Trait<Args> for Type` 中的 `Args`，对应 trait 声明的
/// 泛型参数顺序；如 `impl From<IoErrorKind> for IoError` 为 `[IoErrorKind]`）。
/// P6c（2026-08-29）：此前丢失，导致关联方法泛型参数无法绑定。
pub trait_type_args: Vec<Type>,
/// 源码位置（PC-4：父协议缺失校验定位用）
pub span: Span,
/// 泛型参数名
    pub type_params: Vec<String>,
    /// 泛型参数 → 约束 trait 名列表（U3：头部 `<T: B>` 与 `where T: B` 合并）。
    /// A4（SH-P1-1，2026-09-02）：impl 方法调用点实例化前经
    /// `check_generic_bounds` 强制校验（此前仅记录不校验）。
    pub bounds: HashMap<String, Vec<String>>,
    /// 关联类型定义（`type Item = Concrete;`，U2）
    pub assoc_types: Vec<(String, Type)>,
    /// 方法（`self` 位于参数首位）
    pub methods: Vec<ImplMethod>,
}

/// 判定类型对应的对象槽标量种类（MVP 布局规则）。
pub fn field_scalar_of(ty: &Type) -> rlyeh_hir::FieldScalar {
    use rlyeh_hir::FieldScalar;
    match ty {
        Type::F32 | Type::F64 => FieldScalar::Float,
        Type::Bool => FieldScalar::Bool,
        Type::Char => FieldScalar::Char,
        Type::Str => FieldScalar::Str,
        // &str：data 指针 + 长度双槽胖指针（V2 子区间视图，对齐 Rust fat pointer）
        Type::Ref(inner, _, _) if matches!(&**inner, Type::Str) => FieldScalar::StrFat,
        // &[T] / &mut [T]：切片胖指针（data 指针 + 长度双槽，与 StrFat 同布局）
        Type::Ref(inner, _, _) if matches!(&**inner, Type::Slice(_)) => FieldScalar::SliceFat,
        // U3 核心项（2026-08-30）：受限标量枚举紧凑为单标量存储，值即 tag。
        Type::ScalarEnum(_) => FieldScalar::Int,
        // 聚合类型 / 引用 / 裸指针 / 数组 / 元组 / trait 对象 / 闭包值均以指针形式存储；函数指针为指针
        Type::Ref(..)
        | Type::RawPtr(..)
        | Type::Array(..)
        | Type::Tuple(..)
        | Type::Named(..)
        | Type::Dyn(..)
        | Type::Fn(..)
        | Type::Closure { .. } => FieldScalar::Ptr,
        // U2：联合值的运行时表示是**匿名 enum 对象**（槽 0 = tag、槽 1 = payload），
        // 故按聚合对象指针存储（与具名 enum 一致），复用现有 enum codegen 通道。
        Type::Union(_) => FieldScalar::Ptr,
        Type::Unit => FieldScalar::Int,
        _ => FieldScalar::Int,
    }
}

/// repr(C) 结构体单字段的 C 布局描述（索引即字段声明序，与 `FieldGet` 的 `index` 一致）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CField {
    /// 标量字段：自然对齐（size）、紧密打包；内存窄类型 + 读取提升方式。
    Scalar {
        /// C 布局字节偏移
        offset: u32,
        /// 字段在内存中的 LLVM 类型
        field_ty: &'static str,
        /// 窄字段值 → Rlyeh 宽值的提升方式
        conv: ReprConv,
    },
    /// 嵌套 repr(C) 结构体字段：在父对象内联（C 规则），`FieldGet` 返回指向
    /// `offset` 的子指针，`.inner` 访问复用内层 C 布局；`FieldSet` 经 memcpy
    /// 拷入 `size` 字节。
    Nested {
        /// C 布局字节偏移（父对象内）
        offset: u32,
        /// 子对象字节大小（memcpy 长度）
        size: u32,
    },
}

/// repr(C) 结构体的完整 C 布局。
#[derive(Debug, Clone, PartialEq)]
pub struct ReprCLayout {
    /// 逐字段布局（索引 = 字段声明序）
    pub fields: Vec<CField>,
    /// 整体字节大小（含尾部对齐 padding）
    pub size: u32,
    /// 整体对齐（max 成员对齐）
    pub align: u32,
}

/// 计算 repr(C) 结构体的完整 C 布局（递归处理嵌套 repr(C) 结构体）。
///
/// 对齐按字段自然对齐（= size，均为 2 的幂；指针 8 字节）；嵌套 repr(C) 结构体
/// 按其自身对齐与大小内联。返回逐字段偏移 / 内存类型 / 提升方式，以及整体 size / align。
pub(crate) fn compute_repr_c(
    fields: &[(String, Type)],
    ctx: &TypeContext,
    span: Span,
) -> Result<ReprCLayout, TypeError> {
    compute_repr_c_depth(fields, ctx, span, 0)
}

fn compute_repr_c_depth(
    fields: &[(String, Type)],
    ctx: &TypeContext,
    span: Span,
    depth: u32,
) -> Result<ReprCLayout, TypeError> {
    if depth > 64 {
        return Err(TypeError::Unsupported {
            what: "repr(C) 嵌套层级过深（疑似递归结构体；C 不允许值递归，须改用指针）".to_string(),
            span,
        });
    }
    let mut offset = 0u32;
    let mut align = 1u32;
    let mut out = Vec::with_capacity(fields.len());
    for (_, ty) in fields {
        let (size, falign, scalar) = c_field_repr(ty, ctx, span, depth)?;
        let falign = falign.max(1) as u32;
        offset = (offset + falign - 1) / falign * falign;
        let foff = offset;
        match scalar {
            Some((field_ty, conv)) => {
                out.push(CField::Scalar {
                    offset: foff,
                    field_ty,
                    conv,
                });
            }
            None => {
                out.push(CField::Nested {
                    offset: foff,
                    size: size as u32,
                });
            }
        }
        offset += size as u32;
        align = align.max(falign);
    }
    let size = (offset + align - 1) / align * align;
    Ok(ReprCLayout {
        fields: out,
        size,
        align,
    })
}

/// 计算类型在内存中的字节大小（供 `mem::swap` 等字节级原语使用）。
///
/// 布局规则与 codegen 对齐：普通结构体 / 元组按「每字段（元素）8 字节槽」对齐
/// （见 `llvm_field.rs` 的 `index * 8` 槽布局，无压缩无 padding）；repr(C)
/// 结构体走紧凑 C 布局；标量 / 引用 / 裸指针按真实字节（`c_field_repr`）。
pub(crate) fn type_byte_size(
    ty: &Type,
    ctx: &TypeContext,
    span: Span,
) -> Result<u32, TypeError> {
    match ty {
        Type::Unit => Ok(0),
        Type::Tuple(elems) => Ok(elems.len() as u32 * 8),
        Type::Array(elem, len) => Ok(type_byte_size(elem, ctx, span)? * (*len as u32)),
        Type::Named(n, _) => {
            if let Some(d) = ctx.lookup_struct(n) {
                if d.repr_c {
                    Ok(compute_repr_c(&d.fields, ctx, span)?.size)
                } else {
                    Ok(d.fields.len() as u32 * 8)
                }
            } else {
                Ok(8)
            }
        }
        // 引用 / 裸指针：存储为 i8* 槽（8 字节）
        Type::Ref(..) | Type::RawPtr(..) => Ok(8),
        // 标量：`c_field_repr` 给出真实字节（≤8）；其余不支持的聚合回退 8
        other => Ok(c_field_repr(other, ctx, span, 0).unwrap_or((8, 8, None)).0 as u32),
    }
}

/// 单字段的 C 表示：返回 `(size, align, scalar?)`；`scalar = Some((field_ty, conv))`
/// 为标量字段，`None` 表示嵌套 repr(C) 结构体（size 为其整体大小）。
fn c_field_repr(
    ty: &Type,
    ctx: &TypeContext,
    span: Span,
    depth: u32,
) -> Result<(u8, u8, Option<(&'static str, ReprConv)>), TypeError> {
    match ty {
        Type::I8 => Ok((1, 1, Some(("i8", ReprConv::Sext)))),
        Type::U8 => Ok((1, 1, Some(("i8", ReprConv::Zext)))),
        Type::I16 => Ok((2, 2, Some(("i16", ReprConv::Sext)))),
        Type::U16 => Ok((2, 2, Some(("i16", ReprConv::Zext)))),
        Type::I32 => Ok((4, 4, Some(("i32", ReprConv::Sext)))),
        Type::U32 => Ok((4, 4, Some(("i32", ReprConv::Zext)))),
        Type::F32 => Ok((4, 4, Some(("float", ReprConv::Fpext)))),
        Type::I64 | Type::U64 => Ok((8, 8, Some(("i64", ReprConv::None)))),
        Type::F64 => Ok((8, 8, Some(("double", ReprConv::None)))),
        Type::Bool => Ok((1, 1, Some(("i1", ReprConv::None)))),
        Type::Char => Ok((4, 4, Some(("i32", ReprConv::None)))),
        Type::RawPtr(..) | Type::Ref(..) => Ok((8, 8, Some(("i8*", ReprConv::None)))),
        Type::Named(n, _) => match ctx.lookup_struct(n) {
            Some(d) if d.repr_c => {
                let nested = compute_repr_c_depth(&d.fields, ctx, span, depth + 1)?;
                Ok((nested.size as u8, nested.align as u8, None))
            }
            Some(_) => Err(TypeError::Unsupported {
                what: format!(
                    "repr(C) 字段 `{n}` 为嵌套结构体，但其未标注 #[repr(C)]；嵌套聚合内联要求内层结构体同样采用 C 布局"
                ),
                span,
            }),
            None => Err(TypeError::Unsupported {
                what: format!("repr(C) 字段 `{n}` 引用的结构体未定义"),
                span,
            }),
        },
        other => Err(TypeError::Unsupported {
            what: format!(
                "repr(C) 结构体字段 `{other}` 为不支持的聚合类型（枚举 / 联合 / 数组 / 字符串视图 / dyn Trait / 元组 / 切片）"
            ),
            span,
        }),
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
        Type::Ref(t, m, _) => format!(
            "ref{}_{}",
            if *m == Mutability::Mutable { "mut" } else { "imm" },
            type_mono_key(t)
        ),
        Type::RawPtr(t, m) => format!(
            "rawptr{}_{}",
            if *m { "mut" } else { "const" },
            type_mono_key(t)
        ),
        Type::Tuple(ts) => {
            let inner = ts.iter().map(type_mono_key).collect::<Vec<_>>().join("_");
            format!("tup_{inner}")
        }
        Type::Fn(sig) => {
            let inner = sig
                .params
                .iter()
                .map(type_mono_key)
                .collect::<Vec<_>>()
                .join("_");
            format!("fn_{inner}->{}", type_mono_key(&sig.return_type))
        }
        _ => ty.to_string(),
    }
}
