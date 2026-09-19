//! 表达式检查子模块：control。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

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
    let AstType::Ref(inner, false, _) = &f.params[0].type_ else {
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
    let Some(spanned) = type_anno else {
        panic!("expected type anno");
    };
    let AstType::Path(name, args) = &spanned.ty else {
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
    assert!(matches!(&*lower.as_ref().unwrap().kind, ExprKind::IntLiteral(0)));
    assert!(matches!(&*upper.as_ref().unwrap().kind, ExprKind::IntLiteral(10000)));
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
