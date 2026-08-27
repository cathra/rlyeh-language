//! 表达式检查子模块：misc。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

pub fn builtin_signature(name: &str) -> Option<(Vec<Type>, Type)> {
    let dyn_arr = || Type::Array(Box::new(Type::Infer), 0);
    match name {
        "print" | "println" | "eprint" | "eprintln" => Some((vec![Type::Infer], Type::Unit)),
        "alloc_array" => Some((vec![Type::I64], dyn_arr())),
        "array_copy" => Some((vec![dyn_arr(), dyn_arr(), Type::I64], Type::Unit)),
        "array_free" => Some((vec![dyn_arr()], Type::Unit)),
        // String 动态缓冲（按字节）：
        "alloc_bytes" => Some((vec![Type::I64], Type::Array(Box::new(Type::U8), 0))),
        "copy_bytes" => Some((vec![Type::Infer, Type::Infer, Type::I64], Type::Unit)),
        // String 内容相等：`bytes_eq(a, b, n)` → `memcmp(a, b, n) == 0`
        "bytes_eq" => Some((vec![Type::Infer, Type::Infer, Type::I64], Type::Bool)),
        // String 字典序：`bytes_cmp(a, b, n)` → `memcmp(a, b, n)` 有符号扩展为 i64
        // （负/零/正 → 小于/等于/大于；前缀相等时长度兜底由 desugar 层处理）
        "bytes_cmp" => Some((vec![Type::Infer, Type::Infer, Type::I64], Type::I64)),
        // String 打印：`print_string` / `println_string` 接收 String 对象指针
        "print_string" | "println_string" => Some((vec![Type::Infer], Type::Unit)),
        // HashMap 键散列：Knuth 乘法混合散列（MVP 仅支持整数键），返回非负散列值
        "hash_value" => Some((vec![Type::Infer], Type::I64)),
        _ => None,
    }
}

pub(crate) fn coerce_to_dyn(
    ctx: &mut TypeContext,
    data_ptr: HirExpr,
    concrete: &Type,
    trait_name: &str,
    span: Span,
) -> Result<HirExpr, TypeError> {
    let trait_def = ctx.trait_defs.get(trait_name).cloned().ok_or_else(|| {
        TypeError::UndefinedType {
            name: trait_name.to_string(),
            span,
        }
    })?;
    if !trait_def.type_params.is_empty() {
        return Err(TypeError::Unsupported {
            what: format!("`dyn {trait_name}`：泛型 trait 实例化（trait 对象泛型参数规划中）"),
            span,
        });
    }
    // 找 `impl Trait for 具体类型`（MVP：trait/impl 均非泛型，直接按 self_type 精确匹配）
    let impl_def = ctx
        .impl_defs
        .iter()
        .find(|d| d.trait_name.as_deref() == Some(trait_name) && d.self_type == *concrete)
        .cloned()
        .ok_or_else(|| TypeError::Unsupported {
            what: format!(
                "类型 `{concrete}` 未实现 trait `{trait_name}`，无法转换为 `dyn {trait_name}`"
            ),
            span,
        })?;
    if !impl_def.type_params.is_empty() {
        return Err(TypeError::Unsupported {
            what: format!("`dyn {trait_name}`：泛型 impl（`impl<T> {trait_name} for ...`）规划中"),
            span,
        });
    }
    let n = trait_def.methods.len();
    let mut stmts = Vec::new();
    // 1) vtable 数组：3 元槽（drop/size/align，MVP = 0）+ N 方法槽
    let vt = ctx.fresh_temp();
    stmts.push(HirStmt::Let {
        name: vt.clone(),
        init: HirExpr::Alloc {
            slots: 3 + n,
            by_value: false,
            is_strfat: false,
        },
        mutable: false,
    });
    for i in 0..3 {
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(vt.clone())),
            index: i,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }));
    }
    // 2) 方法表：按 trait 方法声明顺序填充具体 impl 方法函数指针
    let subst = HashMap::new();
    for (i, m) in trait_def.methods.iter().enumerate() {
        let impl_method = impl_def.methods.iter().find(|im| im.sig.name == m.name).ok_or_else(|| {
            TypeError::Unsupported {
                what: format!(
                    "`{trait_name}` 的 impl for `{concrete}` 缺少方法 `{}`",
                    m.name
                ),
                span,
            }
        })?;
        let fn_name = instantiate_impl_method(ctx, &impl_def, impl_method, &subst, span)?;
        let m_var = ctx.fresh_temp();
        stmts.push(HirStmt::Let {
            name: m_var.clone(),
            init: HirExpr::FnPtr(fn_name),
            mutable: false,
        });
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(vt.clone())),
            index: 3 + i,
            value: Box::new(HirExpr::Variable(m_var)),
            ty: FieldScalar::Ptr,
        }));
    }
    // 3) 胖指针：槽 0 = 数据指针，槽 1 = vtable 指针
    let dyn_var = ctx.fresh_temp();
    stmts.push(HirStmt::Let {
        name: dyn_var.clone(),
        init: HirExpr::Alloc {
            slots: 2,
            by_value: false,
            is_strfat: false,
        },
        mutable: false,
    });
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(dyn_var.clone())),
        index: 0,
        value: Box::new(data_ptr),
        ty: FieldScalar::Ptr,
    }));
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(dyn_var.clone())),
        index: 1,
        value: Box::new(HirExpr::Variable(vt)),
        ty: FieldScalar::Ptr,
    }));
    Ok(HirExpr::Block(Box::new(HirBlock {
        stmts,
        final_expr: Some(HirExpr::Variable(dyn_var)),
    })))
}

pub(crate) fn type_mentions_self(ty: &Type) -> bool {
    match ty {
        // `Self::Item` 关联类型占位（trait 声明收集时无 impl 上下文，
        // 退化为 `Generic("Self::Item")`）同样视为 `Self` 提及。
        Type::Generic(n) => n == "Self" || n.starts_with("Self::"),
        Type::Ref(t, _) | Type::RawPtr(t, _) | Type::Array(t, _) => type_mentions_self(t),
        Type::Named(_, ps) | Type::Tuple(ps) => ps.iter().any(type_mentions_self),
        _ => false,
    }
}
