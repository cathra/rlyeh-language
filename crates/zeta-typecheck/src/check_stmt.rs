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

            // 类型标注一致性检查
            if let Some(anno) = type_anno {
                let anno_ty = resolve_ast_type(ctx, anno, span)?;
                if !anno_ty.compatible_with(&ty) {
                    return Err(TypeError::WrongType {
                        expected: anno_ty.to_string(),
                        found: ty.to_string(),
                        span,
                    });
                }
            }

            match pattern {
                AstPattern::Ident(name) => {
                    ctx.insert_variable(name.clone(), ty.clone());
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
            crate::check_item::check_item(ctx, item)?;
            Ok((HirStmt::Semi(HirExpr::Unit), Type::Unit))
        }
    }
}
