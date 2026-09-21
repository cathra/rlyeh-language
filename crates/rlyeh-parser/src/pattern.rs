//! 模式解析。

use crate::error::ParseError;
use crate::parser::Parser;
use rlyeh_ast::{AstExpr, AstPattern, ExprKind, LiteralValue};
use rlyeh_lexer::{Span, Token};

impl<'src> Parser<'src> {
    /// 解析模式（字面量 / 通配符 / 标识符 / 元组 / 结构体 / 枚举 / 范围 / ref）
    pub(crate) fn parse_pattern(&mut self) -> Result<AstPattern, ParseError> {
        let atom = self.parse_pattern_atom()?;
        // 范围模式：`a..<b` / `a...b` / `a<..b`
        if self.check(&Token::DotDotLt)
            || self.check(&Token::DotDotDot)
            || self.check(&Token::LtDotDot)
        {
            let (lower_inclusive, upper_inclusive) = match self.current() {
                Some(Token::DotDotLt) => (true, false),
                Some(Token::DotDotDot) => (true, true),
                Some(Token::LtDotDot) => (false, true),
                _ => unreachable!("guarded by range token check"),
            };
            self.bump();
            let lower = self.pattern_as_expr(&atom)?;
            let upper_pat = self.parse_pattern_atom()?;
            let upper = self.pattern_as_expr(&upper_pat)?;
            return Ok(AstPattern::Range {
                lower,
                upper,
                lower_inclusive,
                upper_inclusive,
            });
        }
        // 旧范围模式 `a..b` / `a..=b` 已废弃
        if self.check(&Token::Range) {
            return Err(self.unexpected("new range syntax `..<` or `...`"));
        }
        Ok(atom)
    }

    /// 或模式 `A | B | ..`（SH-P0-7 P-M3）。
    ///
    /// **仅用于 match 臂与 `if let` / `while let` 的模式位置**——不可并入
    /// `parse_pattern` 本身：闭包参数列表以 `|` 作分隔符与结束符
    /// （`|x, y| ..`），若让模式解析吞 `|` 会误食参数列表结束符（与类型注解处
    /// 不收集 `|` 联合同理，见 `expr/control.rs::parse_closure`）。
    /// `||` 是独立 token（`Token::OrOr`），不会被此处的 `BitOr` 误匹配。
    pub(crate) fn parse_or_pattern(&mut self) -> Result<AstPattern, ParseError> {
        let first = self.parse_pattern()?;
        if !self.check(&Token::BitOr) {
            return Ok(first);
        }
        let mut alts = vec![first];
        while self.eat(&Token::BitOr) {
            alts.push(self.parse_pattern()?);
        }
        Ok(AstPattern::Or(alts))
    }

    /// 解析模式原子（不含范围后缀）
    fn parse_pattern_atom(&mut self) -> Result<AstPattern, ParseError> {
        match self.current().cloned() {
            Some(Token::Mut) => {
                // `mut` 绑定修饰符：`mut x` / `mut (a, b)` / `mut Point { x }` ——令该绑定可变
                self.bump();
                let inner = self.parse_pattern()?;
                Ok(AstPattern::Mut(Box::new(inner)))
            }
            Some(Token::IntLiteral(v)) => {
                self.bump();
                Ok(AstPattern::Literal(LiteralValue::Int(v)))
            }
            Some(Token::FloatLiteral { value: f, .. }) => {
                self.bump();
                Ok(AstPattern::Literal(LiteralValue::Float(f)))
            }
            Some(Token::StringLiteral(s)) => {
                self.bump();
                Ok(AstPattern::Literal(LiteralValue::Str(s)))
            }
            Some(Token::CharLiteral(c)) => {
                self.bump();
                Ok(AstPattern::Literal(LiteralValue::Char(c)))
            }
            Some(Token::BoolLiteral(b)) => {
                self.bump();
                Ok(AstPattern::Literal(LiteralValue::Bool(b)))
            }
            Some(Token::True) => {
                self.bump();
                Ok(AstPattern::Literal(LiteralValue::Bool(true)))
            }
            Some(Token::False) => {
                self.bump();
                Ok(AstPattern::Literal(LiteralValue::Bool(false)))
            }
            Some(Token::TimeLiteral {
                hour,
                minute,
                is_pm,
            }) => {
                self.bump();
                Ok(AstPattern::Literal(LiteralValue::Time {
                    hour,
                    minute,
                    is_pm,
                }))
            }
            Some(Token::Ident(name)) => self.parse_ident_pattern(name),
            Some(Token::LParen) => self.parse_tuple_pattern(),
            _ => Err(self.unexpected("pattern")),
        }
    }

    /// 标识符开头的模式
    fn parse_ident_pattern(&mut self, name: String) -> Result<AstPattern, ParseError> {
        self.bump();
        if name == "_" {
            return Ok(AstPattern::Wildcard);
        }
        // ref 模式：`ref [mut] pat`
        if name == "ref"
            && (self.check(&Token::Mut) || matches!(self.current(), Some(Token::Ident(_))))
        {
            let is_mut = self.eat(&Token::Mut);
            let inner = self.parse_pattern()?;
            return Ok(AstPattern::Ref(Box::new(inner), is_mut));
        }
        // 路径枚举：`Path::Variant(...)` / `mod::Enum::Variant(...)`
        // （lexer 将 `::` 拆为两个 `:`；多段路径逐段收集，最后一段为变体名）
        if self.eat_colon_colon() {
            let mut segments = vec![name];
            let mut variant = self.expect_ident()?;
            while self.eat_colon_colon() {
                segments.push(variant);
                variant = self.expect_ident()?;
            }
            segments.push(variant);
            let args = if self.check(&Token::LParen) {
                self.parse_pattern_paren_args()?
            } else if self.check(&Token::LBrace) {
                // 枚举结构式负载模式 `Enum::Variant { x, y }`（SH-P1-2 续）
                let fields = self.parse_pattern_struct_fields()?;
                return Ok(AstPattern::EnumStructPath(segments, fields));
            } else {
                Vec::new()
            };
            return Ok(AstPattern::EnumPath(segments, args));
        }
        // 枚举模式：`Variant(args)`
        if self.check(&Token::LParen) {
            let args = self.parse_pattern_paren_args()?;
            return Ok(AstPattern::Enum(name, args));
        }
        // 结构体模式：`Point { x, y }`
        if self.check(&Token::LBrace) {
            return self.parse_struct_pattern(name);
        }
        Ok(AstPattern::Ident(name))
    }

    /// 元组模式 `(a, b, c)`
    fn parse_tuple_pattern(&mut self) -> Result<AstPattern, ParseError> {
        // SH-P2-6 L2：记录元组模式整体的源码位置，供解构绑定类型不匹配时回指
        let start = self.peek().map(|lt| lt.span).unwrap_or(Span::dummy());
        self.expect(&Token::LParen, "'('")?;
        let mut elems = Vec::new();
        while !self.check(&Token::RParen) {
            if self.at_eof() {
                return Err(self.unexpected("')'"));
            }
            // 剩余模式 `..`（SH-P1-2 收尾）：吸收末位剩余元素；须位于末位
            if self.check(&Token::Range) {
                self.bump();
                elems.push(AstPattern::Rest);
                if !self.eat(&Token::Comma) {
                    break;
                }
                continue;
            }
            elems.push(self.parse_pattern()?);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RParen, "')'")?;
        let end = self.peek().map(|lt| lt.span).unwrap_or(start);
        let span = Span {
            start: start.start,
            end: end.start,
            line: start.line,
            col: start.col,
        };
        Ok(AstPattern::Tuple(elems, span))
    }

    /// 枚举负载模式 `(a, b)`
    fn parse_pattern_paren_args(&mut self) -> Result<Vec<AstPattern>, ParseError> {
        self.expect(&Token::LParen, "'('")?;
        let mut args = Vec::new();
        while !self.check(&Token::RParen) {
            if self.at_eof() {
                return Err(self.unexpected("')'"));
            }
            // 剩余模式 `..`（`Some(x, ..)` 跳过其余负载字段）；须位于末位
            if self.check(&Token::Range) {
                self.bump();
                args.push(AstPattern::Rest);
                if !self.eat(&Token::Comma) {
                    break;
                }
                continue;
            }
            args.push(self.parse_pattern()?);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RParen, "')'")?;
        Ok(args)
    }

    /// 解析结构体 / 枚举结构式负载模式内部的 `{ field: pat, other }` 字段列表
    /// （含两侧花括号），供 `parse_struct_pattern` 与路径枚举结构式模式共用。
    fn parse_pattern_struct_fields(
        &mut self,
    ) -> Result<Vec<(String, AstPattern)>, ParseError> {
        self.expect(&Token::LBrace, "'{'")?;
        let mut fields = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            // 剩余模式 `..`（`Point { x, .. }` 跳过其余字段）；须位于末位
            if self.check(&Token::Range) {
                self.bump();
                fields.push(("..".to_string(), AstPattern::Rest));
                break;
            }
            // 字段级 `mut` 修饰符：`Point { mut x }` / `Point { x: mut px }` 令该字段绑定为可变
            let field_mut = self.eat(&Token::Mut);
            let fname = self.expect_ident()?;
            let pat = if self.eat(&Token::Colon) {
                self.parse_pattern()?
            } else {
                AstPattern::Ident(fname.clone())
            };
            let pat = if field_mut {
                AstPattern::Mut(Box::new(pat))
            } else {
                pat
            };
            fields.push((fname, pat));
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RBrace, "'}'")?;
        Ok(fields)
    }

    /// 结构体模式 `Point { field: pat, other }`
    fn parse_struct_pattern(&mut self, name: String) -> Result<AstPattern, ParseError> {
        let fields = self.parse_pattern_struct_fields()?;
        Ok(AstPattern::Struct(name, fields))
    }

    /// 将模式转换为表达式（仅字面量模式可用于范围上/下界）
    fn pattern_as_expr(&self, pat: &AstPattern) -> Result<AstExpr, ParseError> {
        match pat {
            AstPattern::Literal(lit) => {
                let kind = match lit {
                    LiteralValue::Int(v) => ExprKind::IntLiteral(*v),
                    LiteralValue::Float(f) => ExprKind::FloatLiteral(*f),
                    LiteralValue::Str(s) => ExprKind::StringLiteral(s.clone()),
                    LiteralValue::Char(c) => ExprKind::CharLiteral(*c),
                    LiteralValue::Bool(b) => ExprKind::BoolLiteral(*b),
                    LiteralValue::Time {
                        hour,
                        minute,
                        is_pm,
                    } => ExprKind::TimeLiteral {
                        hour: *hour,
                        minute: *minute,
                        is_pm: *is_pm,
                    },
                };
                Ok(AstExpr::new(
                    kind,
                    Span {
                        start: 0,
                        end: 0,
                        line: 0,
                        col: 0,
                    },
                ))
            }
            _ => Err(ParseError::MissingExpr {
                expected: "literal for range pattern".to_string(),
                line: 0,
                col: 0,
            }),
        }
    }
}
