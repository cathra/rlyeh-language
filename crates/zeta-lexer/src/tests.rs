//! 词法分析器单元测试。

// 3.14 等是 P001 规定的验收用例值
#![allow(clippy::approx_constant)]

use crate::{LexError, Lexer, Token};

fn tokens(source: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(source);
    lexer
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|t| t.token)
        .collect()
}

#[test]
fn test_all_keywords() {
    let src = "let mut const static fn return pub priv if else match for while loop break \
               continue true false and or not struct enum trait impl type where Self region in \
               transfer out of unsafe actor async await spawn send recv module import as extern";
    let toks = tokens(src);
    let expected = [
        Token::Let,
        Token::Mut,
        Token::Const,
        Token::Static,
        Token::Fn,
        Token::Return,
        Token::Pub,
        Token::Priv,
        Token::If,
        Token::Else,
        Token::Match,
        Token::For,
        Token::While,
        Token::Loop,
        Token::Break,
        Token::Continue,
        Token::True,
        Token::False,
        Token::And,
        Token::Or,
        Token::Not,
        Token::Struct,
        Token::Enum,
        Token::Trait,
        Token::Impl,
        Token::Type,
        Token::Where,
        Token::SelfKw,
        Token::Region,
        Token::In,
        Token::Transfer,
        Token::Out,
        Token::Of,
        Token::Unsafe,
        Token::Actor,
        Token::Async,
        Token::Await,
        Token::Spawn,
        Token::Send,
        Token::Recv,
        Token::Mod,
        Token::Use,
        Token::As,
        Token::Extern,
    ];
    assert_eq!(toks, expected);
}

#[test]
fn test_all_operators() {
    let src = "+ - * / % == != < <= > >= && || ! & | ^ << >> = += -= *= /= %= ..< ... <.. -> =>";
    let toks = tokens(src);
    let expected = [
        Token::Plus,
        Token::Minus,
        Token::Star,
        Token::Slash,
        Token::Percent,
        Token::Eq,
        Token::Ne,
        Token::Lt,
        Token::Le,
        Token::Gt,
        Token::Ge,
        Token::AndAnd,
        Token::OrOr,
        Token::NotNot,
        Token::BitAnd,
        Token::BitOr,
        Token::BitXor,
        Token::Shl,
        Token::Shr,
        Token::Assign,
        Token::PlusEq,
        Token::MinusEq,
        Token::StarEq,
        Token::SlashEq,
        Token::PercentEq,
        Token::DotDotLt,
        Token::DotDotDot,
        Token::LtDotDot,
        Token::Arrow,
        Token::FatArrow,
    ];
    assert_eq!(toks, expected);
}

#[test]
fn test_deprecated_range_tokens() {
    // 旧范围语法 `..` / `..=` 仍产生 token，由 parser 层报"已废弃"错误
    assert_eq!(tokens(".."), vec![Token::Range]);
    assert_eq!(tokens("..="), vec![Token::Range, Token::Assign]);
}

#[test]
fn test_separators_and_at() {
    assert_eq!(
        tokens("( ) { } [ ] , : ; . @"),
        vec![
            Token::LParen,
            Token::RParen,
            Token::LBrace,
            Token::RBrace,
            Token::LBracket,
            Token::RBracket,
            Token::Comma,
            Token::Colon,
            Token::Semicolon,
            Token::Dot,
            Token::At,
        ]
    );
}

#[test]
fn test_hex_bin_oct_radix() {
    assert_eq!(tokens("0xFF"), vec![Token::IntLiteral(255)]);
    assert_eq!(tokens("0xDEAD_BEEF"), vec![Token::IntLiteral(0xDEAD_BEEF)]);
    assert_eq!(tokens("0b1010"), vec![Token::IntLiteral(10)]);
    assert_eq!(tokens("0o755"), vec![Token::IntLiteral(0o755)]);
}

#[test]
fn test_number_suffixes() {
    assert_eq!(tokens("42i32"), vec![Token::IntLiteral(42)]);
    assert_eq!(tokens("3.14f64"), vec![Token::FloatLiteral(3.14)]);
}

#[test]
fn test_exponent_floats() {
    assert_eq!(tokens("1e10"), vec![Token::FloatLiteral(1e10)]);
    assert_eq!(tokens("2.5e-10"), vec![Token::FloatLiteral(2.5e-10)]);
    assert_eq!(tokens("1.0f32"), vec![Token::FloatLiteral(1.0)]);
}

#[test]
fn test_escapes_and_unicode() {
    assert_eq!(
        tokens(r#""\u{1F600}""#),
        vec![Token::StringLiteral("\u{1F600}".to_string())]
    );
    assert_eq!(
        tokens(r#"'\u{1F600}'"#),
        vec![Token::CharLiteral('\u{1F600}')]
    );
    assert_eq!(
        tokens(r#""\x41""#),
        vec![Token::StringLiteral("A".to_string())]
    );
}

#[test]
fn test_raw_string_no_escape() {
    assert_eq!(
        tokens(r#"r"hello\nworld""#),
        vec![Token::StringLiteral(r"hello\nworld".to_string())]
    );
}

#[test]
fn test_raw_hash_string() {
    // `r#"..."#`：无转义（`\n` 保留字面反斜杠）
    assert_eq!(
        tokens("r#\"hello\\nworld\"#"),
        vec![Token::StringLiteral(r"hello\nworld".to_string())]
    );
    // `r##"..."##`：内容可含单个 `"` 与转义序列
    assert_eq!(
        tokens("r##\"a\"b \\n c\"##"),
        vec![Token::StringLiteral(r#"a"b \n c"#.to_string())]
    );
    // 短内容
    assert_eq!(
        tokens("r#\"hi\"#"),
        vec![Token::StringLiteral("hi".to_string())]
    );
    // 多哈希定界与内容含双引号
    assert_eq!(
        tokens("r###\"x \"\" y\"###"),
        vec![Token::StringLiteral(r#"x "" y"#.to_string())]
    );
    // 与原始标识符 `r#type` 区分（`r#"` 前缀须走字符串分支）
    assert_eq!(
        tokens("r#\"a\"# b"),
        vec![
            Token::StringLiteral("a".to_string()),
            Token::Ident("b".to_string()),
        ]
    );
}

#[test]
fn test_raw_identifier() {
    // `r#` 前缀保留：raw identifier 承载"根命名空间显式引用"语义
    // （typecheck 解析时去前缀并跳过模块内优先）。
    assert_eq!(tokens("r#type"), vec![Token::Ident("r#type".to_string())]);
    assert_eq!(tokens("r#rename"), vec![Token::Ident("r#rename".to_string())]);
    assert_eq!(tokens("r#send"), vec![Token::Ident("r#send".to_string())]);
}

#[test]
fn test_multiline_string() {
    let src = "\"line1\nline2\"";
    assert_eq!(
        tokens(src),
        vec![Token::StringLiteral("line1\nline2".to_string())]
    );
}

#[test]
fn test_lifetime_vs_char_literal() {
    // 生命周期标签
    assert_eq!(
        tokens("region 'r { }"),
        vec![
            Token::Region,
            Token::Lifetime("r".to_string()),
            Token::LBrace,
            Token::RBrace,
        ]
    );
    assert_eq!(tokens("'r1"), vec![Token::Lifetime("r1".to_string())]);
    // 字符字面量
    assert_eq!(tokens("'a'"), vec![Token::CharLiteral('a')]);
    assert_eq!(tokens("'\\n'"), vec![Token::CharLiteral('\n')]);
}

#[test]
fn test_comment_skipping() {
    // 单行注释
    assert_eq!(tokens("// comment\nlet x = 1;"), tokens("let x = 1;"));
    // 块注释
    assert_eq!(tokens("/* c */ +"), vec![Token::Plus]);
    // 嵌套块注释
    assert_eq!(
        tokens("/* outer /* inner */ still comment */ +"),
        vec![Token::Plus]
    );
}

#[test]
fn test_peek_lookahead() {
    let mut lexer = Lexer::new("a b");
    let first = lexer.peek().unwrap().clone();
    assert_eq!(first.token, Token::Ident("a".to_string()));
    // peek 不消费
    let second = lexer.peek().unwrap().clone();
    assert_eq!(second.token, Token::Ident("a".to_string()));
    let toks = lexer.tokenize().unwrap();
    assert_eq!(toks.len(), 2);
    assert_eq!(toks[0].token, Token::Ident("a".to_string()));
    assert_eq!(toks[1].token, Token::Ident("b".to_string()));
}

#[test]
fn test_peek_at_eof() {
    let mut lexer = Lexer::new("a");
    // 连续 peek 返回同一 token（1 token lookahead 不推进）
    let first = lexer.peek().unwrap().clone();
    assert_eq!(first.token, Token::Ident("a".to_string()));
    let second = lexer.peek().unwrap().clone();
    assert_eq!(second.token, Token::Ident("a".to_string()));
    // 消费全部后 peek 应返回 Eof
    let toks = lexer.tokenize().unwrap();
    assert_eq!(toks.len(), 1);
    let eof = lexer.peek().unwrap();
    assert_eq!(eof.token, Token::Eof);
}

#[test]
fn test_invalid_char() {
    // `#` 自阶段 Q1b 起为合法 token（Pound：attribute 前缀），改用 `~` 验证非法字符
    let mut lexer = Lexer::new("let ~ = 1;");
    let err = lexer.tokenize().unwrap_err();
    assert!(matches!(err, LexError::InvalidChar { ch: '~', .. }));
}

#[test]
fn test_pound_token() {
    // Q1b：`#[derive(Serialize, Deserialize)]` 中的 `#` 产生 Pound token
    let mut lexer = Lexer::new("#[derive(Serialize, Deserialize)]");
    let toks = lexer.tokenize().unwrap();
    assert_eq!(toks[0].token, Token::Pound);
    assert_eq!(toks[1].token, Token::LBracket);
    assert_eq!(toks[2].token, Token::Ident("derive".to_string()));
    assert_eq!(toks[3].token, Token::LParen);
    assert_eq!(toks[4].token, Token::Ident("Serialize".to_string()));
    assert_eq!(toks[5].token, Token::Comma);
    assert_eq!(toks[6].token, Token::Ident("Deserialize".to_string()));
    assert_eq!(toks[7].token, Token::RParen);
    assert_eq!(toks[8].token, Token::RBracket);
}

#[test]
fn test_unterminated_block_comment() {
    let mut lexer = Lexer::new("/* never closed");
    let err = lexer.tokenize().unwrap_err();
    assert!(matches!(err, LexError::UnterminatedBlockComment { .. }));
}

#[test]
fn test_span_positions_utf8() {
    let mut lexer = Lexer::new("let x = 中;");
    let toks = lexer.tokenize().unwrap();
    // let 在 0..3
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 3);
    // x 在字节偏移 4
    assert_eq!(toks[1].span.start, 4);
    // 中 占 3 字节，col 应为 9（字符计数）
    assert_eq!(toks[3].span.col, 9);
    assert_eq!(toks[3].span.start, 8);
}

#[test]
fn test_not_in_requires_full_word() {
    // "not interval" 不应合并为 NotIn
    assert_eq!(
        tokens("x not interval"),
        vec![
            Token::Ident("x".to_string()),
            Token::Not,
            Token::Ident("interval".to_string()),
        ]
    );
    // "not in" 应合并
    assert_eq!(
        tokens("x not in (1)"),
        vec![
            Token::Ident("x".to_string()),
            Token::NotIn,
            Token::LParen,
            Token::IntLiteral(1),
            Token::RParen,
        ]
    );
}

#[test]
fn test_time_literals_full() {
    let toks = tokens("9am 6pm 22:00 09:30am 12pm");
    let expect = [
        (9u8, 0u8, false),
        (18u8, 0u8, true),
        (22u8, 0u8, false),
        (9u8, 30u8, false),
        (12u8, 0u8, true),
    ];
    assert_eq!(toks.len(), 5);
    for (t, (h, m, pm)) in toks.iter().zip(expect) {
        match t {
            Token::TimeLiteral {
                hour,
                minute,
                is_pm,
            } => {
                assert_eq!(*hour, h);
                assert_eq!(*minute, m);
                assert_eq!(*is_pm, pm);
            }
            other => panic!("expected TimeLiteral, got {other:?}"),
        }
    }
}

#[test]
fn test_int_too_large() {
    let mut lexer = Lexer::new("99999999999999999999999999999999999999999999");
    let err = lexer.tokenize().unwrap_err();
    assert!(matches!(err, LexError::IntTooLarge { .. }));
}
