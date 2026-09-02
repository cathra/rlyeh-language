//! construct/collection：`HashSet` / `BTreeMap` / 迭代器构造器。
//! （由 construct.rs 拆分而来，保持语义等价）

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
        HirExpr::Call {
            callee: "next_pow2".to_string(),
            args: vec![cap_hir],
        }
    } else {
        cap_hir
    };

    let items_tmp = ctx.fresh_temp();
    let states_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let cap_for_alloc = cap_hir.clone();
    let stmts = vec![
        HirStmt::Let {
            name: items_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_for_alloc],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: states_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_hir.clone()],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: base.clone(),
            init: HirExpr::Alloc {
                slots: 5,
                by_value: false,
                is_strfat: false,
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(items_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(HirExpr::Variable(states_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 2,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 3,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 4,
            value: Box::new(cap_hir),
            ty: FieldScalar::Int,
        }),
    ];

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
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
        HirExpr::Call {
            callee: "next_pow2".to_string(),
            args: vec![cap_hir],
        }
    } else {
        cap_hir
    };

    let keys_tmp = ctx.fresh_temp();
    let vals_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let cap_for_alloc = cap_hir.clone();
    let stmts = vec![
        HirStmt::Let {
            name: keys_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_for_alloc],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: vals_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_hir.clone()],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: base.clone(),
            init: HirExpr::Alloc {
                slots: 3,
                by_value: false,
                is_strfat: false,
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(keys_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(HirExpr::Variable(vals_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 2,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }),
    ];

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
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
            });
        }
        cur_hir
    } else {
        HirExpr::IntLiteral(0)
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
        HirStmt::Let {
            name: base.clone(),
            init: HirExpr::Alloc {
                slots: expected,
                by_value: name == "Iter",
                is_strfat: false,
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 0,
            value: Box::new(data_hir),
            ty: FieldScalar::Ptr,
        }),
    ];
    // IterMut 额外写 cur 槽（槽 1）；Iter 直接由 len 写槽 1
    if name == "IterMut" {
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(cur_hir),
            ty: FieldScalar::Ptr,
        }));
    }
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(base.clone())),
        index: expected - 1,
        value: Box::new(len_hir),
        ty: FieldScalar::Int,
    }));

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named(name.to_string(), vec![*elem]),
    ))
}

