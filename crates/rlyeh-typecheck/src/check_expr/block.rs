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
        // M2：`check_stmt` 返回语句序列（元组解构展开为多条 Let）
        let (hir_stmts, _) = crate::check_stmt::check_stmt(ctx, stmt)?;
        stmts.extend(hir_stmts);
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
    // Q-M2（SH-P0-8）：块尾析构——本层实现 `Drop` 的拥有所有权变量，逆声明序
    // 插入 `x.drop()`。无 Drop 实现时不产生任何语句，行为与拆分前完全一致。
    //
    // `Never` 结尾（如 `return`）块不会正常走到结尾，跳过注入以免在终结指令
    // 之后追加不可达语句。
    if final_ty != Type::Never {
        let drop_stmts = crate::check_expr::misc::build_scope_drops(ctx, block.span)?;
        if !drop_stmts.is_empty() {
            if let Some(fe) = final_expr.take() {
                if final_ty == Type::Unit {
                    // 无值可保存：直接作为尾语句求值，再析构
                    stmts.push(HirStmt::Expr(fe));
                } else {
                    // 有值：先求块值存入临时，再析构，最后以该临时作为块结果
                    // （保证析构发生在块值计算**之后**，与 Rust 作用域语义一致）
                    let tmp = ctx.fresh_temp();
                    stmts.push(HirStmt::Let {
                        name: tmp.clone(),
                        init: fe,
                        mutable: false,
                    });
                    final_expr = Some(HirExpr::Variable(tmp));
                }
            }
            stmts.extend(drop_stmts);
        }
    }
    Ok((HirBlock { stmts, final_expr }, final_ty))
}
