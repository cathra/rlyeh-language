//! 表达式检查子模块：closure。
//! （由 call.rs 二次拆分而来，保持语义等价）

use super::*;

pub(crate) fn check_closure_body_with_captures(
    ctx: &mut TypeContext,
    param_pairs: &[(String, Type)],
    body: &AstExpr,
    span: Span,
) -> Result<
    (
        HirExpr,
        Type,
        Vec<String>,
        Vec<Type>,
        Vec<String>,
        Vec<String>,
    ),
    TypeError,
> {
    let mut captures: Vec<String> = Vec::new();
    let mut outer_capture_slots: Vec<String> = Vec::new();
    let mut capture_tys: Vec<Type> = Vec::new();
    let body_result: Result<(HirExpr, Type, Vec<String>, Vec<String>), TypeError> = loop {
        ctx.push_scope(true);
        // 闭包 fn 层实际存储槽名（捕获/参数可能被二次 mangle）
        let mut closure_capture_slots = Vec::with_capacity(captures.len());
        for (nm, ty) in captures.iter().zip(capture_tys.clone()) {
            let s = ctx.insert_variable(nm.clone(), ty);
            closure_capture_slots.push(s);
        }
        let mut param_slots = Vec::with_capacity(param_pairs.len());
        for (nm, ty) in param_pairs.iter() {
            let s = ctx.insert_variable(nm.clone(), ty.clone());
            param_slots.push(s);
        }
        let r = infer_expr(ctx, body);
        ctx.pop_scope();
        match r {
            Ok((h, t)) => break Ok((h, t, closure_capture_slots, param_slots)),
            Err(TypeError::UndefinedVariable { name, .. }) if !captures.contains(&name) => {
                // 环境已恢复为外层（闭包 fn 层已弹出）——在外层变量环境中
                // 能找到类型者即为捕获变量（原名用于检测去重；外层槽名供调用
                // 实参 / 闭包对象捕获字段引用；类型快照供捕获参数绑定）
                if let Some((slot, ty)) = ctx.resolve_variable(&name) {
                    captures.push(name);
                    outer_capture_slots.push(slot.to_string());
                    capture_tys.push(ty.clone());
                    continue;
                }
                break Err(TypeError::UndefinedVariable { name, span });
            }
            Err(e) => break Err(e),
        }
    };
    let (body_hir, body_ty, closure_capture_slots, param_slots) = body_result?;
    Ok((
        body_hir,
        body_ty,
        outer_capture_slots,
        capture_tys,
        closure_capture_slots,
        param_slots,
    ))
}

pub(crate) fn emit_closure_fn(
    ctx: &mut TypeContext,
    captures: &[String],
    capture_tys: &[Type],
    param_names: &[String],
    param_tys: &[Type],
    body_hir: HirExpr,
    body_ty: Type,
) -> String {
    let name = format!("__closure_{}", ctx.closure_seq);
    ctx.closure_seq += 1;
    let mut fn_params = capture_tys.to_vec();
    fn_params.extend(param_tys.iter().cloned());
    let mut fn_names = captures.to_vec();
    fn_names.extend(param_names.iter().cloned());
    ctx.insert_fn_signature(
        name.clone(),
        FnSignature {
            params: fn_params,
            return_type: body_ty.clone(),
        },
    );
    ctx.mono_items.push(HirItem {
        name: name.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params: fn_names
                .iter()
                .map(|n| HirParam { name: n.clone() })
                .collect(),
            body: Some(HirBlock {
                stmts: vec![],
                final_expr: Some(body_hir),
            }),
            is_extern: false,
            extern_sig: None,
        }),
    });
    name
}

pub(crate) fn check_capture_closure_iife(
    ctx: &mut TypeContext,
    params: &[AstPattern],
    body: &AstExpr,
    _capture: &CaptureMode,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if params.len() != args.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "<捕获闭包>".to_string(),
            expected: params.len(),
            found: args.len(),
            span,
        });
    }
    // 闭包参数名（Ident / Wildcard）
    let mut names = Vec::with_capacity(params.len());
    for (i, p) in params.iter().enumerate() {
        match p {
            AstPattern::Ident(n) => names.push(n.clone()),
            AstPattern::Wildcard => names.push(format!("__arg{i}")),
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（H3 仅支持简单标识符参数）"),
                    span,
                })
            }
        }
    }
    // 实参检查（外层环境）：闭包参数类型从实参推断
    let mut arg_hirs = Vec::with_capacity(args.len());
    let mut arg_tys = Vec::with_capacity(args.len());
    for a in args {
        let (h, t) = infer_expr(ctx, a)?;
        if matches!(t, Type::Str) {
            // 字符串字面量实参升级为 String 语义（与 `let s = "..."` 绑定一致）：
            // 展开 `String::from` 深拷贝（data/len/cap 三槽），使闭包体内
            // 拼接 / 方法调用按 String 对象解析（槽数匹配）
            let (h2, t2) = check_string_from(ctx, std::slice::from_ref(a), a.span)?;
            arg_hirs.push(h2);
            arg_tys.push(t2);
        } else {
            arg_hirs.push(h);
            arg_tys.push(t);
        }
    }
    // 迭代检查闭包体 + 收集捕获（外层槽名供调用实参、闭包层槽名供 emit 签名）
    let pairs: Vec<(String, Type)> = names.iter().cloned().zip(arg_tys.clone()).collect();
    let (body_hir, body_ty, outer_capture_slots, capture_tys, closure_capture_slots, param_slots) =
        check_closure_body_with_captures(ctx, &pairs, body, span)?;
    // 匿名函数生成（参数 = [捕获变量, 闭包参数]）
    let name = emit_closure_fn(
        ctx,
        &closure_capture_slots,
        &capture_tys,
        &param_slots,
        &arg_tys,
        body_hir,
        body_ty.clone(),
    );
    // 调用：捕获变量（闭包定义处外层槽名，按名引用）+ 实参
    let mut call_args: Vec<HirExpr> = outer_capture_slots
        .iter()
        .map(|c| HirExpr::Variable(c.clone()))
        .collect();
    call_args.extend(arg_hirs);
    Ok((
        HirExpr::Call {
            callee: name,
            args: call_args,
        },
        body_ty,
    ))
}

pub(crate) fn check_closure_value_call(
    ctx: &mut TypeContext,
    name: &str,
    closure_ty: &Type,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let Type::Closure { captures, params, ret, fn_name, .. } = closure_ty else {
        unreachable!("check_closure_value_call 仅接受闭包值类型");
    };
    // 未固化延迟闭包（非注解绑定 `let f = |x| ..; f(..)`）：
    // 绑定处参数类型未知，首次调用点由实参类型推断参数类型后固化。
    if fn_name.is_empty() {
        return check_deferred_closure_call(ctx, name, args, span);
    }
    if args.len() != params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("<闭包值 {name}>"),
            expected: params.len(),
            found: args.len(),
            span,
        });
    }
    // 实参检查（String 字面量升级语义与 IIFE 一致）
    let mut arg_hirs = Vec::with_capacity(args.len());
    for (i, a) in args.iter().enumerate() {
        let (h, t) = infer_expr(ctx, a)?;
        let expected = &params[i];
        let (h, t) = if matches!(t, Type::Str) && !matches!(expected, Type::Str) {
            let (h2, t2) = check_string_from(ctx, std::slice::from_ref(a), a.span)?;
            (h2, t2)
        } else {
            (h, t)
        };
        if !t.compatible_with(expected) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: format!("<闭包值 {name}>"),
                index: i,
                expected: expected.to_string(),
                found: t.to_string(),
                span,
            });
        }
        arg_hirs.push(h);
    }
    // 捕获字段读取 + 实参组装
    let mut call_args: Vec<HirExpr> = captures
        .iter()
        .enumerate()
        .map(|(i, cap_ty)| HirExpr::FieldGet {
            base: Box::new(HirExpr::Variable(name.to_string())),
            index: i,
            ty: field_scalar_of(cap_ty),
        })
        .collect();
    call_args.extend(arg_hirs);
    Ok((
        HirExpr::Call {
            callee: fn_name.to_string(),
            args: call_args,
        },
        (**ret).clone(),
    ))
}

pub(crate) fn check_deferred_closure_call(
    ctx: &mut TypeContext,
    name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let Some(idx) = ctx
        .deferred_closures
        .iter()
        .position(|d| d.var_name == name)
    else {
        return Err(TypeError::Unsupported {
            what: format!("闭包值 `{name}` 的延迟绑定记录不存在（内部错误）"),
            span,
        });
    };
    let binding = ctx.deferred_closures.remove(idx);
    let ExprKind::Closure { params, param_types, body, capture: _ } = &*binding.closure.kind else {
        unreachable!("延迟闭包绑定仅接受闭包表达式");
    };
    if params.len() != args.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("<闭包值 {name}>"),
            expected: params.len(),
            found: args.len(),
            span,
        });
    }
    // 闭包参数名（Ident / Wildcard）
    let mut names = Vec::with_capacity(params.len());
    for (i, p) in params.iter().enumerate() {
        match p {
            AstPattern::Ident(n) => names.push(n.clone()),
            AstPattern::Wildcard => names.push(format!("__arg{i}")),
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（仅支持简单标识符参数）"),
                    span,
                })
            }
        }
    }
    // 实参检查（外层环境）。参数类型确定规则（半注解语义）：
    // - 有类型注解的参数：用注解类型，实参须与之兼容（`|x: i64, y|` + `f("s", 2)` 报错）；
    // - 无类型注解的参数：由实参推断（String 字面量升级语义与 IIFE 一致）。
    let mut arg_hirs = Vec::with_capacity(args.len());
    let mut arg_tys = Vec::with_capacity(args.len());
    for (i, a) in args.iter().enumerate() {
        let (h, t) = infer_expr(ctx, a)?;
        let (h, t) = if matches!(t, Type::Str) {
            check_string_from(ctx, std::slice::from_ref(a), a.span)?
        } else {
            (h, t)
        };
        arg_hirs.push(h);
        if let Some(anno) = &param_types[i] {
            let at = resolve_ast_type(ctx, anno, a.span)?;
            if !t.compatible_with(&at) {
                return Err(TypeError::ArgumentTypeMismatch {
                    name: format!("<闭包值 {name}>"),
                    index: i,
                    expected: at.to_string(),
                    found: t.to_string(),
                    span: a.span,
                });
            }
            arg_tys.push(at);
        } else {
            arg_tys.push(t);
        }
    }
    // 迭代检查闭包体 + 收集捕获（按值捕获类型快照；错误定位到绑定处闭包体；
    // 外层槽名供对象字段引用、闭包层槽名供 emit 签名）
    let pairs: Vec<(String, Type)> = names.iter().cloned().zip(arg_tys.clone()).collect();
    let (body_hir, body_ty, captures, capture_tys, closure_capture_slots, param_slots) =
        check_closure_body_with_captures(ctx, &pairs, body, binding.span)?;
    // 匿名函数生成（参数名用闭包 fn 层 insert 的槽名）
    let fn_name = emit_closure_fn(
        ctx,
        &closure_capture_slots,
        &capture_tys,
        &param_slots,
        &arg_tys,
        body_hir,
        body_ty.clone(),
    );
    // 捕获聚合对象构造（内联到调用点块）+ 真实对象重新绑定到变量名
    let cv = format!("__cv_{}", ctx.closure_seq - 1);
    let mut stmts = vec![HirStmt::Let {
        name: cv.clone(),
        init: HirExpr::Alloc {
            slots: captures.len(),
            by_value: false,
            is_strfat: false,
        },
        mutable: false,
    }];
    for (i, c) in captures.iter().enumerate() {
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(cv.clone())),
            index: i,
            value: Box::new(HirExpr::Variable(c.clone())),
            ty: field_scalar_of(&capture_tys[i]),
        }));
    }
    // 覆盖绑定处占位（`Alloc{slots:0}`）：后续 `f(args)` 常规路径读 f 捕获槽。
    // U1：Let 绑定名用绑定处 insert 的存储槽名（遮蔽时 mangle），与引用一致。
    let slot_name = ctx
        .resolve_variable(&name)
        .map(|(s, _)| s.to_string())
        .unwrap_or_else(|| name.to_string());
    stmts.push(HirStmt::Let {
        name: slot_name.clone(),
        init: HirExpr::Variable(cv),
        mutable: false,
    });
    // 调用：捕获字段读取（base 为槽名 f）+ 实参
    let mut call_args: Vec<HirExpr> = captures
        .iter()
        .enumerate()
        .map(|(i, _)| HirExpr::FieldGet {
            base: Box::new(HirExpr::Variable(slot_name.clone())),
            index: i,
            ty: field_scalar_of(&capture_tys[i]),
        })
        .collect();
    call_args.extend(arg_hirs);
    let call = HirExpr::Call {
        callee: fn_name.clone(),
        args: call_args,
    };
    // 固化变量类型（后续调用按常规闭包值调用路径检查）
    ctx.insert_variable(
        name.to_string(),
        Type::Closure {
            captures: capture_tys,
            params: arg_tys,
            ret: Box::new(body_ty.clone()),
            fn_name,
            is_move: matches!(
                &*binding.closure.kind,
                ExprKind::Closure { capture: rlyeh_ast::CaptureMode::Move, .. }
            ),
        },
    );
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(call),
        })),
        body_ty,
    ))
}

pub(crate) fn check_deferred_closure_binding(
    ctx: &mut TypeContext,
    closure: &AstExpr,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let ExprKind::Closure { params, param_types: _, body: _, capture: _ } = &*closure.kind else {
        return Err(TypeError::Unsupported {
            what: "延迟闭包绑定需要闭包表达式".to_string(),
            span,
        });
    };
    // 参数模式校验（Ident / Wildcard；无注解参数允许，类型由首次调用点推断）
    for p in params.iter() {
        match p {
            AstPattern::Ident(_) | AstPattern::Wildcard => {}
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（仅支持简单标识符参数）"),
                    span,
                })
            }
        }
    }
    // 注册延迟绑定（变量名由 check_stmt 绑定 Ident 分支填充）
    ctx.deferred_closures.push(DeferredClosure {
        var_name: String::new(),
        closure: closure.clone(),
        span,
    });
    // 未固化闭包类型：fn_name 为空标记延迟（params/ret 为占位，固化时回写）
    let ty = Type::Closure {
        captures: vec![],
        params: vec![],
        ret: Box::new(Type::Unit),
        fn_name: String::new(),
        is_move: false,
    };
    Ok((HirExpr::Alloc {
        slots: 0,
        by_value: false,
        is_strfat: false,
    }, ty))
}

pub(crate) fn check_closure_value_binding(
    ctx: &mut TypeContext,
    closure: &AstExpr,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let ExprKind::Closure { params, param_types, body, capture } = &*closure.kind else {
        return Err(TypeError::Unsupported {
            what: "闭包值绑定需要闭包表达式".to_string(),
            span,
        });
    };
    // 参数名 + 类型（要求全注解）
    let mut names = Vec::with_capacity(params.len());
    let mut param_tys = Vec::with_capacity(params.len());
    for (i, (p, anno)) in params.iter().zip(param_types.iter()).enumerate() {
        let nm = match p {
            AstPattern::Ident(n) => n.clone(),
            AstPattern::Wildcard => format!("__arg{i}"),
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（仅支持简单标识符参数）"),
                    span,
                })
            }
        };
        let Some(a) = anno else {
            return Err(TypeError::Unsupported {
                what: format!(
                    "闭包参数 `{nm}` 缺少类型注解（闭包值对象需要 `|x: i64|` 形式；无注解闭包可用 IIFE 或 fn 类型上下文）"
                ),
                span,
            });
        };
        let ty = resolve_ast_type(ctx, a, span)?;
        names.push(nm);
        param_tys.push(ty);
    }
    // 迭代检查闭包体 + 收集捕获（按值捕获类型快照；
    // 外层槽名供对象字段引用、闭包层槽名供 emit 签名）
    let pairs: Vec<(String, Type)> = names.iter().cloned().zip(param_tys.clone()).collect();
    let (body_hir, body_ty, captures, capture_tys, closure_capture_slots, param_slots) =
        check_closure_body_with_captures(ctx, &pairs, body, span)?;
    // 匿名函数生成（参数名用闭包 fn 层 insert 的槽名）
    let fn_name = emit_closure_fn(
        ctx,
        &closure_capture_slots,
        &capture_tys,
        &param_slots,
        &param_tys,
        body_hir,
        body_ty.clone(),
    );
    // 闭包值构造：聚合对象（每捕获一槽）+ 逐槽写入捕获变量（按值拷贝）
    let cv = format!("__cv_{}", ctx.closure_seq - 1);
    let mut stmts = vec![HirStmt::Let {
        name: cv.clone(),
        init: HirExpr::Alloc {
            slots: captures.len(),
            by_value: false,
            is_strfat: false,
        },
        mutable: false,
    }];
    for (i, c) in captures.iter().enumerate() {
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(cv.clone())),
            index: i,
            value: Box::new(HirExpr::Variable(c.clone())),
            ty: field_scalar_of(&capture_tys[i]),
        }));
    }
    let init = HirExpr::Block(Box::new(HirBlock {
        stmts,
        final_expr: Some(HirExpr::Variable(cv)),
    }));
    Ok((
        init,
        Type::Closure {
            captures: capture_tys,
            params: param_tys,
            ret: Box::new(body_ty),
            fn_name,
            is_move: matches!(capture, rlyeh_ast::CaptureMode::Move),
        },
    ))
}

pub(crate) fn try_closure_value_as_fn(ty: &Type) -> Option<(HirExpr, Type)> {
    let Type::Closure { captures, params, ret, fn_name, .. } = ty else {
        return None;
    };
    if !captures.is_empty() || fn_name.is_empty() {
        return None;
    }
    let sig = FnSignature {
        params: params.clone(),
        return_type: (**ret).clone(),
    };
    Some((
        HirExpr::FnPtr(fn_name.clone()),
        Type::Fn(Box::new(sig)),
    ))
}

pub(crate) fn fix_deferred_closure_with_sig(
    ctx: &mut TypeContext,
    name: &str,
    sig: &FnSignature,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let Some(idx) = ctx
        .deferred_closures
        .iter()
        .position(|d| d.var_name == name)
    else {
        return Err(TypeError::Unsupported {
            what: format!("闭包值 `{name}` 的延迟绑定记录不存在（内部错误）"),
            span,
        });
    };
    let binding = ctx.deferred_closures.remove(idx);
    let ExprKind::Closure { params, param_types, body, capture: _ } = &*binding.closure.kind else {
        unreachable!("延迟闭包绑定仅接受闭包表达式");
    };
    if params.len() != sig.params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("<闭包值 {name}>"),
            expected: sig.params.len(),
            found: params.len(),
            span,
        });
    }
    let mut names = Vec::with_capacity(params.len());
    for (i, p) in params.iter().enumerate() {
        match p {
            AstPattern::Ident(n) => names.push(n.clone()),
            AstPattern::Wildcard => names.push(format!("__arg{i}")),
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（仅支持简单标识符参数）"),
                    span,
                })
            }
        }
    }
    // 参数类型：有注解用注解类型（须与 fn 签名兼容），无注解用签名类型
    let mut param_tys = Vec::with_capacity(sig.params.len());
    for (i, s) in sig.params.iter().enumerate() {
        if let Some(anno) = &param_types[i] {
            let at = resolve_ast_type(ctx, anno, span)?;
            if !at.compatible_with(s) {
                return Err(TypeError::ArgumentTypeMismatch {
                    name: format!("<闭包值 {name}>"),
                    index: i,
                    expected: s.to_string(),
                    found: at.to_string(),
                    span,
                });
            }
            param_tys.push(at);
        } else {
            param_tys.push(s.clone());
        }
    }
    // 迭代检查闭包体 + 收集捕获（参数类型来自 fn 签名 / 注解；
    // 无捕获时闭包层槽名与外层槽名相同，emit 用闭包层槽名）
    let pairs: Vec<(String, Type)> = names.iter().cloned().zip(param_tys.clone()).collect();
    let (body_hir, body_ty, captures, capture_tys, closure_capture_slots, param_slots) =
        check_closure_body_with_captures(ctx, &pairs, body, binding.span)?;
    if !captures.is_empty() {
        return Err(TypeError::Unsupported {
            what: format!(
                "闭包值 `{name}` 捕获 {} 个变量，捕获闭包值不能经 fn 签名传递（MVP 限制：捕获对象不跨函数边界）",
                captures.len()
            ),
            span,
        });
    }
    let fn_name = emit_closure_fn(
        ctx,
        &closure_capture_slots,
        &capture_tys,
        &param_slots,
        &param_tys,
        body_hir,
        body_ty.clone(),
    );
    // 固化变量类型（无捕获 → 完整闭包类型；后续 `f(..)` 调用按常规路径展开）
    ctx.insert_variable(
        name.to_string(),
        Type::Closure {
            captures: vec![],
            params: param_tys,
            ret: Box::new(body_ty),
            fn_name: fn_name.clone(),
            is_move: matches!(
                &*binding.closure.kind,
                ExprKind::Closure { capture: rlyeh_ast::CaptureMode::Move, .. }
            ),
        },
    );
    Ok((
        HirExpr::FnPtr(fn_name),
        Type::Fn(Box::new(sig.clone())),
    ))
}

pub(crate) fn check_closure_expected(
    ctx: &mut TypeContext,
    closure: &AstExpr,
    expected: &Type,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let ExprKind::Closure { params, param_types: _, body, capture: _ } = &*closure.kind else {
        unreachable!("check_closure_expected 仅接受闭包表达式");
    };
    let FnSignature {
        params: sig_params,
        return_type,
    } = match expected {
        Type::Fn(sig) => (**sig).clone(),
        _ => {
            return Err(TypeError::Unsupported {
                what: "闭包需要 fn 类型上下文（H2 无捕获闭包：用作 fn 形参实参，或 `let f: fn(..) = |..| ..` 注解绑定）"
                    .to_string(),
                span,
            })
        }
    };
    if params.len() != sig_params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "<闭包>".to_string(),
            expected: sig_params.len(),
            found: params.len(),
            span,
        });
    }
    // 闭包参数名（AstPattern::Ident / Wildcard；其余模式 MVP 不支持）
    let mut names = Vec::with_capacity(params.len());
    for (i, p) in params.iter().enumerate() {
        match p {
            AstPattern::Ident(n) => names.push(n.clone()),
            AstPattern::Wildcard => names.push(format!("__arg{i}")),
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（H2 仅支持简单标识符参数）"),
                    span,
                })
            }
        }
    }
    // 匿名函数名：全局唯一（`__closure_` 前缀不会与用户符号冲突）
    let name = format!("__closure_{}", ctx.closure_seq);
    ctx.closure_seq += 1;

    // 闭包体检查：函数边界作用域（隔离，body 引用外部变量将报 UndefinedVariable
    // → 转为捕获闭包 Unsupported，H3 规划）。
    ctx.push_scope(true);
    for (nm, ty) in names.iter().zip(sig_params.clone()) {
        ctx.insert_variable(nm.clone(), ty);
    }
    let body_result = infer_expr(ctx, body);
    // 无论成败都弹出作用域（调用方变量环境不得泄漏）
    ctx.pop_scope();

    let (body_hir, body_ty) = match body_result {
        Ok(v) => v,
        Err(TypeError::UndefinedVariable { name, span }) => {
            return Err(TypeError::Unsupported {
                what: format!(
                    "闭包捕获外部变量 `{name}`（捕获闭包 H3 规划；H2 无捕获闭包仅可用参数与字面量）"
                ),
                span,
            })
        }
        Err(e) => return Err(e),
    };
    // 返回类型兼容性：body 尾部表达式须兼容预期返回类型
    if body_ty != Type::Never && !body_ty.compatible_with(&return_type) {
        return Err(TypeError::WrongType {
            expected: return_type.to_string(),
            found: body_ty.to_string(),
            span: body.span,
        });
    }

    // 注册匿名函数签名 + HIR 函数项（注入全局 items，MIR/LIR/codegen 与普通函数同路径）
    ctx.insert_fn_signature(
        name.clone(),
        FnSignature {
            params: sig_params.clone(),
            return_type: return_type.clone(),
        },
    );
    ctx.mono_items.push(HirItem {
        name: name.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params: names
                .iter()
                .map(|n| HirParam { name: n.clone() })
                .collect(),
            body: Some(HirBlock {
                stmts: vec![],
                final_expr: Some(body_hir),
            }),
            is_extern: false,
            extern_sig: None,
        }),
    });
    Ok((
        HirExpr::FnPtr(name),
        Type::Fn(Box::new(FnSignature {
            params: sig_params,
            return_type,
        })),
    ))
}
