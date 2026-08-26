//! 顶层项检查（函数签名收集 + 函数体 / const 检查 + 模块 / use 支持
//! + enum / trait / impl 收集）。

use rlyeh_ast::{
    AstActorDecl, AstEnumDecl, AstFnDecl, AstImplBlock, AstItem, AstModDecl, AstProgram,
    AstStructDecl, AstTraitDecl, AstUseDecl,
};
use rlyeh_hir::{
    FieldScalar, HirBinaryOp, HirBlock, HirConstDecl, HirExpr, HirFnDecl, HirItem, HirItemKind,
    HirParam, HirProgram, HirStmt,
};
use rlyeh_lexer::Span;

use crate::check_expr::{
    check_block, check_block_inner, fix_deferred_closure_with_sig, infer_expr, resolve_ast_type,
    try_closure_value_as_fn,
};
use crate::context::{FnTemplate, TypeContext};
use crate::error::TypeError;
use crate::types::{
    field_scalar_of, EnumDef, FnSignature, ImplDef, ImplMethod, MethodSig, Mutability, StructDef,
    TraitDef, Type, VariantDef,
};

/// 类型检查完整程序。
///
/// 两遍流程：
/// 1. 收集结构体 / 枚举 / trait / impl 定义、类型别名与所有函数签名
///    （支持函数间互调），并注册 `import` 导入别名；
/// 2. 检查函数体与 const 初始值，生成 HIR（模块项以 `mod::item` 扁平化命名）。
///
/// 泛型函数 / 泛型方法在调用点实例化，实例化产生的函数项追加到输出末尾。
pub fn typecheck(program: &AstProgram) -> Result<HirProgram, TypeError> {
    typecheck_with_region_hints(program, &Default::default())
}

/// 类型检查完整程序，并注入 L3 PGO 回灌提示（区域名 → 推荐初始容量）。
///
/// 无提示时等价于 [`typecheck`]；`rlyeh build --profile` 编译路径使用本入口。
pub fn typecheck_with_region_hints(
    program: &AstProgram,
    region_hints: &std::collections::HashMap<String, usize>,
) -> Result<HirProgram, TypeError> {
    let mut ctx = TypeContext::new();
    ctx.region_hints = region_hints.clone();
    collect_declarations(&mut ctx, program)?;
    // 第二遍：所有 struct 名注册后解析字段（支持自引用/前向引用递归类型）。
    resolve_all_struct_fields(&mut ctx, program)?;

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
                        type_params: f.generics.iter().map(|p| p.name.clone()).collect(),
                        bounds: f
                            .generics
                            .iter()
                            .filter(|p| !p.bounds.is_empty())
                            .map(|p| (p.name.clone(), p.bounds.clone()))
                            .collect(),
                        sig,
                        ast: (**f).clone(),
                    },
                );
            } else {
                let sig = fn_signature(ctx, f, f.span)?;
                let full = full_name(prefix, &f.name);
                // 顶层裸名与已注册函数（std 预置根函数 / 用户先声明者）重名：
                // 签名不同 → 后声明者 mangle 重命名（`read@shadow<N>`），先声明者保留原名。
                // std 根函数（如 extern `read`）被用户顶层 `fn read` 遮蔽时，
                // std 原名留在表中供 std 模块内部裸名调用绑定；用户顶层代码
                // 经 `fn_shadow_of` 绑定自身版本；下游符号名因此唯一不冲突。
                // 签名相同 → 视为无害重声明（FFI 惯用法：用户重复 extern 声明
                // 同一符号，如 `extern fn __rlyeh_target_os() -> i32`），保留原名。
                if prefix.is_empty()
                    && ctx.fn_signatures.get(&full).is_some_and(|existing| existing != &sig)
                {
                    let seq = ctx.fn_shadow_seq.get(&f.name).copied().unwrap_or(0) + 1;
                    ctx.fn_shadow_seq.insert(f.name.clone(), seq);
                    let mangled = format!("{}@shadow{}", f.name, seq);
                    ctx.fn_decl_shadow.insert((f.span.start, f.span.end), mangled.clone());
                    ctx.fn_shadow_of.insert(f.name.clone(), mangled.clone());
                    ctx.insert_fn_signature(mangled, sig);
                } else {
                    ctx.insert_fn_signature(full, sig);
                }
            }
        }
        AstItem::EnumDecl(e) => collect_enum(ctx, e, prefix)?,
        AstItem::TraitDecl(t) => collect_trait(ctx, t, prefix)?,
        AstItem::ImplBlock(imp) => collect_impl(ctx, imp, prefix)?,
        AstItem::ActorDecl(a) => collect_actor(ctx, a, prefix)?,
        AstItem::ModDecl(m) => {
            let new_prefix = full_name(prefix, &m.name);
            // Q3a 修复：模块内符号（struct/trait/impl）的短名解析须感知模块前缀。
            // 模块内 trait/impl 方法签名在收集阶段即 resolve_ast_type（如
            // `fmt/module.rl` 的 `trait Display { fn fmt(&self, f: &mut Formatter) }`），
            // 此时文件后部的 use 段尚未注册 use_aliases，须按 `mod::Name` 前缀回退。
            let old_prefix = std::mem::replace(&mut ctx.module_prefix, new_prefix.clone());
            for inner in &m.items {
                collect_item_decls(ctx, inner, &new_prefix)?;
            }
            ctx.module_prefix = old_prefix;
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

/// 将 typecheck 类型序列化为 extern 签名类型名（LIR 侧解析为 `LirType`）。
/// MVP 仅支持标量类型与单元类型；引用参数视为标量指针（按值传递数字）。
pub(crate) fn type_to_extern_name(ty: &Type) -> String {
    match ty {
        Type::I8 => "i8".into(),
        Type::I16 => "i16".into(),
        Type::I32 => "i32".into(),
        Type::I64 => "i64".into(),
        Type::ISize => "isize".into(),
        Type::U8 => "u8".into(),
        Type::U16 => "u16".into(),
        Type::U32 => "u32".into(),
        Type::U64 => "u64".into(),
        Type::USize => "usize".into(),
        Type::F32 => "f32".into(),
        Type::F64 => "f64".into(),
        Type::Bool => "bool".into(),
        Type::Char => "char".into(),
        Type::Unit => "()".into(),
        Type::Ref(inner, _) => format!("&{inner}"),
        t => t.to_string(),
    }
}

/// 注册 use 导入别名（`use path::to::item [as alias];`）。
///
/// MVP 限制：路径从根开始解析；暂不支持 glob 导入（`use a::*;`）。
fn register_use(ctx: &mut TypeContext, u: &AstUseDecl) -> Result<(), TypeError> {
    // `r#` 前缀（关键字转义 / 根命名空间显式引用标记）在符号注册时归一化，
    // 与 extern 声明注册名保持一致（`use r#rename` → 目标 "rename"）。
    let norm = |s: &str| s.strip_prefix("r#").unwrap_or(s).to_string();
    let path = u
        .path
        .iter()
        .map(|s| norm(s))
        .collect::<Vec<String>>()
        .join("::");
    if u.path.last().map(String::as_str) == Some("*") {
        return Err(TypeError::Unsupported {
            what: "glob 导入 use a::*".to_string(),
            span: u.span,
        });
    }
    let local = match &u.alias {
        Some(a) => norm(a),
        None => u.path.last().map(|s| norm(s)).unwrap_or_default(),
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
    // K4：GC 运行时 extern 声明（程序级去重，有 item 即生成；
    // `Gc::new` / `gc_region` 依赖，未使用也无 harm——LLVM declare 未引用符号不报错）
    emit_gc_runtime_externs(ctx, out);
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
            // extern 声明：序列化签名（参数类型名 + 返回类型名）供 LIR 解析
            let extern_sig = if f.is_extern {
                let sig = crate::check_item::fn_signature_with_self(ctx, f, None, f.span)?;
                Some((
                    sig.params
                        .iter()
                        .map(type_to_extern_name)
                        .collect::<Vec<_>>(),
                    type_to_extern_name(&sig.return_type),
                ))
            } else {
                None
            };
            // 被遮蔽重命名的声明按收集阶段的记录使用 mangle 名（两遍按 Span 对齐），
            // 保证 HIR 符号名与 fn_signatures / 调用解析结果一致。
            let item_name = ctx
                .fn_decl_shadow
                .get(&(f.span.start, f.span.end))
                .cloned()
                .unwrap_or_else(|| full_name(prefix, &f.name));
            out.push(HirItem {
                name: item_name,
                kind: HirItemKind::Fn(HirFnDecl {
                    params,
                    body,
                    is_extern: f.is_extern,
                    extern_sig,
                }),
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
            // 与收集阶段一致：模块内短名解析感知模块前缀（Q3a）
            let old_prefix = std::mem::replace(&mut ctx.module_prefix, new_prefix.clone());
            for inner in &m.items {
                check_item(ctx, inner, &new_prefix, out)?;
            }
            ctx.module_prefix = old_prefix;
        }
        // actor：展开为状态初始化函数 + 方法函数 + dispatch handle + runtime extern 声明
        AstItem::ActorDecl(a) => expand_actor(ctx, a, prefix, out)?,
        // use 导入在收集阶段（第一遍）已注册；其余项 MVP 阶段不生成 HIR
        AstItem::UseDecl(_) | AstItem::StructDecl(_) | AstItem::TraitDecl(_)
        | AstItem::ImplBlock(_) | AstItem::EnumDecl(_)
        | AstItem::MacroDecl(_) | AstItem::Statement(_) => {}
    }
    Ok(())
}

/// 收集结构体字段定义。
///
/// 泛型结构体（`struct Vec<T>`）在解析字段类型时，`T` 应处于类型参数作用域内
/// （与 `collect_enum` / `collect_impl` 保持一致）。
/// 收集 struct 声明（第一遍：仅注册名与泛型参数，字段类型延迟到
/// `resolve_all_struct_fields` 第二遍解析——支持 struct 自引用 / 前向引用
/// （如 `struct Node { next: Box<Node> }` 递归类型，为 W6 递归 async fn
/// 提供类型层地基）。
fn collect_struct(ctx: &mut TypeContext, s: &AstStructDecl, prefix: &str) -> Result<(), TypeError> {
    ctx.insert_struct(
        full_name(prefix, &s.name),
        StructDef {
            fields: Vec::new(),
            type_params: s.generics.iter().map(|p| p.name.clone()).collect(),
        },
    );
    Ok(())
}

/// 第二遍：解析所有 struct 字段类型（所有 struct 名已注册，支持自引用/前向引用）。
///
/// 在 `collect_declarations` 完成后、`check_item` 之前调用。遍历顶层与嵌套
/// 模块的 struct 声明，重建 `type_params`/`module_prefix` 上下文后解析字段。
fn resolve_all_struct_fields(ctx: &mut TypeContext, program: &AstProgram) -> Result<(), TypeError> {
    resolve_struct_fields_items(ctx, &program.items, "")
}

fn resolve_struct_fields_items(
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

fn resolve_struct_fields(
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

/// 收集 actor 定义（查重 + 字段/方法 MVP 限制校验 + 注册）。
fn collect_actor(ctx: &mut TypeContext, a: &AstActorDecl, prefix: &str) -> Result<(), TypeError> {
    let full = full_name(prefix, &a.name);
    if ctx.structs.contains_key(&full)
        || ctx.enum_defs.contains_key(&full)
        || ctx.trait_defs.contains_key(&full)
        || ctx.actors.contains_key(&full)
    {
        return Err(TypeError::Unsupported {
            what: format!("重复定义 `{full}`（已存在同名 struct/enum/trait/actor）"),
            span: a.span,
        });
    }

    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);

    // 字段：类型必须为 MVP 标量（i64/f64/bool/char），且必须有默认值（spawn 时初始化状态）
    for f in &a.fields {
        let ty = resolve_ast_type(ctx, &f.type_, f.span)?;
        if !is_actor_scalar_type(&ty) {
            return Err(TypeError::Unsupported {
                what: format!(
                    "actor 字段类型 {}（MVP 阶段仅支持标量 i64/f64/bool/char）",
                    ty
                ),
                span: f.span,
            });
        }
        if f.default.is_none() {
            return Err(TypeError::Unsupported {
                what: format!("actor 字段 `{}` 缺少默认值（spawn 时按默认值初始化状态）", f.name),
                span: f.span,
            });
        }
    }

    // 方法：参数 ≤ 3（对应消息槽 a/b/c）且类型为 i64（MVP 消息槽整数协议）；
    // 必须显式声明返回类型（MVP 为 i64，保证 dispatch handle 返回类型统一）
    for m in &a.methods {
        if m.params.len() > 3 {
            return Err(TypeError::Unsupported {
                what: format!(
                    "actor 方法 `{}` 参数超过 3 个（MVP 阶段消息协议仅 3 个参数槽）",
                    m.name
                ),
                span: m.span,
            });
        }
        for p in &m.params {
            let ty = resolve_ast_type(ctx, &p.type_, p.span)?;
            if !matches!(ty, Type::I64) {
                return Err(TypeError::Unsupported {
                    what: format!(
                        "actor 方法参数类型 {}（MVP 阶段消息协议仅支持 i64 参数）",
                        ty
                    ),
                    span: p.span,
                });
            }
        }
        let ret_ty = match &m.return_type {
            Some(rt) => resolve_ast_type(ctx, rt, m.span)?,
            None => {
                return Err(TypeError::Unsupported {
                    what: format!(
                        "actor 方法 `{}` 缺少返回类型（MVP 阶段必须声明 `-> i64`）",
                        m.name
                    ),
                    span: m.span,
                })
            }
        };
        if !matches!(ret_ty, Type::I64) {
            return Err(TypeError::Unsupported {
                what: format!(
                    "actor 方法返回类型 {}（MVP 阶段仅支持 i64）",
                    ret_ty
                ),
                span: m.span,
            });
        }
    }

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    ctx.actors.insert(full, a.clone());
    Ok(())
}

/// MVP 阶段 actor 字段允许的标量类型。
fn is_actor_scalar_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::F64 | Type::Bool | Type::Char
    )
}

/// actor 展开（check 阶段）：生成状态初始化函数 + 方法函数 + dispatch handle
/// 与 `rlyeh_actor_*` runtime extern 声明，全部为普通 HirItem，
/// 下游 MIR / LIR / codegen 复用现有机制。
fn expand_actor(
    ctx: &mut TypeContext,
    a: &AstActorDecl,
    prefix: &str,
    out: &mut Vec<HirItem>,
) -> Result<(), TypeError> {
    let actor_full = full_name(prefix, &a.name);
    let state_new = format!("{actor_full}::__state_new");
    let handle = format!("{actor_full}::__handle");

    // 1. runtime extern 声明（程序级去重）
    emit_actor_runtime_externs(ctx, out);

    // 2. 字段槽布局：名称 → 标量种类（状态结构体 = 槽数组）
    let mut slots: Vec<(String, FieldScalar)> = Vec::new();
    let mut field_tys: Vec<(String, Type)> = Vec::new();
    for f in &a.fields {
        let ty = resolve_ast_type(ctx, &f.type_, f.span)?;
        let scalar = field_scalar_of(&ty);
        slots.push((f.name.clone(), scalar));
        field_tys.push((f.name.clone(), ty));
    }

    // 3. 状态初始化函数 `<actor>::__state_new() -> i64`
    //    `let __s = alloc(N); set(__s, 0, v0); ...; __s`
    //    U1：函数边界作用域（隔离，与调用方变量环境互不可见）。
    ctx.push_scope(true);
    let mut stmts = vec![HirStmt::Let {
        name: "__s".to_string(),
        init: HirExpr::Alloc {
            slots: slots.len(),
            by_value: false,
            is_strfat: false,
        },
        mutable: true,
    }];
    for (idx, f) in a.fields.iter().enumerate() {
        let (v_hir, v_ty) = infer_expr(ctx, f.default.as_ref().unwrap())?;
        if !v_ty.compatible_with(&field_tys[idx].1) {
            return Err(TypeError::WrongType {
                expected: field_tys[idx].1.to_string(),
                found: v_ty.to_string(),
                span: f.span,
            });
        }
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable("__s".to_string())),
            index: idx,
            value: Box::new(v_hir),
            ty: slots[idx].1,
        }));
    }
    ctx.pop_scope();
    out.push(HirItem {
        name: state_new.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params: vec![],
            body: Some(HirBlock {
                stmts,
                final_expr: Some(HirExpr::Variable("__s".to_string())),
            }),
            is_extern: false,
            extern_sig: None,
        }),
    });

    // 4. 方法函数 `<actor>::__m<i>(self, p0, p1, p2) -> i64`
    //    签名固定 4 个 i64 参数（self = 状态指针 + 3 个消息槽），
    //    handle 按位置传参；body 内 `self` 绑定状态指针、字段访问走 actor 分支。
    for (i, m) in a.methods.iter().enumerate() {
        let m_name = format!("{actor_full}::__m{i}");
        let mut params = vec![HirParam {
            name: "self".to_string(),
        }];
        for p in &m.params {
            params.push(HirParam {
                name: p.name.clone(),
            });
        }
        for j in params.len()..4 {
            params.push(HirParam {
                name: format!("__p{j}"),
            });
        }
        let body = check_actor_method_body(ctx, m, &actor_full)?;
        out.push(HirItem {
            name: m_name,
            kind: HirItemKind::Fn(HirFnDecl {
                params,
                body: Some(body),
                is_extern: false,
                extern_sig: None,
            }),
        });
    }

    // 5. dispatch handle `<actor>::__handle(self, kind, a, b, c) -> i64`
    //    按方法索引分发；未命中（kind 越界）返回 -1 = u64::MAX 崩溃信号。
    out.push(HirItem {
        name: handle.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params: (0..5)
                .map(|j| HirParam {
                    name: ["self", "kind", "a", "b", "c"][j].to_string(),
                })
                .collect(),
            body: Some(HirBlock {
                stmts: vec![],
                final_expr: Some(build_actor_dispatch(ctx, a, &actor_full, 0)),
            }),
            is_extern: false,
            extern_sig: None,
        }),
    });

    let _ = &state_new;
    Ok(())
}

/// 递归构造 dispatch：`if kind == i { return m_i(self, a, b, c); } else { ... }`，
/// 方法耗尽时尾表达式为 `-1`（未命中方法 → 崩溃信号 u64::MAX）。
fn build_actor_dispatch(
    _ctx: &TypeContext,
    a: &AstActorDecl,
    actor_full: &str,
    idx: usize,
) -> HirExpr {
    if idx >= a.methods.len() {
        return HirExpr::IntLiteral(-1);
    }
    let m_name = format!("{actor_full}::__m{idx}");
    let call = HirExpr::Call {
        callee: m_name,
        args: vec![
            HirExpr::Variable("self".to_string()),
            HirExpr::Variable("a".to_string()),
            HirExpr::Variable("b".to_string()),
            HirExpr::Variable("c".to_string()),
        ],
    };
    let then_block = HirBlock {
        stmts: vec![HirStmt::Semi(HirExpr::Return(Some(Box::new(call))))],
        final_expr: None,
    };
    // else 分支以递归 if 为块尾表达式（值传递），
    // 底层 `IntLiteral(-1)` 必须经 final_expr 产出，否则该路径无值 → MIR 生成 `ret void`。
    let else_block = HirBlock {
        stmts: vec![],
        final_expr: Some(build_actor_dispatch(_ctx, a, actor_full, idx + 1)),
    };
    HirExpr::If {
        cond: Box::new(HirExpr::Binary(
            HirBinaryOp::Eq,
            Box::new(HirExpr::Variable("kind".to_string())),
            Box::new(HirExpr::IntLiteral(idx as i128)),
        )),
        then_block: Box::new(then_block),
        else_block: Some(Box::new(else_block)),
    }
}

/// 检查 actor 方法体：`self` 隐式绑定状态指针（类型为 actor 名），
/// 参数入作用域，检查块，返回类型一致性（方法与生成函数均返回 i64）。
fn check_actor_method_body(
    ctx: &mut TypeContext,
    m: &AstFnDecl,
    actor_full: &str,
) -> Result<rlyeh_hir::HirBlock, TypeError> {
    // U1：函数边界作用域（隔离：方法体内看不到调用方变量）。
    ctx.push_scope(true);
    ctx.insert_variable(
        "self".to_string(),
        Type::Named(actor_full.to_string(), vec![]),
    );
    for p in &m.params {
        let ty = resolve_ast_type(ctx, &p.type_, m.span)?;
        ctx.insert_variable(p.name.clone(), ty);
    }

    // actor 方法必须有函数体（extern 声明不适用）
    let body = m.body.as_ref().ok_or_else(|| TypeError::MissingFunctionBody {
        name: format!("{actor_full}::{}", m.name),
        span: m.span,
    })?;
    let (hir_body, body_ty) = check_block(ctx, body)?;

    // 返回类型一致性（collect 阶段已限定 i64）
    let return_type = fn_signature_with_self(ctx, m, None, m.span)?.return_type;
    if body_ty != Type::Never && !body_ty.compatible_with(&return_type) {
        ctx.pop_scope();
        return Err(TypeError::WrongType {
            expected: return_type.to_string(),
            found: body_ty.to_string(),
            span: m.span,
        });
    }

    ctx.pop_scope();
    Ok(hir_body)
}

/// 生成 `rlyeh_actor_*` runtime extern 声明（程序级去重，多 actor 只生成一份）。
fn emit_actor_runtime_externs(ctx: &mut TypeContext, out: &mut Vec<HirItem>) {
    let specs: &[(&str, &[&str], &str)] = &[
        ("rlyeh_actor_spawn", &["String", "i64"], "i64"),
        // supervised 的 factory 是符号名字符串（runtime 内部 dlsym 解析）
        ("rlyeh_actor_spawn_supervised", &["String", "String", "i64"], "i64"),
        ("rlyeh_actor_ask", &["i64", "i64", "i64", "i64", "i64"], "i64"),
        // send 返回 i32（runtime 消息 ID）：extern_ret32 标记 → `declare i32` + sext
        ("rlyeh_actor_send", &["i64", "i64", "i64", "i64", "i64"], "i32"),
        ("rlyeh_actor_stop", &["i64"], "i64"),
        ("rlyeh_actor_shutdown", &[], "i64"),
    ];
    for (name, args, ret) in specs {
        if !ctx.generated_actor_externs.insert((*name).to_string()) {
            continue;
        }
        // 用户源码已显式声明同名 extern → 跳过（避免 LLVM 重复 declare），
        // 且用户声明已注册进函数表，源码内的显式调用可正常解析。
        if ctx.lookup_fn_signature(name).is_some() {
            continue;
        }
        out.push(HirItem {
            name: (*name).to_string(),
            kind: HirItemKind::Fn(HirFnDecl {
                params: args
                    .iter()
                    .enumerate()
                    .map(|(i, _)| HirParam {
                        name: format!("__a{i}"),
                    })
                    .collect(),
                body: None,
                is_extern: true,
                extern_sig: Some((
                    args.iter().map(|s| s.to_string()).collect(),
                    (*ret).to_string(),
                )),
            }),
        });
    }
}

/// 生成 `rlyeh_gc_*` runtime extern 声明（程序级去重，K4 追踪 GC）。
///
/// 类型名：`Ptr` 经 LIR `parse_extern_type` 解析为指针（未知名默认 Ptr）；
/// `rlyeh_gc_alloc` 返回堆块基址（指针），`rlyeh_gc_escape` 接收 Gc 对象指针。
fn emit_gc_runtime_externs(ctx: &mut TypeContext, out: &mut Vec<HirItem>) {
    let specs: &[(&str, &[&str], &str)] = &[
        ("rlyeh_gc_alloc", &["i64"], "Ptr"),
        ("rlyeh_gc_region_begin", &[], "()"),
        ("rlyeh_gc_escape", &["Ptr"], "()"),
        ("rlyeh_gc_collect", &[], "()"),
    ];
    for (name, args, ret) in specs {
        if !ctx.generated_gc_externs.insert((*name).to_string()) {
            continue;
        }
        // 用户源码已显式声明同名 extern → 跳过（避免 LLVM 重复 declare）
        if ctx.lookup_fn_signature(name).is_some() {
            continue;
        }
        out.push(HirItem {
            name: (*name).to_string(),
            kind: HirItemKind::Fn(HirFnDecl {
                params: args
                    .iter()
                    .enumerate()
                    .map(|(i, _)| HirParam { name: format!("__a{i}") })
                    .collect(),
                body: None,
                is_extern: true,
                extern_sig: Some((
                    args.iter().map(|s| s.to_string()).collect(),
                    (*ret).to_string(),
                )),
            }),
        });
    }
}

/// 收集枚举定义（变体 + 字段类型 + 对象槽数布局）。
fn collect_enum(ctx: &mut TypeContext, e: &AstEnumDecl, prefix: &str) -> Result<(), TypeError> {
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

/// 收集 trait 定义（抽象方法签名）。
fn collect_trait(ctx: &mut TypeContext, t: &AstTraitDecl, prefix: &str) -> Result<(), TypeError> {
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

/// 收集 impl 块（inherent 或 trait impl），方法保存原始 AST 供调用点实例化。
fn collect_impl(ctx: &mut TypeContext, imp: &AstImplBlock, prefix: &str) -> Result<(), TypeError> {
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

/// 仅收集函数签名（不检查函数体），供增量编译提取模块接口。
///
/// 递归处理嵌套模块（符号名带 `mod::` 前缀）。输出按函数名排序，
/// 保证接口哈希稳定。
pub fn collect_fn_signatures(
    program: &AstProgram,
) -> Result<Vec<(String, FnSignature)>, TypeError> {
    // S1c：async/await 状态机 desugar——接口签名须反映 desugar 后的真实签名
    // （async fn 实际签名返回 `__Fut_X`，且用户类型可能引用生成项）。
    // 与 typecheck_source 挂载点保持一致，clone 后处理，不修改调用方持有的 AST。
    let mut program = program.clone();
    rlyeh_desugar::desugar_program(&mut program).map_err(|e| TypeError::Unsupported {
        what: e.to_string(),
        span: e.span(),
    })?;
    let program = &program;
    let mut ctx = TypeContext::new();
    // 先收集结构体 / 枚举 / trait / impl / use 导入别名（签名可能引用这些类型）
    for item in &program.items {
        match item {
            AstItem::StructDecl(s) => collect_struct(&mut ctx, s, "")?,
            AstItem::EnumDecl(e) => collect_enum(&mut ctx, e, "")?,
            AstItem::TraitDecl(t) => collect_trait(&mut ctx, t, "")?,
            AstItem::ImplBlock(imp) => collect_impl(&mut ctx, imp, "")?,
            AstItem::ModDecl(m) => collect_mod_types(&mut ctx, m)?,
            AstItem::UseDecl(u) => register_use(&mut ctx, u)?,
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
///
/// 模块前缀与 `collect_item_decls` 一致地**逐级累积**（`io::error::IoErrorKind`），
/// 否则 `impl` 方法签名在收集时经 `resolve_ast_type` 解析 `mod::Type` 引用会
/// 因注册名缺前缀（`error::IoErrorKind`）而报 UndefinedType（S1 修复，2026-08）。
fn collect_mod_types(ctx: &mut TypeContext, m: &AstModDecl) -> Result<(), TypeError> {
    collect_mod_types_inner(ctx, m, "")
}

fn collect_mod_types_inner(
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
    ctx.type_params = f.generics.iter().map(|p| p.name.clone()).collect();

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
    // H4 MVP 限制：`dyn Trait` 为 2 槽胖指针，暂不支持作为函数/方法参数与返回值
    // （LIR 参数/返回为标量槽，无法表达胖指针；局部变量 + vtable 调用为主路径）。
    for p in &params {
        if matches!(p, Type::Dyn(_)) {
            return Err(TypeError::Unsupported {
                what: format!(
                    "`{}` 作为函数/方法参数（H4 MVP 仅支持 `let d: dyn Trait = &obj;` 局部变量）",
                    p
                ),
                span,
            });
        }
    }
    if matches!(&return_type, Type::Dyn(_)) {
        return Err(TypeError::Unsupported {
            what: format!(
                "`{return_type}` 作为函数/方法返回类型（H4 MVP 仅支持局部变量绑定）"
            ),
            span,
        });
    }

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
) -> Result<Option<rlyeh_hir::HirBlock>, TypeError> {
    check_fn_body_with_self(ctx, f, None)
}

/// 检查函数体（支持 `self` 参数，供 impl 方法实例化使用）。
pub(crate) fn check_fn_body_with_self(
    ctx: &mut TypeContext,
    f: &AstFnDecl,
    self_ty: Option<&Type>,
) -> Result<Option<rlyeh_hir::HirBlock>, TypeError> {
    let body = match &f.body {
        Some(b) => b,
        None => {
            // trait 抽象方法 / extern 声明允许无函数体
            if f.is_pub || f.is_extern {
                return Ok(None);
            }
            return Err(TypeError::MissingFunctionBody {
                name: f.name.clone(),
                span: f.span,
            });
        }
    };

    // 参数进入局部作用域：函数边界（隔离，防止本函数体局部变量名污染
    // 调用方——尤其 std 方法体同名变量会覆盖调用方 `local_inits` 的字面量
    // 绑定追踪，导致 `String::from(s)` / Str 值升级查不到绑定内容）
    ctx.push_scope(true);
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

    // `fn make() -> fn(i64) -> i64 { |x| x + 1 }`：返回类型为 fn 且函数体
    // 尾表达式为闭包时，按 H2 无捕获闭包签名检查（H5 补全，返回闭包的函数）。
    // U1：函数体不额外开块作用域（`check_block_inner`），体内部 let 直接留在
    // 函数 fn 层——尾表达式处理（延迟闭包固化、返回闭包签名检查等）仍需
    // 访问体内变量（如 `let base = 10; let f = |x| x + base; f` 的 base）。
    // 函数结束时 fn 层随 `pop_scope` 整体弹出（隔离调用方环境）。
    let (mut hir_body, body_ty) = if matches!(&return_type, Type::Fn(_))
        && body
            .final_expr
            .as_ref()
            .is_some_and(|e| matches!(&*e.kind, rlyeh_ast::ExprKind::Closure { .. }))
    {
        check_block_inner(ctx, body, Some(&return_type))?
    } else {
        check_block_inner(ctx, body, None)?
    };

    // 返回类型一致性：函数体类型应兼容声明的返回类型
    if body_ty != Type::Never && !body_ty.compatible_with(&return_type) {
        // 无捕获闭包值 → fn 指针降级（H5 补全）：
        // `fn make() -> fn(i64) -> i64 { let f = |x: i64| x + 1; f }`——
        // 函数体尾表达式为闭包值变量，闭包对象与 fn 返回类型不兼容，但
        // 零捕获闭包值等价于 fn 指针（调用展开为空字段读取），签名匹配时
        // 将尾表达式替换为 `FnPtr(__closure_N)`。
        let mut downgraded = false;
        if let Type::Fn(sig) = &return_type {
            // 已固化无捕获闭包值 → fn 指针降级
            if let Some((fp_hir, fp_ty)) = try_closure_value_as_fn(&body_ty) {
                if fp_ty.compatible_with(&return_type) && hir_body.final_expr.is_some() {
                    hir_body.final_expr = Some(fp_hir);
                    downgraded = true;
                }
            }
            // 未固化延迟闭包值 → 按返回签名固化
            // （`fn make() -> fn(..) { let f = |x| ..; f }`：参数类型由返回
            // 签名给出，无捕获时降级为 fn 指针；捕获非空报 Unsupported）
            if !downgraded
                && matches!(&body_ty, Type::Closure { fn_name, .. } if fn_name.is_empty())
            {
                let var_name = hir_body.final_expr.as_ref().and_then(|fe| match fe {
                    HirExpr::Variable(n) => Some(n.to_string()),
                    _ => None,
                });
                if let Some(n) = var_name {
                    let (fp_hir, fp_ty) = fix_deferred_closure_with_sig(ctx, &n, sig, f.span)?;
                    if fp_ty.compatible_with(&return_type) {
                        hir_body.final_expr = Some(fp_hir);
                        downgraded = true;
                    }
                }
            }
        }
        if !downgraded {
            ctx.pop_scope();
            return Err(TypeError::WrongType {
                expected: return_type.to_string(),
                found: body_ty.to_string(),
                span: f.span,
            });
        }
    }

    ctx.pop_scope();
    Ok(Some(hir_body))
}
