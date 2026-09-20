//! 表达式检查子模块：actor。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use super::*;

pub(crate) fn collect_actor(ctx: &mut TypeContext, a: &AstActorDecl, prefix: &str) -> Result<(), TypeError> {
    let full = full_name(prefix, &a.name);
    if ctx.structs.contains_key(&full)
        || ctx.enum_defs.contains_key(&full)
        || ctx.protocol_defs.contains_key(&full)
        || ctx.actors.contains_key(&full)
    {
        return Err(TypeError::Unsupported {
            what: format!("重复定义 `{full}`（已存在同名 struct/enum/protocol/actor）"),
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

pub(crate) fn is_actor_scalar_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::F64 | Type::Bool | Type::Char
    )
}

pub(crate) fn expand_actor(
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
    let mut stmts = vec![HirStmt::new(HirStmtKind::Let{
        name: "__s".to_string(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: slots.len(),
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
        mutable: true,
    }, Span::dummy())];
    for (idx, f) in a.fields.iter().enumerate() {
        let (v_hir, v_ty) = infer_expr(ctx, f.default.as_ref().unwrap())?;
        if !v_ty.compatible_with(&field_tys[idx].1) {
            return Err(TypeError::WrongType {
                expected: field_tys[idx].1.to_string(),
                found: v_ty.to_string(),
                span: f.span,
                related: vec![],
            });
        }
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable("__s".to_string()), Span::dummy())),
            index: idx,
            value: Box::new(v_hir),
            ty: slots[idx].1,
        }, Span::dummy())), Span::dummy()));
    }
    ctx.pop_scope();
    out.push(HirItem {
        name: state_new.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params: vec![],
            body: Some(HirBlock { span: Span::dummy(),
                stmts,
                final_expr: Some(HirExpr::new(HirExprKind::Variable("__s".to_string()), Span::dummy())),
            }),
            is_extern: false,
            extern_sig: None,
        }),
        span: a.span,
    });

    // 4. 方法函数 `<actor>::__m<i>(self, p0, p1, p2) -> i64`
    //    签名固定 4 个 i64 参数（self = 状态指针 + 3 个消息槽），
    //    handle 按位置传参；body 内 `self` 绑定状态指针、字段访问走 actor 分支。
    for (i, m) in a.methods.iter().enumerate() {
        let m_name = format!("{actor_full}::__m{i}");
        let mut params = vec![HirParam { span: Span::dummy(),
            name: "self".to_string(),
            is_ref: true,
        }];
        for p in &m.params {
            params.push(HirParam { span: Span::dummy(),
                name: p.name.clone(),
                is_ref: true,
            });
        }
        for j in params.len()..4 {
            params.push(HirParam { span: Span::dummy(),
                name: format!("__p{j}"),
                is_ref: true,
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
            span: a.span,
        });
    }

    // 5. dispatch handle `<actor>::__handle(self, kind, a, b, c) -> i64`
    //    按方法索引分发；未命中（kind 越界）返回 -1 = u64::MAX 崩溃信号。
    out.push(HirItem {
        name: handle.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params: (0..5)
                .map(|j| HirParam { span: Span::dummy(),
                    name: ["self", "kind", "a", "b", "c"][j].to_string(),
                    is_ref: true,
                })
                .collect(),
            body: Some(HirBlock { span: Span::dummy(),
                stmts: vec![],
                final_expr: Some(build_actor_dispatch(ctx, a, &actor_full, 0)),
            }),
            is_extern: false,
            extern_sig: None,
        }),
        span: a.span,
    });

    let _ = &state_new;
    Ok(())
}

pub(crate) fn build_actor_dispatch(
    _ctx: &TypeContext,
    a: &AstActorDecl,
    actor_full: &str,
    idx: usize,
) -> HirExpr {
    if idx >= a.methods.len() {
        return HirExpr::new(HirExprKind::IntLiteral(-1), Span::dummy());
    }
    let m_name = format!("{actor_full}::__m{idx}");
    let call = HirExpr::new(HirExprKind::Call{
        callee: m_name,
        args: vec![
            HirExpr::new(HirExprKind::Variable("self".to_string()), Span::dummy()),
            HirExpr::new(HirExprKind::Variable("a".to_string()), Span::dummy()),
            HirExpr::new(HirExprKind::Variable("b".to_string()), Span::dummy()),
            HirExpr::new(HirExprKind::Variable("c".to_string()), Span::dummy()),
        ],
    }, Span::dummy());
    let then_block = HirBlock { span: Span::dummy(),
        stmts: vec![HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::Return(Some(Box::new(call))), Span::dummy())), Span::dummy())],
        final_expr: None,
    };
    // else 分支以递归 if 为块尾表达式（值传递），
    // 底层 `IntLiteral(-1)` 必须经 final_expr 产出，否则该路径无值 → MIR 生成 `ret void`。
    let else_block = HirBlock { span: Span::dummy(),
        stmts: vec![],
        final_expr: Some(build_actor_dispatch(_ctx, a, actor_full, idx + 1)),
    };
    HirExpr::new(HirExprKind::If{
        cond: Box::new(HirExpr::new(HirExprKind::Binary(
            HirBinaryOp::Eq,
            Box::new(HirExpr::new(HirExprKind::Variable("kind".to_string()), Span::dummy())),
            Box::new(HirExpr::new(HirExprKind::IntLiteral(idx as i128), Span::dummy())),
        ), Span::dummy())),
        then_block: Box::new(then_block),
        else_block: Some(Box::new(else_block)),
    }, Span::dummy())
}

pub(crate) fn check_actor_method_body(
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
            related: vec![(
                m.span,
                format!("期望返回类型 `{}` 声明于此", return_type),
            )],
        });
    }

    ctx.pop_scope();
    Ok(hir_body)
}

pub(crate) fn emit_actor_runtime_externs(ctx: &mut TypeContext, out: &mut Vec<HirItem>) {
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
                    .map(|(i, _)| HirParam { span: Span::dummy(),
                        name: format!("__a{i}"),
                        is_ref: true,
                    })
                    .collect(),
                body: None,
                is_extern: true,
                extern_sig: Some((
                    args.iter().map(|s| s.to_string()).collect(),
                    (*ret).to_string(),
                )),
            }),
            span: crate::DUMMY_SPAN,
        });
    }
}

pub(crate) fn emit_gc_runtime_externs(ctx: &mut TypeContext, out: &mut Vec<HirItem>) {
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
                    .map(|(i, _)| HirParam { span: Span::dummy(), name: format!("__a{i}"), is_ref: true })
                    .collect(),
                body: None,
                is_extern: true,
                extern_sig: Some((
                    args.iter().map(|s| s.to_string()).collect(),
                    (*ret).to_string(),
                )),
            }),
            span: crate::DUMMY_SPAN,
        });
    }
}
