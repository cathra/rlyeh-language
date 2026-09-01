//! rlyeh-parser 集成测试（对应 P002 验收用例）。
//!
//! 覆盖：let / if / 比较链 / in 集合 / not in /
//! region（含选项与嵌套）/ transfer / actor / match / 时间字面量。

use rlyeh_ast::{
    AstItem, AstPattern, AstProgram, AstStmt, AstType, CompareOp, ExprKind, LiteralValue,
};
use rlyeh_parser::parse;

/// 断言源码可成功解析为程序
fn parse_ok(src: &str) -> AstProgram {
    parse(src).expect("source should parse successfully")
}

/// 提取顶层语句中的表达式
fn top_expr(program: &AstProgram) -> &rlyeh_ast::AstExpr {
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!("expected statement item");
    };
    match &**stmt {
        AstStmt::Expr(e) | AstStmt::Semi(e) => e,
        _ => panic!("expected expression statement"),
    }
}

#[test]
fn test_let_statement() {
    let program = parse_ok("let x = 42;");
    assert_eq!(program.items.len(), 1);
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!("expected statement");
    };
    let AstStmt::Let { pattern, init, .. } = &**stmt else {
        panic!("expected let");
    };
    assert_eq!(pattern, &AstPattern::Ident("x".to_string()));
    assert!(matches!(&*init.kind, ExprKind::IntLiteral(42)));
}

#[test]
fn test_if_expression() {
    let program = parse_ok("if 0 < x < 10 {}");
    let e = top_expr(&program);
    let ExprKind::If {
        cond,
        then_block,
        else_block,
    } = &*e.kind
    else {
        panic!("expected if");
    };
    let ExprKind::ComparisonChain {
        elements,
        operators,
    } = &*cond.kind
    else {
        panic!("expected comparison chain");
    };
    assert_eq!(elements.len(), 3);
    assert_eq!(operators, &vec![CompareOp::Lt, CompareOp::Lt]);
    assert!(then_block.stmts.is_empty() && then_block.final_expr.is_none());
    assert!(else_block.is_none());
}

#[test]
fn test_reverse_comparison_chain() {
    // 0 > x > 10 表示区间外（x < 0 || x > 10），解析为反向比较链
    let program = parse_ok("if 0 > x > 10 {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!("expected if");
    };
    let ExprKind::ComparisonChain { operators, .. } = &*cond.kind else {
        panic!();
    };
    assert_eq!(operators, &vec![CompareOp::Gt, CompareOp::Gt]);
}

#[test]
fn test_in_set_expression() {
    let program = parse_ok("if x in {1, 3, 5} {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::InSet {
        value,
        set,
        negated,
    } = &*cond.kind
    else {
        panic!("expected in-set");
    };
    assert!(!negated);
    assert!(matches!(&*value.kind, ExprKind::Ident(ref n) if n == "x"));
    assert_eq!(set.len(), 3);
    assert!(matches!(&*set[0].kind, ExprKind::IntLiteral(1)));
    assert!(matches!(&*set[1].kind, ExprKind::IntLiteral(3)));
    assert!(matches!(&*set[2].kind, ExprKind::IntLiteral(5)));
}

#[test]
fn test_not_in_expression() {
    let program = parse_ok("if x not in {1, 2, 3} {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::InSet { negated, .. } = &*cond.kind else {
        panic!("expected in-set");
    };
    assert!(negated, "not in 应解析为取反的 InSet");
}

#[test]
fn test_in_bare_range_expression() {
    // `x in 0..<10`：裸范围解析为范围判断 InRange（区间语义）
    let program = parse_ok("if x in 0..<10 {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::InRange {
        value,
        range,
        negated,
    } = &*cond.kind
    else {
        panic!("expected in-range");
    };
    assert!(!negated);
    assert!(matches!(&*value.kind, ExprKind::Ident(ref n) if n == "x"));
    assert!(matches!(
        &*range.kind,
        ExprKind::Range {
            lower_inclusive: true,
            upper_inclusive: false,
            ..
        }
    ));
}

#[test]
fn test_in_set_range_vs_bare_range() {
    // 括号集合 `(0..<10)` 是 InSet（成员判断）
    let program = parse_ok("if x in {0..<10} {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    assert!(
        matches!(&*cond.kind, ExprKind::InSet { .. }),
        "括号集合应为 InSet"
    );

    // 裸范围 `0..<10` 是 InRange（区间判断）
    let program = parse_ok("if x in 0..<10 {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    assert!(
        matches!(&*cond.kind, ExprKind::InRange { .. }),
        "裸范围应为 InRange"
    );
}

#[test]
fn test_region_basic() {
    // 验证 InRegion 表达式（区域归属）
    let program = parse_ok("region 'r { let x = 42 in 'r; }");
    let e = top_expr(&program);
    let ExprKind::Region { name, body, .. } = &*e.kind else {
        panic!("expected region");
    };
    assert_eq!(name.as_deref(), Some("r"));
    let AstStmt::Let { init, .. } = &body.stmts[0] else {
        panic!("expected let");
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
    let ExprKind::Region { options, .. } = &*e.kind else {
        panic!();
    };
    assert_eq!(options.size, Some(1024));
    assert!(options.allow_growth);
    assert_eq!(options.growth_factor, Some(2.0));
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
fn test_actor_declaration() {
    let program = parse_ok(
        "actor Counter { value: u32 = 0, pub fn increment(amount: u32) -> u32 { self.value += amount; self.value } }",
    );
    let AstItem::ActorDecl(actor) = &program.items[0] else {
        panic!("expected actor");
    };
    assert_eq!(actor.name, "Counter");
    assert_eq!(actor.fields.len(), 1);
    assert_eq!(actor.fields[0].name, "value");
    assert!(actor.fields[0].default.is_some());
    assert_eq!(actor.methods.len(), 1);
    assert_eq!(actor.methods[0].name, "increment");
    assert!(actor.methods[0].is_pub);
    assert!(actor.methods[0].body.is_some());
}

#[test]
fn test_match_expression() {
    let program = parse_ok(
        "match x { 0 => \"zero\", 1...5 => \"small\", 6<..10 => \"medium\", _ => \"large\" }",
    );
    let e = top_expr(&program);
    let ExprKind::Match { expr, arms } = &*e.kind else {
        panic!("expected match");
    };
    assert!(matches!(&*expr.kind, ExprKind::Ident(ref n) if n == "x"));
    assert_eq!(arms.len(), 4);
    assert!(matches!(
        &arms[0].pattern,
        AstPattern::Literal(LiteralValue::Int(0))
    ));
    let AstPattern::Range {
        lower_inclusive: true,
        upper_inclusive: true,
        ..
    } = &arms[1].pattern
    else {
        panic!("expected inclusive range pattern");
    };
    assert!(matches!(
        &arms[2].pattern,
        AstPattern::Range {
            lower_inclusive: false,
            upper_inclusive: true,
            ..
        }
    ));
    assert!(matches!(&arms[3].pattern, AstPattern::Wildcard));
}

#[test]
fn test_time_literal_in_set() {
    let program = parse_ok("if hour in {9am...6pm} {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::InSet { set, .. } = &*cond.kind else {
        panic!("expected in-set");
    };
    let ExprKind::Range {
        lower,
        upper,
        lower_inclusive: true,
        upper_inclusive: true,
        ..
    } = &*set[0].kind
    else {
        panic!("expected inclusive range");
    };
    assert!(matches!(
        &*lower.as_ref().unwrap().kind,
        ExprKind::TimeLiteral {
            hour: 9,
            minute: 0,
            is_pm: false
        }
    ));
    assert!(matches!(
        &*upper.as_ref().unwrap().kind,
        ExprKind::TimeLiteral {
            hour: 18,
            minute: 0,
            is_pm: true
        }
    ));
}

#[test]
fn test_nested_regions() {
    let program = parse_ok("region 'r { region 'inner { let x = 1 in 'inner; }; }");
    let e = top_expr(&program);
    let ExprKind::Region { body, .. } = &*e.kind else {
        panic!();
    };
    let AstStmt::Expr(inner) = &body.stmts[0] else {
        panic!();
    };
    let ExprKind::Region { name, .. } = &*inner.kind else {
        panic!("expected nested region");
    };
    assert_eq!(name.as_deref(), Some("inner"));
}

#[test]
fn test_complete_function() {
    let src = r#"
pub async fn fetch(url: &str, timeout: Duration = Duration::seconds(30)) -> Result<Response, Error> {
    region 'r {
        let client = HttpClient::new() in 'r;
        let resp = client.get(url).await in 'r;
        let _ = resp;
    };
}
"#;
    let program = parse_ok(src);
    let AstItem::FnDecl(f) = &program.items[0] else {
        panic!("expected fn");
    };
    assert!(f.is_pub);
    assert!(f.is_async);
    assert_eq!(f.name, "fetch");
    assert_eq!(f.params.len(), 2);
    let AstType::Ref(inner, false) = &f.params[0].type_ else {
        panic!("expected &str");
    };
    assert!(matches!(**inner, AstType::Path(ref n, _) if n == "str"));
    assert!(f.params[1].default.is_some());
    let Some(AstType::Path(name, args)) = &f.return_type else {
        panic!("expected return type");
    };
    assert_eq!(name, "Result");
    assert_eq!(args.len(), 2);
    // 函数体含 region，区域内 3 条 let 语句
    let body = f.body.as_ref().expect("fn body");
    let AstStmt::Expr(region) = &body.stmts[0] else {
        panic!("expected region");
    };
    let ExprKind::Region { body: rbody, .. } = &*region.kind else {
        panic!();
    };
    assert_eq!(rbody.stmts.len(), 3);
    let AstStmt::Let { init, .. } = &rbody.stmts[1] else {
        panic!();
    };
    let ExprKind::InRegion { expr, .. } = &*init.kind else {
        panic!("expected in-region");
    };
    let ExprKind::Await(call) = &*expr.kind else {
        panic!("expected await");
    };
    assert!(matches!(&*call.kind, ExprKind::MethodCall { .. }));
}

#[test]
fn test_invalid_comparison_chain_parses() {
    // 0 < x > 10：解析器接受（收集链），方向冲突由语义分析阶段报错
    let program = parse_ok("if 0 < x > 10 {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    assert!(matches!(&*cond.kind, ExprKind::ComparisonChain { .. }));
}
