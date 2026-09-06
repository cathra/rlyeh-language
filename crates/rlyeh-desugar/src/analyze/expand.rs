//! 表达式检查子模块：expand。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

pub(super) fn is_control_flow_expr(e: &AstExpr) -> bool {
    matches!(
        &*e.kind,
        ExprKind::If { .. }
            | ExprKind::Match { .. }
            | ExprKind::For { .. }
            | ExprKind::While { .. }
            | ExprKind::Loop { .. }
            | ExprKind::Region { .. }
            | ExprKind::GcRegion { .. }
    )
}

pub(super) fn expand_control_stmt(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    e: &AstExpr,
    _cont: usize,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> Result<(), DesugarError> {
    let start = out.len();
    // 展开控制流，cont = CONT_PENDING（分支汇合占位）
    expand_control(ctx, out, e, CONT_PENDING, first, last)?;
    // 在控制流段之后追加汇合占位段 ph
    let ph = out.len();
    out.push(Segment {
        stmts: Vec::new(),
        await_info: None,
        final_expr: None,
        next: None,
        inline_jump: false,
    });
    ctx.seg_uses.push(HashSet::new());
    // 回填 [start, ph) 段的 CONT_PENDING → ph（Segment.next 与内联 state 跳转）
    for seg in &mut out[start..ph] {
        if seg.next == Some(CONT_PENDING) {
            seg.next = Some(ph);
        }
        for stmt in &mut seg.stmts {
            replace_cont_pending_stmt(stmt, ph);
        }
    }
    // ph.next = 控制流之后第一段编号（ph 创建后 out.len()）。
    // 若控制流后无后续，expand_block 末尾回填将覆盖为块级 cont。
    out[ph].next = Some(out.len());
    // 使 ph 成为块末段：无后续时末尾回填作用于 ph；有后续时被后续段覆盖。
    *last = Some(ph);
    Ok(())
}

pub(super) fn replace_cont_pending_stmt(stmt: &mut AstStmt, ph: usize) {
    match stmt {
        AstStmt::Semi(e) | AstStmt::Expr(e) => replace_cont_pending_expr(e, ph),
        AstStmt::Let { init, .. } => replace_cont_pending_expr(init, ph),
        AstStmt::Item(_) => {}
    }
}

pub(super) fn replace_cont_pending_expr(e: &mut AstExpr, ph: usize) {
    if let ExprKind::Assign { target, value, .. } = &mut *e.kind {
        if let ExprKind::FieldAccess { expr, field } = &*target.kind {
            if field == "state"
                && matches!(&*expr.kind, ExprKind::Ident(name) if name == "self")
                && matches!(&*value.kind, ExprKind::IntLiteral(v) if *v == CONT_PENDING_TARGET)
            {
                *value.kind = ExprKind::IntLiteral((2 * ph) as i128);
                return;
            }
        }
    }
    let children = expr_children_mut(e);
    for child in children {
        replace_cont_pending_expr(child, ph);
    }
}

pub(super) fn expand_control(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    e: &AstExpr,
    cont: usize,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> Result<(), DesugarError> {
    match &*e.kind {
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => expand_if(ctx, out, cond, then_block, else_block.as_ref(), cont, first, last),
        ExprKind::Match { expr, arms } => expand_match(ctx, out, expr, arms, cont, first, last),
        ExprKind::Loop { body } => expand_loop(ctx, out, body, cont, first, last),
        ExprKind::While { cond, body } => expand_while(ctx, out, cond, body, cont, first, last),
        ExprKind::For {
            pattern,
            iterator,
            body,
        } => expand_for(ctx, out, pattern, iterator, body, cont, first, last),
        ExprKind::Region { .. } | ExprKind::GcRegion { .. } => Err(DesugarError::Unsupported {
            what: "W2 MVP：region 块内 await 暂不支持".to_string(),
            span: e.span,
        }),
        _ => unreachable!("expand_control: 非控制流"),
    }
}

pub(super) fn expand_if(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    cond: &AstExpr,
    then_block: &AstBlock,
    else_block: Option<&AstBlock>,
    cont: usize,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> Result<(), DesugarError> {
    // cond 含嵌套 await → 提取（生成前置 await 段）
    let mut cur_uses = HashSet::new();
    let cond2 = if contains_await_expr(cond) {
        extract_expr_awaits(ctx, out, cond, &mut cur_uses, first, last)?
    } else {
        cond.clone()
    };
    scan_expr(&cond2, &mut cur_uses).map_err(|_| DesugarError::Unsupported {
        what: "if 条件含嵌套 await（MVP）".to_string(),
        span: cond.span,
    })?;

    let entry = out.len();
    out.push(Segment {
        stmts: Vec::new(),
        await_info: None,
        final_expr: None,
        next: None,
        inline_jump: false,
    });
    ctx.seg_uses.push(std::mem::take(&mut cur_uses));
    if first.is_none() {
        *first = Some(entry);
    }
    *last = Some(entry);

    // 展开 then / else
    let t_first = expand_block(ctx, out, then_block, cont)?.0;
    let e_first = match else_block {
        Some(eb) => expand_block(ctx, out, eb, cont)?.0,
        None => None,
    };
    // 回填 entry：if cond { state=T } else { state=E }
    let t_state = t_first.unwrap_or(cont);
    let e_state = e_first.unwrap_or(cont);
    out[entry].stmts = vec![AstStmt::Semi(mk_if(
        cond2,
        vec![state_assign(t_state, ctx.span)],
        vec![state_assign(e_state, ctx.span)],
        ctx.span,
    ))];
    out[entry].inline_jump = true;
    Ok(())
}

pub(super) fn expand_loop(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    body: &AstBlock,
    cont: usize,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> Result<(), DesugarError> {
    let head = out.len();
    out.push(Segment {
        stmts: Vec::new(),
        await_info: None,
        final_expr: None,
        next: None,
        inline_jump: true,
    });
    ctx.seg_uses.push(HashSet::new());
    if first.is_none() {
        *first = Some(head);
    }
    *last = Some(head);
    // body 展开：cont = head（末段回跳）
    let b_first = expand_block(ctx, out, body, head)?.0;
    let b_state = b_first.unwrap_or(cont);
    // 回填 head：self.state = 2*b_first
    out[head].stmts = vec![state_assign(b_state, ctx.span)];
    Ok(())
}

pub(super) fn expand_while(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    cond: &AstExpr,
    body: &AstBlock,
    cont: usize,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> Result<(), DesugarError> {
    let mut cur_uses = HashSet::new();
    let cond2 = if contains_await_expr(cond) {
        extract_expr_awaits(ctx, out, cond, &mut cur_uses, first, last)?
    } else {
        cond.clone()
    };
    scan_expr(&cond2, &mut cur_uses)?;
    let head = out.len();
    out.push(Segment {
        stmts: Vec::new(),
        await_info: None,
        final_expr: None,
        next: None,
        inline_jump: true,
    });
    ctx.seg_uses.push(std::mem::take(&mut cur_uses));
    if first.is_none() {
        *first = Some(head);
    }
    *last = Some(head);
    let b_first = expand_block(ctx, out, body, head)?.0;
    let b_state = b_first.unwrap_or(cont);
    // 回填 head：if cond { state=B } else { state=cont }
    out[head].stmts = vec![AstStmt::Semi(mk_if(
        cond2,
        vec![state_assign(b_state, ctx.span)],
        vec![state_assign(cont, ctx.span)],
        ctx.span,
    ))];
    Ok(())
}

pub(super) fn expand_for(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    pattern: &AstPattern,
    iterator: &AstExpr,
    body: &AstBlock,
    cont: usize,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> Result<(), DesugarError> {
    let (lo, hi, lower_inclusive, upper_inclusive) = match &*iterator.kind {
        ExprKind::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => {
            // P8：for 区间迭代须显式边界（省略边界 `..` 仅切片 `v[..]` 支持）
            let lower = lower.as_ref().ok_or_else(|| DesugarError::Unsupported {
                what: "W2 MVP：for 内 await 区间迭代器须显式下界（省略边界 `..` 仅切片支持）"
                    .to_string(),
                span: iterator.span,
            })?;
            let upper = upper.as_ref().ok_or_else(|| DesugarError::Unsupported {
                what: "W2 MVP：for 内 await 区间迭代器须显式上界（省略边界 `..` 仅切片支持）"
                    .to_string(),
                span: iterator.span,
            })?;
            let lo = if contains_await_expr(lower) {
                let mut u = HashSet::new();
                extract_expr_awaits(ctx, out, lower, &mut u, first, last)?
            } else {
                lower.clone()
            };
            let hi = if contains_await_expr(upper) {
                let mut u = HashSet::new();
                extract_expr_awaits(ctx, out, upper, &mut u, first, last)?
            } else {
                upper.clone()
            };
            (lo, hi, *lower_inclusive, *upper_inclusive)
        }
        _ => {
            return Err(DesugarError::Unsupported {
                what: "W2 MVP：for 内 await 仅支持数值区间迭代器（`0..<n`/`0...n` 等），\
                     数组/Vec/自定义迭代器内 await 规划中"
                    .to_string(),
                span: iterator.span,
            })
        }
    };
    let lv = match pattern {
        AstPattern::Ident(x) => x.clone(),
        _ => {
            return Err(DesugarError::Unsupported {
                what: "W2 MVP：for 内 await 的循环变量限简单标识符".to_string(),
                span: iterator.span,
            })
        }
    };
    let span = iterator.span;
    let i64_ty = AstType::Path("i64".to_string(), Vec::new());

    // 内部迭代字段：__r_i（游标）、__r_hi（上界）；循环变量提升（i64，init 0）
    let it_var = format!("__r_{}", ctx.internal.len());
    ctx.internal.push((format!("{it_var}_i"), i64_ty.clone()));
    ctx.internal.push((format!("{it_var}_hi"), i64_ty.clone()));
    ctx.lifted.insert(format!("{it_var}_i"));
    ctx.lifted.insert(format!("{it_var}_hi"));
    if !ctx.lifted.contains(&lv) {
        ctx.lifted.insert(lv.clone());
        ctx.cross.push((lv.clone(), i64_ty.clone(), int_expr(0, span)));
    }

    // init 段：self.__r_i = 起始值; self.__r_hi = hi;
    let start = if lower_inclusive {
        lo
    } else {
        binop(BinaryOp::Add, lo, int_expr(1, span), span)
    };
    let mut cur_stmts = Vec::new();
    cur_stmts.push(assign(self_field(&format!("{it_var}_i"), span), start, span));
    cur_stmts.push(assign(self_field(&format!("{it_var}_hi"), span), hi, span));
    flush_raw(ctx, out, &mut cur_stmts, first, last);

    // head 段（占位，inline jump）
    let head = out.len();
    out.push(Segment {
        stmts: Vec::new(),
        await_info: None,
        final_expr: None,
        next: None,
        inline_jump: true,
    });
    ctx.seg_uses.push(HashSet::new());
    if first.is_none() {
        *first = Some(head);
    }
    *last = Some(head);

    // body 展开：cont = head（末段回跳）
    let b_first = expand_block(ctx, out, body, head)?.0;
    let b_state = b_first.unwrap_or(cont);

    // 回填 head：
    //   if self.__r_i < self.__r_hi {
    //       self.<lv> = self.__r_i;
    //       self.__r_i = self.__r_i + 1;
    //       self.state = 2*b_state;
    //   } else { self.state = 2*cont; }
    let cmp_op = if upper_inclusive { CompareOp::Le } else { CompareOp::Lt };
    let cond = cmp(
        cmp_op,
        self_field(&format!("{it_var}_i"), span),
        self_field(&format!("{it_var}_hi"), span),
        span,
    );
    let then_stmts = vec![
        assign(self_field(&lv, span), self_field(&format!("{it_var}_i"), span), span),
        assign(
            self_field(&format!("{it_var}_i"), span),
            binop(
                BinaryOp::Add,
                self_field(&format!("{it_var}_i"), span),
                int_expr(1, span),
                span,
            ),
            span,
        ),
        state_assign(b_state, span),
    ];
    out[head].stmts = vec![AstStmt::Semi(mk_if(
        cond,
        then_stmts,
        vec![state_assign(cont, span)],
        span,
    ))];
    Ok(())
}

pub(super) fn expand_match(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    expr: &AstExpr,
    arms: &[MatchArm],
    cont: usize,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> Result<(), DesugarError> {
    // 被匹配表达式含嵌套 await → 提取
    let mut cur_uses = HashSet::new();
    let expr2 = if contains_await_expr(expr) {
        extract_expr_awaits(ctx, out, expr, &mut cur_uses, first, last)?
    } else {
        expr.clone()
    };
    scan_expr(&expr2, &mut cur_uses)?;

    // entry 段（占位，inline jump）
    let entry = out.len();
    out.push(Segment {
        stmts: Vec::new(),
        await_info: None,
        final_expr: None,
        next: None,
        inline_jump: true,
    });
    ctx.seg_uses.push(std::mem::take(&mut cur_uses));
    if first.is_none() {
        *first = Some(entry);
    }
    *last = Some(entry);

    // 展开各臂（臂体限块表达式；臂 pattern 限无绑定变量）
    let mut arm_states: Vec<(AstPattern, usize)> = Vec::new();
    for arm in arms {
        if arm.guard.is_some() {
            return Err(DesugarError::Unsupported {
                what: "W2 MVP：match 臂 guard 与 await 组合暂不支持".to_string(),
                span: arm.span,
            });
        }
        if pattern_has_bindings(&arm.pattern) {
            return Err(DesugarError::Unsupported {
                what: "W2 MVP：match 臂内 await 时，臂模式绑定变量暂不支持（绑定变量跨 await 需类型信息）"
                    .to_string(),
                span: arm.span,
            });
        }
        let a_first = match &*arm.body.kind {
            ExprKind::Block(block) | ExprKind::UnsafeBlock(block) => expand_block(ctx, out, block, cont)?.0,
            _ => {
                return Err(DesugarError::Unsupported {
                    what: "W2 MVP：match 臂体限块表达式（含 await 时）".to_string(),
                    span: arm.span,
                })
            }
        };
        arm_states.push((arm.pattern.clone(), a_first.unwrap_or(cont)));
    }

    // 回填 entry：match expr { pat1 => { state=A1 }, pat2 => { state=A2 }, ... }
    let new_arms: Vec<MatchArm> = arm_states
        .into_iter()
        .map(|(p, s)| {
            let body_expr = AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![state_assign(s, ctx.span)],
                    final_expr: None,
                    span: ctx.span,
                }),
                ctx.span,
            );
            MatchArm {
                pattern: p,
                guard: None,
                body: body_expr,
                span: ctx.span,
            }
        })
        .collect();
    let match_expr = AstExpr::new(
        ExprKind::Match {
            expr: expr2,
            arms: new_arms,
        },
        ctx.span,
    );
    out[entry].stmts = vec![AstStmt::Semi(match_expr)];
    Ok(())
}

pub(super) fn pattern_has_bindings(p: &AstPattern) -> bool {
    match p {
        AstPattern::Ident(_) => true,
        // `_`（通配符）不绑定变量，不应阻止 match 内 await
        AstPattern::Wildcard => false,
        AstPattern::Tuple(ps, _) => ps.iter().any(pattern_has_bindings),
        AstPattern::Struct(_, fields) => fields.iter().any(|(_, x)| pattern_has_bindings(x)),
        AstPattern::Enum(_, ps) | AstPattern::EnumPath(_, ps) => ps.iter().any(pattern_has_bindings),
        AstPattern::EnumStructPath(_, ps) => ps.iter().any(|(_, x)| pattern_has_bindings(x)),
        AstPattern::Ref(inner, _) => pattern_has_bindings(inner),
        _ => false,
    }
}
