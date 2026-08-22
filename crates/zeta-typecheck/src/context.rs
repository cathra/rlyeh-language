//! 类型检查上下文（符号表）。

use std::collections::HashMap;

use zeta_hir::HirExpr;
use zeta_lexer::Span;

use zeta_ast::{AstActorDecl, AstFnDecl};

use crate::error::TypeError;
use crate::types::{EnumDef, FnSignature, ImplDef, StructDef, TraitDef, Type};

/// 泛型函数模板（实例化前不生成 HIR，调用点按实参类型实例化）。
#[derive(Debug, Clone)]
pub struct FnTemplate {
    /// 模板名（未实例化的原始名）
    #[allow(dead_code)]
    pub name: String,
    /// 泛型参数名
    pub type_params: Vec<String>,
    /// 签名（参数 / 返回类型中可含 [`Type::Generic`]）
    pub sig: FnSignature,
    /// 原始函数 AST（实例化时克隆并替换类型参数）
    pub ast: AstFnDecl,
}

/// 类型检查上下文。
///
/// 维护函数签名、局部变量、结构体定义与类型别名。
/// 函数签名在检查函数体之前先收集完毕，因此支持函数间互相调用。
#[derive(Debug, Clone, Default)]
pub struct TypeContext {
    /// 函数签名表（函数名 → 签名）
    pub fn_signatures: HashMap<String, FnSignature>,
    /// 泛型函数模板表（泛型函数实例化前的原始定义）
    pub fn_templates: HashMap<String, FnTemplate>,
    /// 泛型实例化缓存（实例键 → 实例函数名）
    pub mono_instances: HashMap<String, String>,
    /// 泛型实例化生成的函数项（检查过程中追加）
    pub mono_items: Vec<zeta_hir::HirItem>,
    /// 局部变量表（变量名 → 类型）
    pub variables: HashMap<String, Type>,
    /// 局部变量初始化表达式表（变量名 → 初始化 HIR）。
    ///
    /// 供 `String::from(s)` 在 `s` 为字面量绑定的变量时追踪字面量值
    /// （`String::from` 的展开需要编译期字符串内容；非字面量 Str 的长度
    /// 表达尚未实现）。与 `variables` 表同节奏维护。
    pub local_inits: HashMap<String, HirExpr>,
    /// 结构体定义表
    pub structs: HashMap<String, StructDef>,
    /// 枚举定义表
    pub enum_defs: HashMap<String, EnumDef>,
    /// trait 定义表
    pub trait_defs: HashMap<String, TraitDef>,
    /// impl 块列表（inherent 与 trait impl 统一存放）
    pub impl_defs: Vec<ImplDef>,
    /// 变体名索引（变体名 → (枚举名, 变体名)，支持裸名 `Some(x)` 构造）
    pub variant_index: HashMap<String, (String, String)>,
    /// 类型别名表
    pub type_aliases: HashMap<String, Type>,
    /// use 导入别名表（本地名 → 完整符号名，如 `"add"` → `"math::add"`）
    pub use_aliases: HashMap<String, String>,
    /// 模块常量表（完整符号名 → (HIR 值, 类型)，如 `"math::MAX"`）
    pub constants: HashMap<String, (HirExpr, Type)>,
    /// Actor 定义表（完整符号名 → 声明，如 `"Counter"` / `"math::Counter"`）
    pub actors: HashMap<String, AstActorDecl>,
    /// 已生成的 `zeta_actor_*` extern 声明名（actor 展开去重用）
    pub generated_actor_externs: std::collections::HashSet<String>,
    /// 当前检查的模块前缀（顶层为空串，`mod math` 内为 `"math"`）
    pub module_prefix: String,
    /// 当前作用域的泛型参数名（如 `["T"]`）
    pub type_params: Vec<String>,
    /// 当前泛型替换表（泛型参数名 → 具体类型，实例化 body 检查时有效）
    pub generic_subst: HashMap<String, Type>,
    /// 临时变量名计数器
    pub temp_counter: usize,
    /// 顶层裸名 fn 被遮蔽的重命名表（裸名 → mangle 名）。
    ///
    /// 用户顶层函数与 std 预置根函数重名时（如用户 `fn read` 与 std extern `read`），
    /// 用户声明注册为 `read@shadow<N>`，std 原名保留——模块内部裸名调用仍绑定 std 版本，
    /// 用户顶层代码经本表绑定用户自身版本；同时避免下游 MIR/LIR/codegen 同名符号冲突。
    pub fn_shadow_of: HashMap<String, String>,
    /// 被遮蔽声明的重命名记录（声明 Span(start,end) → mangle 名）。
    ///
    /// 收集阶段（第一遍）决定重命名并记录，检查阶段（第二遍）生成 HIR 项时
    /// 按同一 Span 取出 mangle 名，保证两遍一致。
    pub fn_decl_shadow: HashMap<(usize, usize), String>,
    /// 同名遮蔽计数器（裸名 → 已遮蔽次数，用于生成唯一 mangle 名）。
    pub fn_shadow_seq: HashMap<String, usize>,
}

impl TypeContext {
    /// 创建空上下文
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录一个变量绑定。
    pub fn insert_variable(&mut self, name: String, type_: Type) {
        self.variables.insert(name, type_);
    }

    /// 查找变量的类型。
    pub fn lookup_variable(&self, name: &str) -> Option<&Type> {
        self.variables.get(name)
    }

    /// 记录一个变量绑定的初始化表达式。
    pub fn insert_local_init(&mut self, name: String, init: HirExpr) {
        self.local_inits.insert(name, init);
    }

    /// 查找变量绑定的初始化表达式。
    pub fn lookup_local_init(&self, name: &str) -> Option<&HirExpr> {
        self.local_inits.get(name)
    }

    /// 记录一个函数签名。
    pub fn insert_fn_signature(&mut self, name: String, signature: FnSignature) {
        self.fn_signatures.insert(name, signature);
    }

    /// 查找函数签名。
    pub fn lookup_fn_signature(&self, name: &str) -> Option<&FnSignature> {
        self.fn_signatures.get(name)
    }

    /// 记录一个结构体定义。
    pub fn insert_struct(&mut self, name: String, def: StructDef) {
        self.structs.insert(name, def);
    }

    /// 查找结构体定义。
    pub fn lookup_struct(&self, name: &str) -> Option<&StructDef> {
        self.structs.get(name)
    }

    /// 记录一个类型别名。
    #[allow(dead_code)]
    pub fn insert_type_alias(&mut self, name: String, type_: Type) {
        self.type_aliases.insert(name, type_);
    }

    /// 记录一个 use 导入别名（本地名 → 完整符号名）。
    pub fn insert_use_alias(&mut self, local: String, full: String) {
        self.use_aliases.insert(local, full);
    }

    /// 记录一个模块常量（完整符号名 → (HIR 值, 类型)）。
    pub fn insert_constant(&mut self, name: String, value: HirExpr, type_: Type) {
        self.constants.insert(name, (value, type_));
    }

    /// 查找模块常量（支持短名 → 完整名解析，与 struct/actor 一致：
    /// 裸名查表，失败后回退 `use` 导入别名）。
    pub fn lookup_constant(&self, name: &str) -> Option<&(HirExpr, Type)> {
        if let Some(v) = self.constants.get(name) {
            return Some(v);
        }
        self.resolve_full_name(name)
            .and_then(|full| self.constants.get(&full))
    }

    /// 将局部名解析为完整符号名。
    ///
    /// 优先级：直接存在的符号名（函数 / 结构体 / actor / 常量）→ use 导入别名。
    /// 无法解析时返回 `None`（由调用方决定如何报错）。
    pub fn resolve_full_name(&self, name: &str) -> Option<String> {
        if self.structs.contains_key(name)
            || self.fn_signatures.contains_key(name)
            || self.actors.contains_key(name)
            || self.constants.contains_key(name)
        {
            return Some(name.to_string());
        }
        self.use_aliases.get(name).cloned()
    }

    /// 按名字查找 actor 定义（支持短名 → 完整名解析，与 struct 一致）。
    pub fn lookup_actor(&self, name: &str) -> Option<&AstActorDecl> {
        self.resolve_full_name(name)
            .and_then(|full| self.actors.get(&full))
    }

    /// 解析一个具名类型（查结构体 / 枚举 / 别名 / 预置内置类型名）。
    ///
    /// 内置类型名（`i64`、`bool` 等）直接映射到 [`Type`]；
    /// 其余名称查结构体 / 枚举表与别名表，并支持 `math::Point` 形式的模块路径
    /// 与 `use` 导入的别名。当前作用域的泛型参数返回 [`Type::Generic`]；
    /// 若存在泛型替换表则返回替换后的具体类型。查不到时返回
    /// [`TypeError::UndefinedType`]。
    pub fn resolve_named_type(&self, name: &str, span: Span) -> Result<Type, TypeError> {
        // 泛型替换优先（实例化 body 检查时 `T` → 具体类型）
        if let Some(t) = self.generic_subst.get(name) {
            return Ok(t.clone());
        }
        if self.type_params.iter().any(|p| p == name) {
            return Ok(Type::Generic(name.to_string()));
        }
        let builtin = builtin_type(name);
        if let Some(t) = builtin {
            return Ok(t);
        }
        if let Some(alias) = self.type_aliases.get(name) {
            return Ok(alias.clone());
        }
        if self.structs.contains_key(name) {
            return Ok(Type::Named(name.to_string(), Vec::new()));
        }
        if self.enum_defs.contains_key(name) {
            return Ok(Type::Named(name.to_string(), Vec::new()));
        }
        // 模块路径（如 `math::Point`）或 use 导入的别名
        if let Some(full) = self.resolve_full_name(name) {
            if self.structs.contains_key(&full) {
                return Ok(Type::Named(full, Vec::new()));
            }
            if self.enum_defs.contains_key(&full) {
                return Ok(Type::Named(full, Vec::new()));
            }
        }
        Err(TypeError::UndefinedType {
            name: name.to_string(),
            span,
        })
    }

    /// 记录一个枚举定义。
    pub fn insert_enum(&mut self, name: String, def: EnumDef) {
        for variant in &def.variants {
            self.variant_index
                .insert(variant.name.clone(), (name.clone(), variant.name.clone()));
        }
        self.enum_defs.insert(name, def);
    }

    /// 查找枚举定义。
    pub fn lookup_enum(&self, name: &str) -> Option<&EnumDef> {
        self.enum_defs.get(name)
    }

    /// 记录一个 trait 定义。
    pub fn insert_trait(&mut self, name: String, def: TraitDef) {
        self.trait_defs.insert(name, def);
    }

    /// 查找 trait 定义。
    #[allow(dead_code)]
    pub fn lookup_trait(&self, name: &str) -> Option<&TraitDef> {
        self.trait_defs.get(name)
    }

    /// 记录一个 impl 块。
    pub fn insert_impl(&mut self, def: ImplDef) {
        self.impl_defs.push(def);
    }

    /// 按目标类型解析变体名：`EnumName::Variant` 或裸 `Variant`。
    ///
    /// 返回 `(枚举名, 变体名)`。
    pub fn resolve_variant(&self, enum_name: Option<&str>, variant: &str) -> Option<(String, String)> {
        if let Some(en) = enum_name {
            if self.enum_defs.contains_key(en) {
                let def = self.enum_defs.get(en)?;
                if def.variants.iter().any(|v| v.name == variant) {
                    return Some((en.to_string(), variant.to_string()));
                }
            }
            return None;
        }
        self.variant_index
            .get(variant)
            .map(|(e, v)| (e.clone(), v.clone()))
    }

    /// 分配一个临时变量名（用于 match 展开 / 聚合构造）。
    pub fn fresh_temp(&mut self) -> String {
        let n = self.temp_counter;
        self.temp_counter += 1;
        format!("__tmp{n}")
    }

    /// 在 inherent impl 中按目标类型查找方法所属的 impl 块。
    #[allow(dead_code)]
    pub fn find_impl(&self, self_type: &Type) -> Option<&ImplDef> {
        self.impl_defs
            .iter()
            .find(|d| d.trait_name.is_none() && type_matches(d, self_type))
    }

    /// 在 trait impl 中按目标类型查找方法所属的 impl 块。
    #[allow(dead_code)]
    pub fn find_trait_impl(&self, self_type: &Type) -> Option<&ImplDef> {
        self.impl_defs
            .iter()
            .find(|d| d.trait_name.is_some() && type_matches(d, self_type))
    }

    /// 按目标类型查找含指定方法的 impl 块（inherent 优先，trait 次之）。
    pub fn find_impl_for_method(&self, self_type: &Type, method: &str) -> Option<&ImplDef> {
        self.impl_defs
            .iter()
            .find(|d| type_matches(d, self_type) && d.methods.iter().any(|m| m.sig.name == method))
    }

    /// 当前作用域是否为泛型参数。
    #[allow(dead_code)]
    pub fn is_type_param(&self, name: &str) -> bool {
        self.type_params.iter().any(|p| p == name)
    }
}

/// impl 块目标类型与具体类型匹配（未含泛型参数的 impl 需精确匹配；
/// 含泛型参数的 impl 匹配同名类型，参数在调用点替换）。
fn type_matches(imp: &ImplDef, concrete: &Type) -> bool {
    let Type::Named(name, _) = &imp.self_type else {
        return false;
    };
    match concrete {
        Type::Named(cname, _) => name == cname,
        Type::Ref(inner, _) => type_matches(imp, inner),
        _ => false,
    }
}

/// 将内置类型名解析为 [`Type`]。
fn builtin_type(name: &str) -> Option<Type> {
    Some(match name {
        "i8" => Type::I8,
        "i16" => Type::I16,
        "i32" => Type::I32,
        "i64" => Type::I64,
        "i128" => Type::I128,
        "isize" => Type::ISize,
        "u8" => Type::U8,
        "u16" => Type::U16,
        "u32" => Type::U32,
        "u64" => Type::U64,
        "u128" => Type::U128,
        "usize" => Type::USize,
        "f32" => Type::F32,
        "f64" => Type::F64,
        "bool" => Type::Bool,
        "char" => Type::Char,
        "string" => Type::Str,
        "()" | "unit" => Type::Unit,
        _ => return None,
    })
}
