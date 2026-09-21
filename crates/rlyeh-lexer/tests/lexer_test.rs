//! rlyeh-lexer 集成测试（对应 P001 验收用例）。

// 3.14 等是 P001 规定的验收用例值
#![allow(clippy::approx_constant)]

use rlyeh_lexer::{LexError, Lexer, Token};

#[test]
fn test_basic_keywords() {
    let mut lexer = Lexer::new("let mut fn return if else");
    let tokens = lexer.tokenize().unwrap();
    assert_eq!(tokens[0].token, Token::Let);
    assert_eq!(tokens[1].token, Token::Mut);
    assert_eq!(tokens[2].token, Token::Fn);
    assert_eq!(tokens[3].token, Token::Return);
    assert_eq!(tokens[4].token, Token::If);
    assert_eq!(tokens[5].token, Token::Else);
}

#[test]
fn test_numbers() {
    let mut lexer = Lexer::new("42 3.14 0xFF 0b1010 1_000_000");
    let tokens = lexer.tokenize().unwrap();
    assert_eq!(tokens[0].token, Token::IntLiteral(42));
    assert_eq!(
        tokens[1].token,
        Token::FloatLiteral {
            value: 3.14,
            raw: "3.14".to_string()
        }
    );
    assert_eq!(tokens[2].token, Token::IntLiteral(255));
    assert_eq!(tokens[3].token, Token::IntLiteral(10));
    assert_eq!(tokens[4].token, Token::IntLiteral(1000000));
}

#[test]
fn test_strings() {
    let mut lexer = Lexer::new(r#""hello\nworld" 'a' '\n'"#);
    let tokens = lexer.tokenize().unwrap();
    assert_eq!(
        tokens[0].token,
        Token::StringLiteral("hello\nworld".to_string())
    );
    assert_eq!(tokens[1].token, Token::CharLiteral('a'));
    assert_eq!(tokens[2].token, Token::CharLiteral('\n'));
}

#[test]
fn test_time_literals() {
    let mut lexer = Lexer::new("9am 6pm 22:00 09:30am");
    let tokens = lexer.tokenize().unwrap();

    if let Token::TimeLiteral {
        hour,
        minute,
        is_pm,
    } = tokens[0].token
    {
        assert_eq!(hour, 9);
        assert_eq!(minute, 0);
        assert!(!is_pm);
    } else {
        panic!("expected TimeLiteral");
    }

    if let Token::TimeLiteral {
        hour,
        minute,
        is_pm,
    } = tokens[1].token
    {
        assert_eq!(hour, 18);
        assert_eq!(minute, 0);
        assert!(is_pm);
    } else {
        panic!("expected TimeLiteral");
    }
}

#[test]
fn test_comparison_operators() {
    let mut lexer = Lexer::new("0 < x < 10");
    let tokens = lexer.tokenize().unwrap();
    // 应该识别出：IntLiteral(0), Lt, Ident("x"), Lt, IntLiteral(10)
    assert_eq!(tokens[0].token, Token::IntLiteral(0));
    assert_eq!(tokens[1].token, Token::Lt);
    assert_eq!(tokens[2].token, Token::Ident("x".to_string()));
    assert_eq!(tokens[3].token, Token::Lt);
    assert_eq!(tokens[4].token, Token::IntLiteral(10));
}

#[test]
fn test_not_in_combo() {
    let mut lexer = Lexer::new("x not in (1, 2, 3)");
    let tokens = lexer.tokenize().unwrap();
    assert_eq!(tokens[0].token, Token::Ident("x".to_string()));
    assert_eq!(tokens[1].token, Token::NotIn);
    assert_eq!(tokens[2].token, Token::LParen);
}

#[test]
fn test_region_keyword() {
    let mut lexer = Lexer::new("region 'r { let x = 42 in 'r; }");
    let tokens = lexer.tokenize().unwrap();
    assert_eq!(tokens[0].token, Token::Region);
    // 完整 token 序列
    assert_eq!(tokens[1].token, Token::Lifetime("r".to_string()));
    assert_eq!(tokens[2].token, Token::LBrace);
    assert_eq!(tokens[3].token, Token::Let);
    assert_eq!(tokens[4].token, Token::Ident("x".to_string()));
    assert_eq!(tokens[5].token, Token::Assign);
    assert_eq!(tokens[6].token, Token::IntLiteral(42));
    assert_eq!(tokens[7].token, Token::In);
    assert_eq!(tokens[8].token, Token::Lifetime("r".to_string()));
    assert_eq!(tokens[9].token, Token::Semicolon);
    assert_eq!(tokens[10].token, Token::RBrace);
}

#[test]
fn test_unterminated_string() {
    let mut lexer = Lexer::new(r#""hello world"#);
    let err = lexer.tokenize().unwrap_err();
    assert!(matches!(err, LexError::UnterminatedString { .. }));
}

#[test]
fn test_invalid_escape() {
    let mut lexer = Lexer::new(r#""hello\qworld""#);
    let err = lexer.tokenize().unwrap_err();
    assert!(matches!(err, LexError::InvalidEscape { .. }));
}

#[test]
fn test_position_tracking() {
    let source = "let x = 42;";
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();

    // 'let' 从位置 0 开始
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[0].span.line, 1);
    assert_eq!(tokens[0].span.col, 1);

    // '42' 的位置
    assert_eq!(tokens[3].span.col, 9);
}

#[test]
fn test_nested_block_comments() {
    let source = "/* outer /* inner */ still comment */ let x = 1;";
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    assert_eq!(tokens[0].token, Token::Let);
}

#[test]
fn test_actor_snippet() {
    let src = "actor Counter { value: u32 = 0, pub fn inc() -> u32 { return self.value; } }";
    let mut lexer = Lexer::new(src);
    let tokens = lexer.tokenize().unwrap();
    assert_eq!(tokens[0].token, Token::Actor);
    assert_eq!(tokens[1].token, Token::Ident("Counter".to_string()));
    assert!(tokens.iter().any(|t| t.token == Token::Return));
    assert!(tokens.iter().any(|t| t.token == Token::Arrow));
}

#[test]
fn test_transfer_keyword_snippet() {
    let src = "return transfer data out of 'r;";
    let mut lexer = Lexer::new(src);
    let tokens = lexer.tokenize().unwrap();
    assert_eq!(tokens[0].token, Token::Return);
    assert_eq!(tokens[1].token, Token::Transfer);
    assert_eq!(tokens[2].token, Token::Ident("data".to_string()));
    assert_eq!(tokens[3].token, Token::Out);
    assert_eq!(tokens[4].token, Token::Of);
    assert_eq!(tokens[5].token, Token::Lifetime("r".to_string()));
    assert_eq!(tokens[6].token, Token::Semicolon);
}
