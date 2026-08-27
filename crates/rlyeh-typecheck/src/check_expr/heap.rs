//! 表达式检查子模块：Box/RC/GC/Weak 智能指针。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use super::*;

pub(super) fn check_box_new(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "Box::new".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let (v_hir, v_ty) = infer_expr(ctx, &args[0])?;
    let n_slots = type_slot_count(ctx, &v_ty, span)?;
    if n_slots == 0 {
        return Err(TypeError::Unsupported {
            what: "`Box::new(())`：单元类型无法装箱".to_string(),
            span,
        });
    }
    let data = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: data.clone(),
        init: HirExpr::Call {
            callee: "alloc_bytes".to_string(),
            args: vec![HirExpr::IntLiteral(n_slots as i128 * 8)],
        },
        mutable: false,
    }];
    if v_ty.is_numeric() || matches!(v_ty, Type::Bool | Type::Char) {
        // 标量 T：直接写入堆首槽
        stmts.push(HirStmt::Semi(HirExpr::DerefSet {
            base: Box::new(HirExpr::Variable(data.clone())),
            value: Box::new(v_hir),
            ty: field_scalar_of(&v_ty),
        }));
    } else {
        // 聚合 T：整槽区 memcpy（浅拷贝）
        stmts.push(HirStmt::Semi(HirExpr::Call {
            callee: "array_copy".to_string(),
            args: vec![
                HirExpr::Variable(data.clone()),
                v_hir,
                HirExpr::IntLiteral(n_slots as i128),
            ],
        }));
    }
    let box_base = ctx.fresh_temp();
    stmts.push(HirStmt::Let {
        name: box_base.clone(),
        init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
            is_strfat: false,
        },
        mutable: false,
    });
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(box_base.clone())),
        index: 0,
        value: Box::new(HirExpr::Variable(data)),
        ty: FieldScalar::Ptr,
    }));
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(box_base)),
        })),
        Type::Named("Box".to_string(), vec![v_ty]),
    ))
}

pub(super) fn check_rc_new(
    ctx: &mut TypeContext,
    ty_name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{ty_name}::new"),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let (v_hir, v_ty) = infer_expr(ctx, &args[0])?;
    let n_slots = type_slot_count(ctx, &v_ty, span)?;
    if n_slots == 0 {
        return Err(TypeError::Unsupported {
            what: format!("`{ty_name}::new(())`：单元类型无法装箱"),
            span,
        });
    }
    let inner = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: inner.clone(),
        init: HirExpr::Call {
            callee: "alloc_bytes".to_string(),
            args: vec![HirExpr::IntLiteral((n_slots + 2) as i128 * 8)],
        },
        mutable: false,
    }];
    // `T` 值区自堆首槽起（与 `Box<T>` 同构）
    if v_ty.is_numeric() || matches!(v_ty, Type::Bool | Type::Char) {
        stmts.push(HirStmt::Semi(HirExpr::DerefSet {
            base: Box::new(HirExpr::Variable(inner.clone())),
            value: Box::new(v_hir),
            ty: field_scalar_of(&v_ty),
        }));
    } else {
        stmts.push(HirStmt::Semi(HirExpr::Call {
            callee: "array_copy".to_string(),
            args: vec![
                HirExpr::Variable(inner.clone()),
                v_hir,
                HirExpr::IntLiteral(n_slots as i128),
            ],
        }));
    }
    // 计数槽（FieldSet base 即堆地址，GEP + store）：strong = n、weak = n + 1
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(inner.clone())),
        index: n_slots,
        value: Box::new(HirExpr::IntLiteral(1)),
        ty: FieldScalar::Int,
    }));
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(inner.clone())),
        index: n_slots + 1,
        value: Box::new(HirExpr::IntLiteral(0)),
        ty: FieldScalar::Int,
    }));
    let rc_base = ctx.fresh_temp();
    stmts.push(HirStmt::Let {
        name: rc_base.clone(),
        init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
            is_strfat: false,
        },
        mutable: false,
    });
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(rc_base.clone())),
        index: 0,
        value: Box::new(HirExpr::Variable(inner)),
        ty: FieldScalar::Ptr,
    }));
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(rc_base)),
        })),
        Type::Named(ty_name.to_string(), vec![v_ty]),
    ))
}

pub(super) fn check_gc_new(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "Gc::new".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let (v_hir, v_ty) = infer_expr(ctx, &args[0])?;
    let n_slots = type_slot_count(ctx, &v_ty, span)?;
    if n_slots == 0 {
        return Err(TypeError::Unsupported {
            what: "`Gc::new(())`：单元类型无法装箱".to_string(),
            span,
        });
    }
    let inner = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: inner.clone(),
        init: HirExpr::Call {
            callee: "rlyeh_gc_alloc".to_string(),
            args: vec![HirExpr::IntLiteral(n_slots as i128)],
        },
        mutable: false,
    }];
    // `T` 值区自堆首槽起（与 `Box<T>` 同构）
    if v_ty.is_numeric() || matches!(v_ty, Type::Bool | Type::Char) {
        stmts.push(HirStmt::Semi(HirExpr::DerefSet {
            base: Box::new(HirExpr::Variable(inner.clone())),
            value: Box::new(v_hir),
            ty: field_scalar_of(&v_ty),
        }));
    } else {
        stmts.push(HirStmt::Semi(HirExpr::Call {
            callee: "array_copy".to_string(),
            args: vec![
                HirExpr::Variable(inner.clone()),
                v_hir,
                HirExpr::IntLiteral(n_slots as i128),
            ],
        }));
    }
    let gc_base = ctx.fresh_temp();
    stmts.push(HirStmt::Let {
        name: gc_base.clone(),
        init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
            is_strfat: false,
        },
        mutable: false,
    });
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(gc_base.clone())),
        index: 0,
        value: Box::new(HirExpr::Variable(inner)),
        ty: FieldScalar::Ptr,
    }));
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(gc_base)),
        })),
        Type::Named("Gc".to_string(), vec![v_ty]),
    ))
}

pub(super) fn check_rc_method(
    ctx: &mut TypeContext,
    method: &str,
    recv_ty: &Type,
    recv_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Option<Result<(HirExpr, Type), TypeError>> {
    let base = peel_ref(recv_ty);
    let (wrapper, inner) = match &base {
        Type::Named(n, ps) if matches!(n.as_str(), "Rc" | "Arc") && ps.len() == 1 => {
            (n.clone(), ps[0].clone())
        }
        Type::Named(n, ps) if n == "Weak" && ps.len() == 1 => {
            if method == "upgrade" {
                return Some(weak_upgrade(ctx, ps[0].clone(), recv_hir, args, span));
            }
            return None;
        }
        _ => return None,
    };
    let r = match method {
        "clone" => rc_clone(ctx, &wrapper, inner, recv_hir, args, span),
        "strong_count" => rc_count(ctx, &wrapper, &inner, 0, recv_hir, args, span),
        "weak_count" => rc_count(ctx, &wrapper, &inner, 1, recv_hir, args, span),
        "downgrade" => rc_downgrade(ctx, &wrapper, inner, recv_hir, args, span),
        "try_unwrap" => rc_try_unwrap(ctx, &wrapper, inner, recv_hir, args, span),
        _ => return None,
    };
    Some(r)
}

pub(super) fn rc_clone(
    ctx: &mut TypeContext,
    wrapper: &str,
    inner: Type,
    recv_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if !args.is_empty() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{wrapper}::clone"),
            expected: 0,
            found: args.len(),
            span,
        });
    }
    let n = type_slot_count(ctx, &inner, span)?;
    let inner_t = ctx.fresh_temp();
    let cnt = ctx.fresh_temp();
    let rc_t = ctx.fresh_temp();
    let stmts = vec![
        HirStmt::Let {
            name: inner_t.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(recv_hir),
                index: 0,
                ty: FieldScalar::Ptr,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: cnt.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(inner_t.clone())),
                index: n,
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(inner_t.clone())),
            index: n,
            value: Box::new(HirExpr::Binary(
                HirBinaryOp::Add,
                Box::new(HirExpr::Variable(cnt)),
                Box::new(HirExpr::IntLiteral(1)),
            )),
            ty: FieldScalar::Int,
        }),
        HirStmt::Let {
            name: rc_t.clone(),
            init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
            is_strfat: false,
        },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(rc_t.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(inner_t)),
            ty: FieldScalar::Ptr,
        }),
    ];
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(rc_t)),
        })),
        Type::Named(wrapper.to_string(), vec![inner]),
    ))
}

pub(super) fn rc_count(
    ctx: &mut TypeContext,
    wrapper: &str,
    inner: &Type,
    off: usize,
    recv_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let name = if off == 0 { "strong_count" } else { "weak_count" };
    if !args.is_empty() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{wrapper}::{name}"),
            expected: 0,
            found: args.len(),
            span,
        });
    }
    let n = type_slot_count(ctx, inner, span)?;
    let v = HirExpr::FieldGet {
        base: Box::new(HirExpr::FieldGet {
            base: Box::new(recv_hir),
            index: 0,
            ty: FieldScalar::Ptr,
        }),
        index: n + off,
        ty: FieldScalar::Int,
    };
    Ok((v, Type::USize))
}

pub(super) fn rc_downgrade(
    ctx: &mut TypeContext,
    wrapper: &str,
    inner: Type,
    recv_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if !args.is_empty() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{wrapper}::downgrade"),
            expected: 0,
            found: args.len(),
            span,
        });
    }
    let n = type_slot_count(ctx, &inner, span)?;
    let inner_t = ctx.fresh_temp();
    let cnt = ctx.fresh_temp();
    let w_t = ctx.fresh_temp();
    let stmts = vec![
        HirStmt::Let {
            name: inner_t.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(recv_hir),
                index: 0,
                ty: FieldScalar::Ptr,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: cnt.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(inner_t.clone())),
                index: n + 1,
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(inner_t.clone())),
            index: n + 1,
            value: Box::new(HirExpr::Binary(
                HirBinaryOp::Add,
                Box::new(HirExpr::Variable(cnt)),
                Box::new(HirExpr::IntLiteral(1)),
            )),
            ty: FieldScalar::Int,
        }),
        HirStmt::Let {
            name: w_t.clone(),
            init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
            is_strfat: false,
        },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(w_t.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(inner_t)),
            ty: FieldScalar::Ptr,
        }),
    ];
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(w_t)),
        })),
        Type::Named("Weak".to_string(), vec![inner]),
    ))
}

pub(super) fn rc_try_unwrap(
    ctx: &mut TypeContext,
    wrapper: &str,
    inner: Type,
    recv_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if !args.is_empty() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{wrapper}::try_unwrap"),
            expected: 0,
            found: args.len(),
            span,
        });
    }
    // 值区首槽 = RcInner 指针（`T` 值区自堆首槽起，与 `*rc` 解引用同构）
    let value_base = HirExpr::FieldGet {
        base: Box::new(recv_hir.clone()),
        index: 0,
        ty: FieldScalar::Ptr,
    };
    let inner_init = HirExpr::FieldGet {
        base: Box::new(recv_hir.clone()),
        index: 0,
        ty: FieldScalar::Ptr,
    };
    // `Ok(v)` 负载：标量 load 值区 / 聚合取值区指针（与 `*rc` 解引用一致）
    let ok_val = if inner.is_numeric() || matches!(inner, Type::Bool | Type::Char) {
        HirExpr::Deref {
            expr: Box::new(value_base),
            ty: field_scalar_of(&inner),
        }
    } else {
        value_base
    };
    let (ok_tag, res_slots) = enum_variant_info(ctx, "Result", "Ok", span)?;
    let (err_tag, _) = enum_variant_info(ctx, "Result", "Err", span)?;
    let inner_t = ctx.fresh_temp();
    let cnt = ctx.fresh_temp();
    let res_t = ctx.fresh_temp();
    let then_stmts = vec![
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(res_t.clone())),
            index: 0,
            value: Box::new(HirExpr::IntLiteral(ok_tag as i128)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(res_t.clone())),
            index: 1,
            value: Box::new(ok_val),
            ty: field_scalar_of(&inner),
        }),
    ];
    let else_stmts = vec![
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(res_t.clone())),
            index: 0,
            value: Box::new(HirExpr::IntLiteral(err_tag as i128)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(res_t.clone())),
            index: 1,
            value: Box::new(recv_hir),
            ty: FieldScalar::Ptr,
        }),
    ];
    let stmts = vec![
        HirStmt::Let {
            name: inner_t.clone(),
            init: inner_init,
            mutable: false,
        },
        HirStmt::Let {
            name: cnt.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(inner_t)),
                index: 0,
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: res_t.clone(),
            init: HirExpr::Alloc {
            slots: res_slots,
            by_value: false,
            is_strfat: false,
        },
            mutable: false,
        },
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Eq,
                Box::new(HirExpr::Variable(cnt)),
                Box::new(HirExpr::IntLiteral(1)),
            )),
            then_block: Box::new(HirBlock {
                stmts: then_stmts,
                final_expr: None,
            }),
            else_block: Some(Box::new(HirBlock {
                stmts: else_stmts,
                final_expr: None,
            })),
        }),
    ];
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(res_t)),
        })),
        Type::Named(
            "Result".to_string(),
            vec![inner.clone(), Type::Named(wrapper.to_string(), vec![inner])],
        ),
    ))
}

pub(super) fn weak_upgrade(
    ctx: &mut TypeContext,
    inner: Type,
    w_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if !args.is_empty() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "Weak::upgrade".to_string(),
            expected: 0,
            found: args.len(),
            span,
        });
    }
    let n = type_slot_count(ctx, &inner, span)?;
    let (some_tag, opt_slots) = enum_variant_info(ctx, "Option", "Some", span)?;
    let (none_tag, _) = enum_variant_info(ctx, "Option", "None", span)?;
    let inner_t = ctx.fresh_temp();
    let cnt = ctx.fresh_temp();
    let opt_t = ctx.fresh_temp();
    let c2 = ctx.fresh_temp();
    let rc_t = ctx.fresh_temp();
    let then_stmts = vec![
        HirStmt::Let {
            name: c2.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(inner_t.clone())),
                index: n,
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(inner_t.clone())),
            index: n,
            value: Box::new(HirExpr::Binary(
                HirBinaryOp::Add,
                Box::new(HirExpr::Variable(c2)),
                Box::new(HirExpr::IntLiteral(1)),
            )),
            ty: FieldScalar::Int,
        }),
        HirStmt::Let {
            name: rc_t.clone(),
            init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
            is_strfat: false,
        },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(rc_t.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(inner_t.clone())),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(opt_t.clone())),
            index: 0,
            value: Box::new(HirExpr::IntLiteral(some_tag as i128)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(opt_t.clone())),
            index: 1,
            value: Box::new(HirExpr::Variable(rc_t)),
            ty: FieldScalar::Ptr,
        }),
    ];
    let else_stmts = vec![HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(opt_t.clone())),
        index: 0,
        value: Box::new(HirExpr::IntLiteral(none_tag as i128)),
        ty: FieldScalar::Int,
    })];
    let stmts = vec![
        HirStmt::Let {
            name: inner_t.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(w_hir),
                index: 0,
                ty: FieldScalar::Ptr,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: cnt.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(inner_t)),
                index: 0,
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: opt_t.clone(),
            init: HirExpr::Alloc {
            slots: opt_slots,
            by_value: false,
            is_strfat: false,
        },
            mutable: false,
        },
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Gt,
                Box::new(HirExpr::Variable(cnt)),
                Box::new(HirExpr::IntLiteral(0)),
            )),
            then_block: Box::new(HirBlock {
                stmts: then_stmts,
                final_expr: None,
            }),
            else_block: Some(Box::new(HirBlock {
                stmts: else_stmts,
                final_expr: None,
            })),
        }),
    ];
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(opt_t)),
        })),
        Type::Named("Option".to_string(), vec![Type::Named("Rc".to_string(), vec![inner])]),
    ))
}

pub(super) fn enum_variant_info(
    ctx: &mut TypeContext,
    enum_name: &str,
    variant: &str,
    span: Span,
) -> Result<(usize, usize), TypeError> {
    let def = ctx.lookup_enum(enum_name).cloned().ok_or_else(|| TypeError::UndefinedType {
        name: enum_name.to_string(),
        span,
    })?;
    let tag = def
        .variants
        .iter()
        .find(|v| v.name == variant)
        .map(|v| v.tag)
        .ok_or_else(|| TypeError::FunctionNotFound {
            name: format!("{enum_name}::{variant}"),
            span,
        })?;
    Ok((tag, def.slot_count))
}
