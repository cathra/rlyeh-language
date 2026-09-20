//! 表达式检查子模块：scan。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

pub(super) fn resolve_target(
    inner: &AstExpr,
    ctx: &mut Ctx,
) -> Result<(AwaitTarget, Option<String>), DesugarError> {
    match &*inner.kind {
        ExprKind::Call { callee, args, .. } => match &*callee.kind {
            ExprKind::Ident(name) => {
                if ctx.async_names.contains(name) {
                    // W6：泛型 async fn 作为子 future await 暂不支持（槽类型需带
                    // type_args + 泛型字段零值，单态化后类型未知）；独立 block_on 支持。
                    if ctx.generic_async.contains(name) {
                        return Err(DesugarError::Unsupported {
                            what: format!(
                                "泛型 async fn `{name}` 作为子 future await 暂不支持（MVP，独立 block_on 支持）"
                            ),
                            span: inner.span,
                        });
                    }
                    if !ctx.deps.iter().any(|d| d == name) {
                        ctx.deps.push(name.clone());
                    }
                    let ty = format!("__Fut_{}", name);
                    let slot = if let Some((n, _)) = ctx.slots.iter().find(|(n, _)| n == &ty) {
                        n.clone()
                    } else {
                        ctx.slots.push((ty.clone(), ty.clone()));
                        ty
                    };
                    Ok((AwaitTarget::AsyncFnCall { callee: name.clone(), args: args.clone() }, Some(slot)))
                } else {
                    Err(DesugarError::Unsupported {
                        what: format!("await 目标 `{name}(...)` 必须是 async fn（MVP）"),
                        span: inner.span,
                    })
                }
            }
            _ => Err(DesugarError::Unsupported {
                what: "await 目标须为 async fn 直接调用（MVP，方法调用 await 规划中）".to_string(),
                span: inner.span,
            }),
        },
        ExprKind::Ident(name) => Ok((AwaitTarget::IdentVar { var: name.clone() }, None)),
        _ => Err(DesugarError::Unsupported {
            what: "await 目标须为 async fn 调用或 let 绑定变量（MVP）".to_string(),
            span: inner.span,
        }),
    }
}

pub(super) fn contains_await_expr(e: &AstExpr) -> bool {
    let mut uses = HashSet::new();
    scan_expr(e, &mut uses).is_err()
}

pub(super) fn check_no_nested_await(e: &AstExpr, span: Span) -> Result<(), DesugarError> {
    if contains_await_expr(e) {
        return Err(DesugarError::Unsupported {
            what: "await 表达式内禁止嵌套 await（MVP）".to_string(),
            span,
        });
    }
    Ok(())
}

pub(super) fn scan_expr(e: &AstExpr, uses: &mut HashSet<String>) -> Result<(), ()> {
    match &*e.kind {
        ExprKind::Await(_) => Err(()),
        ExprKind::Ident(name) => {
            uses.insert(name.clone());
            Ok(())
        }
        ExprKind::Path(_) => Ok(()),
        ExprKind::IntLiteral(_)
        | ExprKind::FloatLiteral(_)
        | ExprKind::StringLiteral(_)
        | ExprKind::BoolLiteral(_)
        | ExprKind::CharLiteral(_)
        | ExprKind::TimeLiteral { .. }
        | ExprKind::Unit => Ok(()),
        ExprKind::Set(items) | ExprKind::ArrayLit(items) | ExprKind::TupleLit(items) => {
            for it in items {
                scan_expr(it, uses)?;
            }
            Ok(())
        }
        ExprKind::Range {
            lower,
            upper,
            ..
        } => {
            // P8：边界可为 None（切片省略边界），仅扫描 Some 侧
            if let Some(l) = lower {
                scan_expr(l, uses)?;
            }
            if let Some(u) = upper {
                scan_expr(u, uses)?
            }
            Ok(())
        }
        ExprKind::Unary { operand, .. } => scan_expr(operand, uses),
        ExprKind::Binary { left, right, .. } => {
            scan_expr(left, uses)?;
            scan_expr(right, uses)
        }
        ExprKind::ComparisonChain { elements, .. } => {
            for el in elements {
                scan_expr(el, uses)?;
            }
            Ok(())
        }
        ExprKind::InSet { value, set, .. } => {
            scan_expr(value, uses)?;
            for s in set {
                scan_expr(s, uses)?;
            }
            Ok(())
        }
        ExprKind::InRange { value, range, .. } => {
            scan_expr(value, uses)?;
            scan_expr(range, uses)
        }
        ExprKind::InContainer { value, container, .. } => {
            scan_expr(value, uses)?;
            scan_expr(container, uses)
        }
        ExprKind::InRegion { expr, .. } => scan_expr(expr, uses),
        ExprKind::Assign { target, value, .. } => {
            scan_expr(target, uses)?;
            scan_expr(value, uses)
        }
        ExprKind::FieldAccess { expr, .. } | ExprKind::Cast { expr, .. } => scan_expr(expr, uses),
        ExprKind::Index { expr, index } => {
            scan_expr(expr, uses)?;
            scan_expr(index, uses)
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                scan_expr(v, uses)?;
            }
            Ok(())
        }
        ExprKind::Call {
            callee, args, ..
        } => {
            // callee 为函数名/枚举路径，不收集（非变量）；其余保守扫描
            match &*callee.kind {
                ExprKind::Ident(_) | ExprKind::Path(_) => {}
                _ => scan_expr(callee, uses)?,
            }
            for a in args {
                scan_expr(a, uses)?;
            }
            Ok(())
        }
        ExprKind::MacroCall { args, .. } => {
            for a in args {
                scan_expr(a, uses)?;
            }
            Ok(())
        }
        ExprKind::MethodCall {
            receiver,
            args,
            ..
        } => {
            scan_expr(receiver, uses)?;
            for a in args {
                scan_expr(a, uses)?;
            }
            Ok(())
        }
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            scan_expr(cond, uses)?;
            scan_block(then_block, uses)?;
            if let Some(eb) = else_block {
                scan_block(eb, uses)?;
            }
            Ok(())
        }
        ExprKind::Match { expr, arms } => {
            scan_expr(expr, uses)?;
            for arm in arms {
                scan_expr(&arm.body, uses)?;
            }
            Ok(())
        }
        ExprKind::Block(block) | ExprKind::UnsafeBlock(block) => scan_block(block, uses),
        ExprKind::Loop { body } => scan_block(body, uses),
        ExprKind::While { cond, body } => {
            scan_expr(cond, uses)?;
            scan_block(body, uses)
        }
        ExprKind::For {
            iterator, body, ..
        } => {
            scan_expr(iterator, uses)?;
            scan_block(body, uses)
        }
        ExprKind::Region { body, .. } | ExprKind::GcRegion { body } => scan_block(body, uses),
        ExprKind::Transfer { expr, .. } => scan_expr(expr, uses),
        ExprKind::Return(ret) => {
            if let Some(r) = ret {
                scan_expr(r, uses)?;
            }
            Ok(())
        }
        ExprKind::Closure { body, .. } => scan_expr(body, uses),
        ExprKind::Question(inner) => scan_expr(inner, uses),
        ExprKind::Break(ret) => {
            if let Some(r) = ret {
                scan_expr(r, uses)?;
            }
            Ok(())
        }
        ExprKind::Send { actor, args, .. } => {
            scan_expr(actor, uses)?;
            for a in args {
                scan_expr(a, uses)?;
            }
            Ok(())
        }
        ExprKind::Continue => Ok(()),
    }
}

pub(super) fn scan_block(b: &AstBlock, uses: &mut HashSet<String>) -> Result<(), ()> {
    for s in &b.stmts {
        scan_stmt(s, uses)?;
    }
    if let Some(fe) = &b.final_expr {
        scan_expr(fe, uses)?;
    }
    Ok(())
}

pub(super) fn scan_stmt(s: &AstStmt, uses: &mut HashSet<String>) -> Result<(), ()> {
    match s {
        AstStmt::Let { init, .. } => scan_expr(init, uses),
        AstStmt::Semi(e) | AstStmt::Expr(e) => scan_expr(e, uses),
        _ => Err(()),
    }
}

pub(super) fn is_i64_ty(ty: &AstType) -> bool {
    matches!(ty, AstType::Path(n, args) if n == "i64" && args.is_empty())
}

pub(super) fn is_generic_ty(ty: &AstType, gen_names: &HashSet<String>) -> bool {
    match ty {
        AstType::Path(n, args) if args.is_empty() => gen_names.contains(n),
        _ => false,
    }
}

pub(super) fn is_liftable_ty(ty: &AstType) -> bool {
    match ty {
        AstType::Path(n, args) if args.is_empty() => {
            matches!(n.as_str(), "i64" | "f64" | "bool" | "char" | "String")
        }
        _ => false,
    }
}

pub(super) fn infer_ast_type(e: &AstExpr) -> Option<AstType> {
    match &*e.kind {
        ExprKind::IntLiteral(_) => Some(AstType::Path("i64".to_string(), Vec::new())),
        ExprKind::FloatLiteral(_) => Some(AstType::Path("f64".to_string(), Vec::new())),
        ExprKind::BoolLiteral(_) => Some(AstType::Path("bool".to_string(), Vec::new())),
        ExprKind::CharLiteral(_) => Some(AstType::Path("char".to_string(), Vec::new())),
        ExprKind::StringLiteral(_) => Some(AstType::Path("String".to_string(), Vec::new())),
        _ => None,
    }
}

pub(super) fn extract_expr_awaits(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    e: &AstExpr,
    cur_uses: &mut HashSet<String>,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> Result<AstExpr, DesugarError> {
    let span = e.span;
    let kind = match &*e.kind {
        ExprKind::Await(inner) => {
            // 先递归提取 inner（inner 可能含嵌套 await）
            let inner2 = extract_expr_awaits(ctx, out, inner, cur_uses, first, last)?;
            // 解析目标
            let (target, slot) = resolve_target(&inner2, ctx)?;
            scan_expr(&inner2, cur_uses)?;
            // 生成临时变量
            let tmp = format!("__w_{}", ctx.temp_seq);
            ctx.temp_seq += 1;
            ctx.await_results.push(tmp.clone());
            ctx.lifted.insert(tmp.clone());
            push_await(
                ctx,
                out,
                cur_uses,
                AwaitInfo::LetBind {
                    var: tmp.clone(),
                    target,
                    slot,
                },
                false,
                first,
                last,
            );
            ExprKind::Ident(tmp)
        }
        // 递归重建（跳过不适用节点）
        ExprKind::Unary { op, operand } => ExprKind::Unary {
            op: *op,
            operand: extract_expr_awaits(ctx, out, operand, cur_uses, first, last)?,
        },
        ExprKind::Binary { op, left, right } => ExprKind::Binary {
            op: *op,
            left: extract_expr_awaits(ctx, out, left, cur_uses, first, last)?,
            right: extract_expr_awaits(ctx, out, right, cur_uses, first, last)?,
        },
        ExprKind::ComparisonChain { elements, operators } => {
            let mut es = Vec::new();
            for el in elements {
                es.push(extract_expr_awaits(ctx, out, el, cur_uses, first, last)?);
            }
            ExprKind::ComparisonChain {
                elements: es,
                operators: operators.clone(),
            }
        }
        ExprKind::InSet { value, set, negated } => ExprKind::InSet {
            value: extract_expr_awaits(ctx, out, value, cur_uses, first, last)?,
            set: {
                let mut s = Vec::new();
                for el in set {
                    s.push(extract_expr_awaits(ctx, out, el, cur_uses, first, last)?);
                }
                s
            },
            negated: *negated,
        },
        ExprKind::InRange { value, range, negated } => ExprKind::InRange {
            value: extract_expr_awaits(ctx, out, value, cur_uses, first, last)?,
            range: extract_expr_awaits(ctx, out, range, cur_uses, first, last)?,
            negated: *negated,
        },
        ExprKind::InContainer { value, container, negated } => ExprKind::InContainer {
            value: extract_expr_awaits(ctx, out, value, cur_uses, first, last)?,
            container: extract_expr_awaits(ctx, out, container, cur_uses, first, last)?,
            negated: *negated,
        },
        ExprKind::InRegion { expr, region } => ExprKind::InRegion {
            expr: extract_expr_awaits(ctx, out, expr, cur_uses, first, last)?,
            region: region.clone(),
        },
        ExprKind::Assign { target, op, value } => ExprKind::Assign {
            target: extract_expr_awaits(ctx, out, target, cur_uses, first, last)?,
            op: *op,
            value: extract_expr_awaits(ctx, out, value, cur_uses, first, last)?,
        },
        ExprKind::FieldAccess { expr, field } => ExprKind::FieldAccess {
            expr: extract_expr_awaits(ctx, out, expr, cur_uses, first, last)?,
            field: field.clone(),
        },
        ExprKind::Index { expr, index } => ExprKind::Index {
            expr: extract_expr_awaits(ctx, out, expr, cur_uses, first, last)?,
            index: extract_expr_awaits(ctx, out, index, cur_uses, first, last)?,
        },
        ExprKind::Call {
            callee,
            args,
            type_args,
        } => {
            let mut as_ = Vec::new();
            for a in args {
                as_.push(extract_expr_awaits(ctx, out, a, cur_uses, first, last)?);
            }
            ExprKind::Call {
                callee: callee.clone(),
                args: as_,
                type_args: type_args.clone(),
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            protocol_hint,
        } => {
            let mut as_ = Vec::new();
            for a in args {
                as_.push(extract_expr_awaits(ctx, out, a, cur_uses, first, last)?);
            }
            ExprKind::MethodCall {
                receiver: extract_expr_awaits(ctx, out, receiver, cur_uses, first, last)?,
                method: method.clone(),
                args: as_,
                protocol_hint: protocol_hint.clone(),
            }
        }
        ExprKind::Return(ret) => {
            let r = match ret {
                Some(r) => Some(extract_expr_awaits(ctx, out, r, cur_uses, first, last)?),
                None => None,
            };
            ExprKind::Return(r)
        }
        ExprKind::Cast { expr, target_type } => ExprKind::Cast {
            expr: extract_expr_awaits(ctx, out, expr, cur_uses, first, last)?,
            target_type: target_type.clone(),
        },
        ExprKind::Question(inner) => ExprKind::Question(Box::new(extract_expr_awaits(
            ctx, out, inner, cur_uses, first, last,
        )?)),
        ExprKind::Set(items) => ExprKind::Set({
            let mut s = Vec::new();
            for el in items {
                s.push(extract_expr_awaits(ctx, out, el, cur_uses, first, last)?);
            }
            s
        }),
        ExprKind::TupleLit(items) => ExprKind::TupleLit({
            let mut s = Vec::new();
            for el in items {
                s.push(extract_expr_awaits(ctx, out, el, cur_uses, first, last)?);
            }
            s
        }),
        ExprKind::ArrayLit(items) => ExprKind::ArrayLit({
            let mut s = Vec::new();
            for el in items {
                s.push(extract_expr_awaits(ctx, out, el, cur_uses, first, last)?);
            }
            s
        }),
        ExprKind::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => ExprKind::Range {
            // P8：边界可为 None（切片省略边界）
            lower: lower
                .as_ref()
                .map(|e| extract_expr_awaits(ctx, out, e, cur_uses, first, last))
                .transpose()?,
            upper: upper
                .as_ref()
                .map(|e| extract_expr_awaits(ctx, out, e, cur_uses, first, last))
                .transpose()?,
            lower_inclusive: *lower_inclusive,
            upper_inclusive: *upper_inclusive,
        },
        ExprKind::StructCtor {
            type_name,
            type_args,
            fields,
            ..
        } => ExprKind::StructCtor {
            type_name: type_name.clone(),
            type_args: type_args.clone(),
            fields: {
                let mut f = Vec::new();
                for (n, v) in fields {
                    f.push((n.clone(), extract_expr_awaits(ctx, out, v, cur_uses, first, last)?));
                }
                f
            },
            base: None,
        },
        ExprKind::MacroCall { name, args } => ExprKind::MacroCall {
            name: name.clone(),
            args: {
                let mut a = Vec::new();
                for el in args {
                    a.push(extract_expr_awaits(ctx, out, el, cur_uses, first, last)?);
                }
                a
            },
        },
        ExprKind::Transfer { expr, region } => ExprKind::Transfer {
            expr: extract_expr_awaits(ctx, out, expr, cur_uses, first, last)?,
            region: region.clone(),
        },
        ExprKind::Send {
            actor,
            method,
            args,
        } => ExprKind::Send {
            actor: extract_expr_awaits(ctx, out, actor, cur_uses, first, last)?,
            method: method.clone(),
            args: {
                let mut a = Vec::new();
                for el in args {
                    a.push(extract_expr_awaits(ctx, out, el, cur_uses, first, last)?);
                }
                a
            },
        },
        ExprKind::Break(ret) => ExprKind::Break(match ret {
            Some(r) => Some(extract_expr_awaits(ctx, out, r, cur_uses, first, last)?),
            None => None,
        }),
        // 不支持嵌套 await 的节点：按原样保留（内部不可能含 await，否则扫描报错）
        other => other.clone(),
    };
    Ok(AstExpr::new(kind, span))
}
