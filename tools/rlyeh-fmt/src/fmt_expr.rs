//! fmt_expr：表达式的单行重建（含紧凑块 / 单行块语句回退）。
//! （由 lib.rs 拆分而来，保持语义等价）

use super::*;
use crate::fmt_pattern::{fmt_pattern, fmt_range_expr, fmt_type};

// ==================== 表达式（单行重建） ====================

/// 判断表达式是否为块类（需要多行展开）。
pub(crate) fn is_block_like(e: &AstExpr) -> bool {
    matches!(
        e.kind.as_ref(),
        ExprKind::Block(_)
            | ExprKind::UnsafeBlock(_)
            | ExprKind::TryBlock(_)
            | ExprKind::If { .. }
            | ExprKind::Match { .. }
            | ExprKind::For { .. }
            | ExprKind::While { .. }
            | ExprKind::Loop { .. }
            | ExprKind::Region { .. }
            | ExprKind::GcRegion { .. }
    )
}

/// 表达式 → 单行字符串（块类压缩为 `{ ... }` 单行形式）。
pub(crate) fn fmt_expr(e: &AstExpr) -> String {
    match e.kind.as_ref() {
        ExprKind::IntLiteral(v) => v.to_string(),
        ExprKind::FloatLiteral(v) => fmt_float(*v),
        ExprKind::StringLiteral(s) => escape_string(s),
        ExprKind::CharLiteral(c) => escape_char(*c),
        ExprKind::BoolLiteral(b) => b.to_string(),
        ExprKind::TimeLiteral {
            hour,
            minute,
            is_pm,
        } => fmt_time(*hour, *minute, *is_pm),
        ExprKind::Unit => "()".to_string(),
        ExprKind::Ident(name) => name.clone(),
        ExprKind::Path(seg) => seg.join("::"),
        ExprKind::Set(elems) => format!(
            "({})",
            elems.iter().map(fmt_expr).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::TupleLit(elems) => format!(
            "({})",
            elems.iter().map(fmt_expr).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => fmt_range_expr(
            lower.as_ref(),
            upper.as_ref(),
            *lower_inclusive,
            *upper_inclusive,
        ),
        ExprKind::Binary { op, left, right } => {
            let p = bin_prec(*op);
            let l = fmt_operand(left, p, false);
            let r = fmt_operand(right, p, true);
            format!("{} {} {}", l, bin_op_str(*op), r)
        }
        ExprKind::Unary { op, operand } => {
            let s = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
                UnaryOp::Deref => "*",
                UnaryOp::AddrOf => "&",
                UnaryOp::AddrOfMut => "&mut ",
            };
            format!("{}{}", s, fmt_operand(operand, PREC_UNARY, false))
        }
        ExprKind::ComparisonChain {
            elements,
            operators,
        } => {
            let mut out = String::new();
            for (i, op) in operators.iter().enumerate() {
                out.push_str(&fmt_operand(&elements[i], PREC_COMPARE, false));
                out.push(' ');
                out.push_str(cmp_op_str(*op));
                out.push(' ');
            }
            out.push_str(&fmt_operand(
                elements.last().unwrap(),
                PREC_COMPARE,
                false,
            ));
            out
        }
        ExprKind::InSet {
            value,
            set,
            negated,
        } => {
            let v = fmt_operand(value, PREC_COMPARE, false);
            let elems = set.iter().map(fmt_expr).collect::<Vec<_>>().join(", ");
            if *negated {
                format!("{} not in ({})", v, elems)
            } else {
                format!("{} in ({})", v, elems)
            }
        }
        ExprKind::InRange {
            value,
            range,
            negated,
        } => {
            let v = fmt_operand(value, PREC_COMPARE, false);
            if *negated {
                format!("{} not in {}", v, fmt_expr(range))
            } else {
                format!("{} in {}", v, fmt_expr(range))
            }
        }
        ExprKind::InContainer {
            value,
            container,
            negated,
        } => {
            let v = fmt_operand(value, PREC_COMPARE, false);
            if *negated {
                format!("{} not in {}", v, fmt_expr(container))
            } else {
                format!("{} in {}", v, fmt_expr(container))
            }
        }
        ExprKind::InRegion { expr, region } => {
            format!("{} in '{}", fmt_expr(expr), region)
        }
        ExprKind::Assign { target, op, value } => {
            let t = fmt_operand(target, PREC_ASSIGN, false);
            let v = fmt_operand(value, PREC_ASSIGN, true);
            format!("{} {} {}", t, assign_op_str(*op), v)
        }
        ExprKind::If { .. }
        | ExprKind::Match { .. }
        | ExprKind::For { .. }
        | ExprKind::While { .. }
        | ExprKind::Loop { .. }
        | ExprKind::Region { .. }
        | ExprKind::GcRegion { .. } => fmt_expr_compact_block(e),
        ExprKind::Transfer { expr, region } => {
            format!("transfer {} out of '{}", fmt_expr(expr), region)
        }
        ExprKind::Call { callee, args, .. } => {
            let c = fmt_operand(callee, PREC_POSTFIX, false);
            let a = args.iter().map(fmt_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", c, a)
        }
        ExprKind::MacroCall { name, args } => {
            let a = args.iter().map(fmt_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", name, a)
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            protocol_hint: _,
        } => {
            let r = fmt_operand(receiver, PREC_POSTFIX, false);
            let a = args.iter().map(fmt_expr).collect::<Vec<_>>().join(", ");
            format!("{}.{}({})", r, method, a)
        }
        ExprKind::FieldAccess { expr, field } => {
            format!(
                "{}.{}",
                fmt_operand(expr, PREC_POSTFIX, false),
                field
            )
        }
        ExprKind::StructCtor {
            type_name,
            type_args,
            fields,
            ..
        } => {
            // 泛型实参 MVP 不美化输出（`Foo<T> { .. }` 原样保留路径，实参暂略）
            let _ = type_args;
            let fields = fields
                .iter()
                .map(|(n, v)| format!("{}: {}", n, fmt_expr(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} {{ {} }}", type_name.join("::"), fields)
        }
        ExprKind::Index { expr, index } => format!(
            "{}[{}]",
            fmt_operand(expr, PREC_POSTFIX, false),
            fmt_expr(index)
        ),
        ExprKind::ArrayLit(elems) => format!(
            "[{}]",
            elems.iter().map(fmt_expr).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Closure {
            params,
            param_types,
            body,
            capture,
        } => {
            let cap = match capture {
                CaptureMode::Move => "move ",
                CaptureMode::Borrow => "",
            };
            // 参数类型注解 `|x: i64, y|`：有注解的参数拼上类型
            let ps = params
                .iter()
                .zip(param_types.iter())
                .map(|(p, t)| match t {
                    Some(ty) => format!("{}: {}", fmt_pattern(p), fmt_type(ty)),
                    None => fmt_pattern(p),
                })
                .collect::<Vec<_>>()
                .join(", ");
            let b = if is_block_like(body) {
                fmt_expr_compact_block(body)
            } else {
                fmt_expr(body)
            };
            format!("{}|{}| {}", cap, ps, b)
        }
        ExprKind::Cast {
            expr,
            target_type,
        } => format!(
            "{} as {}",
            fmt_operand(expr, PREC_CAST, false),
            fmt_type(target_type)
        ),
        ExprKind::Await(inner) => format!(
            "{}.await",
            fmt_operand(inner, PREC_POSTFIX, false)
        ),
        ExprKind::Block(b) => fmt_block_compact(b),
        ExprKind::UnsafeBlock(b) => format!("unsafe {}", fmt_block_compact(b)),
        ExprKind::TryBlock(b) => format!("try {}", fmt_block_compact(b)),
        ExprKind::TryBreak(inner) => format!("break {}", fmt_operand(inner, PREC_ASSIGN, false)),
        ExprKind::Question(inner) => format!("{}?", fmt_operand(inner, PREC_POSTFIX, false)),
        ExprKind::Return(Some(v)) => format!("return {}", fmt_operand(v, PREC_ASSIGN, false)),
        ExprKind::Return(None) => "return".to_string(),
        ExprKind::Break(Some(v)) => format!("break {}", fmt_operand(v, PREC_ASSIGN, false)),
        ExprKind::Break(None) => "break".to_string(),
        ExprKind::Continue => "continue".to_string(),
        ExprKind::Send {
            actor,
            method,
            args,
        } => {
            let a = args.iter().map(fmt_expr).collect::<Vec<_>>().join(", ");
            format!("send {}.{}({})", fmt_expr(actor), method, a)
        }
    }
}

/// 块类表达式在单行位置时的压缩表示（如闭包体）。
pub(crate) fn fmt_expr_compact_block(e: &AstExpr) -> String {
    match e.kind.as_ref() {
        ExprKind::Block(b) => fmt_block_compact(b),
        ExprKind::UnsafeBlock(b) => format!("unsafe {}", fmt_block_compact(b)),
        ExprKind::TryBlock(b) => format!("try {}", fmt_block_compact(b)),
        ExprKind::TryBreak(inner) => format!("break {}", fmt_operand(inner, PREC_ASSIGN, false)),
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            let t = fmt_block_compact(then_block);
            match else_block {
                Some(el) => format!("if {} {} else {}", fmt_expr(cond), t, fmt_block_compact(el)),
                None => format!("if {} {}", fmt_expr(cond), t),
            }
        }
        ExprKind::Match { expr, arms } => {
            let arms = arms
                .iter()
                .map(|a| {
                    let mut s = fmt_pattern(&a.pattern);
                    if let Some(g) = &a.guard {
                        s.push_str(&format!(" if {}", fmt_expr(g)));
                    }
                    format!("{} => {}", s, fmt_expr(&a.body))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("match {} {{ {} }}", fmt_expr(expr), arms)
        }
        ExprKind::For {
            pattern,
            iterator,
            body,
        } => format!(
            "for {} in {} {}",
            fmt_pattern(pattern),
            fmt_expr(iterator),
            fmt_block_compact(body)
        ),
        ExprKind::While { cond, body } => {
            format!("while {} {}", fmt_expr(cond), fmt_block_compact(body))
        }
        ExprKind::Loop { body } => format!("loop {}", fmt_block_compact(body)),
        ExprKind::Region {
            name,
            options,
            body,
        } => {
            let mut head = String::from("region");
            if let Some(n) = name {
                head.push_str(&format!(" '{}", n));
            }
            if options.adaptive {
                head.push_str(" adaptive");
            }
            format!("{} {}", head, fmt_block_compact(body))
        }
        ExprKind::GcRegion { body } => {
            format!("gc_region {}", fmt_block_compact(body))
        }
        _ => fmt_expr(e),
    }
}

pub(crate) fn fmt_block_compact(b: &AstBlock) -> String {
    let mut parts: Vec<String> = b
        .stmts
        .iter()
        .map(fmt_stmt_compact)
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(fe) = &b.final_expr {
        parts.push(fmt_expr(fe));
    }
    if parts.is_empty() {
        "{}".to_string()
    } else {
        format!("{{ {} }}", parts.join("; "))
    }
}

pub(crate) fn fmt_stmt_compact(s: &AstStmt) -> String {
    match s {
        AstStmt::Let {
            pattern,
            type_anno,
            init,
            mutable,
        } => {
            let mut out = String::from("let ");
            if *mutable {
                out.push_str("mut ");
            }
            out.push_str(&fmt_pattern(pattern));
            if let Some(t) = type_anno {
                out.push_str(&format!(": {}", fmt_type(&t.ty)));
            }
            out.push_str(&format!(" = {};", fmt_expr(init)));
            out
        }
        AstStmt::Expr(e) => format!("{};", fmt_expr(e)),
        AstStmt::Semi(e) => fmt_expr(e),
        AstStmt::Item(_) => String::new(),
    }
}

