//! 语句类型检查。

use zeta_ast::{AstPattern, AstStmt, ExprKind};
use zeta_hir::{HirExpr, HirStmt};

use crate::check_expr::{
    check_closure_expected, check_closure_value_binding, check_deferred_closure_binding, coerce_to_dyn,
    infer_expr, resolve_ast_type,
};
use crate::context::TypeContext;
use crate::error::TypeError;
use crate::types::{field_scalar_of, Mutability, Type};

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
                        h_init = coerce_to_dyn(ctx, h_init, inner, trait_name, span)?;
                        ty = at.clone();
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
                    ctx.insert_variable(name.clone(), bind_ty);
                    // 记录初始化表达式，供 `String::from(s)` 追踪字面量绑定
                    ctx.insert_local_init(name.clone(), h_init.clone());
                    Ok((
                        HirStmt::Let {
                            name: name.clone(),
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
                        ctx.insert_variable(name.clone(), bind_ty);
                        // 记录初始化表达式，供 `String::from(s)` 追踪字面量绑定
                        ctx.insert_local_init(name.clone(), h_init.clone());
                        Ok((
                            HirStmt::Let {
                                name: name.clone(),
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
