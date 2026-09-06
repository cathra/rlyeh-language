//! 表达式检查子模块：for 循环与迭代器适配器。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use super::*;

pub(super) fn check_gc_region(
    ctx: &mut TypeContext,
    body: &AstBlock,
    _span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let (mut hir, ty) = check_block(ctx, body)?;
    let mut stmts = std::mem::take(&mut hir.stmts);
    // 块入口：快照外层活跃对象
    stmts.insert(
        0,
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::Call{
            callee: "rlyeh_gc_region_begin".to_string(),
            args: vec![],
        }, Span::dummy())), Span::dummy()),
    );
    if let Some(f) = hir.final_expr.take() {
        // 逃逸值保护：仅直接 `Gc<T>` 类型登记（标量 / () / 非 Gc 聚合直接收集）
        if matches!(peel_ref(&ty), Type::Named(n, _) if n == "Gc") {
            let esc = ctx.fresh_temp();
            stmts.push(HirStmt::new(HirStmtKind::Let{
                name: esc.clone(),
                init: f,
                mutable: false,
            }, Span::dummy()));
            stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::Call{
                callee: "rlyeh_gc_escape".to_string(),
                // Gc<T> 变量 = 栈槽 → 堆 1-槽包装（槽 0 存 GcInner base）；
                // heap_ptr_hir 解包装槽 0 得对象 base（与 rlyeh_gc_alloc 注册一致），
                // 传包装指针会导致 mark 线性查找失配、对象被误回收（悬垂读取）。
                args: vec![heap_ptr_hir(HirExpr::new(HirExprKind::Variable(esc.clone()), Span::dummy()), &ty)],
            }, Span::dummy())), Span::dummy()));
            stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::Call{
                callee: "rlyeh_gc_collect".to_string(),
                args: vec![],
            }, Span::dummy())), Span::dummy()));
            return Ok((
                HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                    stmts,
                    final_expr: Some(HirExpr::new(HirExprKind::Variable(esc), Span::dummy())),
                })), Span::dummy()),
                ty,
            ));
        }
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::Call{
            callee: "rlyeh_gc_collect".to_string(),
            args: vec![],
        }, Span::dummy())), Span::dummy()));
        Ok((
            HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                stmts,
                final_expr: Some(f),
            })), Span::dummy()),
            ty,
        ))
    } else {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::Call{
            callee: "rlyeh_gc_collect".to_string(),
            args: vec![],
        }, Span::dummy())), Span::dummy()));
        Ok((
            HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                stmts,
                final_expr: None,
            })), Span::dummy()),
            Type::Unit,
        ))
    }
}

pub(super) fn check_for(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iterator: &AstExpr,
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if matches!(&*iterator.kind, ExprKind::Range { .. }) {
        return check_for_range(ctx, pattern, iterator, body, span);
    }
    let (iter_hir, iter_ty) = infer_expr(ctx, iterator)?;
    if let Type::Named(n, args) = peel_ref(&iter_ty) {
        let full = ctx.resolve_full_name(&n).unwrap_or_else(|| n.clone());
        if full == "Vec" && ctx.lookup_struct(&full).is_some() {
            return check_for_vec(ctx, pattern, iter_hir, &args, body, span);
        }
        if full == "HashMap" && ctx.lookup_struct(&full).is_some() {
            return check_for_hashmap(ctx, pattern, iter_hir, &args, body, span);
        }
    }
    if let Type::Array(elem, n) = peel_ref(&iter_ty) {
        return check_for_array(ctx, pattern, iter_hir, elem.as_ref(), n, body, span);
    }
    // 自定义迭代器（J2）：接收者类型存在 `next() -> Option<Item>` 方法
    // （inherent 或 trait impl）→ 经 check_for_iterator 接入 for 循环。
    let concrete = peel_ref(&iter_ty);
    if let Some(imp) = ctx.find_impl_for_method(&concrete, "next").cloned() {
        if let Some(md) = imp.methods.iter().find(|m| m.sig.name == "next") {
            let mut subst: HashMap<String, Type> = HashMap::new();
            if unify(&imp.self_type, &concrete, &mut subst).is_ok() {
                let ret = substitute(&md.sig.return_type, &subst);
                let named = match peel_ref(&ret) {
                    Type::Named(n, _) => ctx.resolve_full_name(n.as_str()).unwrap_or_else(|| n.clone()),
                    _ => String::new(),
                };
                if named == "Option" {
                    return check_for_iterator(ctx, pattern, iterator, body, span);
                }
            }
        }
    }
    Err(TypeError::Unsupported {
        what: "非 range / Vec / HashMap / 数组 / 迭代器（含 next() 方法的类型）的 for 循环（集合 / 容器迭代 MVP 阶段仅支持 Vec、HashMap、数组与自定义迭代器）"
            .to_string(),
        span,
    })
}

pub(super) fn check_for_range(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iterator: &AstExpr,
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 1. 迭代器必须为 range 表达式（集合 / 容器迭代待后续支持）
    let (lower, upper, lower_inclusive, upper_inclusive) = match &*iterator.kind {
        ExprKind::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => {
            // P8：for 范围迭代须显式边界（省略 `..` 仅切片 `v[..]` 支持）
            let lower = lower.as_ref().ok_or_else(|| TypeError::Unsupported {
                what: "for 范围迭代器缺少下界（省略边界 `..` 仅切片 `v[..]` 支持）".to_string(),
                span,
            })?;
            let upper = upper.as_ref().ok_or_else(|| TypeError::Unsupported {
                what: "for 范围迭代器缺少上界（省略边界 `..` 仅切片 `v[..]` 支持）".to_string(),
                span,
            })?;
            (lower, upper, *lower_inclusive, *upper_inclusive)
        }
        _ => {
            return Err(TypeError::Unsupported {
                what: "非 range 迭代器的 for 循环（集合 / 容器迭代在 MVP 阶段）".to_string(),
                span,
            })
        }
    };

    // 2. 循环变量必须是标识符
    let name = match pattern {
        AstPattern::Ident(n) => n.clone(),
        _ => {
            return Err(TypeError::Unsupported {
                what: "for 循环复杂模式（元组 / 结构体解构）".to_string(),
                span,
            })
        }
    };

    // 3. 边界表达式类型检查（必须为整数且类型一致）
    let (lo_hir, lo_ty) = infer_expr(ctx, lower)?;
    let (hi_hir, hi_ty) = infer_expr(ctx, upper)?;
    if !lo_ty.is_integer() || !hi_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: lo_ty.to_string(),
            span,
        });
    }
    if !lo_ty.compatible_with(&hi_ty) {
        return Err(TypeError::ChainTypeMismatch { span });
    }

    // 4. 唯一临时名（避免与用户变量冲突）
    let lo_name = format!("__for_lo_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let hi_name = format!("__for_hi_{}", ctx.temp_counter);
    ctx.temp_counter += 1;

    // 5. 进入循环作用域（迭代变量 / 临时变量在作用域内；结束弹出）。
    //    存储槽名：循环变量被外层同名遮蔽时 mangle，Let/引用/Assign 全部用槽名。
    ctx.push_scope(false);
    let stored_lo = ctx.insert_variable(lo_name.clone(), lo_ty.clone());
    let stored_hi = ctx.insert_variable(hi_name.clone(), hi_ty.clone());
    let stored_name = ctx.insert_variable(name.clone(), lo_ty.clone());

    // 6. 起始值：下界开区间时从 lo + 1 开始；
    //    循环变量初始化为 `start - 1`，配合循环体开头的 `pat += 1`，
    //    使第一次迭代 pat == start。
    let start = if lower_inclusive {
        HirExpr::new(HirExprKind::Variable(stored_lo.clone()), Span::dummy())
    } else {
        HirExpr::new(HirExprKind::Binary(
            HirBinaryOp::Add,
            Box::new(HirExpr::new(HirExprKind::Variable(stored_lo.clone()), Span::dummy())),
            Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
        ), Span::dummy())
    };
    let init = HirExpr::new(HirExprKind::Binary(
        HirBinaryOp::Sub,
        Box::new(start),
        Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
    ), Span::dummy());

    let mut stmts = vec![
        HirStmt::new(HirStmtKind::Let{
            name: stored_lo.clone(),
            init: lo_hir,
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: stored_hi.clone(),
            init: hi_hir,
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: stored_name.clone(),
            init,
            mutable: true,
        }, Span::dummy()),
    ];

    // 7. 循环体检查
    let (b_hir, _) = check_block(ctx, body)?;
    ctx.pop_scope();

    // 8. 退出条件：`pat >= hi`（上界闭区间为 `pat > hi`）。
    //    注意用 `loop` 而非 `while`：循环体开头的 `pat += 1` 在每次
    //    迭代（含 continue 回跳）时都会执行，保证 continue 不会跳过递增。
    let exit_op = if upper_inclusive {
        HirBinaryOp::Gt
    } else {
        HirBinaryOp::Ge
    };
    let exit_cond = HirExpr::new(HirExprKind::Binary(
        exit_op,
        Box::new(HirExpr::new(HirExprKind::Variable(stored_name.clone()), Span::dummy())),
        Box::new(HirExpr::new(HirExprKind::Variable(stored_hi), Span::dummy())),
    ), Span::dummy());

    // 9. loop 体：`pat += 1` → 退出判断 → 原 body 语句
    let mut loop_body_stmts = vec![
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Assign{
            target: stored_name.clone(),
            op: HirAssignOp::AddAssign,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::If{
            cond: Box::new(exit_cond),
            then_block: Box::new(HirBlock { span: Span::dummy(),
                stmts: vec![HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Break(None), Span::dummy())), Span::dummy())],
                final_expr: None,
            }),
            else_block: None,
        }, Span::dummy())), Span::dummy()),
    ];
    loop_body_stmts.extend(b_hir.stmts);
    if let Some(fe) = b_hir.final_expr {
        loop_body_stmts.push(HirStmt::new(HirStmtKind::Expr(fe), Span::dummy()));
    }
    let loop_expr = HirExpr::new(HirExprKind::Loop{
        body: Box::new(HirBlock { span: Span::dummy(),
            stmts: loop_body_stmts,
            final_expr: None,
        }),
    }, Span::dummy());

    stmts.push(HirStmt::new(HirStmtKind::Expr(loop_expr), Span::dummy()));
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: None,
        })), Span::dummy()),
        Type::Unit,
    ))
}

pub(super) fn check_for_vec(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iter_hir: HirExpr,
    args: &[Type],
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 1. 元素类型：`Vec<T>` 的类型参数经泛型替换；Infer 无法确定槽标量
    let elem_ty = substitute(args.first().unwrap_or(&Type::Infer), &ctx.generic_subst);
    if matches!(elem_ty, Type::Infer) {
        return Err(TypeError::Unsupported {
            what: "`for ... in` 的 Vec 迭代要求元素类型确定（如 `let v: Vec<i64> = ...`）"
                .to_string(),
            span,
        });
    }

    // 2. 循环变量必须是标识符
    let name = match pattern {
        AstPattern::Ident(n) => n.clone(),
        _ => {
            return Err(TypeError::Unsupported {
                what: "for 循环复杂模式（元组 / 结构体解构）".to_string(),
                span,
            })
        }
    };

    // 3. 唯一临时名（避免与用户变量冲突）
    let v_name = format!("__for_v_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let len_name = format!("__for_len_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let i_name = format!("__for_i_{}", ctx.temp_counter);
    ctx.temp_counter += 1;

    // 4. 进入循环作用域 + 绑定临时变量（结束弹出）。
    //    存储槽名：循环变量被外层同名遮蔽时 mangle，Let/引用/Assign 全部用槽名。
    ctx.push_scope(false);
    let vec_ty = Type::Named("Vec".to_string(), vec![elem_ty.clone()]);
    let stored_v = ctx.insert_variable(v_name.clone(), vec_ty);
    let stored_len = ctx.insert_variable(len_name.clone(), Type::I64);
    let stored_i = ctx.insert_variable(i_name.clone(), Type::I64);
    let stored_name = ctx.insert_variable(name.clone(), elem_ty.clone());

    // 5. 前缀语句：绑定容器、缓存长度、初始化计数器
    let mut stmts = vec![
        HirStmt::new(HirStmtKind::Let{
            name: stored_v.clone(),
            init: iter_hir,
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: stored_len.clone(),
            init: HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(stored_v.clone()), Span::dummy())),
                index: 1, // Vec 槽 1 = len
                ty: FieldScalar::Int,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: stored_i.clone(),
            init: HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy()),
            mutable: true,
        }, Span::dummy()),
    ];

    // 7. 循环体检查
    let (b_hir, _) = check_block(ctx, body)?;
    ctx.pop_scope();

    // 8. loop 体：边界检查 → 取元素绑定 → 递增 → 原 body 语句
    let elem_scalar = field_scalar_of(&elem_ty);
    let mut loop_body_stmts = vec![
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::If{
            cond: Box::new(HirExpr::new(HirExprKind::Binary(
                HirBinaryOp::Ge,
                Box::new(HirExpr::new(HirExprKind::Variable(stored_i.clone()), Span::dummy())),
                Box::new(HirExpr::new(HirExprKind::Variable(stored_len.clone()), Span::dummy())),
            ), Span::dummy())),
            then_block: Box::new(HirBlock { span: Span::dummy(),
                stmts: vec![HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Break(None), Span::dummy())), Span::dummy())],
                final_expr: None,
            }),
            else_block: None,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: stored_name.clone(),
            init: HirExpr::new(HirExprKind::Index{
                base: Box::new(HirExpr::new(HirExprKind::FieldGet{
                    base: Box::new(HirExpr::new(HirExprKind::Variable(stored_v.clone()), Span::dummy())),
                    index: 0, // Vec 槽 0 = data 指针
                    ty: FieldScalar::Ptr,
                }, Span::dummy())),
                index: Box::new(HirExpr::new(HirExprKind::Variable(stored_i.clone()), Span::dummy())),
                elem: elem_scalar,
                is_str: false,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Assign{
            target: stored_i.clone(),
            op: HirAssignOp::AddAssign,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
        }, Span::dummy())), Span::dummy()),
    ];
    loop_body_stmts.extend(b_hir.stmts);
    if let Some(fe) = b_hir.final_expr {
        loop_body_stmts.push(HirStmt::new(HirStmtKind::Expr(fe), Span::dummy()));
    }
    let loop_expr = HirExpr::new(HirExprKind::Loop{
        body: Box::new(HirBlock { span: Span::dummy(),
            stmts: loop_body_stmts,
            final_expr: None,
        }),
    }, Span::dummy());

    stmts.push(HirStmt::new(HirStmtKind::Expr(loop_expr), Span::dummy()));
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: None,
        })), Span::dummy()),
        Type::Unit,
    ))
}

pub(super) fn check_for_array(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iter_hir: HirExpr,
    elem_ty: &Type,
    n: usize,
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 1. 元素类型经泛型替换；Infer 无法确定槽标量
    let elem_ty = substitute(elem_ty, &ctx.generic_subst);
    if matches!(elem_ty, Type::Infer) {
        return Err(TypeError::Unsupported {
            what: "`for ... in` 的数组迭代要求元素类型确定（如 `let arr: [i64; N] = ...`）"
                .to_string(),
            span,
        });
    }

    // 2. 循环变量必须是标识符
    let name = match pattern {
        AstPattern::Ident(n) => n.clone(),
        _ => {
            return Err(TypeError::Unsupported {
                what: "for 循环复杂模式（元组 / 结构体解构）".to_string(),
                span,
            })
        }
    };

    // 3. 唯一临时名（避免与用户变量冲突）
    let arr_name = format!("__for_arr_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let i_name = format!("__for_i_{}", ctx.temp_counter);
    ctx.temp_counter += 1;

    // 4. 进入循环作用域 + 绑定临时变量（结束弹出）。
    //    存储槽名：循环变量被外层同名遮蔽时 mangle，Let/引用/Assign 全部用槽名。
    ctx.push_scope(false);
    let arr_ty = Type::Array(Box::new(elem_ty.clone()), n);
    let stored_arr = ctx.insert_variable(arr_name.clone(), arr_ty);
    let stored_i = ctx.insert_variable(i_name.clone(), Type::I64);
    let stored_name = ctx.insert_variable(name.clone(), elem_ty.clone());

    // 5. 前缀语句：绑定数组、初始化计数器
    let mut stmts = vec![
        HirStmt::new(HirStmtKind::Let{
            name: stored_arr.clone(),
            init: iter_hir,
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: stored_i.clone(),
            init: HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy()),
            mutable: true,
        }, Span::dummy()),
    ];

    // 6. 循环体检查
    let (b_hir, _) = check_block(ctx, body)?;
    ctx.pop_scope();

    // 7. loop 体：边界检查 → 取元素绑定 → 递增 → 原 body 语句
    let elem_scalar = field_scalar_of(&elem_ty);
    let is_byte = matches!(elem_ty, Type::U8);
    let mut loop_body_stmts = vec![
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::If{
            cond: Box::new(HirExpr::new(HirExprKind::Binary(
                HirBinaryOp::Ge,
                Box::new(HirExpr::new(HirExprKind::Variable(stored_i.clone()), Span::dummy())),
                Box::new(HirExpr::new(HirExprKind::IntLiteral(n as i128), Span::dummy())),
            ), Span::dummy())),
            then_block: Box::new(HirBlock { span: Span::dummy(),
                stmts: vec![HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Break(None), Span::dummy())), Span::dummy())],
                final_expr: None,
            }),
            else_block: None,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: stored_name.clone(),
            init: HirExpr::new(HirExprKind::Index{
                base: Box::new(HirExpr::new(HirExprKind::Variable(stored_arr.clone()), Span::dummy())),
                index: Box::new(HirExpr::new(HirExprKind::Variable(stored_i.clone()), Span::dummy())),
                elem: elem_scalar,
                is_str: is_byte,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Assign{
            target: stored_i.clone(),
            op: HirAssignOp::AddAssign,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
        }, Span::dummy())), Span::dummy()),
    ];
    loop_body_stmts.extend(b_hir.stmts);
    if let Some(fe) = b_hir.final_expr {
        loop_body_stmts.push(HirStmt::new(HirStmtKind::Expr(fe), Span::dummy()));
    }
    let loop_expr = HirExpr::new(HirExprKind::Loop{
        body: Box::new(HirBlock { span: Span::dummy(),
            stmts: loop_body_stmts,
            final_expr: None,
        }),
    }, Span::dummy());

    stmts.push(HirStmt::new(HirStmtKind::Expr(loop_expr), Span::dummy()));
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: None,
        })), Span::dummy()),
        Type::Unit,
    ))
}


pub(super) fn check_for_hashmap(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iter_hir: HirExpr,
    args: &[Type],
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 1. 键值类型：`HashMap<K, V>` 的类型参数经泛型替换；Infer 无法确定槽标量
    let k_ty = substitute(args.first().unwrap_or(&Type::Infer), &ctx.generic_subst);
    let v_ty = substitute(args.get(1).unwrap_or(&Type::Infer), &ctx.generic_subst);
    if matches!(k_ty, Type::Infer) || matches!(v_ty, Type::Infer) {
        return Err(TypeError::Unsupported {
            what: "`for ... in` 的 HashMap 迭代要求键值类型确定（如 `let m: HashMap<i64, i64> = ...`）"
                .to_string(),
            span,
        });
    }

    // 2. 模式必须为 `(k, v)` 二元元组，元素均为标识符
    let (k_name, v_name) = match pattern {
        AstPattern::Tuple(pats, _) if pats.len() == 2 => {
            match (&pats[0], &pats[1]) {
                (AstPattern::Ident(k), AstPattern::Ident(v)) => (k.clone(), v.clone()),
                _ => {
                    return Err(TypeError::Unsupported {
                        what: "HashMap 迭代模式必须为 `(k, v)` 标识符对".to_string(),
                        span,
                    })
                }
            }
        }
        _ => {
            return Err(TypeError::Unsupported {
                what: "HashMap 迭代必须使用 `for (k, v) in m` 元组模式".to_string(),
                span,
            })
        }
    };

    // 3. 唯一临时名（避免与用户变量冲突）
    let m_name = format!("__for_m_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let cap_name = format!("__for_cap_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let i_name = format!("__for_i_{}", ctx.temp_counter);
    ctx.temp_counter += 1;

    // 4. 进入循环作用域 + 绑定临时变量（结束弹出）。
    //    存储槽名：k/v 被外层同名遮蔽时 mangle，Let/引用/Assign 全部用槽名。
    ctx.push_scope(false);
    let map_ty = Type::Named("HashMap".to_string(), vec![k_ty.clone(), v_ty.clone()]);
    let stored_m = ctx.insert_variable(m_name.clone(), map_ty);
    let stored_cap = ctx.insert_variable(cap_name.clone(), Type::I64);
    let stored_i = ctx.insert_variable(i_name.clone(), Type::I64);
    let stored_k = ctx.insert_variable(k_name.clone(), k_ty.clone());
    let stored_v = ctx.insert_variable(v_name.clone(), v_ty.clone());

    // 5. 前缀语句：绑定容器、缓存容量、初始化计数器
    let mut stmts = vec![
        HirStmt::new(HirStmtKind::Let{
            name: stored_m.clone(),
            init: iter_hir,
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: stored_cap.clone(),
            init: HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(stored_m.clone()), Span::dummy())),
                index: 5, // HashMap 槽 5 = cap
                ty: FieldScalar::Int,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: stored_i.clone(),
            init: HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy()),
            mutable: true,
        }, Span::dummy()),
    ];

    // 6. 循环体检查
    let (b_hir, _) = check_block(ctx, body)?;
    ctx.pop_scope();

    // 7. loop 体：边界检查 → 跳槽 → 绑定 k/v → 递增 → 原 body 语句
    let k_scalar = field_scalar_of(&k_ty);
    let v_scalar = field_scalar_of(&v_ty);
    let mut loop_body_stmts = vec![
        // if __for_i >= __for_cap { break }
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::If{
            cond: Box::new(HirExpr::new(HirExprKind::Binary(
                HirBinaryOp::Ge,
                Box::new(HirExpr::new(HirExprKind::Variable(stored_i.clone()), Span::dummy())),
                Box::new(HirExpr::new(HirExprKind::Variable(stored_cap.clone()), Span::dummy())),
            ), Span::dummy())),
            then_block: Box::new(HirBlock { span: Span::dummy(),
                stmts: vec![HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Break(None), Span::dummy())), Span::dummy())],
                final_expr: None,
            }),
            else_block: None,
        }, Span::dummy())), Span::dummy()),
        // if __for_m.states[__for_i] != 1 { __for_i += 1; continue; }
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::If{
            cond: Box::new(HirExpr::new(HirExprKind::Binary(
                HirBinaryOp::Ne,
                Box::new(HirExpr::new(HirExprKind::Index{
                    base: Box::new(HirExpr::new(HirExprKind::FieldGet{
                        base: Box::new(HirExpr::new(HirExprKind::Variable(stored_m.clone()), Span::dummy())),
                        index: 2, // HashMap 槽 2 = states 指针
                        ty: FieldScalar::Ptr,
                    }, Span::dummy())),
                    index: Box::new(HirExpr::new(HirExprKind::Variable(stored_i.clone()), Span::dummy())),
                    elem: FieldScalar::Int,
                    is_str: false,
                }, Span::dummy())),
                Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
            ), Span::dummy())),
            then_block: Box::new(HirBlock { span: Span::dummy(),
                stmts: vec![
                    HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Assign{
                        target: stored_i.clone(),
                        op: HirAssignOp::AddAssign,
                        value: Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
                    }, Span::dummy())), Span::dummy()),
                    HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Continue, Span::dummy())), Span::dummy()),
                ],
                final_expr: None,
            }),
            else_block: None,
        }, Span::dummy())), Span::dummy()),
        // let k = __for_m.keys[__for_i]
        HirStmt::new(HirStmtKind::Let{
            name: stored_k.clone(),
            init: HirExpr::new(HirExprKind::Index{
                base: Box::new(HirExpr::new(HirExprKind::FieldGet{
                    base: Box::new(HirExpr::new(HirExprKind::Variable(stored_m.clone()), Span::dummy())),
                    index: 0, // HashMap 槽 0 = keys 指针
                    ty: FieldScalar::Ptr,
                }, Span::dummy())),
                index: Box::new(HirExpr::new(HirExprKind::Variable(stored_i.clone()), Span::dummy())),
                elem: k_scalar,
                is_str: false,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        // let v = __for_m.vals[__for_i]
        HirStmt::new(HirStmtKind::Let{
            name: stored_v.clone(),
            init: HirExpr::new(HirExprKind::Index{
                base: Box::new(HirExpr::new(HirExprKind::FieldGet{
                    base: Box::new(HirExpr::new(HirExprKind::Variable(stored_m.clone()), Span::dummy())),
                    index: 1, // HashMap 槽 1 = vals 指针
                    ty: FieldScalar::Ptr,
                }, Span::dummy())),
                index: Box::new(HirExpr::new(HirExprKind::Variable(stored_i.clone()), Span::dummy())),
                elem: v_scalar,
                is_str: false,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        // __for_i += 1
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Assign{
            target: stored_i.clone(),
            op: HirAssignOp::AddAssign,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
        }, Span::dummy())), Span::dummy()),
    ];
    loop_body_stmts.extend(b_hir.stmts);
    if let Some(fe) = b_hir.final_expr {
        loop_body_stmts.push(HirStmt::new(HirStmtKind::Expr(fe), Span::dummy()));
    }
    let loop_expr = HirExpr::new(HirExprKind::Loop{
        body: Box::new(HirBlock { span: Span::dummy(),
            stmts: loop_body_stmts,
            final_expr: None,
        }),
    }, Span::dummy());

    stmts.push(HirStmt::new(HirStmtKind::Expr(loop_expr), Span::dummy()));
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: None,
        })), Span::dummy()),
        Type::Unit,
    ))
}
