//! 表达式检查子模块：collect。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

pub(crate) fn collect_struct(ctx: &mut TypeContext, s: &AstStructDecl, prefix: &str) -> Result<(), TypeError> {
    ctx.insert_struct(
        full_name(prefix, &s.name),
        StructDef {
            fields: Vec::new(),
            type_params: s.generics.iter().map(|p| p.name.clone()).collect(),
        },
    );
    Ok(())
}

pub(crate) fn resolve_all_struct_fields(ctx: &mut TypeContext, program: &AstProgram) -> Result<(), TypeError> {
    resolve_struct_fields_items(ctx, &program.items, "")
}

pub(crate) fn resolve_struct_fields_items(
    ctx: &mut TypeContext,
    items: &[AstItem],
    prefix: &str,
) -> Result<(), TypeError> {
    for item in items {
        match item {
            AstItem::StructDecl(s) => resolve_struct_fields(ctx, s, prefix)?,
            AstItem::ModDecl(m) => {
                let new_prefix = full_name(prefix, &m.name);
                let old_prefix = std::mem::replace(&mut ctx.module_prefix, new_prefix.clone());
                resolve_struct_fields_items(ctx, &m.items, &new_prefix)?;
                ctx.module_prefix = old_prefix;
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn resolve_struct_fields(
    ctx: &mut TypeContext,
    s: &AstStructDecl,
    prefix: &str,
) -> Result<(), TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = s.generics.iter().map(|p| p.name.clone()).collect();

    let mut fields = Vec::with_capacity(s.fields.len());
    for field in &s.fields {
        let ty = resolve_ast_type(ctx, &field.type_, field.span)?;
        // H4 MVP 限制：`dyn Trait` 暂不支持作为 struct 字段（2 槽胖指针字段布局规划中）
        if matches!(&ty, Type::Dyn(_)) {
            return Err(TypeError::Unsupported {
                what: format!("`{ty}` 作为 struct 字段（H4 MVP 仅支持局部变量绑定）"),
                span: field.span,
            });
        }
        fields.push((field.name.clone(), ty));
    }
    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    let full = full_name(prefix, &s.name);
    if let Some(def) = ctx.structs.get_mut(&full) {
        def.fields = fields;
    }
    Ok(())
}

pub(crate) fn collect_enum(ctx: &mut TypeContext, e: &AstEnumDecl, prefix: &str) -> Result<(), TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = e.generics.iter().map(|p| p.name.clone()).collect();

    let mut variants = Vec::with_capacity(e.variants.len());
    let mut max_fields = 0usize;
    for (tag, v) in e.variants.iter().enumerate() {
        let mut fields = Vec::new();
        // 元组负载字段（`Some(T)`）以 `f0/f1/...` 命名
        for (i, t) in v.tuple_fields.iter().enumerate() {
            let ty = resolve_ast_type(ctx, t, v.span)?;
            fields.push((format!("f{i}"), ty));
        }
        // 命名负载字段（`Variant { x: T }`）
        for sf in &v.struct_fields {
            let ty = resolve_ast_type(ctx, &sf.type_, sf.span)?;
            fields.push((sf.name.clone(), ty));
        }
        max_fields = max_fields.max(fields.len());
        variants.push(VariantDef {
            name: v.name.clone(),
            fields,
            tag,
        });
    }

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    ctx.insert_enum(
        full_name(prefix, &e.name),
        EnumDef {
            name: e.name.clone(),
            type_params: e.generics.iter().map(|p| p.name.clone()).collect(),
            variants,
            slot_count: 1 + max_fields,
        },
    );
    Ok(())
}

pub(crate) fn collect_trait(ctx: &mut TypeContext, t: &AstTraitDecl, prefix: &str) -> Result<(), TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = t.generics.iter().map(|p| p.name.clone()).collect();

    let mut methods = Vec::new();
    for m in &t.methods {
        let mut params = Vec::with_capacity(m.params.len());
        for p in &m.params {
            if p.name == "self" {
                // trait 方法签名中 `self` 用占位类型，具体类型由 impl 决定
                params.push(Type::Generic("Self".to_string()));
                continue;
            }
            params.push(resolve_ast_type(ctx, &p.type_, m.span)?);
        }
        let return_type = match &m.return_type {
            Some(rt) => resolve_ast_type(ctx, rt, m.span)?,
            None => Type::Unit,
        };
        methods.push(MethodSig {
            name: m.name.clone(),
            params,
            return_type,
            // V3 trait 默认方法：trait 方法带 body（`fn f(...) { ... }`）时保留其
            // 完整方法 AST（含签名与默认实现体）；`impl Trait for X` 未实现该方法
            // 时回退（`check_method_call`）。抽象方法（无 body）为 `None`。
            default_body: m.body.is_some().then(|| m.clone()),
        });
    }

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    ctx.insert_trait(
        full_name(prefix, &t.name),
        TraitDef {
            name: t.name.clone(),
            type_params: t.generics.iter().map(|p| p.name.clone()).collect(),
            assoc_types: t.types.clone(),
            methods,
        },
    );
    Ok(())
}

pub(crate) fn collect_impl(ctx: &mut TypeContext, imp: &AstImplBlock, prefix: &str) -> Result<(), TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = imp.generics.iter().map(|p| p.name.clone()).collect();

    // self 类型：`impl<T> Vec<T>` → `Named("Vec", [Generic("T")])`
    let self_type = Type::Named(
        full_name(prefix, &imp.type_name),
        imp.generics
            .iter()
            .map(|p| Type::Generic(p.name.clone()))
            .collect(),
    );
    // trait 名解析为完整符号名（与 `dyn Trait` 解析一致）：
    // 1) 显式 `mod::Trait` 路径原样使用；2) 当前模块前缀下存在（`impl Trait` 定义于 trait 同模块内）；
    // 3) 顶层已定义；4) use 导入别名；5) 原样回退。inherent impl（无 trait）保持 None。
    let trait_name = match &imp.trait_name {
        Some(tn) if tn.contains("::") => Some(tn.clone()),
        Some(tn) if !prefix.is_empty()
            && ctx
                .trait_defs
                .contains_key(&format!("{}::{}", prefix, tn)) =>
        {
            Some(format!("{}::{}", prefix, tn))
        }
        Some(tn) if ctx.trait_defs.contains_key(tn) => Some(tn.clone()),
        Some(tn) => Some(
            ctx.use_aliases
                .get(tn)
                .cloned()
                .unwrap_or_else(|| tn.clone()),
        ),
        None => None,
    };

    // 关联类型定义：`type Item = Concrete;`（U2）。先解析（此时 assoc_types
    // 为空，`Self::Item` 自引用退化为占位 `Generic("Self::Item")`），再填充
    // 映射供方法签名中的 `Self::Item` 替换。
    let assoc_types = imp
        .types
        .iter()
        .map(|(n, ty)| Ok((n.clone(), resolve_ast_type(ctx, ty, imp.span)?)))
        .collect::<Result<Vec<(String, Type)>, TypeError>>()?;
    let saved_assoc = std::mem::take(&mut ctx.assoc_types);
    ctx.assoc_types = assoc_types.iter().cloned().collect();

    // U4：方法签名内 `Self` 解析为 impl 目标类型（static 方法同样适用）
    let saved_self = ctx.self_type.clone();
    ctx.self_type = Some(self_type.clone());

    let mut methods = Vec::new();
    for m in &imp.methods {
        // U7：方法泛型参数并入类型参数作用域（`fn map<U>(..)` 的 U 可解析），
        // 解析签名后恢复 impl 泛型上下文。
        let saved_mparams = std::mem::take(&mut ctx.type_params);
        let mut tparams = imp.generics.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
        for gp in &m.generics {
            tparams.push(gp.name.clone());
        }
        ctx.type_params = tparams;
        let mut params = Vec::with_capacity(m.params.len());
        for p in &m.params {
            if p.name == "self" {
                // `&self` / `&mut self` / `self` 统一按聚合指针传递：
                // 引用形式记 Ref，值形式记 self 类型本身（MIR 层均为指针）
                let ty = match &p.type_ {
                    rlyeh_ast::AstType::Ref(_, is_mut) => {
                        let m = if *is_mut {
                            Mutability::Mutable
                        } else {
                            Mutability::Immutable
                        };
                        Type::Ref(Box::new(self_type.clone()), m)
                    }
                    _ => self_type.clone(),
                };
                params.push(ty);
            } else {
                params.push(resolve_ast_type(ctx, &p.type_, p.span)?);
            }
        }
        let return_type = match &m.return_type {
            Some(rt) => resolve_ast_type(ctx, rt, m.span)?,
            None => Type::Unit,
        };
        ctx.type_params = saved_mparams;
        methods.push(ImplMethod {
            sig: MethodSig {
                name: m.name.clone(),
                params,
                return_type,
                default_body: None,
            },
            body: Some(m.clone()),
        });
    }

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    ctx.assoc_types = saved_assoc;
    ctx.self_type = saved_self;
    ctx.insert_impl(ImplDef {
        trait_name,
        self_type,
        type_params: imp.generics.iter().map(|p| p.name.clone()).collect(),
        bounds: imp
            .generics
            .iter()
            .filter(|p| !p.bounds.is_empty())
            .map(|p| (p.name.clone(), p.bounds.clone()))
            .collect(),
        assoc_types,
        methods,
    });
    Ok(())
}

pub(crate) fn collect_mod_types(ctx: &mut TypeContext, m: &AstModDecl) -> Result<(), TypeError> {
    collect_mod_types_inner(ctx, m, "")
}

pub(crate) fn collect_mod_types_inner(
    ctx: &mut TypeContext,
    m: &AstModDecl,
    prefix: &str,
) -> Result<(), TypeError> {
    let new_prefix = full_name(prefix, &m.name);
    // Q3a：与 `collect_item_decls` 的 ModDecl 分支一致，模块内短名解析须感知
    // 模块前缀（`fmt/module.rl` 的 `trait Display { fn fmt(&self, f: &mut Formatter) }`
    // 等——collect_impl/collect_trait 收集阶段即 resolve_ast_type，use 段未注册）。
    let old_prefix = std::mem::replace(&mut ctx.module_prefix, new_prefix.clone());
    for inner in &m.items {
        match inner {
            AstItem::StructDecl(s) => collect_struct(ctx, s, &new_prefix)?,
            AstItem::EnumDecl(e) => collect_enum(ctx, e, &new_prefix)?,
            AstItem::TraitDecl(t) => collect_trait(ctx, t, &new_prefix)?,
            AstItem::ImplBlock(imp) => collect_impl(ctx, imp, &new_prefix)?,
            AstItem::ModDecl(inner_mod) => collect_mod_types_inner(ctx, inner_mod, &new_prefix)?,
            AstItem::UseDecl(u) => register_use(ctx, u)?,
            _ => {}
        }
    }
    ctx.module_prefix = old_prefix;
    Ok(())
}

pub(crate) fn collect_mod_fn_sigs(
    ctx: &mut TypeContext,
    m: &AstModDecl,
    prefix: &str,
    sigs: &mut Vec<(String, FnSignature)>,
) -> Result<(), TypeError> {
    for inner in &m.items {
        match inner {
            AstItem::FnDecl(f) => {
                let sig = fn_signature(ctx, f, f.span)?;
                sigs.push((full_name(prefix, &f.name), sig));
            }
            AstItem::ModDecl(inner_mod) => {
                let new_prefix = full_name(prefix, &inner_mod.name);
                collect_mod_fn_sigs(ctx, inner_mod, &new_prefix, sigs)?;
            }
            _ => {}
        }
    }
    Ok(())
}
