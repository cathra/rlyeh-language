//! # P2 MutexGuard 作用域守卫自动解锁注入（2026-08）
//!
//! 对程序 AST 后序遍历所有函数体块：块内 `let <var> = <expr>.lock_guard();`
//! （方法名 `lock_guard` 特判，与 std `sync::Mutex::lock_guard` 对应）在
//! 所在块尾（`final_expr` 之前）自动追加 `<var>.unlock();`。
//!
//! MVP 限制：
//! - 仅识别 `let` 绑定为简单标识符的守卫变量（`let _ = ...` / 结构体模式不注入）；
//! - 嵌套块（if/match/while/loop/for/region/闭包/块表达式）内的守卫在各自
//!   块尾注入（与 Rust 词法作用域一致）；`return`/`break` 提前退出不注入
//!   （块尾注入前置，显式解锁仍可用 `g.unlock()` 手动调用）；
//! - 按方法名 `lock_guard` 特判（AST 阶段无类型信息）。

use rlyeh_ast::{AstBlock, AstExpr, AstItem, AstProgram, AstStmt, ExprKind};
use rlyeh_lexer::Span;

/// 对程序执行 guard 注入（原地修改 items）。
pub fn inject_guard_unlocks(program: &mut AstProgram) {
    for item in &mut program.items {
        inject_in_item(item);
    }
}

fn inject_in_item(item: &mut AstItem) {
    match item {
        AstItem::FnDecl(f) => {
            if let Some(body) = &mut f.body {
                inject_in_block(body);
            }
        }
        AstItem::ImplBlock(imp) => {
            for m in &mut imp.methods {
                if let Some(body) = &mut m.body {
                    inject_in_block(body);
                }
            }
        }
        AstItem::ModDecl(m) => {
            for it in &mut m.items {
                inject_in_item(it);
            }
        }
        _ => {}
    }
}

/// 后序处理块：先递归嵌套块，再对直接声明的守卫在块尾注入 unlock。
fn inject_in_block(b: &mut AstBlock) {
    // 1. 递归嵌套块
    for stmt in &mut b.stmts {
        match stmt {
            AstStmt::Let { init, .. } => walk_expr(init),
            AstStmt::Semi(e) | AstStmt::Expr(e) => walk_expr(e),
            _ => {}
        }
    }
    if let Some(fe) = &mut b.final_expr {
        walk_expr(fe);
    }
    // 2. 本块直接声明的守卫变量
    let guards: Vec<String> = b
        .stmts
        .iter()
        .filter_map(|s| match s {
            AstStmt::Let { pattern, init, .. } => match pattern {
                rlyeh_ast::AstPattern::Ident(name) if is_lock_guard_call(init) => {
                    Some(name.clone())
                }
                _ => None,
            },
            _ => None,
        })
        .collect();
    if guards.is_empty() {
        return;
    }
    let span = b.span;
    for g in guards {
        b.stmts.push(unlock_stmt(&g, span));
    }
}

/// 递归遍历表达式中的嵌套块（if/match/while/loop/for/region/闭包/块表达式/子表达式）。
fn walk_expr(e: &mut AstExpr) {
    match &mut *e.kind {
        ExprKind::If {
            then_block,
            else_block,
            ..
        } => {
            inject_in_block(then_block);
            if let Some(eb) = else_block {
                inject_in_block(eb);
            }
        }
        ExprKind::Match { expr, arms } => {
            walk_expr(expr);
            for arm in arms {
                walk_expr(&mut arm.body);
            }
        }
        ExprKind::Block(b) => inject_in_block(b),
        ExprKind::Closure { body, .. } => walk_expr(body),
        ExprKind::While { cond, body } => {
            walk_expr(cond);
            inject_in_block(body);
        }
        ExprKind::Loop { body } => inject_in_block(body),
        ExprKind::For { iterator, body, .. } => {
            walk_expr(iterator);
            inject_in_block(body);
        }
        ExprKind::Region { body, .. } => inject_in_block(body),
        ExprKind::GcRegion { body } => inject_in_block(body),
        ExprKind::Binary { left, right, .. } => {
            walk_expr(left);
            walk_expr(right);
        }
        ExprKind::Call { callee, args, .. } => {
            walk_expr(callee);
            for a in args {
                walk_expr(a);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            walk_expr(receiver);
            for a in args {
                walk_expr(a);
            }
        }
        ExprKind::MacroCall { args, .. } => {
            for a in args {
                walk_expr(a);
            }
        }
        ExprKind::Index { expr, index } => {
            walk_expr(expr);
            walk_expr(index);
        }
        ExprKind::FieldAccess { expr, .. } => walk_expr(expr),
        ExprKind::Unary { operand, .. } => walk_expr(operand),
        ExprKind::Assign { target, value, .. } => {
            walk_expr(target);
            walk_expr(value);
        }
        ExprKind::ComparisonChain { elements, .. } => {
            for el in elements {
                walk_expr(el);
            }
        }
        ExprKind::InSet { value, set, .. } => {
            walk_expr(value);
            for s in set {
                walk_expr(s);
            }
        }
        ExprKind::InRange { value, range, .. } => {
            walk_expr(value);
            walk_expr(range);
        }
        ExprKind::InRegion { expr, .. } => walk_expr(expr),
        ExprKind::Range { lower, upper, .. } => {
            // P8：边界可为 None（切片省略边界），仅遍历 Some 侧
            if let Some(l) = lower {
                walk_expr(l);
            }
            if let Some(u) = upper {
                walk_expr(u);
            }
        }
        ExprKind::Set(elems) => {
            for el in elems {
                walk_expr(el);
            }
        }
        ExprKind::Transfer { expr, .. } => walk_expr(expr),
        ExprKind::Await(inner) => walk_expr(inner),
        ExprKind::Question(inner) => walk_expr(inner),
        ExprKind::Return(Some(e)) => walk_expr(e),
        ExprKind::Break(Some(e)) => walk_expr(e),
        ExprKind::Cast { expr, .. } => walk_expr(expr),
        ExprKind::ArrayLit(elems) => {
            for el in elems {
                walk_expr(el);
            }
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                walk_expr(v);
            }
        }
        ExprKind::Send { actor, args, .. } => {
            walk_expr(actor);
            for a in args {
                walk_expr(a);
            }
        }
        _ => {}
    }
}

/// `let g = x.lock_guard();` 判定（AST 级方法名特判）。
fn is_lock_guard_call(init: &AstExpr) -> bool {
    matches!(
        &*init.kind,
        ExprKind::MethodCall { method, .. } if method == "lock_guard"
    )
}

/// 构造 `var.unlock();` 语句。
fn unlock_stmt(var: &str, span: Span) -> AstStmt {
    let receiver = AstExpr::new(ExprKind::Ident(var.to_string()), span);
    let call = AstExpr::new(
        ExprKind::MethodCall {
            receiver,
            method: "unlock".to_string(),
            args: vec![],
            trait_hint: None,
        },
        span,
    );
    AstStmt::Semi(call)
}
