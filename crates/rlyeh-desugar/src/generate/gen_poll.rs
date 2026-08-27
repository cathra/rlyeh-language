//! 表达式检查子模块：gen_poll。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

pub(super) fn gen_poll_body(a: &AnalyzedAsync, cyclic: &HashSet<String>) -> AstBlock {
    let span = span_of(a);
    // 递归环内子 future 槽（Box 打破无限大小）——首次求值需 `Box::new` 包装。
    let boxed_slots: HashSet<String> = a
        .slots
        .iter()
        .filter(|(_, ty_name)| {
            let target = ty_name.strip_prefix("__Fut_").unwrap_or(ty_name);
            cyclic.contains(target)
        })
        .map(|(slot, _)| slot.clone())
        .collect();
    let mut stmts = Vec::new();
    for (i, seg) in a.segments.iter().enumerate() {
        let k = i as i64;
        if seg.await_info.is_some() {
            stmts.push(state_if_await(a, seg, k, &boxed_slots));
            stmts.push(resume_if(a, seg, k));
        } else if seg.final_expr.is_some() || seg.is_tail() {
            stmts.push(state_if_last(a, seg, k));
        } else if seg.inline_jump {
            stmts.push(state_if_inline(a, seg, k));
        } else {
            stmts.push(state_if_jump(a, seg, k));
        }
    }
    // poll 末尾兜底：状态未到收尾段（控制流回跳的中间态）时返回 Pending，
    // 由 block_on 循环继续 poll（而非 Ready(0) 提前完成）。
    AstBlock {
        stmts,
        final_expr: Some(poll_pending(span)),
        span,
    }
}

pub(super) fn state_assign_stmt(target_seg: usize, span: Span) -> AstStmt {
    assign(
        self_field("state", span),
        int_expr((2 * target_seg) as i128, span),
        span,
    )
}

pub(super) fn state_if_jump(a: &AnalyzedAsync, seg: &Segment, k: i64) -> AstStmt {
    let span = span_of(a);
    let fs = future_sources(a);
    let next = seg.next.expect("jump segment next (展开后应已回填)");
    let mut block_stmts: Vec<AstStmt> = seg
        .stmts
        .iter()
        .filter_map(|s| rewrite_stmt(s, &a.lifted, &fs, span))
        .collect();
    block_stmts.push(state_assign_stmt(next, span));
    AstStmt::Semi(AstExpr::new(
        ExprKind::If {
            cond: state_eq(2 * k, span),
            then_block: AstBlock {
                stmts: block_stmts,
                final_expr: None,
                span,
            },
            else_block: None,
        },
        span,
    ))
}

pub(super) fn state_if_inline(a: &AnalyzedAsync, seg: &Segment, k: i64) -> AstStmt {
    let span = span_of(a);
    let fs = future_sources(a);
    let block_stmts: Vec<AstStmt> = seg
        .stmts
        .iter()
        .filter_map(|s| rewrite_stmt(s, &a.lifted, &fs, span))
        .collect();
    AstStmt::Semi(AstExpr::new(
        ExprKind::If {
            cond: state_eq(2 * k, span),
            then_block: AstBlock {
                stmts: block_stmts,
                final_expr: None,
                span,
            },
            else_block: None,
        },
        span,
    ))
}

pub(super) fn state_if_last(a: &AnalyzedAsync, seg: &Segment, k: i64) -> AstStmt {
    let span = span_of(a);
    let fs = future_sources(a);
    let mut block_stmts: Vec<AstStmt> = seg
        .stmts
        .iter()
        .filter_map(|s| rewrite_stmt(s, &a.lifted, &fs, span))
        .collect();
    let tail = match &seg.final_expr {
        Some(e) => rewrite_expr(e, &a.lifted, span),
        None => int_expr(0, span),
    };
    block_stmts.push(AstStmt::Semi(ret_expr(poll_ready(tail, span), span)));
    // 与 parser 对 `if` 语句的解析一致：`AstStmt::Semi(ExprKind::If)`。
    // `Expr(If)` 会改变块尾值语义，导致后续语句/兜底 final_expr 行为异常。
    AstStmt::Semi(AstExpr::new(
        ExprKind::If {
            // 段 k 的状态约定为 2k（await 段 k=0 → state 0，其 Ready 推进到 k+2）；
            // 尾段（最后一个 segment）为 2k。
            cond: state_eq(2 * k, span),
            then_block: AstBlock {
                stmts: block_stmts,
                final_expr: None,
                span,
            },
            else_block: None,
        },
        span,
    ))
}

pub(super) fn state_if_await(a: &AnalyzedAsync, seg: &Segment, k: i64, boxed_slots: &HashSet<String>) -> AstStmt {
    let span = span_of(a);
    let ai = seg.await_info.as_ref().expect("await segment");
    let fs = future_sources(a);
    let mut block_stmts: Vec<AstStmt> = seg
        .stmts
        .iter()
        .filter_map(|s| rewrite_stmt(s, &a.lifted, &fs, span))
        .collect();

    // 子 future 求值（async fn 调用才需要：求值结果存入槽）
    let receiver = match ai.target() {
        AwaitTarget::AsyncFnCall { callee, args } => {
            let slot = ai.slot().expect("slot for async fn call");
            let call = AstExpr::new(
                ExprKind::Call {
                    callee: ident(callee, span),
                    args: args
                        .iter()
                        .map(|arg| rewrite_expr(arg, &a.lifted, span))
                        .collect(),
                    type_args: Vec::new(),
                },
                span,
            );
            // 递归环内的槽是 `Box<__Fut_>`：首次求值需 `Box::new(...)` 包装。
            let stored = if boxed_slots.contains(slot) {
                box_expr(call, span)
            } else {
                call
            };
            block_stmts.push(assign(self_field(slot, span), stored, span));
            self_field(slot, span)
        }
        AwaitTarget::IdentVar { var } => self_field(var, span),
    };

    // 首轮询
    block_stmts.push(poll_match(a, seg, ai, receiver, k, false));

    AstStmt::Semi(AstExpr::new(
        ExprKind::If {
            // await 段 k 的首轮询状态为 2k
            cond: state_eq(2 * k, span),
            then_block: AstBlock {
                stmts: block_stmts,
                final_expr: None,
                span,
            },
            else_block: None,
        },
        span,
    ))
}

pub(super) fn resume_if(a: &AnalyzedAsync, seg: &Segment, k: i64) -> AstStmt {
    let span = span_of(a);
    let ai = seg.await_info.as_ref().expect("await segment");
    let receiver = match ai.target() {
        AwaitTarget::AsyncFnCall { .. } => {
            self_field(ai.slot().expect("slot for async fn call"), span)
        }
        AwaitTarget::IdentVar { var } => self_field(var, span),
    };
    let block_stmts = vec![poll_match(a, seg, ai, receiver, k, true)];
    AstStmt::Semi(AstExpr::new(
        ExprKind::If {
            // await 段 k 的恢复轮询状态为 2k+1
            cond: state_eq(2 * k + 1, span),
            then_block: AstBlock {
                stmts: block_stmts,
                final_expr: None,
                span,
            },
            else_block: None,
        },
        span,
    ))
}

pub(super) fn poll_match(
    a: &AnalyzedAsync,
    seg: &Segment,
    ai: &AwaitInfo,
    receiver: AstExpr,
    k: i64,
    resume: bool,
) -> AstStmt {
    let span = span_of(a);
    let arms = vec![
        MatchArm {
            pattern: AstPattern::EnumPath(
                vec!["Poll".to_string(), "Ready".to_string()],
                vec![AstPattern::Ident("__v".to_string())],
            ),
            guard: None,
            body: AstExpr::new(ExprKind::Block(ready_block(a, seg, ai, span)), span),
            span,
        },
        MatchArm {
            pattern: AstPattern::EnumPath(
                vec!["Poll".to_string(), "Pending".to_string()],
                Vec::new(),
            ),
            guard: None,
            body: AstExpr::new(ExprKind::Block(pending_block(k, resume, span)), span),
            span,
        },
    ];
    AstStmt::Semi(AstExpr::new(
        ExprKind::Match {
            expr: poll_call(receiver, span),
            arms,
        },
        span,
    ))
}

pub(super) fn ready_block(_a: &AnalyzedAsync, seg: &Segment, ai: &AwaitInfo, span: Span) -> AstBlock {
    match ai {
        AwaitInfo::LetBind { var, .. } => {
            let next = seg.next.expect("LetBind await 段后继（展开后应已回填）");
            AstBlock {
                stmts: vec![
                    assign(self_field(var, span), ident("__v", span), span),
                    state_assign_stmt(next, span),
                ],
                final_expr: None,
                span,
            }
        }
        AwaitInfo::ExprStmt { .. } => {
            let next = seg.next.expect("ExprStmt await 段后继（展开后应已回填）");
            AstBlock {
                stmts: vec![state_assign_stmt(next, span)],
                final_expr: None,
                span,
            }
        }
        AwaitInfo::ReturnVal { .. } | AwaitInfo::TailValue { .. } => AstBlock {
            stmts: vec![AstStmt::Semi(ret_expr(
                poll_ready(ident("__v", span), span),
                span,
            ))],
            final_expr: None,
            span,
        },
    }
}

pub(super) fn pending_block(k: i64, resume: bool, span: Span) -> AstBlock {
    let mut stmts = Vec::new();
    if !resume {
        stmts.push(assign(
            self_field("state", span),
            int_expr((2 * k + 1) as i128, span),
            span,
        ));
    }
    stmts.push(AstStmt::Semi(ret_expr(poll_pending(span), span)));
    AstBlock {
        stmts,
        final_expr: None,
        span,
    }
}
