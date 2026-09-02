//! 语句与代码块解析。

use crate::error::ParseError;
use crate::parser::Parser;
use rlyeh_ast::{AstBlock, AstStmt, ExprKind, MatchArm};
use rlyeh_lexer::Token;

impl<'src> Parser<'src> {
    /// 解析代码块 `{ stmt* final_expr? }`
    pub(crate) fn parse_block(&mut self) -> Result<AstBlock, ParseError> {
        let lb = self.expect(&Token::LBrace, "'{'")?;
        let mut stmts = Vec::new();
        let mut final_expr = None;
        loop {
            if self.check(&Token::RBrace) || self.at_eof() {
                break;
            }
            if self.eat(&Token::Semicolon) {
                continue;
            }
            if self.is_item_start() {
                let item = self.parse_item()?;
                stmts.push(AstStmt::Item(item));
                continue;
            }
            if self.check(&Token::Let) {
                stmts.push(self.parse_let_stmt()?);
                continue;
            }
            // 表达式语句
            let expr = self.parse_expr()?;
            if self.eat(&Token::Semicolon) {
                stmts.push(AstStmt::Expr(expr));
            } else if matches!(
                *expr.kind,
                ExprKind::If { .. }
                    | ExprKind::Match { .. }
                    | ExprKind::For { .. }
                    | ExprKind::While { .. }
                    | ExprKind::Loop { .. }
                    | ExprKind::Region { .. }
                    | ExprKind::GcRegion { .. }
                    | ExprKind::Block(_)
                    | ExprKind::UnsafeBlock(_)
            ) {
                // 语句式 if / match / for / while / loop / region / gc_region：无分号时，
                // 若块到此结束则作为块尾表达式，否则按语句处理（允许后续继续跟语句）
                if self.check(&Token::RBrace) {
                    final_expr = Some(expr);
                    break;
                }
                stmts.push(AstStmt::Semi(expr));
            } else {
                // 块尾表达式（其后必须是 `}`）
                final_expr = Some(expr);
                break;
            }
        }
        let rb = self.expect(&Token::RBrace, "'}'")?;
        let span = self.merge_span(lb.span, rb.span);
        Ok(AstBlock {
            stmts,
            final_expr,
            span,
        })
    }

    /// 解析 `let [mut] pattern [: type] = expr;` 语句
    fn parse_let_stmt(&mut self) -> Result<AstStmt, ParseError> {
        self.expect(&Token::Let, "'let'")?;
        let mutable = self.eat(&Token::Mut);
        let pattern = self.parse_pattern()?;
        let type_anno = if self.eat(&Token::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(&Token::Assign, "'='")?;
        let init = self.parse_expr()?;
        self.expect(&Token::Semicolon, "';'")?;
        Ok(AstStmt::Let {
            pattern,
            type_anno,
            init,
            mutable,
        })
    }

    /// 解析语句（顶层或块内均可用）
    pub(crate) fn parse_stmt(&mut self) -> Result<AstStmt, ParseError> {
        if self.is_item_start() {
            let item = self.parse_item()?;
            return Ok(AstStmt::Item(item));
        }
        if self.check(&Token::Let) {
            return self.parse_let_stmt();
        }
        let expr = self.parse_expr()?;
        if self.eat(&Token::Semicolon) {
            Ok(AstStmt::Expr(expr))
        } else {
            Ok(AstStmt::Semi(expr))
        }
    }

    /// 解析 match 臂：`pattern [if guard] => body`
    pub(crate) fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let start = match self.peek() {
            Some(lt) => lt.span,
            None => return Err(ParseError::UnexpectedEof),
        };
        // SH-P0-7 P-M3：match 臂支持或模式 `A | B => ..`
        let pattern = self.parse_or_pattern()?;
        let guard = if self.eat(&Token::If) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(&Token::FatArrow, "'=>'")?;
        let body = self.parse_expr()?;
        let is_block = matches!(*body.kind, ExprKind::Block(_) | ExprKind::UnsafeBlock(_));
        if !is_block && !self.check(&Token::Comma) && !self.check(&Token::RBrace) {
            return Err(self.unexpected("',' or '}' after match arm"));
        }
        self.eat(&Token::Comma);
        let span = self.merge_span(start, body.span);
        Ok(MatchArm {
            pattern,
            guard,
            body,
            span,
        })
    }
}
