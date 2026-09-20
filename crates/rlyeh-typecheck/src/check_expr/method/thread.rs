//! method/thread：`Thread::start` 闭包跨线程检查与线程入口 thunk 发射。
//! （由 method.rs 拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use crate::types::is_send_sync;
use crate::{Warning, WarningKind};
use super::*;

pub(super) fn check_thread_start_closure(
    ctx: &mut TypeContext,
    _ty_name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<Option<(HirExpr, Type)>, TypeError> {
    // F-M4：`Thread::start(move || ..)`（零参 move 闭包，单实参）等价 spawn
    // ——跨线程执行 move 闭包（捕获拥有环境，`'static` 约束检查）。
    if args.len() == 1 {
        return check_move_closure_spawn(ctx, &args[0], span);
    }
    // W6：`Thread::start(f, arg)`（带参闭包值 + 输入参数，两实参）
    if args.len() != 2 {
        return Ok(None);
    }
    // 实参 0 须为闭包值变量（`let f = |x: i64| ..; Thread::start(f, arg)`）
    let ExprKind::Ident(f_name) = &*args[0].kind else {
        return Ok(None);
    };
    let Some(ty) = ctx.lookup_variable(f_name).cloned() else {
        return Ok(None);
    };
    let Type::Closure { captures, params, ret, fn_name, .. } = &ty else {
        return Ok(None);
    };
    // 未固化延迟闭包（绑定处参数类型未知）不支持跨线程；MVP 限单参数
    if fn_name.is_empty() {
        return Ok(None);
    }
    if params.len() != 1 {
        return Err(TypeError::Unsupported {
            what: "跨线程闭包限单参数（`|x: i64| ..`）".to_string(),
            span,
        });
    }
    let (arg_hir, arg_ty) = infer_expr(ctx, &args[1])?;
    if !arg_ty.compatible_with(&params[0]) {
        return Err(TypeError::ArgumentTypeMismatch {
            name: "Thread::start 输入参数".to_string(),
            index: 1,
            expected: params[0].to_string(),
            found: arg_ty.to_string(),
            span: args[1].span,
            related: vec![],
        });
    }

    // 1. 线程输入对象 __t_in：槽 = [捕获槽值..., arg]
    let n = captures.len();
    let t_in = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::new(HirStmtKind::Let{
        name: t_in.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: n + 1,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy())];
    for (i, cap_ty) in captures.iter().enumerate() {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(t_in.clone()), Span::dummy())),
            index: i,
            value: Box::new(HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(f_name.clone()), Span::dummy())),
                index: i,
                ty: field_scalar_of(cap_ty),
            }, Span::dummy())),
            ty: field_scalar_of(cap_ty),
        }, Span::dummy())), Span::dummy()));
    }
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(t_in.clone()), Span::dummy())),
        index: n,
        value: Box::new(arg_hir),
        ty: field_scalar_of(&params[0]),
    }, Span::dummy())), Span::dummy()));

    // 2. 生成线程入口 thunk `__thread_entry_N(input: i64)`：读输入对象槽调 __closure_N
    let thunk = emit_thread_entry(ctx, captures, Some(&params[0]), ret, fn_name);

    // 3. thunk 函数指针绑定到局部变量（`let __entry = thunk`），经 fn 形参传 std
    let entry_var = ctx.fresh_temp();
    stmts.push(HirStmt::new(HirStmtKind::Let{
        name: entry_var.clone(),
        init: HirExpr::new(HirExprKind::FnPtr(thunk), Span::dummy()),
        mutable: false,
    }, Span::dummy()));

    // 4. 调用 `thread::__start_with_input(__entry, __t_in)`，返回类型取自 std 签名
    // 经别名解析定位 std 实现：模块化后真实全名为 `thread::entry::__start_with_input`，
    // 由 `thread/module.rl` 的 `pub import` 以 `thread::__start_with_input` 重导出。
    let helper = ctx
        .resolve_full_name("thread::__start_with_input")
        .unwrap_or_else(|| "thread::__start_with_input".to_string());
    let ret_ty = ctx
        .fn_signatures
        .get(&helper)
        .map(|s| s.return_type.clone())
        .unwrap_or(Type::I64);
    let call = HirExpr::new(HirExprKind::Call{
        callee: helper,
        args: vec![
            HirExpr::new(HirExprKind::Variable(entry_var), Span::dummy()),
            HirExpr::new(HirExprKind::Variable(t_in), Span::dummy()),
        ],
    }, Span::dummy());
    Ok(Some((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(call),
        })), Span::dummy()),
        ret_ty,
    )))
}

/// F-M4：`thread::spawn(move_closure)`——将 `move` 闭包（按值捕获的拥有环境）
/// 跨线程执行，等价于 actor-runtime 的 `spawn(move || worker_loop)`。
///
/// 闭包须为 `move`（F-M2：捕获环境所有权转移，可跨线程存活）+ 零参数
/// （`fn() -> i64` 等价）；其捕获类型须全部满足 `'static`（F-M3：禁止捕获
/// 指向外层栈帧的借用引用，否则新线程会读到悬垂指针）。
pub(crate) fn check_move_closure_spawn(
    ctx: &mut TypeContext,
    closure_arg: &AstExpr,
    span: Span,
) -> Result<Option<(HirExpr, Type)>, TypeError> {
    // 1. 将实参解析为「闭包值对象」——支持内联 `move || ..` 与已绑定变量 `f`。
    //    内联闭包经 `check_closure_value_binding` 走 H5 路径（收集捕获、构造
    //    捕获聚合对象），绑定到临时变量后读取其捕获槽。
    let (closure_val_hir, closure_ty) = match &*closure_arg.kind {
        ExprKind::Closure { capture, .. } if matches!(capture, rlyeh_ast::CaptureMode::Move) => {
            check_closure_value_binding(ctx, closure_arg, span)?
        }
        ExprKind::Closure { .. } => {
            return Err(TypeError::Unsupported {
                what: "spawn 需要 `move` 闭包（跨线程须转移捕获环境所有权）".to_string(),
                span,
            })
        }
        ExprKind::Ident(f_name) => {
            let Some(ty) = ctx.lookup_variable(f_name).cloned() else {
                return Ok(None);
            };
            let Type::Closure { captures, is_move, .. } = &ty else {
                return Ok(None);
            };
            // F-M2：有捕获的非 move 闭包（borrow）跨线程会共享外层栈帧，须拒绝；
            // 无捕获闭包 move/borrow 语义等价，放行。
            if !*is_move && !captures.is_empty() {
                return Err(TypeError::Unsupported {
                    what: "spawn 需要 `move` 闭包（跨线程须转移捕获环境所有权）".to_string(),
                    span,
                });
            }
            (
                HirExpr::new(HirExprKind::Variable(f_name.clone()), Span::dummy()),
                ty,
            )
        }
        _ => return Ok(None),
    };
    let Type::Closure { captures, params, ret, fn_name, .. } = &closure_ty else {
        return Ok(None);
    };
    if fn_name.is_empty() {
        return Err(TypeError::Unsupported {
            what: "spawn 不支持未固化的延迟闭包（须全参数注解）".to_string(),
            span,
        });
    }
    if params.len() != 0 {
        return Err(TypeError::Unsupported {
            what: "spawn 闭包须零参数（`fn() -> i64` 等价；带参闭包用 `Thread::start(f, arg)`）"
                .to_string(),
            span,
        });
    }

    // 2. F-M3 `'static` 约束：捕获类型不得含借用引用（指向外层栈帧 → 悬垂）。
    for (i, cap_ty) in captures.iter().enumerate() {
        if type_contains_ref(cap_ty) {
            return Err(TypeError::Unsupported {
                what: format!(
                    "spawn 闭包捕获了第 {} 个变量，其类型为 `{:?}`，含借用引用（非 'static）；跨线程须捕获拥有所有权的数据",
                    i, cap_ty
                ),
                span,
            });
        }
    }

    // 2b. Y（SH-P3-1 M3）：Send + Sync 并发安全基线（MVP 告警式，非硬阻塞）。
    // 捕获类型若不满足 Send + Sync，跨线程共享可能不安全——发出 W002 警告，不阻断编译。
    for cap_ty in captures.iter() {
        if !is_send_sync(cap_ty, ctx) {
            ctx.emit_warning(Warning {
                kind: WarningKind::NotSendSync {
                    ty: cap_ty.to_string(),
                },
                span,
            });
        }
    }

    // 3. 闭包值对象绑定到临时变量（内联情形），供读取捕获槽。
    let f_var = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::new(HirStmtKind::Let{
        name: f_var.clone(),
        init: closure_val_hir,
        mutable: false,
    }, Span::dummy())];

    // 4. 线程输入对象 __t_in：仅捕获槽（无额外参数）。
    let n = captures.len();
    let t_in = ctx.fresh_temp();
    stmts.push(HirStmt::new(HirStmtKind::Let{
        name: t_in.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: n,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy()));
    for (i, cap_ty) in captures.iter().enumerate() {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(t_in.clone()), Span::dummy())),
            index: i,
            value: Box::new(HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(f_var.clone()), Span::dummy())),
                index: i,
                ty: field_scalar_of(cap_ty),
            }, Span::dummy())),
            ty: field_scalar_of(cap_ty),
        }, Span::dummy())), Span::dummy()));
    }

    // 5. 生成零参数线程入口 thunk `__thread_entry_N(input: i64)`。
    let thunk = emit_thread_entry(ctx, captures, None, ret, fn_name);

    // 6. thunk 函数指针绑定到局部变量，调用 `thread::__start_with_input`。
    let entry_var = ctx.fresh_temp();
    stmts.push(HirStmt::new(HirStmtKind::Let{
        name: entry_var.clone(),
        init: HirExpr::new(HirExprKind::FnPtr(thunk), Span::dummy()),
        mutable: false,
    }, Span::dummy()));
    // 经别名解析定位 std 实现：模块化后真实全名为 `thread::entry::__start_with_input`，
    // 由 `thread/module.rl` 的 `pub import` 以 `thread::__start_with_input` 重导出。
    let helper = ctx
        .resolve_full_name("thread::__start_with_input")
        .unwrap_or_else(|| "thread::__start_with_input".to_string());
    let ret_ty = ctx
        .fn_signatures
        .get(&helper)
        .map(|s| s.return_type.clone())
        .unwrap_or(Type::I64);
    let call = HirExpr::new(HirExprKind::Call{
        callee: helper,
        args: vec![
            HirExpr::new(HirExprKind::Variable(entry_var), Span::dummy()),
            HirExpr::new(HirExprKind::Variable(t_in), Span::dummy()),
        ],
    }, Span::dummy());
    Ok(Some((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(call),
        })), Span::dummy()),
        ret_ty,
    )))
}

/// 判断类型是否含有借用引用（`&T` / `&mut T`）——用于 `'static` 约束检查。
/// 含引用的捕获在跨线程后会指向已销毁的外层栈帧，属悬垂指针，必须拒绝。
fn type_contains_ref(ty: &Type) -> bool {
    match ty {
        Type::Ref(..) => true,
        Type::Named(_, args) => args.iter().any(type_contains_ref),
        Type::Closure { captures, params, ret, .. } => captures
            .iter()
            .chain(params)
            .chain(std::iter::once(&**ret))
            .any(type_contains_ref),
        Type::Fn(sig) => sig
            .params
            .iter()
            .chain(std::iter::once(&sig.return_type))
            .any(type_contains_ref),
        Type::Union(members) => members.iter().any(type_contains_ref),
        _ => false,
    }
}

pub(super) fn emit_thread_entry(
    ctx: &mut TypeContext,
    capture_tys: &[Type],
    extra_arg: Option<&Type>,
    ret: &Type,
    closure_fn: &str,
) -> String {
    let name = format!("__thread_entry_{}", ctx.closure_seq);
    ctx.closure_seq += 1;
    let mut call_args: Vec<HirExpr> = capture_tys
        .iter()
        .enumerate()
        .map(|(i, cap_ty)| HirExpr::new(HirExprKind::FieldGet{
            base: Box::new(HirExpr::new(HirExprKind::Variable("__input".to_string()), Span::dummy())),
            index: i,
            ty: field_scalar_of(cap_ty),
        }, Span::dummy()))
        .collect();
    if let Some(arg_ty) = extra_arg {
        call_args.push(HirExpr::new(HirExprKind::FieldGet{
            base: Box::new(HirExpr::new(HirExprKind::Variable("__input".to_string()), Span::dummy())),
            index: capture_tys.len(),
            ty: field_scalar_of(arg_ty),
        }, Span::dummy()));
    }
    let body = HirExpr::new(HirExprKind::Call{
        callee: closure_fn.to_string(),
        args: call_args,
    }, Span::dummy());
    ctx.insert_fn_signature(
        name.clone(),
        FnSignature {
            params: vec![Type::I64],
            param_spans: vec![Span::dummy()],
            return_type: ret.clone(),
        },
    );
    ctx.mono_items.push(HirItem {
        span: crate::DUMMY_SPAN,
        name: name.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params: vec![HirParam { span: Span::dummy(),
                name: "__input".to_string(),
                is_ref: true,
            }],
            body: Some(HirBlock { span: Span::dummy(),
                stmts: vec![],
                final_expr: Some(body),
            }),
            is_extern: false,
            extern_sig: None,
        }),
    });
    name
}

