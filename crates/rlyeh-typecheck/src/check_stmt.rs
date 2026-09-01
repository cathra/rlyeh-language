//! 语句类型检查。

use rlyeh_ast::{AstPattern, AstStmt, ExprKind};
use rlyeh_hir::{FieldScalar, HirBlock, HirExpr, HirStmt};

use crate::check_expr::{
    check_closure_expected, check_closure_value_binding, check_deferred_closure_binding, coerce_to_dyn,
    infer_expr, make_slice_fat, resolve_ast_type,
};
use crate::context::TypeContext;
use crate::error::TypeError;
use crate::types::{field_scalar_of, Mutability, Type};

/// U2：构造联合值——匿名 enum 布局（槽 0 = tag、槽 1 = payload）。
///
/// 联合的运行时表示即**匿名 enum**：不注册 `EnumDef`（避免污染全局变体名空间），
/// 直接内联生成与 `check_variant_construct` 同构的 HIR——`Alloc(2)` +
/// `FieldSet(0, tag)` + `FieldSet(1, value)`。布局与具名 enum 一致，故完全
/// 复用现有 enum codegen 通道，无新增 codegen 逻辑。
///
/// 分配方式统一走 **calloc 堆对象**（`by_value: false`）：联合的不同成员可能
/// 分别是标量（`i64`）与聚合（`String`），若按成员分别决定栈槽 / 堆分配，
/// 同一联合的各构造路径判定会不一致（`check_variant_construct` 明确要求一致），
/// 且栈槽地址存入联合值后传出函数会悬垂。统一堆分配规避这两类问题。
pub(crate) fn make_union_ctor(
    ctx: &mut TypeContext,
    value: HirExpr,
    tag: usize,
    member_ty: &Type,
) -> HirExpr {
    let base = ctx.fresh_temp();
    let stmts = vec![
        HirStmt::Let {
            name: base.clone(),
            init: HirExpr::Alloc {
                slots: 2,
                by_value: false,
                is_strfat: false,
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 0,
            value: Box::new(HirExpr::IntLiteral(tag as i128)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(value),
            ty: field_scalar_of(member_ty),
        }),
    ];
    HirExpr::Block(Box::new(HirBlock {
        stmts,
        final_expr: Some(HirExpr::Variable(base)),
    }))
}

/// 检查语句并生成 HIR 语句。
pub(crate) fn check_stmt(
    ctx: &mut TypeContext,
    stmt: &AstStmt,
) -> Result<(HirStmt, Type), TypeError> {
    match stmt {
        AstStmt::Let {
            pattern,
            type_anno,
            init,
            mutable,
        } => {
            let span = init.span;
            // H4 去虚拟化：`let d: dyn T = &obj;` 时待记录的具体类型
            // （在 Ident 分支按绑定名写入 ctx.dyn_concrete）
            let mut pending_dyn_concrete: Option<Type> = None;
            // 闭包 let 绑定分四路：
            // - fn 类型注解 → H2 无捕获闭包按预期签名检查（`let f: fn(A) -> B = |x| ..;`）；
            // - 无注解且参数全带类型注解 → 闭包值对象（`let f = |x: i64| ..; f(..)`）；
            // - 无注解且存在无注解参数 → 延迟闭包值绑定（参数类型由首次调用点
            //   实参推断，见 check_deferred_closure_binding；`let f = |x| ..; f(..)`）；
            // - 其余 → 常规推断 + 一致性检查。
            let all_annotated = if let ExprKind::Closure { param_types, .. } = &*init.kind {
                param_types.iter().all(|t| t.is_some())
            } else {
                false
            };
            let (mut h_init, mut ty) =
                if type_anno.is_some() && matches!(&*init.kind, ExprKind::Closure { .. }) {
                    let at = resolve_ast_type(ctx, type_anno.as_ref().unwrap(), span)?;
                    if !matches!(&at, Type::Fn(_)) {
                        return Err(TypeError::WrongType {
                            expected: at.to_string(),
                            found: "闭包".to_string(),
                            span,
                        });
                    }
                    check_closure_expected(ctx, init, &at, span)?
                } else if type_anno.is_none() && matches!(&*init.kind, ExprKind::Closure { .. }) {
                    if all_annotated {
                        check_closure_value_binding(ctx, init, span)?
                    } else {
                        check_deferred_closure_binding(ctx, init, span)?
                    }
                } else {
                    infer_expr(ctx, init)?
                };

            // 类型标注一致性检查；标注存在时以标注类型作为绑定类型，
            // 以便统一 init 中残留的 `_`（Infer）占位（如 `Vec::with_capacity` 返回 `Vec<_>`）
            let anno_ty = match type_anno {
                Some(anno) => {
                    let at = resolve_ast_type(ctx, anno, span)?;
                    // H4 `dyn Trait` 转换：注解为 `dyn Trait`、init 为 `&T`
                    // （T 实现了该 trait）时，把 init 转成 trait 对象胖指针，
                    // 并同步绑定类型，使后续 `at.compatible_with(&ty)` 一致。
                    if let (Type::Dyn(trait_name), Type::Ref(inner, _)) = (&at, &ty) {
                        let inner_ty = (**inner).clone();
                        h_init = coerce_to_dyn(ctx, h_init, inner, trait_name, span)?;
                        ty = at.clone();
                        // H4 去虚拟化：记录绑定变量 → 具体类型
                        // （后续 `dyn_var.method()` 可静态分派到具体类型实现）
                        if let Type::Named(_, _) = &inner_ty {
                            pending_dyn_concrete = Some(inner_ty);
                        }
                    }
                    // P4（2026-08-28）：`&dyn Trait` 上转型——注解为 `&dyn Trait`、
                    // init 为 `&T`（T 实现该 trait）时，把 `&T` 引用上转为胖指针引用
                    // `&dyn Trait`（data 指向引用目标、vtable 指向 T 的实现）。
                    // 与 H4 值上转型同构，仅注解为 `Ref(Dyn)`；胖指针布局相同（2 槽）。
                    else if let (Type::Ref(inner_at, _), Type::Ref(inner_init, _)) = (&at, &ty) {
                        if let Type::Dyn(trait_name) = &**inner_at {
                            if let Type::Named(_, _) = &**inner_init {
                                let concrete = (**inner_init).clone();
                                h_init =
                                    coerce_to_dyn(ctx, h_init, &concrete, trait_name, span)?;
                                ty = at.clone();
                                pending_dyn_concrete = Some(concrete);
                            }
                        }
                    }
                    // U2：联合构造——注解为 `A | B`、init 为其中某成员类型的值时，
                    // desugar 为匿名 enum 构造（槽 0 = tag、槽 1 = payload），
                    // 运行时表示与具名 enum 一致，复用现有 enum codegen 通道。
                    // （成员下标即 tag，由成员在联合中的声明顺序决定）
                    if let Type::Union(us) = &at {
                        if let Some(idx) = us.iter().position(|u| ty.compatible_with(u)) {
                            let member_ty = us[idx].clone();
                            h_init = make_union_ctor(ctx, h_init, idx, &member_ty);
                            ty = at.clone();
                        }
                    }
                    // S2 unsize coercion：let s: &[T] = &arr; —— &[T; N] 经 unsize
                    // 降级为切片胖指针 {data, len}（len = 编译期数组长度），与
                    // check_expr::call 实参位置同构；仅当元素类型兼容时转换。
                    if let (Type::Ref(ia, _), Type::Ref(ib, _)) = (&ty, &at) {
                        if let (Type::Array(_, n), Type::Slice(_)) = (&**ia, &**ib) {
                            if ty.compatible_with(&at) {
                                h_init = make_slice_fat(ctx, h_init, *n as i128);
                                ty = at.clone();
                            }
                        }
                    }
                    if !at.compatible_with(&ty) {
                        return Err(TypeError::WrongType {
                            expected: at.to_string(),
                            found: ty.to_string(),
                            span,
                        });
                    }
                    Some(at)
                }
                None => None,
            };

            match pattern {
                AstPattern::Ident(name) => {
                    let bind_ty = anno_ty.clone().unwrap_or_else(|| ty.clone());
                    // 非注解延迟闭包绑定：绑定处 push 记录时变量名未知，
                    // 此处填充（首次调用点按变量名查延迟绑定记录）
                    if matches!(&ty, Type::Closure { fn_name, .. } if fn_name.is_empty()) {
                        if let Some(d) = ctx.deferred_closures.last_mut() {
                            d.var_name = name.clone();
                        }
                    }
                    // H4 去虚拟化：dyn 绑定记录具体类型名
                    if let Some(concrete) = pending_dyn_concrete.take() {
                        ctx.insert_dyn_concrete(name.clone(), concrete);
                    }
                    // U1：insert 返回存储槽名（块级遮蔽时 mangle），
                    // Let 绑定名与后续引用解析的槽名保持一致。
                    let stored = ctx.insert_variable(name.clone(), bind_ty);
                    // 记录初始化表达式，供 `String::from(s)` 追踪字面量绑定
                    ctx.insert_local_init(stored.clone(), h_init.clone());
                    Ok((
                        HirStmt::Let {
                            name: stored.clone(),
                            init: h_init,
                            mutable: *mutable,
                        },
                        ty,
                    ))
                }
                AstPattern::Wildcard => {
                    // `let _ = expr;`：丢弃绑定
                    let _ = &h_init;
                    Ok((
                        HirStmt::Let {
                            name: "_".to_string(),
                            init: h_init,
                            mutable: *mutable,
                        },
                        ty,
                    ))
                }
                AstPattern::Ref(inner, is_mut) => {
                    // `let ref [mut] x = expr;`：绑定为对 init 的引用（`&T` / `&mut T`）。
                    // 仅支持 `ref` 包裹标识符；复杂内层模式暂不支持。
                    if let AstPattern::Ident(name) = &**inner {
                        let mutability = if *is_mut {
                            Mutability::Mutable
                        } else {
                            Mutability::Immutable
                        };
                        let bind_ty = Type::Ref(Box::new(ty.clone()), mutability);
                        let stored = ctx.insert_variable(name.clone(), bind_ty);
                        // 记录初始化表达式，供 `String::from(s)` 追踪字面量绑定
                        ctx.insert_local_init(stored.clone(), h_init.clone());
                        Ok((
                            HirStmt::Let {
                                name: stored.clone(),
                                init: HirExpr::Ref {
                                    expr: Box::new(h_init),
                                    is_mut: *is_mut,
                                    pointee: field_scalar_of(&ty),
                                },
                                mutable: *is_mut,
                            },
                            ty,
                        ))
                    } else {
                        Err(TypeError::Unsupported {
                            what: "复杂 ref 绑定模式（仅支持 `let ref x = expr;`）".to_string(),
                            span,
                        })
                    }
                }
                _ => Err(TypeError::Unsupported {
                    what: "复杂 let 绑定模式（元组 / 结构体等）".to_string(),
                    span,
                }),
            }
        }
        AstStmt::Expr(e) => {
            let (hir, ty) = infer_expr(ctx, e)?;
            Ok((HirStmt::Expr(hir), ty))
        }
        AstStmt::Semi(e) => {
            let (hir, _) = infer_expr(ctx, e)?;
            Ok((HirStmt::Semi(hir), Type::Unit))
        }
        AstStmt::Item(item) => {
            // 语句级嵌套项（如函数体内的局部 fn）：检查但不在顶层生成 HIR
            let mut scratch = Vec::new();
            crate::check_item::check_item(ctx, item, "", &mut scratch)?;
            Ok((HirStmt::Semi(HirExpr::Unit), Type::Unit))
        }
    }
}
