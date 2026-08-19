//! 类型检查上下文（符号表）。

use std::collections::HashMap;

use zeta_lexer::Span;

use crate::error::TypeError;
use crate::types::{FnSignature, StructDef, Type};

/// 类型检查上下文。
///
/// 维护函数签名、局部变量、结构体定义与类型别名。
/// 函数签名在检查函数体之前先收集完毕，因此支持函数间互相调用。
#[derive(Debug, Clone, Default)]
pub struct TypeContext {
    /// 函数签名表（函数名 → 签名）
    pub fn_signatures: HashMap<String, FnSignature>,
    /// 局部变量表（变量名 → 类型）
    pub variables: HashMap<String, Type>,
    /// 结构体定义表
    pub structs: HashMap<String, StructDef>,
    /// 类型别名表
    pub type_aliases: HashMap<String, Type>,
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

    /// 查找变量的类型，未找到时返回 [`TypeError::UndefinedVariable`]。
    pub fn variable_type(&self, name: &str, span: Span) -> Result<Type, TypeError> {
        self.lookup_variable(name)
            .cloned()
            .ok_or_else(|| TypeError::UndefinedVariable {
                name: name.to_string(),
                span,
            })
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

    /// 记录一个类型别名。
    #[allow(dead_code)]
    pub fn insert_type_alias(&mut self, name: String, type_: Type) {
        self.type_aliases.insert(name, type_);
    }

    /// 解析一个具名类型（查结构体 / 别名 / 预置内置类型名）。
    ///
    /// 内置类型名（`i64`、`bool` 等）直接映射到 [`Type`]；
    /// 其余名称查结构体表与别名表，查不到时返回
    /// [`TypeError::UndefinedType`]。
    pub fn resolve_named_type(&self, name: &str, span: Span) -> Result<Type, TypeError> {
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
        Err(TypeError::UndefinedType {
            name: name.to_string(),
            span,
        })
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
