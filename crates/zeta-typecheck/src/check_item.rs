//! 顶层项检查（函数签名收集 + 函数体 / const 检查 + 模块 / use 支持
//! + enum / trait / impl 收集）。

use zeta_ast::{
    AstEnumDecl, AstFnDecl, AstImplBlock, AstItem, AstModDecl, AstProgram, AstStructDecl,
    AstTraitDecl, AstUseDecl,
};
use zeta_hir::{HirConstDecl, HirFnDecl, HirItem, HirItemKind, HirParam, HirProgram};
use zeta_lexer::Span;

use crate::check_expr::{check_block, infer_expr, resolve_ast_type};
use crate::context::{FnTemplate, TypeContext};
use crate::error::TypeError;
use crate::types::{
    EnumDef, FnSignature, ImplDef, ImplMethod, MethodSig, Mutability, StructDef, TraitDef, Type,
    VariantDef,
};

/// 类型检查完整程序。
///
/// 两遍流程：
/// 1. 收集结构体 / 枚举 / trait / impl 定义、类型别名与所有函数签名
///    （支持函数间互调），并注册 `use` 导入别名；
/// 2. 检查函数体与 const 初始值，生成 HIR（模块项以 `mod::item` 扁平化命名）。
///
/// 泛型函数 / 泛型方法在调用点实例化，实例化产生的函数项追加到输出末尾。
pub fn typecheck(program: &AstProgram) -> Result<HirProgram, TypeError> {
    let mut ctx = TypeContext::new();
    collect_declarations(&mut ctx, program)?;

    let mut items = Vec::new();
    for item in &program.items {
        check_item(&mut ctx, item, "", &mut items)?;
    }
    // 泛型实例化产生的函数项追加到末尾
    items.append(&mut ctx.mono_items);
    Ok(HirProgram { items })
}

/// 拼接模块前缀与名称（`mod::name`），顶层直接返回原名。
fn full_name(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}::{name}")
    }
}

/// 第一遍：收集结构体 / 函数签名 / use 导入（递归处理嵌套模块）。
fn collect_declarations(ctx: &mut TypeContext, program: &AstProgram) -> Result<(), TypeError> {
    for item in &program.items {
        collect_item_decls(ctx, item, "")?;
    }
    Ok(())
}

/// 收集单个项的声明（带模块前缀）。
fn collect_item_decls(
    ctx: &mut TypeContext,
    item: &AstItem,
    prefix: &str,
) -> Result<(), TypeError> {
    match item {
        AstItem::StructDecl(s) => collect_struct(ctx, s, prefix)?,
        AstItem::FnDecl(f) => {
            if !f.generics.is_empty() {
                // 泛型函数注册为模板，调用点按实参实例化
                let sig = fn_signature(ctx, f, f.span)?;
                ctx.fn_templates.insert(
                    full_name(prefix, &f.name),
                    FnTemplate {
                        name: f.name.clone(),
                        type_params: f.generics.clone(),
                        sig,
                        ast: (**f).clone(),
                    },
                );
            } else {
                let sig = fn_signature(ctx, f, f.span)?;
                ctx.insert_fn_signature(full_name(prefix, &f.name), sig);
            }
        }
        AstItem::EnumDecl(e) => collect_enum(ctx, e, prefix)?,
        AstItem::TraitDecl(t) => collect_trait(ctx, t, prefix)?,
        AstItem::ImplBlock(imp) => collect_impl(ctx, imp, prefix)?,
        AstItem::ModDecl(m) => {
            let new_prefix = full_name(prefix, &m.name);
            for inner in &m.items {
                collect_item_decls(ctx, inner, &new_prefix)?;
            }
        }
        AstItem::UseDecl(u) => register_use(ctx, u)?,
        // 收集阶段注册模块常量（供函数体 / 其它 const 引用）
        AstItem::ConstDecl(c) => {
            let (value, ty) = infer_expr(ctx, &c.value)?;
            ctx.insert_constant(full_name(prefix, &c.name), value, ty);
        }
        _ => {}
    }
    Ok(())
}

/// 注册 use 导入别名（`use path::to::item [as alias];`）。
///
/// MVP 限制：路径从根开始解析；暂不支持 glob 导入（`use a::*;`）。
fn register_use(ctx: &mut TypeContext, u: &AstUseDecl) -> Result<(), TypeError> {
    let path = u.path.join("::");
    if u.path.last().map(String::as_str) == Some("*") {
        return Err(TypeError::Unsupported {
            what: "glob 导入 use a::*".to_string(),
            span: u.span,
        });
    }
    let local = match &u.alias {
        Some(a) => a.clone(),
        None => u.path.last().cloned().unwrap_or_default(),
    };
    ctx.insert_use_alias(local, path);
    Ok(())
}

/// 第二遍：检查函数体 / const，生成 HIR 项（递归处理嵌套模块）。
pub(crate) fn check_item(
    ctx: &mut TypeContext,
    item: &AstItem,
    prefix: &str,
    out: &mut Vec<HirItem>,
) -> Result<(), TypeError> {
    match item {
        AstItem::FnDecl(f) => {
            // 泛型函数仅在调用点实例化（无调用则不生成代码）
            if !f.generics.is_empty() {
                return Ok(());
            }
            // 函数体中的裸名（函数 / 常量）优先解析到当前模块
            let old_prefix = std::mem::replace(&mut ctx.module_prefix, prefix.to_string());
            let body = check_fn_body(ctx, f)?;
            ctx.module_prefix = old_prefix;
            let params = f
                .params
                .iter()
                .map(|p| HirParam {
                    name: p.name.clone(),
                })
                .collect();
            out.push(HirItem {
                name: full_name(prefix, &f.name),
                kind: HirItemKind::Fn(HirFnDecl { params, body }),
            });
        }
        AstItem::ConstDecl(c) => {
            let old_prefix = std::mem::replace(&mut ctx.module_prefix, prefix.to_string());
            let (value, ty) = infer_expr(ctx, &c.value)?;
            ctx.module_prefix = old_prefix;
            ctx.insert_constant(full_name(prefix, &c.name), value.clone(), ty);
            out.push(HirItem {
                name: full_name(prefix, &c.name),
                kind: HirItemKind::Const(HirConstDecl { value }),
            });
        }
        AstItem::ModDecl(m) => {
            let new_prefix = full_name(prefix, &m.name);
            for inner in &m.items {
                check_item(ctx, inner, &new_prefix, out)?;
            }
        }
        // use 导入在收集阶段（第一遍）已注册；其余项 MVP 阶段不生成 HIR
        AstItem::UseDecl(_) | AstItem::StructDecl(_) | AstItem::TraitDecl(_)
        | AstItem::ImplBlock(_) | AstItem::EnumDecl(_) | AstItem::ActorDecl(_)
        | AstItem::MacroDecl(_) | AstItem::Statement(_) => {}
    }
    Ok(())
}

/// 收集结构体字段定义。
///
/// 泛型结构体（`struct Vec<T>`）在解析字段类型时，`T` 应处于类型参数作用域内
/// （与 `collect_enum` / `collect_impl` 保持一致）。
fn collect_struct(ctx: &mut TypeContext, s: &AstStructDecl, prefix: &str) -> Result<(), TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = s.generics.clone();

    let mut fields = Vec::with_capacity(s.fields.len());
    for field in &s.fields {
        let ty = resolve_ast_type(ctx, &field.type_, field.span)?;
        fields.push((field.name.clone(), ty));
    }
    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    ctx.insert_struct(full_name(prefix, &s.name), StructDef { fields });
    Ok(())
}

/// 收集枚举定义（变体 + 字段类型 + 对象槽数布局）。
fn collect_enum(ctx: &mut TypeContext, e: &AstEnumDecl, prefix: &str) -> Result<(), TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = e.generics.clone();

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
            type_params: e.generics.clone(),
            variants,
            slot_count: 1 + max_fields,
        },
    );
    Ok(())
}

/// 收集 trait 定义（抽象方法签名）。
fn collect_trait(ctx: &mut TypeContext, t: &AstTraitDecl, prefix: &str) -> Result<(), TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = t.generics.clone();

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
        });
    }

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    ctx.insert_trait(
        full_name(prefix, &t.name),
        TraitDef {
            name: t.name.clone(),
            type_params: t.generics.clone(),
            methods,
        },
    );
    Ok(())
}

/// 收集 impl 块（inherent 或 trait impl），方法保存原始 AST 供调用点实例化。
fn collect_impl(ctx: &mut TypeContext, imp: &AstImplBlock, prefix: &str) -> Result<(), TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = imp.generics.clone();

    // self 类型：`impl<T> Vec<T>` → `Named("Vec", [Generic("T")])`
    let self_type = Type::Named(
        full_name(prefix, &imp.type_name),
        imp.generics.iter().map(|g| Type::Generic(g.clone())).collect(),
    );

    let mut methods = Vec::new();
    for m in &imp.methods {
        let mut params = Vec::with_capacity(m.params.len());
        for p in &m.params {
            if p.name == "self" {
                // `&self` / `&mut self` / `self` 统一按聚合指针传递：
                // 引用形式记 Ref，值形式记 self 类型本身（MIR 层均为指针）
                let ty = match &p.type_ {
                    zeta_ast::AstType::Ref(_, is_mut) => {
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
        methods.push(ImplMethod {
            sig: MethodSig {
                name: m.name.clone(),
                params,
                return_type,
            },
            body: Some(m.clone()),
        });
    }

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    ctx.insert_impl(ImplDef {
        trait_name: imp.trait_name.clone(),
        self_type,
        type_params: imp.generics.clone(),
        methods,
    });
    Ok(())
}

/// 仅收集函数签名（不检查函数体），供增量编译提取模块接口。
///
/// 递归处理嵌套模块（符号名带 `mod::` 前缀）。输出按函数名排序，
/// 保证接口哈希稳定。
pub fn collect_fn_signatures(
    program: &AstProgram,
) -> Result<Vec<(String, FnSignature)>, TypeError> {
    let mut ctx = TypeContext::new();
    // 先收集结构体 / 枚举 / trait / impl（签名可能引用这些类型）
    for item in &program.items {
        match item {
            AstItem::StructDecl(s) => collect_struct(&mut ctx, s, "")?,
            AstItem::EnumDecl(e) => collect_enum(&mut ctx, e, "")?,
            AstItem::TraitDecl(t) => collect_trait(&mut ctx, t, "")?,
            AstItem::ImplBlock(imp) => collect_impl(&mut ctx, imp, "")?,
            AstItem::ModDecl(m) => collect_mod_types(&mut ctx, m)?,
            _ => {}
        }
    }
    let mut sigs = Vec::new();
    for item in &program.items {
        match item {
            AstItem::FnDecl(f) => {
                let sig = fn_signature(&mut ctx, f, f.span)?;
                sigs.push((f.name.clone(), sig));
            }
            AstItem::ModDecl(m) => collect_mod_fn_sigs(&mut ctx, m, &m.name, &mut sigs)?,
            _ => {}
        }
    }
    sigs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sigs)
}

/// 递归收集模块内的类型声明（结构体 / 枚举 / trait / impl）。
fn collect_mod_types(ctx: &mut TypeContext, m: &AstModDecl) -> Result<(), TypeError> {
    for inner in &m.items {
        match inner {
            AstItem::StructDecl(s) => collect_struct(ctx, s, &m.name)?,
            AstItem::EnumDecl(e) => collect_enum(ctx, e, &m.name)?,
            AstItem::TraitDecl(t) => collect_trait(ctx, t, &m.name)?,
            AstItem::ImplBlock(imp) => collect_impl(ctx, imp, &m.name)?,
            AstItem::ModDecl(inner_mod) => collect_mod_types(ctx, inner_mod)?,
            _ => {}
        }
    }
    Ok(())
}

/// 递归收集模块内的结构体。
/// 递归收集模块内的函数签名。
fn collect_mod_fn_sigs(
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

/// 从函数声明解析签名（泛型参数名在签名内解析为 [`Type::Generic`]；
/// 若上下文存在泛型替换表则解析为替换后的具体类型）。
fn fn_signature(ctx: &mut TypeContext, f: &AstFnDecl, span: Span) -> Result<FnSignature, TypeError> {
    fn_signature_with_self(ctx, f, None, span)
}

/// 解析函数 / 方法签名；`self_ty` 提供时，名为 `self` 的参数直接取该类型
/// （用于 impl 方法实例化，`&self` 的引用层级在传入前已确定）。
pub(crate) fn fn_signature_with_self(
    ctx: &mut TypeContext,
    f: &AstFnDecl,
    self_ty: Option<&Type>,
    span: Span,
) -> Result<FnSignature, TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    ctx.type_params = f.generics.clone();

    let mut params = Vec::with_capacity(f.params.len());
    for p in &f.params {
        if p.name == "self" {
            if let Some(st) = self_ty {
                params.push(st.clone());
                continue;
            }
        }
        params.push(resolve_ast_type(ctx, &p.type_, span)?);
    }
    let return_type = match &f.return_type {
        Some(t) => resolve_ast_type(ctx, t, span)?,
        None => Type::Unit,
    };

    ctx.type_params = saved_params;
    Ok(FnSignature {
        params,
        return_type,
    })
}

/// 检查函数体：参数入作用域，检查块，返回类型一致性。
fn check_fn_body(
    ctx: &mut TypeContext,
    f: &AstFnDecl,
) -> Result<Option<zeta_hir::HirBlock>, TypeError> {
    check_fn_body_with_self(ctx, f, None)
}

/// 检查函数体（支持 `self` 参数，供 impl 方法实例化使用）。
pub(crate) fn check_fn_body_with_self(
    ctx: &mut TypeContext,
    f: &AstFnDecl,
    self_ty: Option<&Type>,
) -> Result<Option<zeta_hir::HirBlock>, TypeError> {
    let body = match &f.body {
        Some(b) => b,
        None => {
            // trait 抽象方法允许无函数体
            if f.is_pub {
                return Ok(None);
            }
            return Err(TypeError::MissingFunctionBody {
                name: f.name.clone(),
                span: f.span,
            });
        }
    };

    // 参数进入局部作用域
    let saved = std::mem::take(&mut ctx.variables);
    for p in &f.params {
        let ty = if p.name == "self" && self_ty.is_some() {
            self_ty.cloned().unwrap()
        } else {
            resolve_ast_type(ctx, &p.type_, f.span)?
        };
        ctx.insert_variable(p.name.clone(), ty);
    }

    // 返回类型检查（签名在收集阶段已存入，此处重新解析以保持一致性）
    let return_type = fn_signature_with_self(ctx, f, self_ty, f.span)?.return_type;

    let (hir_body, body_ty) = check_block(ctx, body)?;

    // 返回类型一致性：函数体类型应兼容声明的返回类型
    if body_ty != Type::Never && !body_ty.compatible_with(&return_type) {
        ctx.variables = saved;
        return Err(TypeError::WrongType {
            expected: return_type.to_string(),
            found: body_ty.to_string(),
            span: f.span,
        });
    }

    ctx.variables = saved;
    Ok(Some(hir_body))
}
