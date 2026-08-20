//! 语句类型检查。

use zeta_ast::{AstPattern, AstStmt};
use zeta_hir::{HirExpr, HirStmt};

use crate::check_expr::{infer_expr, resolve_ast_type};
use crate::context::TypeContext;
use crate::error::TypeError;
use crate::types::Type;

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
            let (h_init, ty) = infer_expr(ctx, init)?;

            // 类型标注一致性检查；标注存在时以标注类型作为绑定类型，
            // 以便统一 init 中残留的 `_`（Infer）占位（如 `Vec::with_capacity` 返回 `Vec<_>`）
            let anno_ty = match type_anno {
                Some(anno) => {
                    let at = resolve_ast_type(ctx, anno, span)?;
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
                    ctx.insert_variable(name.clone(), bind_ty);
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
