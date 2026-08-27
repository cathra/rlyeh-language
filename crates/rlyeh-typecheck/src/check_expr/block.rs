//! 表达式检查子模块：block。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

pub(crate) fn check_block(
    ctx: &mut TypeContext,
    block: &AstBlock,
) -> Result<(HirBlock, Type), TypeError> {
    check_block_with_expected_final(ctx, block, None)
}

pub(crate) fn check_block_with_expected_final(
    ctx: &mut TypeContext,
    block: &AstBlock,
    expected_final: Option<&Type>,
) -> Result<(HirBlock, Type), TypeError> {
    // U1：块级作用域（查找穿透外层——块内可见外层变量；块内 let 随弹出消失，
    // 与块外同名变量遮蔽时 mangle 槽名，互不干扰）。
    ctx.push_scope(false);
    let result = check_block_inner(ctx, block, expected_final);
    ctx.pop_scope();
    result
}

pub(crate) fn check_block_inner(
    ctx: &mut TypeContext,
    block: &AstBlock,
    expected_final: Option<&Type>,
) -> Result<(HirBlock, Type), TypeError> {
    let mut stmts = Vec::with_capacity(block.stmts.len());
    for stmt in &block.stmts {
        let (hir_stmt, _) = crate::check_stmt::check_stmt(ctx, stmt)?;
        stmts.push(hir_stmt);
    }
    let mut final_ty = Type::Unit;
    let mut final_expr = None;
    if let Some(e) = &block.final_expr {
        let (hir, ty) = if let Some(exp) = expected_final {
            if matches!(exp, Type::Fn(_)) && matches!(&*e.kind, ExprKind::Closure { .. }) {
                check_closure_expected(ctx, e, exp, e.span)?
            } else {
                infer_expr(ctx, e)?
            }
        } else {
            infer_expr(ctx, e)?
        };
        final_ty = ty;
        final_expr = Some(hir);
    }
    Ok((HirBlock { stmts, final_expr }, final_ty))
}
