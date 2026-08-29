//! 表达式检查子模块：rewrite。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

pub(super) fn rewrite_stmt(
    stmt: &AstStmt,
    lifted: &HashSet<String>,
    future_sources: &HashSet<String>,
    span: Span,
) -> Option<AstStmt> {
    match stmt {
        AstStmt::Let {
            pattern,
            type_anno,
            init,
            mutable,
        } => {
            let span = init.span;
            if let AstPattern::Ident(x) = pattern {
                if future_sources.contains(x) {
                    return None;
                }
                if lifted.contains(x) {
                    // 跨段变量：let → self.x = init
                    return Some(AstStmt::Semi(AstExpr::new(
                        ExprKind::Assign {
                            target: self_field(x, span),
                            op: rlyeh_ast::AssignOp::Assign,
                            value: rewrite_expr(init, lifted, span),
                        },
                        span,
                    )));
                }
            }
            // 未提升：保留 let
            Some(AstStmt::Let {
                pattern: pattern.clone(),
                type_anno: type_anno.clone(),
                init: rewrite_expr(init, lifted, span),
                mutable: *mutable,
            })
        }
        AstStmt::Semi(e) | AstStmt::Expr(e) => {
            let mut e2 = rewrite_expr(e, lifted, span);
            // 普通 return → return Poll::Ready(v)
            if let ExprKind::Return(val) = &mut *e2.kind {
                let v = match val.take() {
                    Some(v) => v,
                    None => int_expr(0, span),
                };
                *val = Some(poll_ready(v, span));
            }
            match stmt {
                AstStmt::Semi(_) => Some(AstStmt::Semi(e2)),
                _ => Some(AstStmt::Expr(e2)),
            }
        }
        other => Some(other.clone()),
    }
}

pub(super) fn rewrite_expr(e: &AstExpr, lifted: &HashSet<String>, span: Span) -> AstExpr {
    match &*e.kind {
        ExprKind::Ident(name) => {
            if lifted.contains(name) {
                self_field(name, span)
            } else {
                e.clone()
            }
        }
        ExprKind::Set(items) => AstExpr::new(
            ExprKind::Set(items.iter().map(|i| rewrite_expr(i, lifted, span)).collect()),
            span,
        ),
        ExprKind::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => AstExpr::new(
            ExprKind::Range {
                // P8：边界可为 None（切片省略边界）
                lower: lower.as_ref().map(|e| rewrite_expr(e, lifted, span)),
                upper: upper.as_ref().map(|e| rewrite_expr(e, lifted, span)),
                lower_inclusive: *lower_inclusive,
                upper_inclusive: *upper_inclusive,
            },
            span,
        ),
        ExprKind::Binary { op, left, right } => AstExpr::new(
            ExprKind::Binary {
                op: *op,
                left: rewrite_expr(left, lifted, span),
                right: rewrite_expr(right, lifted, span),
            },
            span,
        ),
        ExprKind::Unary { op, operand } => AstExpr::new(
            ExprKind::Unary {
                op: *op,
                operand: rewrite_expr(operand, lifted, span),
            },
            span,
        ),
        ExprKind::ComparisonChain { elements, operators } => AstExpr::new(
            ExprKind::ComparisonChain {
                elements: elements
                    .iter()
                    .map(|el| rewrite_expr(el, lifted, span))
                    .collect(),
                operators: operators.clone(),
            },
            span,
        ),
        ExprKind::InSet { value, set, negated } => AstExpr::new(
            ExprKind::InSet {
                value: rewrite_expr(value, lifted, span),
                set: set.iter().map(|s| rewrite_expr(s, lifted, span)).collect(),
                negated: *negated,
            },
            span,
        ),
        ExprKind::InRange {
            value,
            range,
            negated,
        } => AstExpr::new(
            ExprKind::InRange {
                value: rewrite_expr(value, lifted, span),
                range: rewrite_expr(range, lifted, span),
                negated: *negated,
            },
            span,
        ),
        ExprKind::InRegion { expr, region } => AstExpr::new(
            ExprKind::InRegion {
                expr: rewrite_expr(expr, lifted, span),
                region: region.clone(),
            },
            span,
        ),
        ExprKind::Assign { target, op, value } => AstExpr::new(
            ExprKind::Assign {
                target: rewrite_expr(target, lifted, span),
                op: *op,
                value: rewrite_expr(value, lifted, span),
            },
            span,
        ),
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => AstExpr::new(
            ExprKind::If {
                cond: rewrite_expr(cond, lifted, span),
                then_block: rewrite_block(then_block, lifted, span),
                else_block: else_block
                    .as_ref()
                    .map(|b| rewrite_block(b, lifted, span)),
            },
            span,
        ),
        ExprKind::Match { expr, arms } => AstExpr::new(
            ExprKind::Match {
                expr: rewrite_expr(expr, lifted, span),
                arms: arms
                    .iter()
                    .map(|arm| MatchArm {
                        pattern: arm.pattern.clone(),
                        guard: arm
                            .guard
                            .as_ref()
                            .map(|g| rewrite_expr(g, lifted, span)),
                        body: rewrite_expr(&arm.body, lifted, span),
                        span: arm.span,
                    })
                    .collect(),
            },
            span,
        ),
        ExprKind::For {
            pattern,
            iterator,
            body,
        } => AstExpr::new(
            ExprKind::For {
                pattern: pattern.clone(),
                iterator: rewrite_expr(iterator, lifted, span),
                body: rewrite_block(body, lifted, span),
            },
            span,
        ),
        ExprKind::While { cond, body } => AstExpr::new(
            ExprKind::While {
                cond: rewrite_expr(cond, lifted, span),
                body: rewrite_block(body, lifted, span),
            },
            span,
        ),
        ExprKind::Loop { body } => AstExpr::new(
            ExprKind::Loop {
                body: rewrite_block(body, lifted, span),
            },
            span,
        ),
        ExprKind::Region {
            name,
            options,
            body,
        } => AstExpr::new(
            ExprKind::Region {
                name: name.clone(),
                options: *options,
                body: rewrite_block(body, lifted, span),
            },
            span,
        ),
        ExprKind::GcRegion { body } => AstExpr::new(
            ExprKind::GcRegion {
                body: rewrite_block(body, lifted, span),
            },
            span,
        ),
        ExprKind::Transfer { expr, region } => AstExpr::new(
            ExprKind::Transfer {
                expr: rewrite_expr(expr, lifted, span),
                region: region.clone(),
            },
            span,
        ),
        ExprKind::Call {
            callee,
            args,
            type_args,
        } => AstExpr::new(
            ExprKind::Call {
                callee: match &*callee.kind {
                    // 函数名/枚举路径不重写
                    ExprKind::Ident(_) | ExprKind::Path(_) => callee.clone(),
                    _ => rewrite_expr(callee, lifted, span),
                },
                args: args
                    .iter()
                    .map(|arg| rewrite_expr(arg, lifted, span))
                    .collect(),
                type_args: type_args.clone(),
            },
            span,
        ),
        ExprKind::MacroCall { name, args } => AstExpr::new(
            ExprKind::MacroCall {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| rewrite_expr(arg, lifted, span))
                    .collect(),
            },
            span,
        ),
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            trait_hint,
        } => AstExpr::new(
            ExprKind::MethodCall {
                receiver: rewrite_expr(receiver, lifted, span),
                method: method.clone(),
                args: args
                    .iter()
                    .map(|arg| rewrite_expr(arg, lifted, span))
                    .collect(),
                trait_hint: trait_hint.clone(),
            },
            span,
        ),
        ExprKind::FieldAccess { expr, field } => AstExpr::new(
            ExprKind::FieldAccess {
                expr: rewrite_expr(expr, lifted, span),
                field: field.clone(),
            },
            span,
        ),
        ExprKind::StructCtor {
            type_name,
            type_args,
            fields,
        } => AstExpr::new(
            ExprKind::StructCtor {
                type_name: type_name.clone(),
                type_args: type_args.clone(),
                fields: fields
                    .iter()
                    .map(|(k, v)| (k.clone(), rewrite_expr(v, lifted, span)))
                    .collect(),
            },
            span,
        ),
        ExprKind::Index { expr, index } => AstExpr::new(
            ExprKind::Index {
                expr: rewrite_expr(expr, lifted, span),
                index: rewrite_expr(index, lifted, span),
            },
            span,
        ),
        ExprKind::ArrayLit(items) => AstExpr::new(
            ExprKind::ArrayLit(
                items
                    .iter()
                    .map(|i| rewrite_expr(i, lifted, span))
                    .collect(),
            ),
            span,
        ),
        ExprKind::Closure {
            params,
            param_types,
            body,
            capture,
        } => AstExpr::new(
            ExprKind::Closure {
                params: params.clone(),
                param_types: param_types.clone(),
                body: rewrite_expr(body, lifted, span),
                capture: *capture,
            },
            span,
        ),
        ExprKind::Cast {
            expr,
            target_type,
        } => AstExpr::new(
            ExprKind::Cast {
                expr: rewrite_expr(expr, lifted, span),
                target_type: target_type.clone(),
            },
            span,
        ),
        ExprKind::Block(block) => AstExpr::new(
            ExprKind::Block(rewrite_block(block, lifted, span)),
            span,
        ),
        ExprKind::Return(Some(v)) => AstExpr::new(
            ExprKind::Return(Some(rewrite_expr(v, lifted, span))),
            span,
        ),
        ExprKind::Question(inner) => AstExpr::new(
            ExprKind::Question(Box::new(rewrite_expr(inner, lifted, span))),
            span,
        ),
        ExprKind::Break(Some(v)) => AstExpr::new(
            ExprKind::Break(Some(rewrite_expr(v, lifted, span))),
            span,
        ),
        ExprKind::Send {
            actor,
            method,
            args,
        } => AstExpr::new(
            ExprKind::Send {
                actor: rewrite_expr(actor, lifted, span),
                method: method.clone(),
                args: args
                    .iter()
                    .map(|arg| rewrite_expr(arg, lifted, span))
                    .collect(),
            },
            span,
        ),
        // 其余无子表达式或已由上层处理
        _ => e.clone(),
    }
}

pub(super) fn future_sources(a: &AnalyzedAsync) -> HashSet<String> {
    a.lifted_future_sources
        .iter()
        .map(|(v, _, _)| v.clone())
        .collect()
}

pub(super) fn rewrite_block(block: &AstBlock, lifted: &HashSet<String>, span: Span) -> AstBlock {
    AstBlock {
        stmts: block
            .stmts
            .iter()
            .map(|s| rewrite_stmt(s, lifted, &HashSet::new(), span).expect("non-top-level stmt"))
            .collect(),
        final_expr: block
            .final_expr
            .as_ref()
            .map(|fe| rewrite_expr(fe, lifted, span)),
        span: block.span,
    }
}
