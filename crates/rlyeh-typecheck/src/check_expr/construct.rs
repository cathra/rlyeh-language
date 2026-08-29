//! 表达式检查子模块：容器/结构体/字符串等构造检查。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use super::*;

pub(super) fn check_struct_construct(
    ctx: &mut TypeContext,
    type_name: &[String],
    type_args: &[AstType],
    fields: &[(String, AstExpr)],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let struct_name = type_name.join("::");
    // 支持 use 导入别名与模块路径（如 std 拆分后 `time::Instant { ... }`）：
    // 裸名构造 `Duration { ... }` 经别名解析为完整符号名 `time::Duration`。
    let struct_name = ctx
        .resolve_full_name(&struct_name)
        .unwrap_or(struct_name);
    let def = ctx.lookup_struct(&struct_name).cloned().ok_or_else(|| {
        TypeError::UndefinedType {
            name: struct_name.clone(),
            span,
        }
    })?;

    // U8：泛型结构体构造——解析 `Pair<i64>` 的类型实参，构造实例类型
    // `Named(struct_name, resolved_args)`；字段类型用实参替换泛型参数后校验。
    let mut resolved_args: Vec<Type> = Vec::new();
    for ta in type_args {
        resolved_args.push(resolve_ast_type(ctx, ta, span)?);
    }
    // Y4a（2026-08-28）：泛型 struct 字面量构造——`type_args` 为空（无 turbofish）
    // 且 struct 含泛型参数时，从字段实参推断泛型参数。
    // P7a（2026-08-29）：改用 `unify` 统一字段类型与实参类型——支持**复合字段**
    // （`Vec<T>` / `HashMap<K,V>` 等；此前仅裸 `Generic(tp)` 直接推断，
    // `Bag { items: Vec::with_capacity(8) }` 无法推断 T）。unify 对复合类型
    // 递归统一 args（`Vec<T>` vs `Vec<i64>` → T = i64），不匹配时静默跳过。
    if resolved_args.is_empty() && !def.type_params.is_empty() {
        let mut field_subst_infer: HashMap<String, Type> = HashMap::new();
        for (fname, fval) in fields {
            if let Some((_, fty)) = def.fields.iter().find(|(n, _)| n == fname) {
                let (_, arg_ty) = infer_expr(ctx, fval)?;
                unify(fty, &arg_ty, &mut field_subst_infer)?;
            }
        }
        // 按 def.type_params 声明顺序提取（非按字段顺序，保证实参位置正确）
        for tp in &def.type_params {
            if let Some(t) = field_subst_infer.get(tp) {
                resolved_args.push(t.clone());
            }
        }
    }
    if !resolved_args.is_empty() && resolved_args.len() != def.type_params.len() {
        return Err(TypeError::GenericArityMismatch {
            name: struct_name.clone(),
            expected: def.type_params.len(),
            found: resolved_args.len(),
            span,
        });
    }
    let mut field_subst: HashMap<String, Type> = HashMap::new();
    for (tp, arg) in def.type_params.iter().zip(&resolved_args) {
        field_subst.insert(tp.clone(), arg.clone());
    }

    // 未知字段校验
    for (fname, fval) in fields {
        if !def.fields.iter().any(|(n, _)| n == fname) {
            return Err(TypeError::UnknownField {
                struct_name: struct_name.clone(),
                field: fname.clone(),
                span: fval.span,
            });
        }
    }

    // 展开为 Alloc + 字段槽
    // 标量结构体（≤2 槽）按值分配（栈槽，免 calloc）；槽区均为 8 字节槽，
    // 兼容栈上 `[2 x i64]` 存储。
    //
    // 按值仅允许"字段全为标量槽"的结构体：按值对象以 `[2 x i64]` 栈槽存储，
    // 当它作为另一聚合（enum/struct）的字段/payload 时以"对象地址"语义写入
    // 外层 Ptr 槽——若其含聚合字段，写入的即内部对象地址（栈地址），函数返回/
    // 跨调用后悬垂（如 `Result<SocketAddr, _>::Ok(sa)`，`SocketAddr` 含
    // String 字段 → tcp_addr SIGSEGV）。含聚合/引用/裸指针/泛型字段者按值
    // 构造一律不安全，退化回堆分配。
    let struct_by_value = def.fields.len() <= 2
        && def.fields.iter().all(|(_, fty)| field_is_scalar_slot(fty));
    let base = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: base.clone(),
        init: HirExpr::Alloc {
            slots: def.fields.len(),
            by_value: struct_by_value,
            is_strfat: false,
        },
        mutable: false,
    }];
    for (i, (fname, fty)) in def.fields.iter().enumerate() {
        // U8：字段类型用泛型实参替换（`Pair<i64>` 的字段 `T` → `i64`）后校验
        let field_fty = substitute(fty, &field_subst);
        let init = fields.iter().find(|(n, _)| n == fname).map(|(_, e)| e);
        let Some(init) = init else {
            // 递归 struct（desugar 生成的 `__Fut_f` 递归环）构造时允许省略
            // `Box<__Fut_>` 递归槽：缺失 = 空指针占位，首次 poll 求值前不 deref。
            // 仅放行堆指针字段，普通字段缺失仍报 MissingField。
            if matches!(&field_fty, Type::Named(n, _) if n == "Box") {
                stmts.push(HirStmt::Semi(HirExpr::FieldSet {
                    base: Box::new(HirExpr::Variable(base.clone())),
                    index: i,
                    // 空指针占位（槽值 0）；首次 poll 被真实子 future Box 覆盖
                    value: Box::new(HirExpr::IntLiteral(0)),
                    ty: field_scalar_of(&field_fty),
                }));
                continue;
            }
            return Err(TypeError::MissingField {
                struct_name: struct_name.clone(),
                field: fname.clone(),
                span,
            });
        };
        let (hir, arg_ty) = infer_expr(ctx, init)?;
        if !arg_ty.compatible_with(&field_fty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: format!("struct `{struct_name}` field `{fname}`"),
                index: i,
                expected: field_fty.to_string(),
                found: arg_ty.to_string(),
                span: init.span,
            });
        }
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: i,
            value: Box::new(hir),
            ty: field_scalar_of(&field_fty),
        }));
    }

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named(struct_name, resolved_args),
    ))
}

pub(super) fn check_vec_construct(
    ctx: &mut TypeContext,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let cap = if method == "new" {
        AstExpr::new(ExprKind::IntLiteral(4), span)
    } else if args.len() == 1 {
        args[0].clone()
    } else {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("Vec::{method}"),
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

    // 展开为 Alloc + 三个字段槽（data / len / cap）
    let data_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let cap_for_alloc = cap_hir.clone();
    let stmts = vec![
        HirStmt::Let {
            name: data_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_for_alloc],
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
            value: Box::new(HirExpr::Variable(data_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 2,
            value: Box::new(cap_hir),
            ty: FieldScalar::Int,
        }),
    ];

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named("Vec".to_string(), vec![Type::Infer]),
    ))
}

pub(super) fn check_string_construct(
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
            name: format!("String::{method}"),
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

    let data_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let cap_for_alloc = cap_hir.clone();
    let stmts = vec![
        HirStmt::Let {
            name: data_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_bytes".to_string(),
                args: vec![cap_for_alloc],
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
            value: Box::new(HirExpr::Variable(data_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 2,
            value: Box::new(cap_hir),
            ty: FieldScalar::Int,
        }),
    ];

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named("String".to_string(), vec![]),
    ))
}

pub(super) fn check_hashmap_construct(
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
            name: format!("HashMap::{method}"),
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
    // with_capacity 的 cap 经标准库 `next_pow2` 规整为 2 的幂：
    // HashMap 的位掩码定位（hash & (cap-1)）与翻倍扩容依赖 cap 恒为 2 的幂，
    // 用户传入任意正整数（如 10）会破坏该不变量。
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
    let states_tmp = ctx.fresh_temp();
    let dist_tmp = ctx.fresh_temp();
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
            name: states_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_hir.clone()],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: dist_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_hir.clone()],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: base.clone(),
            init: HirExpr::Alloc {
            slots: 7,
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
            value: Box::new(HirExpr::Variable(states_tmp)),
            ty: FieldScalar::Ptr,
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
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 5,
            value: Box::new(cap_hir),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 6,
            value: Box::new(HirExpr::Variable(dist_tmp)),
            ty: FieldScalar::Ptr,
        }),
    ];

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named("HashMap".to_string(), vec![Type::Infer, Type::Infer]),
    ))
}

pub(super) fn check_vecdeque_construct(
    ctx: &mut TypeContext,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 校验参数个数：`new` 0 参、`with_capacity` 1 参
    if method == "with_capacity" && args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "VecDeque::with_capacity".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    // 底层 Vec 构造（new → Vec::new / with_capacity → Vec::with_capacity(cap)）
    let (buf_hir, _) = check_vec_construct(ctx, method, args, span)?;

    let base = ctx.fresh_temp();
    let stmts = vec![
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
            value: Box::new(buf_hir),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
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
        Type::Named("VecDeque".to_string(), vec![Type::Infer]),
    ))
}

pub(super) fn check_hashset_construct(
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

pub(super) fn check_btreemap_construct(
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

pub(super) fn check_iter_construct(
    ctx: &mut TypeContext,
    name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let expected = if name == "Iter" { 2 } else { 3 };
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

pub(crate) fn check_string_from(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "String::from".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let (s_hir, s_ty) = infer_expr(ctx, &args[0])?;
    // 运行期 String → 深拷贝：`String::from(s)` ≡ `s.clone()`。
    // clone 是 `&self` 方法（G1 方法调用自动剥引用层），经方法实例化注册
    // 函数体后复用标准库实现；返回独立缓冲，原串不受影响。
    if comparison::is_string_type(ctx, &s_ty) {
        let impl_def = ctx
            .find_impl_for_method(&s_ty, "clone")
            .cloned()
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: "String::clone".to_string(),
                span,
            })?;
        let method_def = impl_def
            .methods
            .iter()
            .find(|m| m.sig.name == "clone")
            .cloned()
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: "String::clone".to_string(),
                span,
            })?;
        let fn_name = instantiate_impl_method(ctx, &impl_def, &method_def, &HashMap::new(), span)?;
        return Ok((
            HirExpr::Call {
                callee: fn_name,
                args: vec![s_hir],
            },
            s_ty,
        ));
    }
    // `&str`（String 对象的只读借用视图）→ 深拷贝：
    // 运行期读 data/len 槽 → 独立 alloc_bytes(len+1) 数据缓冲 + 独立 3 槽 String 对象。
    // 注意：数据缓冲与 String 对象必须**两个独立分配**——若共用 base，copy_bytes 写入的
    // 字符串字节会被随后的 FieldSet 对象头（前 24 字节）覆盖，破坏数据（V2-D 修复）。
    if let Type::Ref(inner, _) = &s_ty {
        if matches!(**inner, Type::Str) {
            let len_tmp = ctx.fresh_temp();
            let data_src = ctx.fresh_temp();
            let data_tmp = ctx.fresh_temp();
            let base = ctx.fresh_temp();
            let len_plus1 = |var: String| {
                HirExpr::Binary(
                    HirBinaryOp::Add,
                    Box::new(HirExpr::Variable(var)),
                    Box::new(HirExpr::IntLiteral(1)),
                )
            };
            let stmts = vec![
                HirStmt::Let {
                    name: len_tmp.clone(),
                    init: HirExpr::FieldGet {
                        base: Box::new(s_hir.clone()),
                        index: 1,
                        ty: FieldScalar::Int,
                    },
                    mutable: false,
                },
                HirStmt::Let {
                    name: data_src.clone(),
                    init: HirExpr::FieldGet {
                        base: Box::new(s_hir),
                        index: 0,
                        ty: FieldScalar::Ptr,
                    },
                    mutable: false,
                },
                // 独立数据缓冲（len+1 字节，含 NUL）
                HirStmt::Let {
                    name: data_tmp.clone(),
                    init: HirExpr::Call {
                        callee: "alloc_bytes".to_string(),
                        args: vec![len_plus1(len_tmp.clone())],
                    },
                    mutable: false,
                },
                HirStmt::Semi(HirExpr::Call {
                    callee: "copy_bytes".to_string(),
                    args: vec![
                        HirExpr::Variable(data_tmp.clone()),
                        HirExpr::Variable(data_src),
                        len_plus1(len_tmp.clone()),
                    ],
                }),
                // 独立 String 对象（3 槽：data/len/cap）
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
                    value: Box::new(HirExpr::Variable(data_tmp)),
                    ty: FieldScalar::Ptr,
                }),
                HirStmt::Semi(HirExpr::FieldSet {
                    base: Box::new(HirExpr::Variable(base.clone())),
                    index: 1,
                    value: Box::new(HirExpr::Variable(len_tmp.clone())),
                    ty: FieldScalar::Int,
                }),
                HirStmt::Semi(HirExpr::FieldSet {
                    base: Box::new(HirExpr::Variable(base.clone())),
                    index: 2,
                    value: Box::new(HirExpr::Variable(len_tmp)),
                    ty: FieldScalar::Int,
                }),
            ];
            return Ok((
                HirExpr::Block(Box::new(HirBlock {
                    stmts,
                    final_expr: Some(HirExpr::Variable(base)),
                })),
                Type::Named("String".to_string(), vec![]),
            ));
        }
    }
    // 字面量直用；`let s = "..."` 绑定的变量经 local_inits 表追踪回字面量，
    // 其余非字面量 Str（裸字面量类型）不支持（须先经 String::from/String 变量）
    let s = match &s_hir {
        HirExpr::StringLiteral(s) => Some(s.clone()),
        HirExpr::Variable(name) => {
            // P9b（2026-08-29）：多层直链追踪——`let a = "x"; let b = a; String::from(b)`
            // （此前仅查一层 `lookup_local_init`，`b` 的 init 是变量 `a` 时失败）。
            // 深度上限 8 防自引用/长链开销；fn 边界由 lookup_local_init 天然不穿透。
            let mut cur = ctx.lookup_local_init(name).cloned();
            let mut depth = 0;
            loop {
                match cur {
                    Some(HirExpr::StringLiteral(s)) => break Some(s),
                    Some(HirExpr::Variable(n)) if depth < 8 => {
                        cur = ctx.lookup_local_init(&n).cloned();
                        depth += 1;
                    }
                    _ => break None,
                }
            }
        }
        _ => None,
    }
    .ok_or_else(|| TypeError::Unsupported {
        what: "String::from 支持字符串字面量（或绑定字面量的变量）与 String 变量；裸 Str 类型不支持"
            .to_string(),
        span: args[0].span,
    })?;
    let len = s.len() as i128;
    // 分配 len+1 字节并连 LLVM 字符串常量自带的 \00 一起拷入：
    // runtime 侧按 C 字符串（NUL 结尾）读取（如 actor 的 handle/factory 符号名
    // 经 dlsym 前由 CStr 扫描），缓冲末尾必须补 NUL，否则读超到相邻堆内存。
    let alloc_len = len + 1;

    let data_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let stmts = vec![
        HirStmt::Let {
            name: data_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_bytes".to_string(),
                args: vec![HirExpr::IntLiteral(alloc_len)],
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::Call {
            callee: "copy_bytes".to_string(),
            args: vec![
                HirExpr::Variable(data_tmp.clone()),
                HirExpr::StringLiteral(s.clone()),
                HirExpr::IntLiteral(alloc_len),
            ],
        }),
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
            value: Box::new(HirExpr::Variable(data_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(HirExpr::IntLiteral(len)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 2,
            value: Box::new(HirExpr::IntLiteral(len)),
            ty: FieldScalar::Int,
        }),
    ];

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named("String".to_string(), vec![]),
    ))
}

pub(super) fn upgrade_str_arg(
    ctx: &mut TypeContext,
    hir: HirExpr,
    ty: Type,
    param_ty: &Type,
    arg: &AstExpr,
) -> Result<(HirExpr, Type), TypeError> {
    if matches!(&ty, Type::Str) && !matches!(param_ty, Type::Str) {
        check_string_from(ctx, std::slice::from_ref(arg), arg.span)
    } else {
        Ok((hir, ty))
    }
}
