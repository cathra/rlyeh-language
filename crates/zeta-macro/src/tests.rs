//! zeta-macro 单元测试。
//!
//! 约定：matcher 传入**剥掉外层 `(...)` 的内容**、transcriber 传入
//! **剥掉外层定界组的内容**（与 parser 收集规则一致；嵌套定界组
//! `[...]`/`{...}`/`(...)` 作为 MatcherToken::Delim / TransToken::Delim 处理）。

use std::collections::HashMap;

use zeta_lexer::Token::{self, *};

use super::*;

/// 简易 token 化（空格分隔；数字/标识符/标点/字符串；`$name` 拆为 Dollar+Ident）。
fn toks(input: &str) -> Vec<Token> {
    let mut out = Vec::new();
    for w in input.split_whitespace() {
        if let Some(rest) = w.strip_prefix('$') {
            out.push(Dollar);
            if rest == "(" {
                out.push(LParen);
            } else if !rest.is_empty() {
                out.push(Ident(rest.to_string()));
            }
        } else {
            out.push(match w {
                "(" => LParen,
                ")" => RParen,
                "[" => LBracket,
                "]" => RBracket,
                "{" => LBrace,
                "}" => RBrace,
                "," => Comma,
                ";" => Semicolon,
                ":" => Colon,
                "+" => Plus,
                "-" => Minus,
                "*" => Star,
                "=" => Assign,
                "=>" => FatArrow,
                "!" => NotNot,
                "?" => Question,
                "&" => BitAnd,
                ">" => Gt,
                "<" => Lt,
                ">=" => Ge,
                "<=" => Le,
                "==" => Eq,
                "!=" => Ne,
                "||" => OrOr,
                "&&" => AndAnd,
                "/" => Slash,
                "%" => Percent,
                "..<" => DotDotLt,
                "..." => DotDotDot,
                "as" => As,
                "." => Dot,
                "not" => Not,
                "in" => In,
                _ if w.starts_with('"') => StringLiteral(w.trim_matches('"').to_string()),
                _ if w.chars().all(|c| c.is_ascii_digit()) => IntLiteral(w.parse().unwrap()),
                _ => Ident(w.to_string()),
            });
        }
    }
    out
}

/// 构造单规则宏表。
fn single_rule(matcher: &str, trans: &str) -> HashMap<String, Vec<MacroRule>> {
    let mut macros = HashMap::new();
    macros.insert(
        "m".to_string(),
        vec![MacroRule {
            matcher: parse_matcher(&toks(matcher)).unwrap(),
            transcriber: parse_transcriber(&toks(trans)).unwrap(),
        }],
    );
    macros
}

#[test]
fn test_meta_expr_expand() {
    let macros = single_rule("$x : expr", "( $x , $x )");
    let out = expand(&macros, "m", &toks("42")).unwrap();
    assert_eq!(out, vec![LParen, IntLiteral(42), Comma, IntLiteral(42), RParen]);
}

#[test]
fn test_ident_and_ty() {
    // ($name:ident, $t:ty) => { let $name: $t = 0; }
    let macros = single_rule("$name : ident , $t : ty", "let $name : $t = 0 ;");
    // `ident ,` 有空格，避免粘连
    let out = expand(&macros, "m", &toks("count , i64")).unwrap();
    assert_eq!(
        out,
        vec![
            Ident("let".into()),
            Ident("count".into()),
            Colon,
            Ident("i64".into()),
            Assign,
            IntLiteral(0),
            Semicolon,
        ]
    );
}

#[test]
fn test_repeat_expand() {
    let macros = single_rule("$ ( $x : expr ) , *", "[ $ ( $x ) , * ]");
    let out = expand(&macros, "m", &toks("1 , 2 , 3")).unwrap();
    assert_eq!(
        out,
        vec![
            LBracket,
            IntLiteral(1),
            Comma,
            IntLiteral(2),
            Comma,
            IntLiteral(3),
            RBracket,
        ]
    );
    // 空迭代（`*` 允许 0 次）
    let empty = expand(&macros, "m", &toks("")).unwrap();
    assert_eq!(empty, vec![LBracket, RBracket]);
}

#[test]
fn test_repeat_plus_requires_one() {
    let macros = single_rule("$ ( $x : expr ) , +", "[ $ ( $x ) , + ]");
    assert!(expand(&macros, "m", &toks("")).is_err());
    let out = expand(&macros, "m", &toks("7")).unwrap();
    assert_eq!(out, vec![LBracket, IntLiteral(7), RBracket]);
}

#[test]
fn test_optional_repeat() {
    let macros = single_rule("$ ( $x : expr ) ?", "[ $ ( $x ) , * ]");
    let empty = expand(&macros, "m", &toks("")).unwrap();
    assert_eq!(empty, vec![LBracket, RBracket]);
    let one = expand(&macros, "m", &toks("7")).unwrap();
    assert_eq!(one, vec![LBracket, IntLiteral(7), RBracket]);
}

#[test]
fn test_group_match_and_trans() {
    // ([$x:tt]) => { ($x) }
    let macros = single_rule("[ $x : tt ]", "( $x )");
    let out = expand(&macros, "m", &toks("[ abc ]")).unwrap();
    assert_eq!(out, vec![LParen, Ident("abc".into()), RParen]);
}

#[test]
fn test_group_expr_atom() {
    // $x:expr 捕获定界组整体；transcriber `f $x` 保留组结构
    let macros = single_rule("$x : expr", "f $x");
    let out = expand(&macros, "m", &toks("( 1 , 2 )")).unwrap();
    assert_eq!(
        out,
        vec![
            Ident("f".into()),
            LParen,
            IntLiteral(1),
            Comma,
            IntLiteral(2),
            RParen,
        ]
    );
}

#[test]
fn test_multi_rule_fallback() {
    let mut macros: HashMap<String, Vec<MacroRule>> = HashMap::new();
    macros.insert(
        "m".to_string(),
        vec![
            MacroRule {
                matcher: parse_matcher(&toks("0")).unwrap(),
                transcriber: parse_transcriber(&toks("a")).unwrap(),
            },
            MacroRule {
                matcher: parse_matcher(&toks("$x : expr")).unwrap(),
                transcriber: parse_transcriber(&toks("b")).unwrap(),
            },
        ],
    );
    let out = expand(&macros, "m", &toks("0")).unwrap();
    assert_eq!(out, vec![Ident("a".into())]);
    let out = expand(&macros, "m", &toks("9")).unwrap();
    assert_eq!(out, vec![Ident("b".into())]);
}

#[test]
fn test_no_match_error() {
    let macros = single_rule("$x : expr", "$x");
    let err = expand(&macros, "m", &toks(", 1")).unwrap_err();
    assert!(err.0.contains("均不匹配"));
}

#[test]
fn test_unknown_macro_error() {
    let macros = HashMap::new();
    let err = expand(&macros, "nope", &toks("1")).unwrap_err();
    assert!(err.0.contains("未定义"));
}

#[test]
fn test_unbound_meta_error() {
    let macros = single_rule("$x : expr", "$y");
    let err = expand(&macros, "m", &toks("1")).unwrap_err();
    assert!(err.0.contains("未绑定"));
}

#[test]
fn test_expr_atom_limits() {
    // 单 token（标识符）
    let macros = single_rule("$x : expr", "$x");
    let out = expand(&macros, "m", &toks("foo")).unwrap();
    assert_eq!(out, vec![Ident("foo".into())]);
    // 字符串字面量
    let out = expand(&macros, "m", &toks("\"hi\"")).unwrap();
    assert_eq!(out, vec![StringLiteral("hi".into())]);
}

#[test]
fn test_expr_multi_token() {
    // 二元中缀：`3 + 4`
    let macros = single_rule("$x : expr", "$x");
    let out = expand(&macros, "m", &toks("3 + 4")).unwrap();
    assert_eq!(out, vec![IntLiteral(3), Plus, IntLiteral(4)]);

    // 比较链：`a > b`
    let out = expand(&macros, "m", &toks("a > b")).unwrap();
    assert_eq!(out, vec![Ident("a".into()), Gt, Ident("b".into())]);

    // 一元负：`-1`
    let out = expand(&macros, "m", &toks("- 1")).unwrap();
    assert_eq!(out, vec![Minus, IntLiteral(1)]);

    // 调用 + 算术：`f ( x ) + 1`
    let out = expand(&macros, "m", &toks("f ( x ) + 1")).unwrap();
    assert_eq!(
        out,
        vec![
            Ident("f".into()),
            LParen,
            Ident("x".into()),
            RParen,
            Plus,
            IntLiteral(1),
        ]
    );

    // 成员访问：`a . b`
    let out = expand(&macros, "m", &toks("a . b")).unwrap();
    assert_eq!(out, vec![Ident("a".into()), Dot, Ident("b".into())]);

    // `as` 转换：`x as i64`
    let out = expand(&macros, "m", &toks("x as i64")).unwrap();
    assert_eq!(
        out,
        vec![Ident("x".into()), As, Ident("i64".into())]
    );

    // 范围：`0..<10`
    let out = expand(&macros, "m", &toks("0 ..< 10")).unwrap();
    assert_eq!(
        out,
        vec![IntLiteral(0), DotDotLt, IntLiteral(10)]
    );
}

#[test]
fn test_expr_boundary_stops() {
    // 逗号分隔（两个 $x:expr 的边界）
    let macros = single_rule("$a : expr , $b : expr", "[ $a , $b ]");
    let out = expand(&macros, "m", &toks("1 + 2 , 3")).unwrap();
    assert_eq!(
        out,
        vec![
            LBracket,
            IntLiteral(1),
            Plus,
            IntLiteral(2),
            Comma,
            IntLiteral(3),
            RBracket,
        ]
    );
}
