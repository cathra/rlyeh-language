//! 表达式检查子模块：iterator。
//! （由 iter.rs 二次拆分而来，保持语义等价）

use super::*;

pub(crate) fn check_for_iterator(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iterator: &AstExpr,
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let it_name = format!("__for_it_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let elem_tmp = format!("__for_elem_{}", ctx.temp_counter);
    ctx.temp_counter += 1;

    // 绑定迭代器：`let mut __for_it = iterator;`（`next(&mut self)` 需可变）
    let bind = AstStmt::Let {
        pattern: AstPattern::Ident(it_name.clone()),
        type_anno: None,
        init: iterator.clone(),
        mutable: true,
    };

    // 循环体：`match __for_it.next() { Some(__elem) => .., None => break }`
    let next_call = AstExpr::new(
        ExprKind::MethodCall {
            receiver: AstExpr::new(ExprKind::Ident(it_name.clone()), span),
            method: "next".to_string(),
            args: vec![],
        },
        span,
    );
    // Some 臂：`{ let pat = __elem; body }`
    let mut arm_stmts = vec![AstStmt::Let {
        pattern: pattern.clone(),
        type_anno: None,
        init: AstExpr::new(ExprKind::Ident(elem_tmp.clone()), span),
        mutable: false,
    }];
    arm_stmts.extend(body.stmts.clone());
    let some_body = AstExpr::new(
        ExprKind::Block(AstBlock {
            stmts: arm_stmts,
            final_expr: body.final_expr.clone(),
            span,
        }),
        span,
    );
    // None 臂：`break`
    let none_body = AstExpr::new(ExprKind::Break(None), span);
    let match_expr = AstExpr::new(
        ExprKind::Match {
            expr: next_call,
            arms: vec![
                rlyeh_ast::MatchArm {
                    pattern: AstPattern::Enum("Some".to_string(), vec![AstPattern::Ident(elem_tmp)]),
                    guard: None,
                    body: some_body,
                    span,
                },
                rlyeh_ast::MatchArm {
                    pattern: AstPattern::Enum("None".to_string(), vec![]),
                    guard: None,
                    body: none_body,
                    span,
                },
            ],
        },
        span,
    );
    let loop_expr = AstExpr::new(
        ExprKind::Loop {
            body: AstBlock {
                stmts: vec![AstStmt::Semi(match_expr)],
                final_expr: None,
                span,
            },
        },
        span,
    );

    let block_ast = AstExpr::new(
        ExprKind::Block(AstBlock {
            stmts: vec![bind, AstStmt::Semi(loop_expr)],
            final_expr: None,
            span,
        }),
        span,
    );
    let (hir, _) = infer_expr(ctx, &block_ast)?;
    Ok((hir, Type::Unit))
}

pub(super) fn closure_return_ty(
    ctx: &mut TypeContext,
    closure: &AstExpr,
    param_tys: &[Type],
    span: Span,
) -> Result<Type, TypeError> {
    let ExprKind::Closure { params, param_types: _, body, capture: _ } = &*closure.kind else {
        return Err(TypeError::Unsupported {
            what: "适配器期望闭包参数".to_string(),
            span,
        });
    };
    if params.len() != param_tys.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "<闭包>".to_string(),
            expected: param_tys.len(),
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
                    what: format!("闭包参数模式 `{other:?}`（仅支持简单标识符）"),
                    span,
                })
            }
        }
    }
    ctx.push_scope(true);
    for (nm, ty) in names.iter().zip(param_tys.iter().cloned()) {
        ctx.insert_variable(nm.clone(), ty);
    }
    let body_result = infer_expr(ctx, body);
    ctx.pop_scope();
    let (_, body_ty) = match body_result {
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
    Ok(body_ty)
}

pub(crate) fn ty_to_ast(ty: &Type) -> AstType {
    use Type::*;
    match ty {
        I64 => AstType::Path("i64".into(), vec![]),
        U8 => AstType::Path("u8".into(), vec![]),
        F64 => AstType::Path("f64".into(), vec![]),
        Bool => AstType::Path("bool".into(), vec![]),
        Unit => AstType::Path("()".into(), vec![]),
        Named(n, args) => AstType::Path(n.clone(), args.iter().map(ty_to_ast).collect()),
        _ => AstType::Path("()".into(), vec![]),
    }
}

pub(crate) fn try_check_adapter(
    ctx: &mut TypeContext,
    receiver: &AstExpr,
    self_ty: &Type,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<Option<(HirExpr, Type)>, TypeError> {
    if !matches!(method, "map" | "filter" | "fold" | "collect" | "take" | "skip") {
        return Ok(None);
    }
    // 元素类型 T：数组直接取；`Vec<T>` 容器取类型参数；否则检测
    // `next() -> Option<T>` 方法的返回项（自定义迭代器）
    let elem_ty = match self_ty {
        Type::Array(elem, _) => substitute(elem, &ctx.generic_subst),
        Type::Named(n, targs) => {
            let full = ctx.resolve_full_name(n.as_str()).unwrap_or_else(|| n.clone());
            if full == "Vec" && ctx.lookup_struct(&full).is_some() {
                let Some(t) = targs.first().cloned() else {
                    return Ok(None);
                };
                substitute(&t, &ctx.generic_subst)
            } else {
                let Some(imp) = ctx.find_impl_for_method(self_ty, "next").cloned() else {
                    return Ok(None);
                };
                let Some(md) = imp.methods.iter().find(|m| m.sig.name == "next") else {
                    return Ok(None);
                };
                let mut subst: HashMap<String, Type> = HashMap::new();
                if unify(&imp.self_type, self_ty, &mut subst).is_err() {
                    return Ok(None);
                }
                let ret = substitute(&md.sig.return_type, &subst);
                let Type::Named(nn, rtargs) = peel_ref(&ret) else {
                    return Ok(None);
                };
                if ctx
                    .resolve_full_name(nn.as_str())
                    .unwrap_or_else(|| nn.clone())
                    != "Option"
                {
                    return Ok(None);
                }
                let Some(inner) = rtargs.first().cloned() else {
                    return Ok(None);
                };
                substitute(&inner, &ctx.generic_subst)
            }
        }
        other => {
            let Some(imp) = ctx.find_impl_for_method(other, "next").cloned() else {
                return Ok(None);
            };
            let Some(md) = imp.methods.iter().find(|m| m.sig.name == "next") else {
                return Ok(None);
            };
            let mut subst: HashMap<String, Type> = HashMap::new();
            if unify(&imp.self_type, other, &mut subst).is_err() {
                return Ok(None);
            }
            let ret = substitute(&md.sig.return_type, &subst);
            let Type::Named(n, targs) = peel_ref(&ret) else {
                return Ok(None);
            };
            if ctx.resolve_full_name(n.as_str()).unwrap_or_else(|| n.clone()) != "Option" {
                return Ok(None);
            }
            let Some(inner) = targs.first() else {
                return Ok(None);
            };
            substitute(inner, &ctx.generic_subst)
        }
    };
    if matches!(elem_ty, Type::Infer) {
        return Ok(None);
    }
    let (hir, ty) = check_iterator_adapter(ctx, receiver, self_ty, method, args, &elem_ty, span)?;
    Ok(Some((hir, ty)))
}

pub(super) fn check_iterator_adapter(
    ctx: &mut TypeContext,
    receiver: &AstExpr,
    self_ty: &Type,
    method: &str,
    args: &[AstExpr],
    elem_ty: &Type,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 数组 `[T; N]` 与 `Vec<T>` 容器经 `for ... in` 分派（J1 / check_for_vec）；
    // 自定义迭代器经 `next()` 循环。
    let is_for_loop = match self_ty {
        Type::Array(_, _) => true,
        Type::Named(n, _) => {
            ctx.resolve_full_name(n.as_str()).unwrap_or_else(|| n.clone()) == "Vec"
        }
        _ => false,
    };
    let mk_ident = |name: &str| AstExpr::new(ExprKind::Ident(name.to_string()), span);
    let mk_block = |stmts: Vec<AstStmt>, final_expr: Option<AstExpr>| AstBlock {
        stmts,
        final_expr,
        span,
    };

    // —— 参数校验 ——
    let expect_args = match method {
        "collect" => 0,
        "fold" => 2,
        _ => 1,
    };
    if args.len() != expect_args {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("<{method}>"),
            expected: expect_args,
            found: args.len(),
            span,
        });
    }

    // —— 唯一临时名 ——
    let f_name = format!("__adp_f_{}", ctx.temp_counter);
    let x_name = format!("__adp_x_{}", ctx.temp_counter);
    let out_name = format!("__adp_out_{}", ctx.temp_counter);
    let it_name = format!("__adp_it_{}", ctx.temp_counter);
    let acc_name = format!("__adp_acc_{}", ctx.temp_counter);
    let n_name = format!("__adp_n_{}", ctx.temp_counter);
    ctx.temp_counter += 6;

    // —— 闭包签名与返回类型 ——
    // u_ty：收集容器元素类型（map/fold 取闭包返回类型；其余取元素类型 T）
    // closure_binding：`__adp_f` 函数指针绑定（map/filter/fold 有，其余无）
    let (u_ty, closure_binding): (Type, Option<(HirExpr, Type)>) = match method {
        // 闭包参数必须为闭包字面量（H2 无捕获闭包）
        "map" | "filter" => {
            let c = &args[0];
            if !matches!(&*c.kind, ExprKind::Closure { .. }) {
                return Err(TypeError::Unsupported {
                    what: format!("`{method}` 的参数必须是闭包 `|x| ..`"),
                    span: c.span,
                });
            }
            let u = closure_return_ty(ctx, c, std::slice::from_ref(elem_ty), c.span)?;
            let fn_sig = FnSignature {
                params: vec![elem_ty.clone()],
                return_type: u.clone(),
            };
            let binding =
                check_closure_expected(ctx, c, &Type::Fn(Box::new(fn_sig)), span)?;
            (u, Some(binding))
        }
        "fold" => {
            let c = &args[1];
            if !matches!(&*c.kind, ExprKind::Closure { .. }) {
                return Err(TypeError::Unsupported {
                    what: "`fold` 的第二个参数必须是闭包 `|acc, x| ..`".to_string(),
                    span: c.span,
                });
            }
            // init 先推断（acc 类型），再推断闭包返回类型
            let (_, acc_ty) = infer_expr(ctx, &args[0])?;
            if matches!(acc_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "`fold` 的初始值要求类型确定（如 `0` / `String::from(\"\")`）".to_string(),
                    span: args[0].span,
                });
            }
            let u = closure_return_ty(ctx, c, &[acc_ty.clone(), elem_ty.clone()], c.span)?;
            let fn_sig = FnSignature {
                params: vec![acc_ty.clone(), elem_ty.clone()],
                return_type: u.clone(),
            };
            let binding =
                check_closure_expected(ctx, c, &Type::Fn(Box::new(fn_sig)), span)?;
            (u, Some(binding))
        }
        _ => {
            // take / skip / collect：无闭包
            (elem_ty.clone(), None)
        }
    };
    if let Some((_, fn_ty)) = &closure_binding {
        ctx.insert_variable(f_name.clone(), fn_ty.clone());
    }

    // —— 收集容器类型 ——
    let result_ty = if method == "fold" {
        // acc 类型：init 的变量类型
        let (_, acc_ty) = infer_expr(ctx, &args[0])?;
        acc_ty
    } else if method == "map" {
        Type::Named("Vec".to_string(), vec![u_ty.clone()])
    } else {
        Type::Named("Vec".to_string(), vec![elem_ty.clone()])
    };
    let vec_ast_ty = match &result_ty {
        Type::Named(n, inner) => AstType::Path(n.clone(), inner.iter().map(ty_to_ast).collect()),
        _ => AstType::Path("Vec".to_string(), vec![ty_to_ast(&result_ty)]),
    };

    // —— apply 语句（循环体内）——
    let push = |out: &str, val: &str| {
        AstExpr::new(
            ExprKind::MethodCall {
                receiver: mk_ident(out),
                method: "push".to_string(),
                args: vec![mk_ident(val)],
            },
            span,
        )
    };
    let apply_stmts: Vec<AstStmt> = match method {
        "map" => {
            let u_name = format!("__adp_u_{}", ctx.temp_counter);
            ctx.temp_counter += 1;
            let call = AstExpr::new(
                ExprKind::Call {
                    callee: mk_ident(&f_name),
                    args: vec![mk_ident(&x_name)],
                    type_args: Vec::new(),
                },
                span,
            );
            vec![
                AstStmt::Let {
                    pattern: AstPattern::Ident(u_name.clone()),
                    type_anno: None,
                    init: call,
                    mutable: false,
                },
                AstStmt::Semi(push(&out_name, &u_name)),
            ]
        }
        "filter" => {
            let cond = AstExpr::new(
                ExprKind::Call {
                    callee: mk_ident(&f_name),
                    args: vec![mk_ident(&x_name)],
                    type_args: Vec::new(),
                },
                span,
            );
            let if_expr = AstExpr::new(
                ExprKind::If {
                    cond,
                    then_block: mk_block(vec![AstStmt::Semi(push(&out_name, &x_name))], None),
                    else_block: None,
                },
                span,
            );
            vec![AstStmt::Semi(if_expr)]
        }
        "fold" => {
            let call = AstExpr::new(
                ExprKind::Call {
                    callee: mk_ident(&f_name),
                    args: vec![mk_ident(&acc_name), mk_ident(&x_name)],
                    type_args: Vec::new(),
                },
                span,
            );
            vec![AstStmt::Semi(AstExpr::new(
                ExprKind::Assign {
                    target: mk_ident(&acc_name),
                    op: AssignOp::Assign,
                    value: call,
                },
                span,
            ))]
        }
        "take" => {
            // if __n < n { __n += 1; __out.push(__x); } else { break }
            let cond = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![mk_ident(&n_name), args[0].clone()],
                    operators: vec![rlyeh_ast::CompareOp::Lt],
                },
                span,
            );
            let inc = AstExpr::new(
                ExprKind::Assign {
                    target: mk_ident(&n_name),
                    op: AssignOp::AddAssign,
                    value: AstExpr::new(ExprKind::IntLiteral(1), span),
                },
                span,
            );
            let then_block = mk_block(
                vec![
                    AstStmt::Semi(inc),
                    AstStmt::Semi(push(&out_name, &x_name)),
                ],
                None,
            );
            let else_block = mk_block(vec![AstStmt::Semi(AstExpr::new(ExprKind::Break(None), span))], None);
            let if_expr = AstExpr::new(
                ExprKind::If {
                    cond,
                    then_block,
                    else_block: Some(else_block),
                },
                span,
            );
            vec![AstStmt::Semi(if_expr)]
        }
        "skip" => {
            // if __n < n { __n += 1 } else { __out.push(__x); }
            let cond = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![mk_ident(&n_name), args[0].clone()],
                    operators: vec![rlyeh_ast::CompareOp::Lt],
                },
                span,
            );
            let inc = AstExpr::new(
                ExprKind::Assign {
                    target: mk_ident(&n_name),
                    op: AssignOp::AddAssign,
                    value: AstExpr::new(ExprKind::IntLiteral(1), span),
                },
                span,
            );
            let if_expr = AstExpr::new(
                ExprKind::If {
                    cond,
                    then_block: mk_block(vec![AstStmt::Semi(inc)], None),
                    else_block: Some(mk_block(
                        vec![AstStmt::Semi(push(&out_name, &x_name))],
                        None,
                    )),
                },
                span,
            );
            vec![AstStmt::Semi(if_expr)]
        }
        _ => {
            // collect：__out.push(__x)
            vec![AstStmt::Semi(push(&out_name, &x_name))]
        }
    };

    // —— 循环构造 ——
    let loop_expr = if is_for_loop {
        AstExpr::new(
            ExprKind::For {
                pattern: AstPattern::Ident(x_name.clone()),
                iterator: receiver.clone(),
                body: mk_block(apply_stmts, None),
            },
            span,
        )
    } else {
        // let mut __it = <recv>;
        // loop { match __it.next() { Some(__x) => { apply }, None => break } }
        let next_call = AstExpr::new(
            ExprKind::MethodCall {
                receiver: mk_ident(&it_name),
                method: "next".to_string(),
                args: vec![],
            },
            span,
        );
        let match_expr = AstExpr::new(
            ExprKind::Match {
                expr: next_call,
                arms: vec![
                    rlyeh_ast::MatchArm {
                        pattern: AstPattern::Enum(
                            "Some".to_string(),
                            vec![AstPattern::Ident(x_name.clone())],
                        ),
                        guard: None,
                        body: AstExpr::new(ExprKind::Block(mk_block(apply_stmts, None)), span),
                        span,
                    },
                    rlyeh_ast::MatchArm {
                        pattern: AstPattern::Enum("None".to_string(), vec![]),
                        guard: None,
                        body: AstExpr::new(ExprKind::Break(None), span),
                        span,
                    },
                ],
            },
            span,
        );
        let loop_ast = AstExpr::new(
            ExprKind::Loop {
                body: mk_block(vec![AstStmt::Semi(match_expr)], None),
            },
            span,
        );
        // 迭代器绑定前缀并入外层块
        loop_ast
    };

    // —— 外层块：`let mut __out: Vec<U> = Vec::new();` (+ fold acc / take n / 迭代器绑定) + 循环 ——
    let mut ast_stmts = vec![AstStmt::Let {
        pattern: AstPattern::Ident(out_name.clone()),
        type_anno: Some(vec_ast_ty),
        init: AstExpr::new(
            ExprKind::Call {
                callee: AstExpr::new(ExprKind::Path(vec!["Vec".to_string(), "new".to_string()]), span),
                args: vec![],
                type_args: Vec::new(),
            },
            span,
        ),
        mutable: true,
    }];
    if method == "fold" {
        ast_stmts.push(AstStmt::Let {
            pattern: AstPattern::Ident(acc_name.clone()),
            type_anno: None,
            init: args[0].clone(),
            mutable: true,
        });
    }
    if matches!(method, "take" | "skip") {
        ast_stmts.push(AstStmt::Let {
            pattern: AstPattern::Ident(n_name.clone()),
            type_anno: None,
            init: AstExpr::new(ExprKind::IntLiteral(0), span),
            mutable: true,
        });
    }
    if !is_for_loop {
        ast_stmts.push(AstStmt::Let {
            pattern: AstPattern::Ident(it_name.clone()),
            type_anno: None,
            init: receiver.clone(),
            mutable: true,
        });
    }
    ast_stmts.push(AstStmt::Semi(loop_expr));
    let final_expr = if method == "fold" {
        mk_ident(&acc_name)
    } else {
        mk_ident(&out_name)
    };
    let block_ast = AstExpr::new(
        ExprKind::Block(mk_block(ast_stmts, Some(final_expr))),
        span,
    );
    let (loop_hir, block_ty) = infer_expr(ctx, &block_ast)?;

    // —— 前缀 HIR：闭包函数指针绑定（map/filter/fold）——
    let mut hir_stmts = vec![];
    if let Some((fnptr_hir, _)) = &closure_binding {
        hir_stmts.push(HirStmt::Let {
            name: f_name.clone(),
            init: fnptr_hir.clone(),
            mutable: false,
        });
    }
    hir_stmts.push(HirStmt::Expr(loop_hir));
    let hir = HirExpr::Block(Box::new(HirBlock {
        stmts: hir_stmts,
        final_expr: Some(if method == "fold" {
            HirExpr::Variable(acc_name)
        } else {
            HirExpr::Variable(out_name)
        }),
    }));
    Ok((hir, block_ty))
}
