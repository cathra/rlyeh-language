//! Actor 声明解析。

use crate::error::ParseError;
use crate::parser::Parser;
use rlyeh_ast::{AstActorDecl, AstActorField};
use rlyeh_lexer::Token;

impl<'src> Parser<'src> {
    /// 解析 actor 声明：
    ///
    /// ```rlyeh
    /// actor Counter {
    ///     value: u32 = 0,
    ///     pub fn increment(amount: u32) -> u32 { ... }
    /// }
    /// ```
    ///
    /// 块内同时包含带默认值的字段与（pub/async）方法。
    pub(crate) fn parse_actor(&mut self) -> Result<AstActorDecl, ParseError> {
        let start = self.expect(&Token::Actor, "'actor'")?.span;
        let name = self.expect_ident()?;
        self.expect(&Token::LBrace, "'{'")?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            // 方法
            if self.check(&Token::Fn)
                || self.check(&Token::Pub)
                || self.check(&Token::Async)
                || self.check(&Token::Unsafe)
            {
                methods.push(self.parse_fn()?);
                continue;
            }
            // 字段：`[pub] name : type [= default]`
            let fstart = self.peek().expect("non-eof").span;
            let is_pub = self.eat(&Token::Pub);
            let fname = self.expect_ident()?;
            self.expect(&Token::Colon, "':'")?;
            let type_ = self.parse_type()?;
            let default = if self.eat(&Token::Assign) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            fields.push(AstActorField {
                name: fname,
                type_,
                default,
                is_pub,
                span: self.span_until_current(fstart),
            });
            if !self.eat(&Token::Comma) && !self.check(&Token::RBrace) {
                return Err(self.unexpected("',' or '}' after actor field"));
            }
        }
        let end = self.expect(&Token::RBrace, "'}'")?.span;
        Ok(AstActorDecl {
            name,
            fields,
            methods,
            span: self.merge_span(start, end),
        })
    }
}
