//! 表达式检查子模块：符号解析与关联投影。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use super::*;

pub(super) fn resolve_callable(ctx: &TypeContext, name: &str) -> String {
    // `r#` 原始标识符：显式引用根命名空间（如 fs 模块内 `r#rename(...)`
    // 引用根 extern `rename`，不被 `fs::rename` 遮蔽）。去前缀后跳过
    // 模块内优先 / 用户遮蔽，直接绑定根命名空间符号（注册名已归一化）。
    if let Some(base) = name.strip_prefix("r#") {
        return base.to_string();
    }
    // 模块内函数引用：优先绑定当前模块内定义（`prefix::name`），
    // 避免子模块内部裸名调用被全局同名函数（含用户顶层覆盖）劫持。
    // 例：std `io.rl` 内部 `read(0, tmp, 256)` 必须绑定 `io::read`，
    // 用户顶层 `fn read(p: &i64)` 只影响用户自己的裸名调用。
    if !name.contains("::") && !ctx.module_prefix.is_empty() {
        let full = format!("{}::{}", ctx.module_prefix, name);
        if ctx.fn_signatures.contains_key(&full) {
            return full;
        }
    }
    // 顶层用户代码：被遮蔽的用户声明优先（std 原名保留供模块内部绑定）
    if !name.contains("::") && ctx.module_prefix.is_empty() {
        if let Some(m) = ctx.fn_shadow_of.get(name) {
            return m.clone();
        }
    }
    if ctx.fn_signatures.contains_key(name) {
        return name.to_string();
    }
    // 兜底：模块前缀 + 裸名（模块内自由函数互相引用）
    if !name.contains("::") && !ctx.module_prefix.is_empty() {
        let full = format!("{}::{}", ctx.module_prefix, name);
        if ctx.fn_signatures.contains_key(&full) {
            return full;
        }
    }
    ctx.use_aliases
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

pub(super) fn split_variant_path(ctx: &TypeContext, resolved: &str) -> Option<(String, String)> {
    if let Some((en, vr)) = resolved.rsplit_once("::") {
        let en_full = ctx
            .resolve_full_name(en)
            .unwrap_or_else(|| en.to_string());
        if ctx.enum_defs.contains_key(&en_full) {
            return Some((en_full, vr.to_string()));
        }
    }
    ctx.resolve_variant(None, resolved)
}

pub(crate) fn resolve_ast_type(
    ctx: &TypeContext,
    ty: &AstType,
    span: Span,
) -> Result<Type, TypeError> {
    match ty {
        AstType::Path(name, args) => {
            // `Self::Item`：关联类型引用（U2）——impl 收集时查当前 assoc_types
            // 映射替换为具体类型；trait 声明收集时（无 impl 上下文）退化为
            // 占位 `Type::Generic("Self::Item")`（仅作记录，不参与实例化替换）。
            if let Some(member) = name.strip_prefix("Self::") {
                if let Some(t) = ctx.assoc_types.get(member) {
                    return Ok(t.clone());
                }
                return Ok(Type::Generic(name.clone()));
            }
            // W4 补全：泛型参数关联类型投影 `F::Output`（base 为当前泛型参数时）。
            // 产出 `Type::AssocProjection`；实例化时 base 已替换为具体类型则立即
            // 求值（查该类型的 trait impl 的关联类型），否则保留投影待实例化替换。
            if args.is_empty() {
                if let Some((base, member)) = name.rsplit_once("::") {
                    // base 为泛型参数：模板收集期 type_params 含 base；实例化期
                    // `cloned.generics` 被清空（type_params=[]）但 generic_subst 含 base
                    //（instantiate_generic_fn 克隆后解析签名），两者任一命中即视为投影。
                    if ctx.type_params.iter().any(|p| p == base)
                        || ctx.generic_subst.contains_key(base)
                    {
                        return resolve_assoc_projection(ctx, base, member, span);
                    }
                }
            }
            // `str`：字符串类型关键字（`&str` 引用切片类型的一部分；G2）
            if name == "str" && args.is_empty() {
                return Ok(Type::Str);
            }
            // 带泛型实参的类型也解析完整名（use 别名 / 模块前缀），与 args 为空时
            // 一致（S1 修复：`Poll<i64>` 注解与 `Poll::Ready` 构造的路径一致性——
            // 裸名不做别名展开会产生 `Poll` vs `future::Poll` 的错配）。
            if args.is_empty() {
                ctx.resolve_named_type(name, span)
            } else {
                // 带泛型实参的类型也解析完整名（use 别名 / 模块前缀），与 args 为空
                // 时一致（S1 修复：`Poll<i64>` 注解与 `Poll::Ready` 构造的路径一致性——
                // 裸名不做别名展开会产生 `Poll` vs `future::Poll` 的错配）。
                let full = ctx
                    .resolve_full_name(name)
                    .unwrap_or_else(|| name.to_string());
                let mut resolved = Vec::with_capacity(args.len());
                for a in args {
                    resolved.push(resolve_ast_type(ctx, a, span)?);
                }
                Ok(Type::Named(full, resolved))
            }
        }
        AstType::Ref(inner, is_mut) => {
            let inner = resolve_ast_type(ctx, inner, span)?;
            let m = if *is_mut {
                Mutability::Mutable
            } else {
                Mutability::Immutable
            };
            Ok(Type::Ref(Box::new(inner), m))
        }
        AstType::RawPtr(inner, is_mut) => {
            let inner = resolve_ast_type(ctx, inner, span)?;
            Ok(Type::RawPtr(Box::new(inner), *is_mut))
        }
        // trait 对象（H4）：`dyn Trait` → `Type::Dyn(完整 trait 名)`。
        // 布局为 2 槽胖指针（数据指针 + vtable 指针），转换与调用见
        // `coerce_to_dyn` / `check_method_call` 的 Dyn 分支。
        AstType::Dyn(name) => {
            let full = if ctx.trait_defs.contains_key(name) {
                name.clone()
            } else {
                // use 导入别名（`use shape::Shape` 后 `dyn Shape`）
                ctx.use_aliases
                    .get(name)
                    .filter(|f| ctx.trait_defs.contains_key(*f))
                    .cloned()
                    .unwrap_or_else(|| name.clone())
            };
            if !ctx.trait_defs.contains_key(&full) {
                return Err(TypeError::UndefinedType {
                    name: name.clone(),
                    span,
                });
            }
            Ok(Type::Dyn(full))
        }
        AstType::Tuple(ts) => {
            let mut resolved = Vec::with_capacity(ts.len());
            for t in ts {
                resolved.push(resolve_ast_type(ctx, t, span)?);
            }
            Ok(Type::Tuple(resolved))
        }
        AstType::Array(inner, size) => {
            let inner = resolve_ast_type(ctx, inner, span)?;
            // MVP：数组大小仅支持整数字面量（`[T; N]`）
            let n = match size {
                Some(e) => match *e.kind {
                    ExprKind::IntLiteral(v) if v >= 0 => v as usize,
                    _ => {
                        return Err(TypeError::Unsupported {
                            what: "数组大小非常量整数字面量在 MVP 阶段".to_string(),
                            span,
                        })
                    }
                },
                None => {
                    return Err(TypeError::Unsupported {
                        what: "数组类型缺少大小 `[T; N]`".to_string(),
                        span,
                    })
                }
            };
            Ok(Type::Array(Box::new(inner), n))
        }
        AstType::Fn(params, ret) => {
            let params = params
                .iter()
                .map(|p| resolve_ast_type(ctx, p, span))
                .collect::<Result<Vec<_>, _>>()?;
            let ret = resolve_ast_type(ctx, ret, span)?;
            Ok(Type::Fn(Box::new(FnSignature {
                params,
                return_type: ret,
            })))
        }
        AstType::Infer => Ok(Type::Infer),
    }
}

pub(super) fn resolve_assoc_projection(
    ctx: &TypeContext,
    base: &str,
    member: &str,
    _span: Span,
) -> Result<Type, TypeError> {
    if let Some(concrete) = ctx.generic_subst.get(base) {
        if let Some(t) = eval_assoc_projection(ctx, concrete, member) {
            return Ok(t);
        }
        // base 已实例化但找不到该类型的关联类型实现：保留投影（防御性，
        // 通常实例化前必能求值）。
        return Ok(Type::AssocProjection {
            base: Box::new(concrete.clone()),
            assoc: member.to_string(),
        });
    }
    Ok(Type::AssocProjection {
        base: Box::new(Type::Generic(base.to_string())),
        assoc: member.to_string(),
    })
}

pub(super) fn eval_assoc_projection(ctx: &TypeContext, base: &Type, member: &str) -> Option<Type> {
    // U8 补全：按 self_type 名匹配（忽略泛型实参——泛型 impl `impl<T> Future
    // for GenOut<T>` 的 self_type 实参为 `Generic("T")`，与具体实例实参 `String`
    // 不相等，`compatible_with` 会误判不匹配），再用 base 具体实参替换 assoc_types
    // 里的 `Generic("T")` 占位求值。
    let Type::Named(base_name, base_args) = base else {
        // base 非 Named（引用等）：退回按名 + 宽松匹配
        for imp in &ctx.impl_defs {
            if imp.self_type.compatible_with(base) {
                for (name, ty) in &imp.assoc_types {
                    if name == member {
                        return Some(ty.clone());
                    }
                }
            }
        }
        return None;
    };
    for imp in &ctx.impl_defs {
        let Type::Named(impl_name, self_args) = &imp.self_type else {
            continue;
        };
        if impl_name != base_name {
            continue;
        }
        // 绑泛型参数：self_args[i] = Generic(tp) → base_args[i]
        let mut subst: HashMap<String, Type> = HashMap::new();
        for (i, tp) in imp.type_params.iter().enumerate() {
            if i < self_args.len()
                && i < base_args.len()
                && self_args[i] == Type::Generic(tp.clone())
            {
                subst.insert(tp.clone(), base_args[i].clone());
            }
        }
        for (name, ty) in &imp.assoc_types {
            if name == member {
                let resolved = if subst.is_empty() {
                    ty.clone()
                } else {
                    substitute(ty, &subst)
                };
                return Some(resolved);
            }
        }
    }
    None
}
