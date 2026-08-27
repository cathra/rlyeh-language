//! 表达式检查子模块：工具函数。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use super::*;

pub(super) fn ty_to_zero_ast(ctx: &TypeContext, ty: &Type, span: Span) -> Result<AstExpr, TypeError> {
    use Type::*;
    match ty {
        I64 | U8 => Ok(AstExpr::new(ExprKind::IntLiteral(0), span)),
        F64 => Ok(AstExpr::new(ExprKind::FloatLiteral(0.0), span)),
        Bool => Ok(AstExpr::new(ExprKind::BoolLiteral(false), span)),
        Named(n, _) if n == "String" => Ok(string_from_lit_ast(String::new(), span)),
        Named(n, _) if n == "Vec" || n == "HashMap" => Ok(mk_path_call(
            vec![n.clone(), "new".to_string()],
            Vec::new(),
            span,
        )),
        Named(n, args) if ctx.lookup_struct(&n).is_some() && args.is_empty() => {
            let def = ctx.lookup_struct(&n).expect("lookup_struct 已验证");
            let mut fields = Vec::new();
            for (fname, fty) in &def.fields {
                fields.push((fname.clone(), ty_to_zero_ast(ctx, fty, span)?));
            }
            Ok(AstExpr::new(
                ExprKind::StructCtor {
                    type_name: vec![n.clone()],
                    type_args: Vec::new(),
                    fields,
                },
                span,
            ))
        }
        other => Err(TypeError::Unsupported {
            what: format!(
                "json.parse：字段类型 `{other}` 的零值构造（MVP 支持 i64 / u8 / f64 / bool / String / Vec / HashMap / 嵌套 struct）"
            ),
            span,
        }),
    }
}

pub(super) fn value_to_string_ast(
    ctx: &mut TypeContext,
    arg: &AstExpr,
    debug: bool,
    span: Span,
) -> Result<AstExpr, TypeError> {
    let (_, ty) = infer_expr(ctx, arg)?;
    value_to_string_for_ty(ctx, &ty, arg, debug, span)
}

pub(super) fn value_to_string_for_ty(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    debug: bool,
    span: Span,
) -> Result<AstExpr, TypeError> {
    let base = peel_ref(ty);
    // String 对象 / &str 视图：`String::from(x)`（视图深拷贝；String 值即对象）
    if crate::comparison::is_string_type(ctx, &base) {
        if matches!(ty, Type::Ref(..)) {
            return Ok(AstExpr::new(
                ExprKind::Call {
                    callee: AstExpr::new(
                        ExprKind::Path(vec!["String".to_string(), "from".to_string()]),
                        span,
                    ),
                    args: vec![arg.clone()],
                    type_args: Vec::new(),
                },
                span,
            ));
        }
        return Ok(arg.clone());
    }
    // 字符串字面量 Str：`String::from(arg)`
    if matches!(ty, Type::Str) {
        return Ok(AstExpr::new(
            ExprKind::Call {
                callee: AstExpr::new(
                    ExprKind::Path(vec!["String".to_string(), "from".to_string()]),
                    span,
                ),
                args: vec![arg.clone()],
                type_args: Vec::new(),
            },
            span,
        ));
    }
    // i64 → `int_to_string(x)`（std）
    if matches!(ty, Type::I64) {
        let callee = AstExpr::new(ExprKind::Ident("int_to_string".to_string()), span);
        return Ok(AstExpr::new(
            ExprKind::Call {
                callee,
                args: vec![arg.clone()],
                type_args: Vec::new(),
            },
            span,
        ));
    }
    // bool → `if b { "true" } else { "false" }`
    if matches!(ty, Type::Bool) {
        let mk = |s: &str| string_from_lit_ast(s.to_string(), span);
        let then_block = AstBlock {
            stmts: Vec::new(),
            final_expr: Some(mk("true")),
            span,
        };
        let else_block = AstBlock {
            stmts: Vec::new(),
            final_expr: Some(mk("false")),
            span,
        };
        return Ok(AstExpr::new(
            ExprKind::If {
                cond: arg.clone(),
                then_block,
                else_block: Some(else_block),
            },
            span,
        ));
    }
    // Q3b：自定义类型走 Display / Debug trait 方法
    // （`{}` → `fmt`，`{:?}` → `fmt_debug`；方法名查找 impl，与手写 impl 调用
    // 路径一致，MVP 无 trait bound 检查）。`fmt` 直接返回 String（拼接模式）。
    let method = if debug { "fmt_debug" } else { "fmt" };
    if ctx.find_impl_for_method(&base, method).is_some() {
        // 块表达式：`let mut __fmt_q3 = Formatter::new(); x.fmt(&mut __fmt_q3)`
        // （MVP `&mut` 仅支持变量目标，临时值 `&mut Formatter::new()` 不可用）
        let tmp = "__fmt_q3".to_string();
        let f_new = AstExpr::new(
            ExprKind::Call {
                callee: AstExpr::new(
                    ExprKind::Path(vec!["Formatter".to_string(), "new".to_string()]),
                    span,
                ),
                args: Vec::new(),
                type_args: Vec::new(),
            },
            span,
        );
        let bind = AstStmt::Let {
            pattern: AstPattern::Ident(tmp.clone()),
            type_anno: None,
            init: f_new,
            mutable: true,
        };
        let f_ref = AstExpr::new(
            ExprKind::Unary {
                op: UnaryOp::AddrOfMut,
                operand: AstExpr::new(ExprKind::Ident(tmp.clone()), span),
            },
            span,
        );
        let call = AstExpr::new(
            ExprKind::MethodCall {
                receiver: arg.clone(),
                method: method.to_string(),
                args: vec![f_ref],
            },
            span,
        );
        return Ok(AstExpr::new(
            ExprKind::Block(AstBlock {
                stmts: vec![bind],
                final_expr: Some(call),
                span,
            }),
            span,
        ));
    }
    let placeholder = if debug { "{:?}" } else { "{}" };
    Err(TypeError::Unsupported {
        what: format!(
            "`{placeholder}` 占位符不支持类型 `{ty}`（MVP 支持 i64 / bool / String / &str / 字符串字面量；自定义类型须 `impl {placeholder}`）",
            placeholder = placeholder,
            ty = ty,
        ),
        span,
    })
}

pub(super) fn check_format_macro(
    ctx: &mut TypeContext,
    name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // `println!()` / `print!()`：无参数（空行 / 无输出）
    if args.is_empty() {
        if name == "format!" {
            return Err(TypeError::UnexpectedArgumentCount {
                name: name.to_string(),
                expected: 1,
                found: 0,
                span,
            });
        }
        let inner = name.trim_end_matches('!').to_string();
        let call = AstExpr::new(
            ExprKind::Call {
                callee: AstExpr::new(ExprKind::Ident(inner), span),
                args: Vec::new(),
                type_args: Vec::new(),
            },
            span,
        );
        let (hir, _) = infer_expr(ctx, &call)?;
        return Ok((hir, Type::Unit));
    }
    // 格式串：第 1 参数须为字符串字面量
    let fmt = match &*args[0].kind {
        ExprKind::StringLiteral(s) => s.clone(),
        _ => {
            return Err(TypeError::Unsupported {
                what: format!("{name} 的第 1 个参数须为字符串字面量格式串"),
                span: args[0].span,
            });
        }
    };
    let segs = parse_format_string(&fmt, args[0].span)?;
    let value_count = segs.iter().filter(|s| s.is_value).count();
    if value_count != args.len() - 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{name} 占位符"),
            expected: value_count,
            found: args.len() - 1,
            span,
        });
    }
    // 逐段拼接：`String::from(字面量) + to_string(值) + ...`
    // （`+` 在 Binary 分支 desugar 为 clone + push_str 深拷贝，A3 语义）
    let mut acc: Option<AstExpr> = None;
    let mut value_idx = 1usize;
    for seg in &segs {
        let seg_ast = if seg.is_value {
            let arg = &args[value_idx];
            value_idx += 1;
            value_to_string_ast(ctx, arg, seg.is_debug, span)?
        } else {
            string_from_lit_ast(seg.text.clone(), span)
        };
        acc = Some(match acc {
            None => seg_ast,
            Some(prev) => AstExpr::new(
                ExprKind::Binary {
                    op: BinaryOp::Add,
                    left: prev,
                    right: seg_ast,
                },
                span,
            ),
        });
    }
    let concat = acc.expect("格式串至少有一段");
    if name == "format!" {
        let (hir, ty) = infer_expr(ctx, &concat)?;
        return Ok((hir, ty));
    }
    // println! / print!：`println(concat)` → 内建 print/println（自动 desugar println_string）
    let inner = name.trim_end_matches('!').to_string();
    let call = AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(ExprKind::Ident(inner), span),
            args: vec![concat],
            type_args: Vec::new(),
        },
        span,
    );
    let (hir, _) = infer_expr(ctx, &call)?;
    Ok((hir, Type::Unit))
}

pub(super) fn check_dbg_macro(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "dbg!".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    // 先推断参数类型（此时 `__tmp` 尚未绑定，必须用预推断类型构造转换）
    let (_, arg_ty) = infer_expr(ctx, &args[0])?;
    let tmp = ctx.fresh_temp();
    // `let __tmp = <expr>; println("dbg: " + to_string(__tmp)); __tmp`
    let prefix = string_from_lit_ast("dbg: ".to_string(), span);
    // `dbg!` 用 Debug 格式（`{:?}` 语义）：自定义类型走 `fmt_debug`
    let value = value_to_string_for_ty(
        ctx,
        &arg_ty,
        &AstExpr::new(ExprKind::Ident(tmp.clone()), span),
        true,
        span,
    )?;
    let concat = AstExpr::new(
        ExprKind::Binary {
            op: BinaryOp::Add,
            left: prefix,
            right: value,
        },
        span,
    );
    let print_call = AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(ExprKind::Ident("println".to_string()), span),
            args: vec![concat],
            type_args: Vec::new(),
        },
        span,
    );
    let block = AstBlock {
        stmts: vec![
            AstStmt::Let {
                pattern: AstPattern::Ident(tmp.clone()),
                type_anno: None,
                init: args[0].clone(),
                mutable: false,
            },
            AstStmt::Expr(print_call),
        ],
        final_expr: Some(AstExpr::new(ExprKind::Ident(tmp), span)),
        span,
    };
    let (hir, ty) = check_block(ctx, &block)?;
    Ok((HirExpr::Block(Box::new(hir)), ty))
}

pub(super) fn substitute(ty: &Type, subst: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Generic(tp) => subst.get(tp).cloned().unwrap_or_else(|| ty.clone()),
        Type::Named(n, ps) => Type::Named(
            n.clone(),
            ps.iter().map(|p| substitute(p, subst)).collect(),
        ),
        Type::Ref(inner, m) => Type::Ref(Box::new(substitute(inner, subst)), *m),
        Type::RawPtr(inner, m) => Type::RawPtr(Box::new(substitute(inner, subst)), *m),
        Type::Tuple(ts) => Type::Tuple(ts.iter().map(|t| substitute(t, subst)).collect()),
        Type::Array(inner, n) => Type::Array(Box::new(substitute(inner, subst)), *n),
        // T1a：函数类型递归替换（fn(T, T) -> i64 中 T 经 impl 泛型统一后须替换为
        // 具体类型——此前 `_` 兜底不深入 Fn 内部，泛型方法 fn 参数（如
        // `Vec::sort_by(cmp: fn(T, T) -> i64)`）调用时形参保持 `fn(T, T)`，
        // 函数名 / 闭包实参均无法 unify（expects fn(T, T) -> i64 错误）。
        Type::Fn(sig) => Type::Fn(Box::new(FnSignature {
            params: sig.params.iter().map(|p| substitute(p, subst)).collect(),
            return_type: substitute(&sig.return_type, subst),
        })),
        Type::AssocProjection { base, assoc } => Type::AssocProjection {
            base: Box::new(substitute(base, subst)),
            assoc: assoc.clone(),
        },
        _ => ty.clone(),
    }
}

pub(super) fn peel_ref(ty: &Type) -> Type {
    match ty {
        Type::Ref(inner, _) => (**inner).clone(),
        _ => ty.clone(),
    }
}

pub(super) fn heap_wrapper_inner(ty: &Type) -> Option<Type> {
    match ty {
        Type::Named(n, args)
            if matches!(n.as_str(), "Box" | "Rc" | "Arc" | "Gc") && args.len() == 1 =>
        {
            Some(args[0].clone())
        }
        _ => None,
    }
}

pub(super) fn peel_refs_and_heap(ty: &Type) -> Type {
    let mut t = ty.clone();
    loop {
        let next = match &t {
            Type::Ref(inner, _) => (**inner).clone(),
            _ => match heap_wrapper_inner(&t) {
                Some(inner) => inner,
                None => return t,
            },
        };
        t = next;
    }
}

pub(super) fn heap_ptr_hir(hir: HirExpr, ty: &Type) -> HirExpr {
    match peel_ref(ty) {
        Type::Named(n, args)
            if matches!(n.as_str(), "Box" | "Rc" | "Arc" | "Gc") && args.len() == 1 =>
        {
            HirExpr::FieldGet {
                base: Box::new(hir),
                index: 0,
                ty: FieldScalar::Ptr,
            }
        }
        _ => hir,
    }
}

pub(super) fn string_hash_hir(ctx: &mut TypeContext, s: &HirExpr) -> HirExpr {
    let s_name = ctx.fresh_temp();
    let data_name = ctx.fresh_temp();
    let len_name = ctx.fresh_temp();
    let h_name = ctx.fresh_temp();
    let i_name = ctx.fresh_temp();
    let b_name = ctx.fresh_temp();

    let s_var = HirExpr::Variable(s_name.clone());
    let data_field = HirExpr::FieldGet {
        base: Box::new(s_var.clone()),
        index: 0,
        ty: FieldScalar::Ptr,
    };
    let len_field = HirExpr::FieldGet {
        base: Box::new(s_var),
        index: 1,
        ty: FieldScalar::Int,
    };

    let loop_body = HirBlock {
        stmts: vec![
            // if __i >= __len { break }
            HirStmt::Expr(HirExpr::If {
                cond: Box::new(HirExpr::Binary(
                    HirBinaryOp::Ge,
                    Box::new(HirExpr::Variable(i_name.clone())),
                    Box::new(HirExpr::Variable(len_name.clone())),
                )),
                then_block: Box::new(HirBlock {
                    stmts: vec![HirStmt::Expr(HirExpr::Break(None))],
                    final_expr: None,
                }),
                else_block: None,
            }),
            // let __b = __data[__i]
            HirStmt::Let {
                name: b_name.clone(),
                init: HirExpr::Index {
                    base: Box::new(HirExpr::Variable(data_name.clone())),
                    index: Box::new(HirExpr::Variable(i_name.clone())),
                    elem: FieldScalar::Int,
                    is_str: true,
                },
                mutable: false,
            },
            // __h = __h * 33 + __b
            HirStmt::Expr(HirExpr::Assign {
                target: h_name.clone(),
                op: HirAssignOp::Assign,
                value: Box::new(HirExpr::Binary(
                    HirBinaryOp::Add,
                    Box::new(HirExpr::Binary(
                        HirBinaryOp::Mul,
                        Box::new(HirExpr::Variable(h_name.clone())),
                        Box::new(HirExpr::IntLiteral(33)),
                    )),
                    Box::new(HirExpr::Variable(b_name)),
                )),
            }),
            // __i = __i + 1
            HirStmt::Expr(HirExpr::Assign {
                target: i_name.clone(),
                op: HirAssignOp::Assign,
                value: Box::new(HirExpr::Binary(
                    HirBinaryOp::Add,
                    Box::new(HirExpr::Variable(i_name.clone())),
                    Box::new(HirExpr::IntLiteral(1)),
                )),
            }),
        ],
        final_expr: None,
    };

    let stmts = vec![
        HirStmt::Let {
            name: s_name,
            init: s.clone(),
            mutable: false,
        },
        HirStmt::Let {
            name: data_name,
            init: data_field,
            mutable: false,
        },
        HirStmt::Let {
            name: len_name,
            init: len_field,
            mutable: false,
        },
        HirStmt::Let {
            name: h_name.clone(),
            init: HirExpr::IntLiteral(5381),
            mutable: true,
        },
        HirStmt::Let {
            name: i_name.clone(),
            init: HirExpr::IntLiteral(0),
            mutable: true,
        },
        HirStmt::Expr(HirExpr::Loop {
            body: Box::new(loop_body),
        }),
    ];

    HirExpr::Block(Box::new(HirBlock {
        stmts,
        final_expr: Some(HirExpr::Variable(h_name)),
    }))
}
