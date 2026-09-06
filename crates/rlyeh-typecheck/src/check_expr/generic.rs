//! 表达式检查子模块：泛型实例化与类型统一。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use rlyeh_hir::HirExprKind;
use rlyeh_lexer::Span;
use super::*;

pub(super) fn check_generic_call(
    ctx: &mut TypeContext,
    resolved: &str,
    args: &[AstExpr],
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let template = ctx
        .fn_templates
        .get(resolved)
        .cloned()
        .ok_or_else(|| TypeError::FunctionNotFound {
            name: resolved.to_string(),
            span,
        })?;
    if args.len() != template.sig.params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: resolved.to_string(),
            expected: template.sig.params.len(),
            found: args.len(),
            span,
        });
    }

    // 由实参类型推断类型参数
    let mut subst: HashMap<String, Type> = HashMap::new();
    // P7b-2（2026-08-29）：turbofish 类型实参预填——`Bag::<i64>::new()` 无实参可
    // 推断时，turbofish 是唯一类型实参来源（`Vec::<i64>::new()` / `Channel::<T>::new()`）。
    if !type_args.is_empty() && !template.type_params.is_empty() {
        for (tp, ta) in template.type_params.iter().zip(type_args) {
            let ta_ty = resolve_ast_type(ctx, ta, span)?;
            subst.insert(tp.clone(), ta_ty);
        }
    }
    let mut hir_args = Vec::with_capacity(args.len());
    for (arg, pty) in args.iter().zip(&template.sig.params) {
        let (hir, ty) = infer_expr(ctx, arg)?;
        // 保守升级：仅形参为具体 String 时升级（`fn f<T>(s: String, x: T)` 的
        // `f("hi", 1)`）；泛型 T 位置保持 Str 值推断（`f("hi")` → T = str 字面量值），
        // 不改变既有泛型推断结果。
        let (hir, ty) = if matches!(&ty, Type::Str)
            && matches!(pty, Type::Named(n, _) if n == "String")
        {
            check_string_from(ctx, std::slice::from_ref(arg), arg.span)?
        } else {
            (hir, ty)
        };
        unify(pty, &ty, &mut subst)?;
        hir_args.push(hir);
    }

    // U3：泛型约束调用点校验（宽松：不推导，仅检查已由实参确定的类型参数）
    check_generic_bounds(ctx, &template.bounds, &subst, span)?;

    // 实例化（或命中缓存）得到具体函数名与替换后的签名
    let (fn_name, signature) = instantiate_generic_fn(ctx, resolved, &template, &subst, span)?;
    if signature.params.len() != hir_args.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: resolved.to_string(),
            expected: signature.params.len(),
            found: hir_args.len(),
            span,
        });
    }
    for (i, (arg, pty)) in args.iter().zip(&signature.params).enumerate() {
        let (_, ty) = infer_expr(ctx, arg)?;
        // Str 值实参 → 非 Str 形参自动升级（与第一个循环一致，仅取类型做兼容检查）
        let ty = if matches!(&ty, Type::Str) && !matches!(pty, Type::Str) {
            let (_, t2) = check_string_from(ctx, std::slice::from_ref(arg), arg.span)?;
            t2
        } else {
            ty
        };
        if !ty.compatible_with(pty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: resolved.to_string(),
                index: i,
                expected: pty.to_string(),
                found: ty.to_string(),
                span: arg.span,
                related: vec![],
            });
        }
    }
    Ok((
        HirExpr::new(HirExprKind::Call{
            callee: fn_name,
            args: hir_args,
        }, Span::dummy()),
        signature.return_type,
    ))
}

pub(super) fn check_generic_bounds(
    ctx: &TypeContext,
    bounds: &HashMap<String, Vec<String>>,
    subst: &HashMap<String, Type>,
    span: Span,
) -> Result<(), TypeError> {
    for (param, bound_list) in bounds {
        let concrete = match subst.get(param) {
            Some(t) => t,
            None => continue, // 未从实参确定（不推导，跳过）
        };
        for bound in bound_list {
            // 把 bound 解析为 trait_defs 的完整键：裸名可能经 import 提升到
            // 根命名空间，但 trait_defs 以模块路径（`future::Future`）注册，
            // 需按 Display 名末段匹配（与 `impl Trait for T` / `dyn Trait` 的
            // 裸名解析一致）。
            let full = resolve_trait_def_name(ctx, bound);
            if !ctx.trait_defs.contains_key(&full) {
                return Err(TypeError::UndefinedType {
                    name: bound.clone(),
                    span,
                });
            }
            if !type_implements_trait(ctx, &full, concrete) {
                return Err(TypeError::GenericBoundMismatch {
                    param: param.clone(),
                    bound: bound.clone(),
                    ty: concrete.to_string(),
                    span,
                });
            }
        }
    }
    Ok(())
}

pub(super) fn resolve_trait_def_name(ctx: &TypeContext, name: &str) -> String {
    if ctx.trait_defs.contains_key(name) {
        return name.to_string();
    }
    if let Some(full) = ctx
        .use_aliases
        .get(name)
        .filter(|f| ctx.trait_defs.contains_key(*f))
    {
        return full.clone();
    }
    ctx.trait_defs
        .keys()
        .find(|k| k.rsplit("::").next() == Some(name))
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

pub(super) fn type_implements_trait(ctx: &TypeContext, bound: &str, concrete: &Type) -> bool {
    ctx.impl_defs.iter().any(|imp| {
        if imp.trait_name.as_deref() != Some(bound) {
            return false;
        }
        let Type::Named(iname, _) = &imp.self_type else {
            return false;
        };
        match concrete {
            Type::Named(cname, _) => iname == cname,
            t => iname == &t.to_string(),
        }
    })
}

pub(super) fn instantiate_generic_fn(
    ctx: &mut TypeContext,
    resolved: &str,
    template: &FnTemplate,
    subst: &HashMap<String, Type>,
    span: Span,
) -> Result<(String, FnSignature), TypeError> {
    let key = mono_key(resolved, template, subst);
    if let Some(existing) = ctx.mono_instances.get(&key) {
        let sig = ctx.lookup_fn_signature(existing).cloned().ok_or_else(|| {
            TypeError::FunctionNotFound {
                name: existing.clone(),
                span,
            }
        })?;
        return Ok((existing.clone(), sig));
    }

    // 实例函数名：`名称__T1_T2`（可读的稳定后缀）
    let suffix = template
        .type_params
        .iter()
        .map(|tp| {
            subst
                .get(tp)
                .map(type_mono_key)
                .unwrap_or_else(|| tp.clone())
        })
        .collect::<Vec<_>>()
        .join("_");
    let mono_name = format!("{resolved}__{suffix}");

    // 克隆 AST 并替换类型参数后检查（body 内 `T` 经 generic_subst 解析）
    let mut cloned = template.ast.clone();
    cloned.name = mono_name.clone();
    cloned.generics.clear();

    // 先注册实例键，防止 body 内递归调用自身导致无限实例化
    ctx.mono_instances.insert(key, mono_name.clone());

    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = template.type_params.clone();
    ctx.generic_subst = subst.clone();

    let sig = crate::check_item::fn_signature_with_self(ctx, &cloned, None, span)?;
    let body = crate::check_item::check_fn_body_with_self(ctx, &cloned, None)?;

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;

    let params = cloned
        .params
        .iter()
        .map(|p| HirParam { span: Span::dummy(),
            name: p.name.clone(),
            is_ref: matches!(&p.type_, rlyeh_ast::AstType::Ref(..)),
        })
        .collect();
    ctx.mono_items.push(HirItem {
        span: crate::DUMMY_SPAN,
        name: mono_name.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params,
            body,
            is_extern: false,
            extern_sig: None,
        }),
    });
    ctx.insert_fn_signature(mono_name.clone(), sig.clone());
    Ok((mono_name, sig))
}

pub(super) fn trait_default_method(
    ctx: &TypeContext,
    impl_def: &ImplDef,
    method: &str,
) -> Option<crate::types::ImplMethod> {
    let trait_name = impl_def.trait_name.as_ref()?;
    let trait_def = ctx.trait_defs.get(trait_name)?;
    let sig = trait_def
        .methods
        .iter()
        .find(|m| m.name == method)?
        .clone();
    // 仅当 trait 方法带默认实现 body 时才回退
    let body = sig.default_body.clone()?;
    Some(crate::types::ImplMethod {
        sig,
        body: Some(body),
    })
}

pub(super) fn instantiate_impl_method(
    ctx: &mut TypeContext,
    impl_def: &ImplDef,
    method_def: &crate::types::ImplMethod,
    subst: &HashMap<String, Type>,
    span: Span,
) -> Result<String, TypeError> {
    let Type::Named(base_name, _) = &impl_def.self_type else {
        return Err(TypeError::Unsupported {
            what: "impl 目标类型必须为具名类型".to_string(),
            span,
        });
    };
    // X4：trait impl 方法符号带 trait 区分（`Point::fmt`（Display）与
    // `Point::fmt`（Debug）同名共存）——trait 名如 `fmt::Debug` 转为
    // `Point::fmt__fmt::Debug`，避免同名方法符号碰撞。
    let base_fn = match &impl_def.trait_name {
        Some(tn) => format!("{base_name}::{}__{tn}", method_def.sig.name),
        None => format!("{base_name}::{}", method_def.sig.name),
    };
    let body_ast = method_def.body.clone().ok_or_else(|| TypeError::Unsupported {
        what: "无函数体的抽象方法被调用".to_string(),
        span,
    })?;
    // U7：impl 泛型（T）+ 方法泛型（U）的确定值并入后缀——方法泛型 U 已在
    // `check_method_call`（7244-7250）并入 subst，但原先 mono 键/后缀只含
    // impl 泛型，不同 U 调用（`bar<i64>` vs `bar<str>`）产生相同键 → 缓存命中
    // 复用首次实例，U 被错误绑定。此处把方法泛型参数并入后缀/键，消除碰撞。
    let mut mono_parts: Vec<String> = impl_def
        .type_params
        .iter()
        .map(|tp| {
            subst
                .get(tp)
                .map(type_mono_key)
                .unwrap_or_else(|| tp.clone())
        })
        .collect();
    // A2（SH-P1-1，2026-09-02）：trait 类型实参须并入 mono 键 / 后缀。
    // 同一 self_type 上同一泛型 trait 的多个 impl（如 `impl Wrap<i64> for W` 与
    // `impl Wrap<bool> for W`）此前仅含 impl 泛型参数（此处为空），产生相同
    // mono 键 → `mono_instances` 缓存命中复用首个实例，后续 impl 被错误复用
    // （方法体 / 接收者错配，表现为跨调用结果异常）。`impl<T> Wrap<T> for Pair<T>`
    // 之类则由 self_type 的 T 实例化后使键不同，天然无碰撞。
    for ta in &impl_def.trait_type_args {
        mono_parts.push(type_mono_key(&substitute(ta, subst)));
    }
    for mp in &body_ast.generics {
        mono_parts.push(
            subst
                .get(&mp.name)
                .map(type_mono_key)
                .unwrap_or_else(|| mp.name.clone()),
        );
    }
    let suffix = mono_parts.join("_");
    let mono_name = if suffix.is_empty() {
        base_fn.clone()
    } else {
        format!("{base_fn}__{suffix}")
    };
    let key = format!("{base_fn}#{suffix}");
    if ctx.mono_instances.contains_key(&key) {
        return Ok(mono_name);
    }

    let mut cloned = body_ast.clone();
    cloned.name = mono_name.clone();
    cloned.generics.clear();

    ctx.mono_instances.insert(key, mono_name.clone());

    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    let saved_assoc = std::mem::take(&mut ctx.assoc_types);
    ctx.type_params = impl_def.type_params.clone();
    // V3-D2（2026-08-27）：方法级泛型参数（`fn map<B>`）加入 type_params，使
    // 方法体检查时泛型参数（如返回 `Map<Self, B>` 的 `B`）可解析（此前仅 impl
    // 泛型参数，方法级泛型 `B` 在 body 中报 `undefined type B`）。
    for mp in &body_ast.generics {
        if !ctx.type_params.contains(&mp.name) {
            ctx.type_params.push(mp.name.clone());
        }
    }
    ctx.generic_subst = subst.clone();
    // 关联类型映射（U2）：`Self::Item` 签名重解析 / 方法体检查时替换为
    // impl 定义的具体类型（经泛型替换）。
    ctx.assoc_types = impl_def
        .assoc_types
        .iter()
        .map(|(n, t)| (n.clone(), substitute(t, subst)))
        .collect();

    // self 参数类型：impl 方法签名的首个参数（`&self` 层级已含）经替换。
    // 静态方法（无 self 参数）传 None。
    // V3 trait 默认方法回退：trait 默认方法的 `self` 参数类型是 `Generic("Self")`
    // 占位，须替换为 impl 目标具体类型（与 `ctx.self_type` 一致），否则方法体内
    // `self.next()` 等调用无法在占位上解析（`find_impl_for_method` 找不到）。
    let self_param = method_def.sig.params.first().map(|p| {
        if matches!(p, Type::Generic(n) if n == "Self") {
            substitute(&impl_def.self_type, subst)
        } else {
            substitute(p, subst)
        }
    });
    // U4：方法体检查时 `Self` 类型解析为 impl 目标类型（经泛型替换）
    let saved_self = ctx.self_type.clone();
    ctx.self_type = Some(substitute(&impl_def.self_type, subst));
    let sig = crate::check_item::fn_signature_with_self(ctx, &cloned, self_param.as_ref(), span)?;
    let body = crate::check_item::check_fn_body_with_self(ctx, &cloned, self_param.as_ref())?;
    ctx.self_type = saved_self;

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    ctx.assoc_types = saved_assoc;

    let params = cloned
        .params
        .iter()
        .map(|p| HirParam { span: Span::dummy(),
            name: p.name.clone(),
            is_ref: matches!(&p.type_, rlyeh_ast::AstType::Ref(..)),
        })
        .collect();
    ctx.mono_items.push(HirItem {
        span: crate::DUMMY_SPAN,
        name: mono_name.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params,
            body,
            is_extern: false,
            extern_sig: None,
        }),
    });
    ctx.insert_fn_signature(mono_name.clone(), sig);
    Ok(mono_name)
}

pub(super) fn mono_key(
    resolved: &str,
    template: &FnTemplate,
    subst: &HashMap<String, Type>,
) -> String {
    let args = template
        .type_params
        .iter()
        .map(|tp| {
            subst
                .get(tp)
                .map(type_mono_key)
                .unwrap_or_else(|| tp.clone())
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{resolved}#{args}")
}

pub(super) fn unify(
    param: &Type,
    arg: &Type,
    subst: &mut HashMap<String, Type>,
) -> Result<(), TypeError> {
    match param {
        Type::Generic(tp) => {
            subst.insert(tp.clone(), arg.clone());
            Ok(())
        }
        Type::Named(pn, ps) => {
            if let Type::Named(an, as_) = arg {
                if pn == an {
                    for (p, a) in ps.iter().zip(as_.iter()) {
                        unify(p, a, subst)?;
                    }
                }
            }
            Ok(())
        }
        Type::Ref(inner, _) => {
            if let Type::Ref(ainner, _) = arg {
                unify(inner, ainner, subst)?;
            }
            Ok(())
        }
        // G3：裸指针与引用同理递归统一内层（泛型结构体字段含 `*const T` /
        // `*mut T` 时，可从裸指针实参反推泛型参数，例如 `KVRef { key: p }`
        // 由 `p: *const K` 定型 `KVRef<K, _>`）。
        Type::RawPtr(inner, _) => {
            if let Type::RawPtr(ainner, _) = arg {
                unify(inner, ainner, subst)?;
            }
            Ok(())
        }
        // T1a：函数类型递归统一（fn 形参含泛型类型参数时按实参 fn 签名反推，
        // 与 substitute 的 Fn 递归替换配套——此前静默接受不反推，导致
        // `Vec::sort_by(cmp: fn(T, T) -> i64)` 传 `fn(i64, i64) -> i64` 报错）。
        Type::Fn(sig) => {
            if let Type::Fn(asig) = arg {
                for (p, a) in sig.params.iter().zip(asig.params.iter()) {
                    unify(p, a, subst)?;
                }
                unify(&sig.return_type, &asig.return_type, subst)?;
            }
            Ok(())
        }
        Type::Tuple(ts) => {
            if let Type::Tuple(ats) = arg {
                for (p, a) in ts.iter().zip(ats.iter()) {
                    unify(p, a, subst)?;
                }
            }
            Ok(())
        }
        // 未定型类型参数（`_`）：用实参类型替换 subst 中所有 Infer 条目。
        // 场景：裸 `Result::Err(7).unwrap_or(100)` —— 接收者 unified 后
        // `T → Infer`、`E → i64`，实参 100 将 T 定型为 i64。
        Type::Infer => {
            for v in subst.values_mut() {
                if matches!(v, Type::Infer) {
                    *v = arg.clone();
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn contains_infer(ty: &Type) -> bool {
    match ty {
        Type::Infer => true,
        Type::Named(_, ps) => ps.iter().any(contains_infer),
        Type::Ref(inner, _) => contains_infer(inner),
        Type::Tuple(ts) => ts.iter().any(contains_infer),
        Type::Array(inner, _) => contains_infer(inner),
        _ => false,
    }
}

/// 类型是否包含名为 `names` 之一的泛型参数（`Type::Generic`）。
/// A2：判断方法预期参数类型中是否还存在未绑定的 impl / 方法级泛型参数，
/// 以决定能否由对应实参类型反推绑定。
pub(super) fn contains_generic_named(ty: &Type, names: &[String]) -> bool {
    match ty {
        Type::Generic(n) => names.contains(n),
        Type::Named(_, ps) => ps.iter().any(|p| contains_generic_named(p, names)),
        Type::Ref(inner, _) => contains_generic_named(inner, names),
        Type::RawPtr(inner, _) => contains_generic_named(inner, names),
        Type::Fn(sig) => {
            sig.params.iter().any(|p| contains_generic_named(p, names))
                || contains_generic_named(&sig.return_type, names)
        }
        Type::Tuple(ts) => ts.iter().any(|p| contains_generic_named(p, names)),
        Type::Array(inner, _) => contains_generic_named(inner, names),
        _ => false,
    }
}
