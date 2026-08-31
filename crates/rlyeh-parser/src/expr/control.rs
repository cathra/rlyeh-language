//! control：LLVM 发射子模块。
//! （由 mod.rs 的 `impl <'src> Parser<'src>` 拆分而来，保持语义等价）

use super::*;

impl <'src> Parser<'src> {
    pub(super) fn parse_if_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::If, "'if'")?.span;
        let cond = self.parse_expr()?;
        let then_block = self.parse_block()?;
        let else_block = if self.eat(&Token::Else) {
            if self.check(&Token::If) {
                let inner = self.parse_if_expr()?;
                let span = inner.span;
                Some(AstBlock {
                    stmts: Vec::new(),
                    final_expr: Some(inner),
                    span,
                })
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        let end = else_block
            .as_ref()
            .map(|b| b.span)
            .unwrap_or(then_block.span);
        let span = self.merge_span(start, end);
        Ok(AstExpr::new(
            ExprKind::If {
                cond,
                then_block,
                else_block,
            },
            span,
        ))
    }

    pub(super) fn parse_match_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::Match, "'match'")?.span;
        let expr = self.parse_expr()?;
        self.expect(&Token::LBrace, "'{'")?;
        let mut arms = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            arms.push(self.parse_match_arm()?);
        }
        let rb = self.expect(&Token::RBrace, "'}'")?;
        let span = self.merge_span(start, rb.span);
        Ok(AstExpr::new(ExprKind::Match { expr, arms }, span))
    }

    pub(super) fn parse_while_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::While, "'while'")?.span;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        let span = self.merge_span(start, body.span);
        Ok(AstExpr::new(ExprKind::While { cond, body }, span))
    }

    pub(super) fn parse_loop_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::Loop, "'loop'")?.span;
        let body = self.parse_block()?;
        let span = self.merge_span(start, body.span);
        Ok(AstExpr::new(ExprKind::Loop { body }, span))
    }

    pub(super) fn parse_for_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::For, "'for'")?.span;
        let pattern = self.parse_pattern()?;
        self.expect(&Token::In, "'in'")?;
        let iterator = self.parse_expr()?;
        let body = self.parse_block()?;
        let span = self.merge_span(start, body.span);
        Ok(AstExpr::new(
            ExprKind::For {
                pattern,
                iterator,
                body,
            },
            span,
        ))
    }

    pub(super) fn parse_return_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::Return, "'return'")?.span;
        let value = if self.stmt_terminator() {
            None
        } else {
            Some(self.parse_expr()?)
        };
        let span = match &value {
            Some(v) => self.merge_span(start, v.span),
            None => self.span_until_current(start),
        };
        Ok(AstExpr::new(ExprKind::Return(value), span))
    }

    pub(super) fn parse_break_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::Break, "'break'")?.span;
        let value = if self.stmt_terminator() {
            None
        } else {
            Some(self.parse_expr()?)
        };
        let span = match &value {
            Some(v) => self.merge_span(start, v.span),
            None => self.span_until_current(start),
        };
        Ok(AstExpr::new(ExprKind::Break(value), span))
    }

    pub(super) fn parse_send_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::Send, "'send'")?.span;
        // 仅解析 Actor 表达式本身（标识符/路径），方法调用由 send 语法消费
        let actor = self.parse_prefix()?;
        self.expect(&Token::Dot, "'.'")?;
        let method = self.expect_ident()?;
        let (args, end) = self.parse_call_args()?;
        let span = self.merge_span(start, end);
        Ok(AstExpr::new(
            ExprKind::Send {
                actor,
                method,
                args,
            },
            span,
        ))
    }

    pub(super) fn parse_closure(&mut self, capture: CaptureMode) -> Result<AstExpr, ParseError> {
        let start = match self.peek() {
            Some(lt) => lt.span,
            None => return Err(ParseError::UnexpectedEof),
        };
        // 开 `|`：单个 `|` 后有参数列表，`||` 表示无参数
        let mut params = Vec::new();
        // 参数类型注解（与 `params` 平行）
        let mut param_types = Vec::new();
        if self.eat(&Token::BitOr) {
            while !self.check(&Token::BitOr) {
                if self.at_eof() {
                    return Err(self.unexpected("'|'"));
                }
                let pat = self.parse_pattern()?;
                // 闭包参数类型注解 `|x: i64, y: String|`（与 Rust 兼容）。
                // U1：此处**不收集 `|` 联合**——注解后的 `|` 是参数列表结束符，
                // 若按联合解析会误吞该 `|`（`|x: i64| x + 1` 报 `expected '|', found Plus`）。
                // 需联合类型时用括号：`|x: (i64 | String)| ..`（括号内仍走 `parse_type`）。
                let anno = if self.eat(&Token::Colon) {
                    Some(self.parse_primary_type()?)
                } else {
                    None
                };
                params.push(pat);
                param_types.push(anno);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(&Token::BitOr, "'|'")?;
        } else {
            self.expect(&Token::OrOr, "'||'")?;
        }
        let body = self.parse_expr()?;
        let span = self.merge_span(start, body.span);
        Ok(AstExpr::new(
            ExprKind::Closure {
                params,
                param_types,
                body,
                capture,
            },
            span,
        ))
    }

    pub(super) fn stmt_terminator(&self) -> bool {
        self.check(&Token::Semicolon)
            || self.check(&Token::RBrace)
            || self.check(&Token::Comma)
            || self.at_eof()
    }}
