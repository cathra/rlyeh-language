//! 语法分析器单元测试。

use crate::{ParseError, Parser};
use rlyeh_ast::{AstItem, AstPattern, AstProgram, AstStmt, AstType, BinaryOp, CompareOp, ExprKind};
use rlyeh_lexer::Token;

/// 解析成功并返回程序
fn parse_ok(src: &str) -> AstProgram {
    Parser::new(src)
        .expect("lex should succeed")
        .parse_program()
        .expect("parse should succeed")
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
fn test_empty_program() {
    let program = parse_ok("");
    assert!(program.items.is_empty());
    let program = parse_ok("  \n\t  ");
    assert!(program.items.is_empty());
}

#[test]
fn test_simple_let() {
    let program = parse_ok("let x = 42;");
    assert_eq!(program.items.len(), 1);
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!("expected statement");
    };
    let AstStmt::Let {
        pattern,
        init,
        mutable,
        type_anno,
    } = &**stmt
    else {
        panic!("expected let stmt");
    };
    assert_eq!(pattern, &AstPattern::Ident("x".to_string()));
    assert!(!mutable);
    assert!(type_anno.is_none());
    assert!(matches!(&*init.kind, ExprKind::IntLiteral(42)));
}

#[test]
fn test_mutable_let_with_type() {
    let program = parse_ok("let mut count: u32 = 10;");
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!();
    };
    let AstStmt::Let {
        pattern,
        mutable,
        type_anno,
        ..
    } = &**stmt
    else {
        panic!();
    };
    assert_eq!(pattern, &AstPattern::Ident("count".to_string()));
    assert!(*mutable);
    let Some(AstType::Path(name, args)) = type_anno else {
        panic!("expected path type");
    };
    assert_eq!(name, "u32");
    assert!(args.is_empty());
}

#[test]
fn test_comparison_chain() {
    let program = parse_ok("if 0 < x < 10 {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!("expected if expr");
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
    assert!(matches!(&*elements[0].kind, ExprKind::IntLiteral(0)));
    assert!(matches!(&*elements[1].kind, ExprKind::Ident(ref n) if n == "x"));
    assert!(matches!(&*elements[2].kind, ExprKind::IntLiteral(10)));
}

#[test]
fn test_reverse_comparison_chain_accepted() {
    // 方向不一致的比较链由解析器接受（收集），语义阶段校验
    let program = parse_ok("if 0 > x > 10 {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::ComparisonChain { operators, .. } = &*cond.kind else {
        panic!();
    };
    assert_eq!(operators, &vec![CompareOp::Gt, CompareOp::Gt]);
}

#[test]
fn test_mixed_direction_chain_accepted_by_parser() {
    // 0 < x > 10：解析器允许收集，方向校验留给语义分析
    let program = parse_ok("if 0 < x > 10 {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::ComparisonChain { operators, .. } = &*cond.kind else {
        panic!();
    };
    assert_eq!(operators, &vec![CompareOp::Lt, CompareOp::Gt]);
}

#[test]
fn test_in_set() {
    let program = parse_ok("if x in (1, 3, 5) {}");
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
}

#[test]
fn test_not_in() {
    let program = parse_ok("if x not in (1, 2, 3) {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::InSet { set, negated, .. } = &*cond.kind else {
        panic!();
    };
    assert!(*negated);
    assert_eq!(set.len(), 3);
}

#[test]
fn test_in_bare_range() {
    // `x in 0..<10`：裸范围（不带括号），解析为范围判断 InRange
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
fn test_in_bare_range_forms() {
    // 三种范围运算符均可直接作为 in 右侧（区间判断）
    for (src, lo, hi) in [
        ("0..<10", true, false),
        ("0...10", true, true),
        ("0<..10", false, true),
    ] {
        let program = parse_ok(&format!("if x in {src} {{}}"));
        let e = top_expr(&program);
        let ExprKind::If { cond, .. } = &*e.kind else {
            panic!();
        };
        let ExprKind::InRange { range, .. } = &*cond.kind else {
            panic!("expected in-range");
        };
        assert!(
            matches!(
                &*range.kind,
                ExprKind::Range {
                    lower_inclusive: l,
                    upper_inclusive: h,
                    ..
                } if *l == lo && *h == hi
            ),
            "in 右侧应解析为范围 {src}"
        );
    }
}

#[test]
fn test_not_in_bare_range() {
    let program = parse_ok("if x not in 0..<10 {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::InRange { range, negated, .. } = &*cond.kind else {
        panic!("expected in-range");
    };
    assert!(*negated);
    assert!(matches!(&*range.kind, ExprKind::Range { .. }));
}

#[test]
fn test_in_bare_range_does_not_swallow_and() {
    // `x in 0..<10 && y`：in 右侧（裸范围）不应吞掉 `&&`
    let program = parse_ok("if x in 0..<10 && y {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::Binary {
        op: BinaryOp::And,
        left,
        right,
    } = &*cond.kind
    else {
        panic!("expected && binary");
    };
    assert!(matches!(&*left.kind, ExprKind::InRange { .. }));
    assert!(matches!(&*right.kind, ExprKind::Ident(ref n) if n == "y"));
}

#[test]
fn test_in_set_with_range_stays_in_set() {
    // `x in (0..<10)`：括号内是集合，范围元素保留在集合中（成员判断语义）
    let program = parse_ok("if x in (0..<10) {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::InSet { set, negated, .. } = &*cond.kind else {
        panic!("expected in-set");
    };
    assert!(!negated);
    assert_eq!(set.len(), 1);
    assert!(matches!(
        &*set[0].kind,
        ExprKind::Range {
            lower_inclusive: true,
            upper_inclusive: false,
            ..
        }
    ));
}

#[test]
fn test_in_requires_set_or_range() {
    // `x in y`：右侧既不是集合也不是范围 → 报错
    let src = "if x in y {}";
    let err = Parser::new(src)
        .unwrap()
        .parse_program()
        .expect_err("in 右侧必须是集合或范围");
    assert!(
        matches!(err, ParseError::UnexpectedToken { .. }),
        "意外错误类型: {err:?}"
    );
}

#[test]
fn test_set_with_ranges() {
    let program = parse_ok("if ch in ('a'..<'z', 'A'..<'Z') {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::InSet { set, .. } = &*cond.kind else {
        panic!();
    };
    assert_eq!(set.len(), 2);
    assert!(matches!(
        &*set[0].kind,
        ExprKind::Range {
            lower_inclusive: true,
            upper_inclusive: false,
            ..
        }
    ));
}

#[test]
fn test_range_expression_forms() {
    // `..<` 左闭右开
    let program = parse_ok("0..<10");
    let e = top_expr(&program);
    assert!(matches!(
        &*e.kind,
        ExprKind::Range {
            lower_inclusive: true,
            upper_inclusive: false,
            ..
        }
    ));
    // `...` 闭区间
    let program = parse_ok("0...10");
    let e = top_expr(&program);
    assert!(matches!(
        &*e.kind,
        ExprKind::Range {
            lower_inclusive: true,
            upper_inclusive: true,
            ..
        }
    ));
    // `<..` 左开右闭
    let program = parse_ok("0<..10");
    let e = top_expr(&program);
    assert!(matches!(
        &*e.kind,
        ExprKind::Range {
            lower_inclusive: false,
            upper_inclusive: true,
            ..
        }
    ));
}

#[test]
fn test_set_literal_with_ranges() {
    // (0...10) 单元素范围构成集合
    let program = parse_ok("(0...10)");
    let e = top_expr(&program);
    let ExprKind::Set(elems) = &*e.kind else {
        panic!("expected set");
    };
    assert_eq!(elems.len(), 1);
    assert!(matches!(
        &*elems[0].kind,
        ExprKind::Range {
            lower_inclusive: true,
            upper_inclusive: true,
            ..
        }
    ));

    // (0..<10) 半开范围集合（不含 10）
    let program = parse_ok("(0..<10)");
    let e = top_expr(&program);
    let ExprKind::Set(elems) = &*e.kind else {
        panic!("expected set");
    };
    assert_eq!(elems.len(), 1);
    assert!(matches!(
        &*elems[0].kind,
        ExprKind::Range {
            lower_inclusive: true,
            upper_inclusive: false,
            ..
        }
    ));

    // (1...10, 20, 30) 混合范围与单值的集合
    let program = parse_ok("(1...10, 20, 30)");
    let e = top_expr(&program);
    let ExprKind::Set(elems) = &*e.kind else {
        panic!("expected set");
    };
    assert_eq!(elems.len(), 3);
    assert!(matches!(&*elems[0].kind, ExprKind::Range { .. }));
    assert!(matches!(&*elems[1].kind, ExprKind::IntLiteral(20)));
    assert!(matches!(&*elems[2].kind, ExprKind::IntLiteral(30)));
}

#[test]
fn test_paren_grouping_not_set() {
    // 单元素非范围仍为分组（(x + y)）
    let program = parse_ok("(1 + 2) * 3");
    let e = top_expr(&program);
    assert!(matches!(
        &*e.kind,
        ExprKind::Binary {
            op: BinaryOp::Mul,
            ..
        }
    ));
}

#[test]
fn test_old_range_syntax_rejected() {
    // 旧范围语法 `..` / `..=` 必须报错
    for src in [
        "0..10",
        "if x in (0..10) {}",
        "match x { 1..=5 => 1, _ => 0 }",
    ] {
        let err = Parser::new(src)
            .unwrap()
            .parse_program()
            .expect_err("旧范围语法应被拒绝");
        assert!(
            matches!(err, ParseError::UnexpectedToken { .. }),
            "意外错误类型: {err:?}"
        );
    }
}

#[test]
fn test_time_literal_in_set() {
    let program = parse_ok("if hour in (9am...6pm) {}");
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
        &*lower.kind,
        ExprKind::TimeLiteral {
            hour: 9,
            minute: 0,
            is_pm: false
        }
    ));
    assert!(matches!(
        &*upper.kind,
        ExprKind::TimeLiteral {
            hour: 18,
            minute: 0,
            is_pm: true
        }
    ));
}

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
    // 臂 0：字面量
    assert!(matches!(
        &arms[0].pattern,
        AstPattern::Literal(rlyeh_ast::LiteralValue::Int(0))
    ));
    // 臂 1：范围
    let AstPattern::Range {
        lower_inclusive: true,
        upper_inclusive: true,
        ..
    } = &arms[1].pattern
    else {
        panic!("expected inclusive range pattern");
    };
    // 臂 2：左开右闭范围 (6, 10]
    assert!(matches!(
        &arms[2].pattern,
        AstPattern::Range {
            lower_inclusive: false,
            upper_inclusive: true,
            ..
        }
    ));
    // 臂 3：通配符
    assert!(matches!(&arms[3].pattern, AstPattern::Wildcard));
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
        panic!("expected fn decl");
    };
    assert!(f.is_pub);
    assert!(f.is_async);
    assert_eq!(f.name, "fetch");
    assert_eq!(f.params.len(), 2);
    // url: &str
    let AstType::Ref(inner, false) = &f.params[0].type_ else {
        panic!("expected ref type");
    };
    assert!(matches!(**inner, AstType::Path(ref n, _) if n == "str"));
    // timeout: Duration = Duration::seconds(30)
    assert!(matches!(
        f.params[1].type_,
        AstType::Path(ref n, _) if n == "Duration"
    ));
    let default = f.params[1].default.as_ref().expect("default value");
    assert!(matches!(&*default.kind, ExprKind::Call { .. }));
    // -> Result<Response, Error>
    let Some(AstType::Path(name, args)) = &f.return_type else {
        panic!("expected return type");
    };
    assert_eq!(name, "Result");
    assert_eq!(args.len(), 2);
    // 函数体 region
    let body = f.body.as_ref().expect("fn body");
    assert_eq!(body.stmts.len(), 1);
    let AstStmt::Expr(region) = &body.stmts[0] else {
        panic!("expected region expr");
    };
    assert!(matches!(&*region.kind, ExprKind::Region { .. }));
}

#[test]
fn test_binary_ops() {
    let program = parse_ok("let r = a + b * c - d / e;");
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!();
    };
    let AstStmt::Let { init, .. } = &**stmt else {
        panic!();
    };
    let ExprKind::Binary { op, left, right } = &*init.kind else {
        panic!("expected binary");
    };
    assert_eq!(*op, BinaryOp::Sub);
    let ExprKind::Binary { op: l_op, .. } = &*left.kind else {
        panic!();
    };
    assert_eq!(*l_op, BinaryOp::Add);
    let ExprKind::Binary { op: r_op, .. } = &*right.kind else {
        panic!();
    };
    assert_eq!(*r_op, BinaryOp::Div);
}

#[test]
fn test_assign_chain() {
    let program = parse_ok("a = b = c;");
    let e = top_expr(&program);
    let ExprKind::Assign { value, .. } = &*e.kind else {
        panic!("expected assign");
    };
    // 右结合：a = (b = c)
    assert!(matches!(&*value.kind, ExprKind::Assign { .. }));
}

#[test]
fn test_method_chain_with_await() {
    let program = parse_ok("let resp = client.get(url).await;");
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!();
    };
    let AstStmt::Let { init, .. } = &**stmt else {
        panic!();
    };
    let ExprKind::Await(inner) = &*init.kind else {
        panic!("expected await");
    };
    let ExprKind::MethodCall { method, args, .. } = &*inner.kind else {
        panic!("expected method call");
    };
    assert_eq!(method, "get");
    assert_eq!(args.len(), 1);
}

#[test]
fn test_cast() {
    let program = parse_ok("let n = x as u32;");
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!();
    };
    let AstStmt::Let { init, .. } = &**stmt else {
        panic!();
    };
    let ExprKind::Cast { target_type, .. } = &*init.kind else {
        panic!("expected cast");
    };
    assert!(matches!(target_type, AstType::Path(ref n, _) if n == "u32"));
}

#[test]
fn test_nested_generics_shr_split() {
    let program = parse_ok("let v: Vec<Vec<u32>> = make();");
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!();
    };
    let AstStmt::Let { type_anno, .. } = &**stmt else {
        panic!();
    };
    let Some(AstType::Path(name, args)) = type_anno else {
        panic!("expected path type");
    };
    assert_eq!(name, "Vec");
    assert_eq!(args.len(), 1);
    let AstType::Path(inner, inner_args) = &args[0] else {
        panic!("expected inner Vec");
    };
    assert_eq!(inner, "Vec");
    assert_eq!(inner_args.len(), 1);
}

#[test]
fn test_turbofish_nested_generics_shr_split() {
    // `json::parse::<HashMap<i64, i64>>(s)`：`>>` 拆分为两层 `>`（HashMap 关闭 + turbofish 关闭）
    let program = parse_ok("let m = json::parse::<HashMap<i64, i64>>(s);");
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!();
    };
    let AstStmt::Let { init, .. } = &**stmt else {
        panic!();
    };
    let ExprKind::Call { callee, args, type_args } = &*init.kind else {
        panic!("expected call");
    };
    assert!(matches!(&*callee.kind, ExprKind::Path(ref segs) if segs == &["json", "parse"]));
    assert_eq!(type_args.len(), 1);
    let AstType::Path(name, ty_args) = &type_args[0] else {
        panic!("expected path type");
    };
    assert_eq!(name, "HashMap");
    assert_eq!(ty_args.len(), 2);
    assert!(matches!(&ty_args[0], AstType::Path(n, _) if n == "i64"));
    assert!(matches!(&ty_args[1], AstType::Path(n, _) if n == "i64"));
    assert_eq!(args.len(), 1);
    assert!(matches!(&*args[0].kind, ExprKind::Ident(n) if n == "s"));
}

#[test]
fn test_for_loop() {
    let program = parse_ok("for i in 0..<10000 { sum += i; }");
    let e = top_expr(&program);
    let ExprKind::For {
        pattern, iterator, ..
    } = &*e.kind
    else {
        panic!("expected for");
    };
    assert!(matches!(pattern, AstPattern::Ident(ref n) if n == "i"));
    let ExprKind::Range {
        lower,
        upper,
        lower_inclusive: true,
        upper_inclusive: false,
    } = &*iterator.kind
    else {
        panic!("expected half-open range iterator");
    };
    assert!(matches!(&*lower.kind, ExprKind::IntLiteral(0)));
    assert!(matches!(&*upper.kind, ExprKind::IntLiteral(10000)));
}

#[test]
fn test_error_unexpected_token() {
    let err = Parser::new("let = 42;")
        .unwrap()
        .parse_program()
        .unwrap_err();
    match err {
        ParseError::UnexpectedToken {
            expected,
            found,
            line,
            col,
        } => {
            assert_eq!(expected, "pattern");
            assert_eq!(found, "Assign");
            assert_eq!(line, 1);
            assert_eq!(col, 5);
        }
        other => panic!("wrong error: {other:?}"),
    }
}

#[test]
fn test_error_unexpected_eof() {
    let err = Parser::new("let x").unwrap().parse_program().unwrap_err();
    assert!(matches!(err, ParseError::UnexpectedEof));
}

#[test]
fn test_error_unclosed_block() {
    let err = Parser::new("fn foo() { let x = 1;")
        .unwrap()
        .parse_program()
        .unwrap_err();
    assert!(matches!(err, ParseError::UnexpectedEof));
}

#[test]
fn test_lex_error_propagates() {
    let err = Parser::new("let x = \";").unwrap_err();
    assert!(matches!(err, ParseError::LexError(_)));
}

#[test]
fn test_error_eof_after_const_keyword() {
    // 回归：源码以 const/static 结尾时不得 panic（此前 peek().expect 会崩）
    for src in ["const", "static", "const ", "static x", "const x ="] {
        let _ = Parser::new(src).map(|mut p| p.parse_program());
    }
}

#[test]
fn test_path_expression() {
    let program = parse_ok("let client = HttpClient::new();");
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!();
    };
    let AstStmt::Let { init, .. } = &**stmt else {
        panic!();
    };
    let ExprKind::Call { callee, args, .. } = &*init.kind else {
        panic!("expected call");
    };
    let ExprKind::Path(segments) = &*callee.kind else {
        panic!("expected path callee");
    };
    assert_eq!(segments, &["HttpClient".to_string(), "new".to_string()]);
    assert!(args.is_empty());
}

#[test]
fn test_send_expression() {
    let program = parse_ok("send counter.increment(10)");
    let e = top_expr(&program);
    let ExprKind::Send { method, args, .. } = &*e.kind else {
        panic!("expected send");
    };
    assert_eq!(method, "increment");
    assert_eq!(args.len(), 1);
}

#[test]
fn test_closure() {
    let program = parse_ok("let f = |x, y| x + y;");
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!();
    };
    let AstStmt::Let { init, .. } = &**stmt else {
        panic!();
    };
    let ExprKind::Closure {
        params,
        param_types,
        body,
        capture,
    } = &*init.kind
    else {
        panic!("expected closure");
    };
    assert_eq!(params.len(), 2);
    assert!(param_types.iter().all(|t| t.is_none()));
    assert!(matches!(capture, rlyeh_ast::CaptureMode::Borrow));
    assert!(matches!(&*body.kind, ExprKind::Binary { .. }));
}

#[test]
fn test_closure_param_anno() {
    let program = parse_ok("let f = |x: i64, y: String| x;");
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!();
    };
    let AstStmt::Let { init, .. } = &**stmt else {
        panic!();
    };
    let ExprKind::Closure { params, param_types, .. } = &*init.kind else {
        panic!("expected closure");
    };
    assert_eq!(params.len(), 2);
    assert!(param_types[0].is_some());
    assert!(param_types[1].is_some());
}

#[test]
fn test_move_closure() {
    let program = parse_ok("let f = move || 42;");
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!();
    };
    let AstStmt::Let { init, .. } = &**stmt else {
        panic!();
    };
    let ExprKind::Closure {
        params, capture, ..
    } = &*init.kind
    else {
        panic!();
    };
    assert!(params.is_empty());
    assert!(matches!(capture, rlyeh_ast::CaptureMode::Move));
}

#[test]
fn test_else_if() {
    let program = parse_ok("if a { 1 } else if b { 2 } else { 3 }");
    let e = top_expr(&program);
    let ExprKind::If { else_block, .. } = &*e.kind else {
        panic!();
    };
    let Some(else_block) = else_block else {
        panic!("expected else block");
    };
    let Some(inner) = &else_block.final_expr else {
        panic!("expected else-if inner expr");
    };
    assert!(matches!(&*inner.kind, ExprKind::If { .. }));
}

#[test]
fn test_return_break_continue() {
    let program = parse_ok("fn f() -> u32 { return 42; }");
    let AstItem::FnDecl(f) = &program.items[0] else {
        panic!();
    };
    let body = f.body.as_ref().expect("body");
    let AstStmt::Expr(ret) = &body.stmts[0] else {
        panic!();
    };
    let ExprKind::Return(Some(v)) = &*ret.kind else {
        panic!("expected return with value");
    };
    assert!(matches!(&*v.kind, ExprKind::IntLiteral(42)));

    let program = parse_ok("loop { break; continue; }");
    let e = top_expr(&program);
    let ExprKind::Loop { body } = &*e.kind else {
        panic!();
    };
    let AstStmt::Expr(brk) = &body.stmts[0] else {
        panic!("expected break stmt");
    };
    assert!(matches!(&*brk.kind, ExprKind::Break(None)));
    let AstStmt::Expr(cont) = &body.stmts[1] else {
        panic!("expected continue stmt");
    };
    assert!(matches!(&*cont.kind, ExprKind::Continue));
}

#[test]
fn test_struct_decl() {
    let program = parse_ok("struct Point { x: f64, pub y: f64 }");
    let AstItem::StructDecl(s) = &program.items[0] else {
        panic!();
    };
    assert_eq!(s.name, "Point");
    assert_eq!(s.fields.len(), 2);
    assert!(s.fields[1].is_pub);
}

#[test]
fn test_enum_decl() {
    let program = parse_ok("enum Result2<T> { Ok(T), Err { msg: String } }");
    let AstItem::EnumDecl(e) = &program.items[0] else {
        panic!();
    };
    assert_eq!(e.name, "Result2");
    assert_eq!(e.generics, &["T".to_string()]);
    assert_eq!(e.variants.len(), 2);
    assert_eq!(e.variants[0].name, "Ok");
    assert_eq!(e.variants[0].tuple_fields.len(), 1);
    assert_eq!(e.variants[1].name, "Err");
    assert_eq!(e.variants[1].struct_fields.len(), 1);
}

#[test]
fn test_trait_and_impl() {
    let program = parse_ok(
        "trait Shape { fn area(&self) -> f64; } impl Shape for Point { fn area(&self) -> f64 { 0.0 } }",
    );
    let AstItem::TraitDecl(t) = &program.items[0] else {
        panic!();
    };
    assert_eq!(t.name, "Shape");
    assert!(t.methods[0].body.is_none());
    let AstItem::ImplBlock(i) = &program.items[1] else {
        panic!();
    };
    assert_eq!(i.trait_name.as_deref(), Some("Shape"));
    assert_eq!(i.type_name, "Point");
    assert!(i.methods[0].body.is_some());
}

#[test]
fn test_use_and_mod() {
    let program = parse_ok("import foo::bar as baz; module m { fn inner() {} }");
    let AstItem::UseDecl(u) = &program.items[0] else {
        panic!();
    };
    assert_eq!(u.path, &["foo".to_string(), "bar".to_string()]);
    assert_eq!(u.alias.as_deref(), Some("baz"));
    let AstItem::ModDecl(m) = &program.items[1] else {
        panic!();
    };
    assert_eq!(m.name, "m");
    assert_eq!(m.items.len(), 1);
}

#[test]
fn test_const_decl() {
    let program = parse_ok("const MAX: u32 = 100; static MIN: u32 = 0;");
    let AstItem::ConstDecl(c) = &program.items[0] else {
        panic!();
    };
    assert_eq!(c.name, "MAX");
    assert!(!c.is_static);
    let AstItem::ConstDecl(c) = &program.items[1] else {
        panic!();
    };
    assert!(c.is_static);
}

#[test]
fn test_index_and_field() {
    let program = parse_ok("let a = m[0].x;");
    let AstItem::Statement(stmt) = &program.items[0] else {
        panic!();
    };
    let AstStmt::Let { init, .. } = &**stmt else {
        panic!();
    };
    let ExprKind::FieldAccess { expr, field } = &*init.kind else {
        panic!("expected field access");
    };
    assert_eq!(field, "x");
    assert!(matches!(&*expr.kind, ExprKind::Index { .. }));
}

#[test]
fn test_precedence_compare_vs_add() {
    // 0 < x + 1：比较链元素可以包含 + - 等算术
    let program = parse_ok("if 0 < x + 1 {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::ComparisonChain { elements, .. } = &*cond.kind else {
        panic!();
    };
    assert_eq!(elements.len(), 2);
    assert!(matches!(&*elements[1].kind, ExprKind::Binary { .. }));
}

#[test]
fn test_peek_token_interface() {
    let mut p = Parser::new("let x = 1;").unwrap();
    assert!(p.check(&Token::Let));
    let _ = p.bump();
    assert!(p.check(&Token::Ident("x".to_string())));
    let _ = p.bump();
    assert!(p.eat(&Token::Assign));
    assert!(p.check(&Token::IntLiteral(1)));
}
