//! 区域（region）与 transfer 语法解析。

use crate::error::ParseError;
use crate::parser::Parser;
use zeta_ast::{AstExpr, ExprKind, RegionOptions};
use zeta_lexer::Token;

impl<'src> Parser<'src> {
    /// 解析 gc_region 表达式：`gc_region { body }`（K4 追踪 GC 生命周期作用域，
    /// 离开块触发 GC 周期）
    pub(crate) fn parse_gc_region_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::GcRegion, "'gc_region'")?.span;
        let body = self.parse_block()?;
        let span = self.merge_span(start, body.span);
        Ok(AstExpr::new(ExprKind::GcRegion { body }, span))
    }

    /// 解析 region 表达式：
    /// `region ['r] [with_size(N)] [allow_growth[(growth_factor=f)]] [adaptive] [exact] { body }`
    pub(crate) fn parse_region_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::Region, "'region'")?.span;
        let name = if self
            .current()
            .is_some_and(|t| matches!(t, Token::Lifetime(_)))
        {
            Some(self.expect_lifetime()?)
        } else {
            None
        };
        let options = self.parse_region_options()?;
        let body = self.parse_block()?;
        let span = self.merge_span(start, body.span);
        Ok(AstExpr::new(
            ExprKind::Region {
                name,
                options,
                body,
            },
            span,
        ))
    }

    /// 区域选项：`with_size`、`allow_growth`、`adaptive`、`exact`
    fn parse_region_options(&mut self) -> Result<RegionOptions, ParseError> {
        let mut opts = RegionOptions::default();
        while let Some(Token::Ident(name)) = self.current().cloned() {
            let handled = match name.as_str() {
                "with_size" => {
                    self.bump();
                    self.expect(&Token::LParen, "'('")?;
                    if let Some(Token::IntLiteral(v)) = self.current().cloned() {
                        opts.size = Some(
                            v.try_into()
                                .map_err(|_| self.unexpected("non-negative size"))?,
                        );
                        self.bump();
                    } else {
                        return Err(self.unexpected("size integer"));
                    }
                    self.expect(&Token::RParen, "')'")?;
                    true
                }
                "allow_growth" => {
                    self.bump();
                    opts.allow_growth = true;
                    if self.eat(&Token::LParen) {
                        if self.eat(&Token::Ident("growth_factor".to_string())) {
                            self.expect(&Token::Assign, "'='")?;
                            match self.current().cloned() {
                                Some(Token::FloatLiteral(f)) => {
                                    opts.growth_factor = Some(f);
                                    self.bump();
                                }
                                Some(Token::IntLiteral(i)) => {
                                    opts.growth_factor = Some(i as f64);
                                    self.bump();
                                }
                                _ => return Err(self.unexpected("growth factor number")),
                            }
                        }
                        self.expect(&Token::RParen, "')'")?;
                    }
                    true
                }
                "adaptive" => {
                    self.bump();
                    opts.adaptive = true;
                    true
                }
                "exact" => {
                    self.bump();
                    opts.exact = true;
                    true
                }
                _ => false,
            };
            if !handled {
                break;
            }
        }
        Ok(opts)
    }

    /// 解析 transfer 表达式：`transfer expr out of 'r`
    pub(crate) fn parse_transfer_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::Transfer, "'transfer'")?.span;
        let expr = self.parse_expr()?;
        self.expect(&Token::Out, "'out'")?;
        self.expect(&Token::Of, "'of'")?;
        let region = self.expect_lifetime()?;
        let span = self.merge_span(start, expr.span);
        Ok(AstExpr::new(ExprKind::Transfer { expr, region }, span))
    }
}
