//! construct/collection：`HashSet` / `BTreeMap` / 迭代器构造器。
//! （由 construct.rs 拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use super::*;

pub(crate) fn check_hashset_construct(
    ctx: &mut TypeContext,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let cap = if method == "new" {
        AstExpr::new(ExprKind::IntLiteral(8), span)
    } else if args.len() == 1 {
        args[0].clone()
    } else {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("HashSet::{method}"),
            expected: 1,
            found: args.len(),
            span,
        });
    };
    let (cap_hir, cap_ty) = infer_expr(ctx, &cap)?;
    if !cap_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: cap_ty.to_string(),
            span,
        });
    }
    let cap_hir = if method == "with_capacity" {
        HirExpr::new(HirExprKind::Call{
            callee: "next_pow2".to_string(),
            args: vec![cap_hir],
        }, Span::dummy())
    } else {
        cap_hir
    };

    let items_tmp = ctx.fresh_temp();
    let states_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let cap_for_alloc = cap_hir.clone();
    let stmts = vec![
        HirStmt::new(HirStmtKind::Let{
            name: items_tmp.clone(),
            init: HirExpr::new(HirExprKind::Call{
                callee: "alloc_array".to_string(),
                args: vec![cap_for_alloc],
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: states_tmp.clone(),
            init: HirExpr::new(HirExprKind::Call{
                callee: "alloc_array".to_string(),
                args: vec![cap_hir.clone()],
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: base.clone(),
            init: HirExpr::new(HirExprKind::Alloc{
                slots: 5,
                by_value: false,
                is_strfat: false,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 0,
            value: Box::new(HirExpr::new(HirExprKind::Variable(items_tmp), Span::dummy())),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 1,
            value: Box::new(HirExpr::new(HirExprKind::Variable(states_tmp), Span::dummy())),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 2,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 3,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 4,
            value: Box::new(cap_hir),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
    ];

    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(base), Span::dummy())),
        })), Span::dummy()),
        Type::Named("HashSet".to_string(), vec![Type::Infer]),
    ))
}

pub(crate) fn check_btreemap_construct(
    ctx: &mut TypeContext,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let cap = if method == "new" {
        AstExpr::new(ExprKind::IntLiteral(8), span)
    } else if args.len() == 1 {
        args[0].clone()
    } else {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("BTreeMap::{method}"),
            expected: 1,
            found: args.len(),
            span,
        });
    };
    let (cap_hir, cap_ty) = infer_expr(ctx, &cap)?;
    if !cap_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: cap_ty.to_string(),
            span,
        });
    }
    let cap_hir = if method == "with_capacity" {
        HirExpr::new(HirExprKind::Call{
            callee: "next_pow2".to_string(),
            args: vec![cap_hir],
        }, Span::dummy())
    } else {
        cap_hir
    };

    let keys_tmp = ctx.fresh_temp();
    let vals_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let cap_for_alloc = cap_hir.clone();
    let stmts = vec![
        HirStmt::new(HirStmtKind::Let{
            name: keys_tmp.clone(),
            init: HirExpr::new(HirExprKind::Call{
                callee: "alloc_array".to_string(),
                args: vec![cap_for_alloc],
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: vals_tmp.clone(),
            init: HirExpr::new(HirExprKind::Call{
                callee: "alloc_array".to_string(),
                args: vec![cap_hir.clone()],
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: base.clone(),
            init: HirExpr::new(HirExprKind::Alloc{
                slots: 3,
                by_value: false,
                is_strfat: false,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 0,
            value: Box::new(HirExpr::new(HirExprKind::Variable(keys_tmp), Span::dummy())),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 1,
            value: Box::new(HirExpr::new(HirExprKind::Variable(vals_tmp), Span::dummy())),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 2,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
    ];

    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(base), Span::dummy())),
        })), Span::dummy()),
        Type::Named("BTreeMap".to_string(), vec![Type::Infer, Type::Infer]),
    ))
}

pub(crate) fn check_iter_construct(
    ctx: &mut TypeContext,
    name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let expected = if name == "Iter" || name == "IterRef" { 2 } else { 3 };
    if args.len() != expected {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{name}::new"),
            expected,
            found: args.len(),
            span,
        });
    }
    // data：裸指针（*const T / *mut T），从内项反推元素类型 T
    let (data_hir, data_ty) = infer_expr(ctx, &args[0])?;
    let Type::RawPtr(elem, _) = data_ty else {
        return Err(TypeError::WrongType {
            expected: "裸指针（*const T / *mut T）".to_string(),
            found: data_ty.to_string(),
            span: args[0].span,
            related: vec![],
        });
    };
    // cur（仅 IterMut）：裸指针
    let cur_hir = if name == "IterMut" {
        let (cur_hir, cur_ty) = infer_expr(ctx, &args[1])?;
        if !matches!(cur_ty, Type::RawPtr(..)) {
            return Err(TypeError::WrongType {
                expected: "裸指针（*mut T）".to_string(),
                found: cur_ty.to_string(),
                span: args[1].span,
                related: vec![],
            });
        }
        cur_hir
    } else {
        HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())
    };
    // len：整数（剩余元素数）
    let (len_hir, len_ty) = infer_expr(ctx, &args[expected - 1])?;
    if !len_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: len_ty.to_string(),
            span: args[expected - 1].span,
        });
    }

    let base = ctx.fresh_temp();
    let mut stmts = vec![
        HirStmt::new(HirStmtKind::Let{
            name: base.clone(),
            init: HirExpr::new(HirExprKind::Alloc{
                slots: expected,
                by_value: name == "Iter",
                is_strfat: false,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 0,
            value: Box::new(data_hir),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
    ];
    // IterMut 额外写 cur 槽（槽 1）；Iter 直接由 len 写槽 1
    if name == "IterMut" {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 1,
            value: Box::new(cur_hir),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()));
    }
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
        index: expected - 1,
        value: Box::new(len_hir),
        ty: FieldScalar::Int,
    }, Span::dummy())), Span::dummy()));

    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(base), Span::dummy())),
        })), Span::dummy()),
        Type::Named(name.to_string(), vec![*elem]),
    ))
}

