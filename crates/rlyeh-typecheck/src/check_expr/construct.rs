//! 表达式检查子模块：容器/结构体/字符串等构造检查。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use super::*;
use rlyeh_hir::FieldScalar;

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
    // SH-P0-1 E2（repr(C) 嵌套聚合内联）：repr(C) 结构体按 C 字节布局分配
    // （slots = ceil(c_size/8)），且不可按值（按值栈槽 `[2 x i64]` 不保留字节偏移）。
    let c_size = if def.repr_c {
        Some(crate::types::compute_repr_c(&def.fields, ctx, span)?.size)
    } else {
        None
    };
    let struct_by_value = !def.repr_c
        && def.fields.len() <= 2
        && def.fields.iter().all(|(_, fty)| field_is_scalar_slot(fty));
    let base = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: base.clone(),
        init: HirExpr::Alloc {
            slots: c_size
                .map(|s| ((s + 7) / 8) as usize)
                .unwrap_or(def.fields.len()),
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
        let (mut hir, mut arg_ty) = infer_expr(ctx, init)?;
        // U4：字段级联合——字段类型为 `A | B`、实参为其中某成员类型的值时，
        // desugar 为匿名 enum 构造（复用 U2 的 `make_union_ctor`），使字段槽存的
        // 是联合值（匿名 enum 对象指针）而非裸成员值；否则后续按联合收窄会读到
        // 错误布局。构造后 `arg_ty` 记为联合类型，与字段类型一致。
        if let Type::Union(us) = &field_fty {
            if let Some(idx) = us.iter().position(|u| arg_ty.compatible_with(u)) {
                let member_ty = us[idx].clone();
                hir = crate::check_stmt::make_union_ctor(ctx, hir, idx, &member_ty);
                arg_ty = field_fty.clone();
            }
        }
        if !arg_ty.compatible_with(&field_fty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: format!("struct `{struct_name}` field `{fname}`"),
                index: i,
                expected: field_fty.to_string(),
                found: arg_ty.to_string(),
                span: init.span,
            });
        }
        // SH-P0-1 E2（repr(C) 嵌套聚合内联）：repr(C) 结构体字面量构造时，字段须按
        // C 字节偏移落位（a@0, b@4 ...），嵌套聚合（结构体）字段经 memcpy 内联。
        // 偏移 / 内存类型 / 提升方式直接烤进 `FieldScalar`，与字段访问下传路径一致。
        let field_scalar = if def.repr_c {
            let layout = crate::types::compute_repr_c(&def.fields, ctx, span)?;
            match &layout.fields[i] {
                crate::types::CField::Scalar {
                    offset,
                    field_ty,
                    conv,
                } => rlyeh_hir::FieldScalar::ReprCField {
                    offset: *offset,
                    field_ty,
                    conv: *conv,
                },
                crate::types::CField::Nested { offset, size } => {
                    rlyeh_hir::FieldScalar::ReprCSubPtr {
                        offset: *offset,
                        size: *size,
                    }
                }
            }
        } else {
            field_scalar_of(&field_fty)
        };
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: i,
            value: Box::new(hir),
            ty: field_scalar,
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

/// 元组值构造检查（M1：`(a, b, c)`）。
///
/// 复用结构体构造的聚合槽布局：展开为 `Alloc{slots}` + 逐槽 `FieldSet`
/// 的 HIR `Block`，codegen 与结构体一致（N 槽聚合对象）。字段按位置命
/// 名为 `f0`/`f1`/...，经 `check_field_access` 的元组分支访问。
///
/// 按值仅允许"字段全为标量槽"的元组（≤2 槽）：与结构体一致——
/// 含聚合/引用字段者按值构造不安全，退化回堆分配。
pub(super) fn check_tuple_construct(
    ctx: &mut TypeContext,
    elems: &[AstExpr],
    _span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 推断每个元素类型
    let mut elem_tys: Vec<Type> = Vec::with_capacity(elems.len());
    let mut elem_hirs: Vec<HirExpr> = Vec::with_capacity(elems.len());
    for e in elems {
        let (hir, ty) = infer_expr(ctx, e)?;
        elem_hirs.push(hir);
        elem_tys.push(ty);
    }

    // 按值仅允许"字段全为标量槽"的元组（≤2 槽），与结构体一致
    let tuple_by_value = elem_tys.len() <= 2
        && elem_tys.iter().all(|t| field_is_scalar_slot(t));
    let base = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: base.clone(),
        init: HirExpr::Alloc {
            slots: elem_tys.len(),
            by_value: tuple_by_value,
            is_strfat: false,
        },
        mutable: false,
    }];
    for (i, (hir, t)) in elem_hirs.into_iter().zip(elem_tys.iter()).enumerate() {
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: i,
            value: Box::new(hir),
            ty: field_scalar_of(t),
        }));
    }

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Tuple(elem_tys),
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


// 构造器按簇下沉到子模块（文件大小约束：单个文件 ≤1000 行）；
// 对外引用路径（`crate::check_expr::construct::check_string_from`）经此 re-export 保持不变。
mod collection;
mod string;

pub(crate) use collection::*;
pub(crate) use string::*;
