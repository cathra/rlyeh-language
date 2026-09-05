//! 表达式检查子模块：Box/RC/GC/Weak 智能指针。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
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
    let mut stmts = vec![HirStmt::new(HirStmtKind::Let{
        name: data.clone(),
        init: HirExpr::new(HirExprKind::Call{
            callee: "alloc_bytes".to_string(),
            args: vec![HirExpr::new(HirExprKind::IntLiteral(n_slots as i128 * 8), Span::dummy())],
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy())];
    if v_ty.is_numeric() || matches!(v_ty, Type::Bool | Type::Char) {
        // 标量 T：直接写入堆首槽
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::DerefSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(data.clone()), Span::dummy())),
            value: Box::new(v_hir),
            ty: field_scalar_of(&v_ty),
        }, Span::dummy())), Span::dummy()));
    } else {
        // 聚合 T：整槽区 memcpy（浅拷贝）
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::Call{
            callee: "array_copy".to_string(),
            args: vec![
                HirExpr::new(HirExprKind::Variable(data.clone()), Span::dummy()),
                v_hir,
                HirExpr::new(HirExprKind::IntLiteral(n_slots as i128), Span::dummy()),
            ],
        }, Span::dummy())), Span::dummy()));
    }
    let box_base = ctx.fresh_temp();
    stmts.push(HirStmt::new(HirStmtKind::Let{
        name: box_base.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: 1,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy()));
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(box_base.clone()), Span::dummy())),
        index: 0,
        value: Box::new(HirExpr::new(HirExprKind::Variable(data), Span::dummy())),
        ty: FieldScalar::Ptr,
    }, Span::dummy())), Span::dummy()));
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(box_base), Span::dummy())),
        })), Span::dummy()),
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
    let mut stmts = vec![HirStmt::new(HirStmtKind::Let{
        name: inner.clone(),
        init: HirExpr::new(HirExprKind::Call{
            callee: "alloc_bytes".to_string(),
            args: vec![HirExpr::new(HirExprKind::IntLiteral((n_slots + 2) as i128 * 8), Span::dummy())],
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy())];
    // `T` 值区自堆首槽起（与 `Box<T>` 同构）
    if v_ty.is_numeric() || matches!(v_ty, Type::Bool | Type::Char) {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::DerefSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(inner.clone()), Span::dummy())),
            value: Box::new(v_hir),
            ty: field_scalar_of(&v_ty),
        }, Span::dummy())), Span::dummy()));
    } else {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::Call{
            callee: "array_copy".to_string(),
            args: vec![
                HirExpr::new(HirExprKind::Variable(inner.clone()), Span::dummy()),
                v_hir,
                HirExpr::new(HirExprKind::IntLiteral(n_slots as i128), Span::dummy()),
            ],
        }, Span::dummy())), Span::dummy()));
    }
    // 计数槽（FieldSet base 即堆地址，GEP + store）：strong = n、weak = n + 1
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(inner.clone()), Span::dummy())),
        index: n_slots,
        value: Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
        ty: FieldScalar::Int,
    }, Span::dummy())), Span::dummy()));
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(inner.clone()), Span::dummy())),
        index: n_slots + 1,
        value: Box::new(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())),
        ty: FieldScalar::Int,
    }, Span::dummy())), Span::dummy()));
    let rc_base = ctx.fresh_temp();
    stmts.push(HirStmt::new(HirStmtKind::Let{
        name: rc_base.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: 1,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy()));
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(rc_base.clone()), Span::dummy())),
        index: 0,
        value: Box::new(HirExpr::new(HirExprKind::Variable(inner), Span::dummy())),
        ty: FieldScalar::Ptr,
    }, Span::dummy())), Span::dummy()));
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(rc_base), Span::dummy())),
        })), Span::dummy()),
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
    let mut stmts = vec![HirStmt::new(HirStmtKind::Let{
        name: inner.clone(),
        init: HirExpr::new(HirExprKind::Call{
            callee: "rlyeh_gc_alloc".to_string(),
            args: vec![HirExpr::new(HirExprKind::IntLiteral(n_slots as i128), Span::dummy())],
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy())];
    // `T` 值区自堆首槽起（与 `Box<T>` 同构）
    if v_ty.is_numeric() || matches!(v_ty, Type::Bool | Type::Char) {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::DerefSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(inner.clone()), Span::dummy())),
            value: Box::new(v_hir),
            ty: field_scalar_of(&v_ty),
        }, Span::dummy())), Span::dummy()));
    } else {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::Call{
            callee: "array_copy".to_string(),
            args: vec![
                HirExpr::new(HirExprKind::Variable(inner.clone()), Span::dummy()),
                v_hir,
                HirExpr::new(HirExprKind::IntLiteral(n_slots as i128), Span::dummy()),
            ],
        }, Span::dummy())), Span::dummy()));
    }
    let gc_base = ctx.fresh_temp();
    stmts.push(HirStmt::new(HirStmtKind::Let{
        name: gc_base.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: 1,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy()));
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(gc_base.clone()), Span::dummy())),
        index: 0,
        value: Box::new(HirExpr::new(HirExprKind::Variable(inner), Span::dummy())),
        ty: FieldScalar::Ptr,
    }, Span::dummy())), Span::dummy()));
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(gc_base), Span::dummy())),
        })), Span::dummy()),
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
        HirStmt::new(HirStmtKind::Let{
            name: inner_t.clone(),
            init: HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(recv_hir),
                index: 0,
                ty: FieldScalar::Ptr,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: cnt.clone(),
            init: HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(inner_t.clone()), Span::dummy())),
                index: n,
                ty: FieldScalar::Int,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(inner_t.clone()), Span::dummy())),
            index: n,
            value: Box::new(HirExpr::new(HirExprKind::Binary(
                HirBinaryOp::Add,
                Box::new(HirExpr::new(HirExprKind::Variable(cnt), Span::dummy())),
                Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
            ), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: rc_t.clone(),
            init: HirExpr::new(HirExprKind::Alloc{
            slots: 1,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(rc_t.clone()), Span::dummy())),
            index: 0,
            value: Box::new(HirExpr::new(HirExprKind::Variable(inner_t), Span::dummy())),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
    ];
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(rc_t), Span::dummy())),
        })), Span::dummy()),
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
    let v = HirExpr::new(HirExprKind::FieldGet{
        base: Box::new(HirExpr::new(HirExprKind::FieldGet{
            base: Box::new(recv_hir),
            index: 0,
            ty: FieldScalar::Ptr,
        }, Span::dummy())),
        index: n + off,
        ty: FieldScalar::Int,
    }, Span::dummy());
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
        HirStmt::new(HirStmtKind::Let{
            name: inner_t.clone(),
            init: HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(recv_hir),
                index: 0,
                ty: FieldScalar::Ptr,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: cnt.clone(),
            init: HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(inner_t.clone()), Span::dummy())),
                index: n + 1,
                ty: FieldScalar::Int,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(inner_t.clone()), Span::dummy())),
            index: n + 1,
            value: Box::new(HirExpr::new(HirExprKind::Binary(
                HirBinaryOp::Add,
                Box::new(HirExpr::new(HirExprKind::Variable(cnt), Span::dummy())),
                Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
            ), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: w_t.clone(),
            init: HirExpr::new(HirExprKind::Alloc{
            slots: 1,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(w_t.clone()), Span::dummy())),
            index: 0,
            value: Box::new(HirExpr::new(HirExprKind::Variable(inner_t), Span::dummy())),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
    ];
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(w_t), Span::dummy())),
        })), Span::dummy()),
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
    let value_base = HirExpr::new(HirExprKind::FieldGet{
        base: Box::new(recv_hir.clone()),
        index: 0,
        ty: FieldScalar::Ptr,
    }, Span::dummy());
    let inner_init = HirExpr::new(HirExprKind::FieldGet{
        base: Box::new(recv_hir.clone()),
        index: 0,
        ty: FieldScalar::Ptr,
    }, Span::dummy());
    // `Ok(v)` 负载：标量 load 值区 / 聚合取值区指针（与 `*rc` 解引用一致）
    let ok_val = if inner.is_numeric() || matches!(inner, Type::Bool | Type::Char) {
        HirExpr::new(HirExprKind::Deref{
            expr: Box::new(value_base),
            ty: field_scalar_of(&inner),
        }, Span::dummy())
    } else {
        value_base
    };
    let (ok_tag, res_slots) = enum_variant_info(ctx, "Result", "Ok", span)?;
    let (err_tag, _) = enum_variant_info(ctx, "Result", "Err", span)?;
    let inner_t = ctx.fresh_temp();
    let cnt = ctx.fresh_temp();
    let res_t = ctx.fresh_temp();
    let then_stmts = vec![
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(res_t.clone()), Span::dummy())),
            index: 0,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(ok_tag as i128), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(res_t.clone()), Span::dummy())),
            index: 1,
            value: Box::new(ok_val),
            ty: field_scalar_of(&inner),
        }, Span::dummy())), Span::dummy()),
    ];
    let else_stmts = vec![
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(res_t.clone()), Span::dummy())),
            index: 0,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(err_tag as i128), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(res_t.clone()), Span::dummy())),
            index: 1,
            value: Box::new(recv_hir),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
    ];
    let stmts = vec![
        HirStmt::new(HirStmtKind::Let{
            name: inner_t.clone(),
            init: inner_init,
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: cnt.clone(),
            init: HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(inner_t), Span::dummy())),
                index: 0,
                ty: FieldScalar::Int,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: res_t.clone(),
            init: HirExpr::new(HirExprKind::Alloc{
            slots: res_slots,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::If{
            cond: Box::new(HirExpr::new(HirExprKind::Binary(
                HirBinaryOp::Eq,
                Box::new(HirExpr::new(HirExprKind::Variable(cnt), Span::dummy())),
                Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
            ), Span::dummy())),
            then_block: Box::new(HirBlock { span: Span::dummy(),
                stmts: then_stmts,
                final_expr: None,
            }),
            else_block: Some(Box::new(HirBlock { span: Span::dummy(),
                stmts: else_stmts,
                final_expr: None,
            })),
        }, Span::dummy())), Span::dummy()),
    ];
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(res_t), Span::dummy())),
        })), Span::dummy()),
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
        HirStmt::new(HirStmtKind::Let{
            name: c2.clone(),
            init: HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(inner_t.clone()), Span::dummy())),
                index: n,
                ty: FieldScalar::Int,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(inner_t.clone()), Span::dummy())),
            index: n,
            value: Box::new(HirExpr::new(HirExprKind::Binary(
                HirBinaryOp::Add,
                Box::new(HirExpr::new(HirExprKind::Variable(c2), Span::dummy())),
                Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
            ), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: rc_t.clone(),
            init: HirExpr::new(HirExprKind::Alloc{
            slots: 1,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(rc_t.clone()), Span::dummy())),
            index: 0,
            value: Box::new(HirExpr::new(HirExprKind::Variable(inner_t.clone()), Span::dummy())),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(opt_t.clone()), Span::dummy())),
            index: 0,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(some_tag as i128), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(opt_t.clone()), Span::dummy())),
            index: 1,
            value: Box::new(HirExpr::new(HirExprKind::Variable(rc_t), Span::dummy())),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
    ];
    let else_stmts = vec![HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(opt_t.clone()), Span::dummy())),
        index: 0,
        value: Box::new(HirExpr::new(HirExprKind::IntLiteral(none_tag as i128), Span::dummy())),
        ty: FieldScalar::Int,
    }, Span::dummy())), Span::dummy())];
    let stmts = vec![
        HirStmt::new(HirStmtKind::Let{
            name: inner_t.clone(),
            init: HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(w_hir),
                index: 0,
                ty: FieldScalar::Ptr,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: cnt.clone(),
            init: HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(inner_t), Span::dummy())),
                index: 0,
                ty: FieldScalar::Int,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: opt_t.clone(),
            init: HirExpr::new(HirExprKind::Alloc{
            slots: opt_slots,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::If{
            cond: Box::new(HirExpr::new(HirExprKind::Binary(
                HirBinaryOp::Gt,
                Box::new(HirExpr::new(HirExprKind::Variable(cnt), Span::dummy())),
                Box::new(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())),
            ), Span::dummy())),
            then_block: Box::new(HirBlock { span: Span::dummy(),
                stmts: then_stmts,
                final_expr: None,
            }),
            else_block: Some(Box::new(HirBlock { span: Span::dummy(),
                stmts: else_stmts,
                final_expr: None,
            })),
        }, Span::dummy())), Span::dummy()),
    ];
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(opt_t), Span::dummy())),
        })), Span::dummy()),
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
