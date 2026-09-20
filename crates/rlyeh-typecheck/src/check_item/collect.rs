//! 表达式检查子模块：collect。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;
use rlyeh_ast::AstTypeAlias;

pub(crate) fn collect_struct(ctx: &mut TypeContext, s: &AstStructDecl, prefix: &str) -> Result<(), TypeError> {
    ctx.insert_struct(
        full_name(prefix, &s.name),
        StructDef {
            fields: Vec::new(),
            field_spans: Vec::new(),
            type_params: s.generics.iter().map(|p| p.name.clone()).collect(),
            repr_c: s.repr_c,
        },
    );
    Ok(())
}

/// 收集类型别名：解析目标类型并登记到 `type_aliases` / `generic_aliases`。
/// 泛型别名的目标类型保留 `Type::Generic` 占位，于使用点经 `substitute` 按实参展开。
pub(crate) fn collect_type_alias(
    ctx: &mut TypeContext,
    ta: &AstTypeAlias,
    prefix: &str,
) -> Result<(), TypeError> {
    let full = full_name(prefix, &ta.name);
    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = ta.generics.iter().map(|p| p.name.clone()).collect();
    let target = resolve_ast_type(ctx, &ta.target, ta.span)?;
    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    if ta.generics.is_empty() {
        ctx.insert_type_alias(full.clone(), target);
    } else {
        ctx.insert_generic_type_alias(
            full.clone(),
            ta.generics.iter().map(|p| p.name.clone()).collect(),
            target,
        );
    }
    if ta.is_pub {
        ctx.pub_symbols.insert(full);
    }
    Ok(())
}

/// 收集作用域内全部类型别名（不动点循环以支持别名→别名前向引用）。
///
/// 必须在结构体 / 枚举 / protocol / impl / 模块等类型名登记之后调用，
/// 故别名目标类型可引用这些已登记类型；别名之间若互为前向引用，则经多轮
/// 重试解析——目标尚为未登记别名时 `resolve_ast_type` 报 `UndefinedType`，
/// 视为待定、下一轮再试，直至收敛。仍无法解析者（引用不存在类型 / 循环别名）
/// 在末轮显式报错。
pub(crate) fn collect_type_aliases_pass(
    ctx: &mut TypeContext,
    items: &[AstItem],
    prefix: &str,
) -> Result<(), TypeError> {
    let aliases: Vec<&AstTypeAlias> = items
        .iter()
        .filter_map(|it| if let AstItem::TypeAlias(ta) = it { Some(&**ta) } else { None })
        .collect();
    if aliases.is_empty() {
        return Ok(());
    }
    let mut pending: Vec<&AstTypeAlias> = aliases;
    let mut progress = true;
    while progress {
        progress = false;
        let mut next = Vec::new();
        for ta in pending {
            match collect_type_alias(ctx, ta, prefix) {
                Ok(()) => progress = true,
                // 前向引用（目标别名尚未登记）→ 下一轮重试
                Err(TypeError::UndefinedType { .. }) => next.push(ta),
                Err(e) => return Err(e),
            }
        }
        pending = next;
        if !progress {
            break;
        }
    }
    if !pending.is_empty() {
        // 仍无法解析（引用不存在的类型或存在循环别名）→ 报错
        return collect_type_alias(ctx, pending[0], prefix);
    }
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
                // B-6：登记 `#[memory(gc)]` 模块前缀，供引用→Gc 默认映射判定
                if m.memory.as_deref() == Some("gc") {
                    ctx.gc_modules.insert(new_prefix.clone());
                }
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
    let mut field_spans = Vec::with_capacity(s.fields.len());
    for field in &s.fields {
        let ty = resolve_ast_type(ctx, &field.type_, field.span)?;
        // H4 MVP 限制：`dyn Protocol` 暂不支持作为 struct 字段（2 槽胖指针字段布局规划中）
        if matches!(&ty, Type::Dyn(_)) {
            return Err(TypeError::Unsupported {
                what: format!("`{ty}` 作为 struct 字段（H4 MVP 仅支持局部变量绑定）"),
                span: field.span,
            });
        }
        // SH-P0-1 E2（repr(C) 嵌套聚合内联）：校验字段为 C 布局兼容类型
        // （标量 / 指针 / 嵌套 repr(C) 结构体）；数组 / 枚举 / 联合 / dyn / 字符串
        // 视图 / 元组 / 切片暂不内联（规划中），非 repr(C) 嵌套结构体须同样标注。
        if s.repr_c {
            repr_c_field_ok(&ty, ctx, field.span)?;
        }
        fields.push((field.name.clone(), ty));
        field_spans.push(field.span);
    }
    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    let full = full_name(prefix, &s.name);
    if let Some(def) = ctx.structs.get_mut(&full) {
        def.fields = fields;
        def.field_spans = field_spans;
    }
    Ok(())
}

/// SH-P0-1 E2 repr(C) 嵌套聚合内联：校验 repr(C) 结构体字段是否为 C 布局兼容类型。
/// 允许：标量（含 sub-8 字节 i8/i16/i32/u8/u16/u32/f32/bool/char）、指针（8 字节）、
/// 嵌套 repr(C) 结构体（内联）。拒绝：数组 / 枚举 / 联合 / dyn Protocol / 字符串视图 /
/// 元组 / 切片（规划中）；非 repr(C) 嵌套结构体须同样标注 #[repr(C)]。
fn repr_c_field_ok(ty: &Type, ctx: &TypeContext, span: Span) -> Result<(), TypeError> {
    match ty {
        Type::I8 | Type::U8 | Type::I16 | Type::U16 | Type::I32 | Type::U32
        | Type::F32 | Type::I64 | Type::U64 | Type::F64 | Type::Bool | Type::Char => Ok(()),
        Type::RawPtr(..) | Type::Ref(..) => Ok(()),
        Type::Named(n, _) => match ctx.lookup_struct(n) {
            Some(d) if d.repr_c => Ok(()),
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
        Type::Array(..) => Err(TypeError::Unsupported {
            what: "repr(C) 结构体数组字段内联尚未实现（规划中）；当前仅支持标量 / 指针 / 嵌套 repr(C) 结构体字段".to_string(),
            span,
        }),
        _ => Err(TypeError::Unsupported {
            what: format!(
                "repr(C) 结构体字段 `{ty}` 为不支持的聚合类型（枚举 / 联合 / 字符串视图 / dyn Protocol / 元组 / 切片）"
            ),
            span,
        }),
    }
}

pub(crate) fn collect_enum(ctx: &mut TypeContext, e: &AstEnumDecl, prefix: &str) -> Result<(), TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = e.generics.iter().map(|p| p.name.clone()).collect();

    let mut variants = Vec::with_capacity(e.variants.len());
    let mut max_fields = 0usize;
    for (idx, v) in e.variants.iter().enumerate() {
        let mut fields = Vec::new();
        let mut field_spans = Vec::new();
        // 元组负载字段（`Some(T)`）以 `f0/f1/...` 命名
        for (i, t) in v.tuple_fields.iter().enumerate() {
            let ty = resolve_ast_type(ctx, t, v.span)?;
            fields.push((format!("f{i}"), ty));
            // 元组域为匿名字段，无独立声明位置，回指至变体声明处
            field_spans.push(v.span);
        }
        // 命名负载字段（`Variant { x: T }`）
        for sf in &v.struct_fields {
            let ty = resolve_ast_type(ctx, &sf.type_, sf.span)?;
            fields.push((sf.name.clone(), ty));
            field_spans.push(sf.span);
        }
        max_fields = max_fields.max(fields.len());
        // U3：显式判别式优先（`Variant = 42`），未标注时按变体声明序号。
        // 构造与 `match` 的 tag 比较均复用该值，故显式判别式自动生效。
        let tag = v.discriminant.unwrap_or(idx as i64) as usize;
        variants.push(VariantDef {
            name: v.name.clone(),
            fields,
            field_spans,
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

pub(crate) fn collect_protocol(ctx: &mut TypeContext, t: &AstProtocolDecl, prefix: &str) -> Result<(), TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = t.generics.iter().map(|p| p.name.clone()).collect();

    let full = full_name(prefix, &t.name);
    // P7d-1（2026-08-29）：标记「正在收集的 protocol」，使方法签名内的自引用
    // （如 `fn source(&self) -> Option<&dyn Error>`）在 protocol 尚未注册进 protocol_defs
    // 前，经 resolve.rs 的 dyn 分支回退到自身名字解析（不提前插入占位 def，避免
    // 干扰泛型 protocol 的方法签名解析）。
    let saved_collecting = ctx.collecting_protocol.take();
    ctx.collecting_protocol = Some(full.clone());

    let mut methods = Vec::new();
    for m in &t.methods {
        let mut params = Vec::with_capacity(m.params.len());
        for p in &m.params {
            if p.name == "self" {
                // protocol 方法签名中 `self` 用占位类型，具体类型由 impl 决定
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
            // V3 protocol 默认方法：protocol 方法带 body（`fn f(...) { ... }`）时保留其
            // 完整方法 AST（含签名与默认实现体）；`impl Protocol for X` 未实现该方法
            // 时回退（`check_method_call`）。抽象方法（无 body）为 `None`。
            default_body: m.body.is_some().then(|| m.clone()),
        });
    }

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    ctx.collecting_protocol = saved_collecting;
    ctx.insert_protocol(
        full,
        ProtocolDef {
            name: t.name.clone(),
            type_params: t.generics.iter().map(|p| p.name.clone()).collect(),
            assoc_types: t.types.clone(),
            superprotocols: t.superprotocols.iter().map(|(n, _)| n.clone()).collect(),
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
    // protocol 名解析为完整符号名（与 `dyn Protocol` 解析一致）：
    // 1) 显式 `mod::Protocol` 路径原样使用；2) 当前模块前缀下存在（`impl Protocol` 定义于 protocol 同模块内）；
    // 3) 顶层已定义；4) use 导入别名；5) 原样回退。inherent impl（无 protocol）保持 None。
    let protocol_name = match &imp.protocol_name {
        Some(tn) if tn.contains("::") => Some(tn.clone()),
        Some(tn) if !prefix.is_empty()
            && ctx
                .protocol_defs
                .contains_key(&format!("{}::{}", prefix, tn)) =>
        {
            Some(format!("{}::{}", prefix, tn))
        }
        Some(tn) if ctx.protocol_defs.contains_key(tn) => Some(tn.clone()),
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
                    rlyeh_ast::AstType::Ref(_, is_mut, _) => {
                        let m = if *is_mut {
                            Mutability::Mutable
                        } else {
                            Mutability::Immutable
                        };
                        Type::Ref(Box::new(self_type.clone()), m, None)
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

    // P6c（2026-08-29）：解析 protocol 泛型实参（如 `From<IoErrorKind>` 的 `IoErrorKind`）。
    // 须在恢复 type_params 之前进行——此时 ctx.type_params 仍为 imp.generics（方法
    // 循环结束后未被外层 restore 覆盖），protocol 类型实参中的 impl 级泛型参数
    // （如 `impl<T> Wrap<T> for Pair<T>` 的 `Wrap<T>` 之 `T`）才能正确解析；否则会因
    // type_params 已清空而报 undefined type。
    let protocol_type_args = imp
        .protocol_type_args
        .iter()
        .map(|t| resolve_ast_type(ctx, t, imp.span))
        .collect::<Result<Vec<Type>, TypeError>>()?;
    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    ctx.assoc_types = saved_assoc;
    ctx.self_type = saved_self;
    ctx.insert_impl(ImplDef {
        protocol_name,
        self_type,
        protocol_type_args,
        span: imp.span,
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
    // 先预注册整棵模块树名（含嵌套子模块，见 `register_module_tree`），使后续 use 导入
    // 解析时 `ctx.modules` 已含全部子模块路径。否则当 `pub import poll::Context` 写在
    // `module poll;` 之前时（future/module.rl），`resolve_import_path` 因 `future::poll`
    // 尚未登记而退化为 `poll::Context`，导致别名链 `Context → poll::Context` 指向不存在
    // 的符号（block_on/Future 等解析失败的根因）。
    register_module_tree(ctx, m, prefix);
    // 登记已声明模块路径（供 `resolve_import_path` 消歧，与 `collect_item_decls` 一致）
    ctx.modules.insert(new_prefix.clone());
    // A：`pub module` 前缀登记到对外公共面（供 P2 可见性判定；本阶段仅记录、不强制）
    if m.is_pub {
        ctx.pub_module_prefixes.insert(new_prefix.clone());
    }
    // B-6：登记 `#[memory(gc)]` 模块前缀，供引用→Gc 默认映射判定
    if m.memory.as_deref() == Some("gc") {
        ctx.gc_modules.insert(new_prefix.clone());
    }
    // Q3a：与 `collect_item_decls` 的 ModDecl 分支一致，模块内短名解析须感知
    // 模块前缀（`fmt/module.rl` 的 `protocol Display { fn fmt(&self, f: &mut Formatter) }`
    // 等——collect_impl/collect_protocol 收集阶段即 resolve_ast_type，use 段未注册）。
    let old_prefix = std::mem::replace(&mut ctx.module_prefix, new_prefix.clone());
    // 先注册本模块全部 use 导入别名，再收集类型 / 子模块——避免子模块声明
    // 先于 `pub import` 时别名未注册导致全限定引用退化为别名串。
    for inner in &m.items {
        if let AstItem::UseDecl(u) = inner {
            register_use(ctx, u, &new_prefix)?;
        }
    }
    for inner in &m.items {
        match inner {
            AstItem::StructDecl(s) => collect_struct(ctx, s, &new_prefix)?,
            AstItem::EnumDecl(e) => collect_enum(ctx, e, &new_prefix)?,
            AstItem::ProtocolDecl(t) => collect_protocol(ctx, t, &new_prefix)?,
            AstItem::ImplBlock(imp) => collect_impl(ctx, imp, &new_prefix)?,
            AstItem::TypeAlias(ta) => collect_type_alias(ctx, ta, &new_prefix)?,
            AstItem::ModDecl(inner_mod) => collect_mod_types_inner(ctx, inner_mod, &new_prefix)?,
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
                let full = full_name(prefix, &f.name);
                let sig = fn_signature(ctx, f, f.span)?;
                if !f.generics.is_empty() {
                    // 模块内泛型函数注册为模板（与顶层 collect_fn_sigs_pass 一致）：
                    // 调用点经 check_generic_call 按实参实例化。若仅登记为普通签名，
                    // 参数中的泛型（如 `&mut F`）会被固化为自由 `Type::Generic`，
                    // 调用点 `fn_templates` 查不到 → 走普通路径报 `expects '&mut F'`
                    // （典型：future::executor::block_on<F: Future>）。
                    ctx.fn_templates.insert(
                        full.clone(),
                        FnTemplate {
                            name: f.name.clone(),
                            type_params: f.generics.iter().map(|p| p.name.clone()).collect(),
                            bounds: f
                                .generics
                                .iter()
                                .filter(|p| !p.bounds.is_empty())
                                .map(|p| (p.name.clone(), p.bounds.clone()))
                                .collect(),
                            sig: sig.clone(),
                            ast: (**f).clone(),
                        },
                    );
                } else {
                    // 登记进主 ctx，使模块内跨函数调用（如 `io::file::open` 调 `io::base::c_str`）
                    // 能在 check_item 阶段经 `lookup_fn_signature` 解析（M0：补齐模块函数签名登记）。
                    ctx.insert_fn_signature(full.clone(), sig.clone());
                    if f.is_extern {
                        ctx.extern_fns.insert(full.clone());
                    }
                    if f.is_pub {
                        ctx.pub_symbols.insert(full.clone());
                    }
                }
                sigs.push((full, sig));
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
