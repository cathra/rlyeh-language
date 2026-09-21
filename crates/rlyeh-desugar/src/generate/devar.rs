//! 表达式检查子模块：devar。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

pub(super) fn devar(e: &AstExpr) -> AstExpr {
    match &*e.kind {
        ExprKind::Ident(_) => AstExpr::new(ExprKind::IntLiteral(0), e.span),
        ExprKind::Set(items) => AstExpr::new(
            ExprKind::Set(items.iter().map(devar).collect()),
            e.span,
        ),
        ExprKind::TupleLit(items) => AstExpr::new(
            ExprKind::TupleLit(items.iter().map(devar).collect()),
            e.span,
        ),
        ExprKind::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => AstExpr::new(
            ExprKind::Range {
                // P8：边界可为 None（切片省略边界），devar 仅作用于 Some 侧
                lower: lower.as_ref().map(|e| devar(e)),
                upper: upper.as_ref().map(|e| devar(e)),
                lower_inclusive: *lower_inclusive,
                upper_inclusive: *upper_inclusive,
            },
            e.span,
        ),
        ExprKind::Binary { op, left, right } => AstExpr::new(
            ExprKind::Binary {
                op: *op,
                left: devar(left),
                right: devar(right),
            },
            e.span,
        ),
        ExprKind::Unary { op, operand } => AstExpr::new(
            ExprKind::Unary {
                op: *op,
                operand: devar(operand),
            },
            e.span,
        ),
        ExprKind::ComparisonChain { elements, operators } => AstExpr::new(
            ExprKind::ComparisonChain {
                elements: elements.iter().map(devar).collect(),
                operators: operators.clone(),
            },
            e.span,
        ),
        ExprKind::InSet { value, set, negated } => AstExpr::new(
            ExprKind::InSet {
                value: devar(value),
                set: set.iter().map(devar).collect(),
                negated: *negated,
            },
            e.span,
        ),
        ExprKind::InRange {
            value,
            range,
            negated,
        } => AstExpr::new(
            ExprKind::InRange {
                value: devar(value),
                range: devar(range),
                negated: *negated,
            },
            e.span,
        ),
        ExprKind::InContainer {
            value,
            container,
            negated,
        } => AstExpr::new(
            ExprKind::InContainer {
                value: devar(value),
                container: devar(container),
                negated: *negated,
            },
            e.span,
        ),
        ExprKind::InRegion { expr, region } => AstExpr::new(
            ExprKind::InRegion {
                expr: devar(expr),
                region: region.clone(),
            },
            e.span,
        ),
        ExprKind::Assign { target, op, value } => AstExpr::new(
            ExprKind::Assign {
                target: devar(target),
                op: *op,
                value: devar(value),
            },
            e.span,
        ),
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => AstExpr::new(
            ExprKind::If {
                cond: devar(cond),
                then_block: devar_block(then_block),
                else_block: else_block.as_ref().map(devar_block),
            },
            e.span,
        ),
        ExprKind::Match { expr, arms } => AstExpr::new(
            ExprKind::Match {
                expr: devar(expr),
                arms: arms
                    .iter()
                    .map(|arm| MatchArm {
                        pattern: arm.pattern.clone(),
                        guard: arm.guard.as_ref().map(devar),
                        body: devar(&arm.body),
                        span: arm.span,
                    })
                    .collect(),
            },
            e.span,
        ),
        ExprKind::For {
            pattern,
            iterator,
            body,
        } => AstExpr::new(
            ExprKind::For {
                pattern: pattern.clone(),
                iterator: devar(iterator),
                body: devar_block(body),
            },
            e.span,
        ),
        ExprKind::While { cond, body } => AstExpr::new(
            ExprKind::While {
                cond: devar(cond),
                body: devar_block(body),
            },
            e.span,
        ),
        ExprKind::Loop { body } => AstExpr::new(
            ExprKind::Loop {
                body: devar_block(body),
            },
            e.span,
        ),
        ExprKind::Region {
            name,
            options,
            body,
        } => AstExpr::new(
            ExprKind::Region {
                name: name.clone(),
                options: *options,
                body: devar_block(body),
            },
            e.span,
        ),
        ExprKind::GcRegion { body } => AstExpr::new(
            ExprKind::GcRegion {
                body: devar_block(body),
            },
            e.span,
        ),
        ExprKind::Transfer { expr, region } => AstExpr::new(
            ExprKind::Transfer {
                expr: devar(expr),
                region: region.clone(),
            },
            e.span,
        ),
        ExprKind::Call {
            callee,
            args,
            type_args,
        } => AstExpr::new(
            ExprKind::Call {
                callee: match &*callee.kind {
                    ExprKind::Ident(_) | ExprKind::Path(_) => callee.clone(),
                    _ => devar(callee),
                },
                args: args.iter().map(devar).collect(),
                type_args: type_args.clone(),
            },
            e.span,
        ),
        ExprKind::MacroCall { name, args } => AstExpr::new(
            ExprKind::MacroCall {
                name: name.clone(),
                args: args.iter().map(devar).collect(),
            },
            e.span,
        ),
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            protocol_hint,
        } => AstExpr::new(
            ExprKind::MethodCall {
                receiver: devar(receiver),
                method: method.clone(),
                args: args.iter().map(devar).collect(),
                protocol_hint: protocol_hint.clone(),
            },
            e.span,
        ),
        ExprKind::FieldAccess { expr, field } => AstExpr::new(
            ExprKind::FieldAccess {
                expr: devar(expr),
                field: field.clone(),
            },
            e.span,
        ),
        ExprKind::StructCtor {
            type_name,
            type_args,
            fields,
            ..
        } => AstExpr::new(
            ExprKind::StructCtor {
                type_name: type_name.clone(),
                type_args: type_args.clone(),
                fields: fields.iter().map(|(k, v)| (k.clone(), devar(v))).collect(),
                base: None,
            },
            e.span,
        ),
        ExprKind::Index { expr, index } => AstExpr::new(
            ExprKind::Index {
                expr: devar(expr),
                index: devar(index),
            },
            e.span,
        ),
        ExprKind::ArrayLit(items) => AstExpr::new(
            ExprKind::ArrayLit(items.iter().map(devar).collect()),
            e.span,
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
                body: devar(body),
                capture: *capture,
            },
            e.span,
        ),
        ExprKind::Cast {
            expr,
            target_type,
        } => AstExpr::new(
            ExprKind::Cast {
                expr: devar(expr),
                target_type: target_type.clone(),
            },
            e.span,
        ),
        ExprKind::Block(block) => AstExpr::new(
            ExprKind::Block(devar_block(block)),
            e.span,
        ),
        ExprKind::UnsafeBlock(block) => AstExpr::new(
            ExprKind::UnsafeBlock(devar_block(block)),
            e.span,
        ),
        // EH-6 M1：递归展开 `try` 块体，使块内 await / 变量重命名一并处理。
        ExprKind::TryBlock(block) => AstExpr::new(
            ExprKind::TryBlock(devar_block(block)),
            e.span,
        ),
        ExprKind::TryBreak(inner) => AstExpr::new(
            ExprKind::TryBreak(Box::new(devar(inner))),
            e.span,
        ),
        ExprKind::Return(Some(v)) => AstExpr::new(
            ExprKind::Return(Some(devar(v))),
            e.span,
        ),
        ExprKind::Question(inner) => AstExpr::new(
            ExprKind::Question(Box::new(devar(inner))),
            e.span,
        ),
        ExprKind::Break(Some(v)) => AstExpr::new(
            ExprKind::Break(Some(devar(v))),
            e.span,
        ),
        ExprKind::Send {
            actor,
            method,
            args,
        } => AstExpr::new(
            ExprKind::Send {
                actor: devar(actor),
                method: method.clone(),
                args: args.iter().map(devar).collect(),
            },
            e.span,
        ),
        _ => e.clone(),
    }
}

pub(super) fn devar_block(block: &AstBlock) -> AstBlock {
    AstBlock {
        stmts: block.stmts.iter().map(devar_stmt).collect(),
        final_expr: block.final_expr.as_ref().map(devar),
        span: block.span,
    }
}

pub(super) fn devar_stmt(stmt: &AstStmt) -> AstStmt {
    match stmt {
        AstStmt::Let {
            pattern,
            type_anno,
            init,
            mutable,
        } => AstStmt::Let {
            pattern: pattern.clone(),
            type_anno: type_anno.clone(),
            init: devar(init),
            mutable: *mutable,
        },
        AstStmt::Semi(e) => AstStmt::Semi(devar(e)),
        AstStmt::Expr(e) => AstStmt::Expr(devar(e)),
        other => other.clone(),
    }
}
