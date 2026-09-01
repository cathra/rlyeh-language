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
}

#[test]
fn test_not_in() {
    let program = parse_ok("if x not in {1, 2, 3} {}");
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
    let program = parse_ok("if x in {0..<10} {}");
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
fn test_in_identifier_is_container() {
    // `x in y`：右侧为标识符 → 视为运行时容器（InContainer）
    let program = parse_ok("if x in y {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::InContainer {
        value,
        container,
        negated,
        ..
    } = &*cond.kind
    else {
        panic!("expected in-container");
    };
    assert!(!negated);
    assert!(matches!(&*value.kind, ExprKind::Ident(ref n) if n == "x"));
    assert!(matches!(
        &*container.kind,
        ExprKind::Ident(ref n) if n == "y"
    ));
}

#[test]
fn test_set_with_ranges() {
    let program = parse_ok("if ch in {'a'..<'z', 'A'..<'Z'} {}");
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
fn test_tuple_literal() {
    // (1, 20, 30) 多元素括号表达式 = 元组值字面量
    let program = parse_ok("(1, 20, 30)");
    let e = top_expr(&program);
    let ExprKind::TupleLit(elems) = &*e.kind else {
        panic!("expected tuple literal");
    };
    assert_eq!(elems.len(), 3);
    assert!(matches!(&*elems[0].kind, ExprKind::IntLiteral(1)));
    assert!(matches!(&*elems[1].kind, ExprKind::IntLiteral(20)));
    assert!(matches!(&*elems[2].kind, ExprKind::IntLiteral(30)));

    // (0...10) 单元素范围 = 分组（非元组）
    let program = parse_ok("(0...10)");
    let e = top_expr(&program);
    assert!(matches!(&*e.kind, ExprKind::Range { .. }));
}

#[test]
fn test_set_literal_brace() {
    // `in {1, 2, 3}` 集合成员判断使用花括号语法（新集合字面量语法）
    let program = parse_ok("if x in {1, 2, 3} {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::InSet { set, .. } = &*cond.kind else {
        panic!("expected in-set");
    };
    assert_eq!(set.len(), 3);
    assert!(matches!(&*set[0].kind, ExprKind::IntLiteral(1)));
    assert!(matches!(&*set[1].kind, ExprKind::IntLiteral(2)));
    assert!(matches!(&*set[2].kind, ExprKind::IntLiteral(3)));

    // 花括号集合（成员判断，编译期离散展开）
    let program = parse_ok("if x in {1, 2, 3} {}");
    let e = top_expr(&program);
    let ExprKind::If { cond, .. } = &*e.kind else {
        panic!();
    };
    let ExprKind::InSet { set, .. } = &*cond.kind else {
        panic!("expected in-set");
    };
    assert_eq!(set.len(), 3);
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


mod region;
mod control;
mod decl;

use region::*;
use control::*;
use decl::*;
