//! 表达式检查子模块：region。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

#[test]
fn test_region_basic() {
    let program = parse_ok("region 'r { let x = 42 in 'r; }");
    let e = top_expr(&program);
    let ExprKind::Region {
        name,
        options,
        body,
    } = &*e.kind
    else {
        panic!("expected region expr");
    };
    assert_eq!(name.as_deref(), Some("r"));
    assert!(options.allow_growth);
    assert_eq!(body.stmts.len(), 1);
    let AstStmt::Let { init, .. } = &body.stmts[0] else {
        panic!("expected let stmt in region");
    };
    let ExprKind::InRegion { expr, region } = &*init.kind else {
        panic!("expected in-region");
    };
    assert_eq!(region, "r");
    assert!(matches!(&*expr.kind, ExprKind::IntLiteral(42)));
}

#[test]
fn test_region_with_options() {
    let program =
        parse_ok("region 'r with_size(1024) allow_growth(growth_factor=2.0) { let x = 42 in 'r; }");
    let e = top_expr(&program);
    let ExprKind::Region { name, options, .. } = &*e.kind else {
        panic!();
    };
    assert_eq!(name.as_deref(), Some("r"));
    assert_eq!(options.size, Some(1024));
    assert!(options.allow_growth);
    assert_eq!(options.growth_factor, Some(2.0));
}

#[test]
fn test_region_adaptive_exact() {
    let program = parse_ok("region 'r adaptive exact { }");
    let e = top_expr(&program);
    let ExprKind::Region { options, .. } = &*e.kind else {
        panic!();
    };
    assert!(options.adaptive);
    assert!(options.exact);
}

#[test]
fn test_transfer_expression() {
    let program = parse_ok("transfer data out of 'r");
    let e = top_expr(&program);
    let ExprKind::Transfer { expr, region } = &*e.kind else {
        panic!("expected transfer");
    };
    assert_eq!(region, "r");
    assert!(matches!(&*expr.kind, ExprKind::Ident(ref n) if n == "data"));
}

#[test]
fn test_nested_regions() {
    let program =
        parse_ok("region 'r { region 'inner { let x = 1 in 'inner; }; let y = 2 in 'r; }");
    let e = top_expr(&program);
    let ExprKind::Region { body, .. } = &*e.kind else {
        panic!();
    };
    assert_eq!(body.stmts.len(), 2);
    let AstStmt::Expr(inner) = &body.stmts[0] else {
        panic!();
    };
    assert!(matches!(&*inner.kind, ExprKind::Region { .. }));
    let AstStmt::Let { init, .. } = &body.stmts[1] else {
        panic!();
    };
    assert!(matches!(&*init.kind, ExprKind::InRegion { .. }));
}

#[test]
fn test_actor_declaration() {
    let program = parse_ok(
        "actor Counter { value: u32 = 0, pub fn increment(amount: u32) -> u32 { self.value += amount; self.value } }",
    );
    let AstItem::ActorDecl(actor) = &program.items[0] else {
        panic!("expected actor");
    };
    assert_eq!(actor.name, "Counter");
    assert_eq!(actor.fields.len(), 1);
    let field = &actor.fields[0];
    assert_eq!(field.name, "value");
    assert!(matches!(field.type_, AstType::Path(ref n, _) if n == "u32"));
    assert!(field.default.is_some());
    assert_eq!(actor.methods.len(), 1);
    let method = &actor.methods[0];
    assert_eq!(method.name, "increment");
    assert!(method.is_pub);
    assert_eq!(method.params.len(), 1);
    // 方法体：self.value += amount; self.value
    let body = method.body.as_ref().expect("method has body");
    assert_eq!(body.stmts.len(), 1);
    let AstStmt::Expr(assign) = &body.stmts[0] else {
        panic!();
    };
    let ExprKind::Assign { target, value, .. } = &*assign.kind else {
        panic!("expected assign");
    };
    assert!(matches!(&*target.kind, ExprKind::FieldAccess { .. }));
    assert!(matches!(&*value.kind, ExprKind::Ident(ref n) if n == "amount"));
    let Some(final_expr) = &body.final_expr else {
        panic!("expected final expr");
    };
    assert!(matches!(&*final_expr.kind, ExprKind::FieldAccess { .. }));
}
