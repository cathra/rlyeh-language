//! control：LLVM 发射子模块。
//! （由 mod.rs 的 `impl <'src> Parser<'src>` 拆分而来，保持语义等价）

use super::*;

impl <'src> Parser<'src> {
    pub(super) fn parse_if_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::If, "'if'")?.span;
        // O1（SH-P0-6）：`if let Pat = expr { .. }` —— 走模式绑定分支
        if self.check(&Token::Let) {
            return self.parse_if_let_expr(start);
        }
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

    /// O1（SH-P0-6，2026-09-02）：`if let Pat = expr { .. } [else { .. }]`。
    ///
    /// **纯语法糖，desugar 为既有 `match`，零新增 IR 节点**：
    /// ```text
    /// if let Pat = e { A } else { B }   ⟶   match e { Pat => { A }, _ => { B } }
    /// ```
    /// 模式绑定与判别（枚举 tag 比较、类型臂收窄、守卫）全部交由既有
    /// `check_match` 处理；缺 `else` 时兜底为空块（`Unit`）。
    /// `else if` / `else if let` 链由 `parse_if_expr` 递归处理（作为 `_` 臂体）。
    fn parse_if_let_expr(&mut self, start: Span) -> Result<AstExpr, ParseError> {
        self.expect(&Token::Let, "'let'")?;
        // SH-P0-7 P-M3：与 match 臂一致，支持或模式 `if let A | B = e { .. }`
        let pattern = self.parse_or_pattern()?;
        self.expect(&Token::Assign, "'='")?;
        let expr = self.parse_expr()?;
        let then_block = self.parse_block()?;
        let then_span = then_block.span;
        let mut arms = vec![MatchArm {
            pattern,
            guard: None,
            body: AstExpr::new(ExprKind::Block(then_block), then_span),
            span: then_span,
        }];
        let end = if self.eat(&Token::Else) {
            let (else_expr, else_span) = if self.check(&Token::If) {
                let inner = self.parse_if_expr()?;
                let s = inner.span;
                (inner, s)
            } else {
                let b = self.parse_block()?;
                let s = b.span;
                (AstExpr::new(ExprKind::Block(b), s), s)
            };
            arms.push(MatchArm {
                pattern: AstPattern::Wildcard,
                guard: None,
                body: else_expr,
                span: else_span,
            });
            else_span
        } else {
            // 无 else：兜底为空块（求值 Unit）
            let empty = AstBlock {
                stmts: Vec::new(),
                final_expr: None,
                span: then_span,
            };
            arms.push(MatchArm {
                pattern: AstPattern::Wildcard,
                guard: None,
                body: AstExpr::new(ExprKind::Block(empty), then_span),
                span: then_span,
            });
            then_span
        };
        let span = self.merge_span(start, end);
        Ok(AstExpr::new(ExprKind::Match { expr, arms }, span))
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
        // O2（SH-P0-6）：`while let Pat = expr { .. }` —— 走模式绑定分支
        if self.check(&Token::Let) {
            self.expect(&Token::Let, "'let'")?;
            // SH-P0-7 P-M3：与 match 臂一致，支持或模式
            let pattern = self.parse_or_pattern()?;
            self.expect(&Token::Assign, "'='")?;
            let expr = self.parse_expr()?;
            let body = self.parse_block()?;
            let body_span = body.span;
            // desugar：`loop { match expr { Pat => { body }, _ => break } }`
            let arms = vec![
                MatchArm {
                    pattern,
                    guard: None,
                    body: AstExpr::new(ExprKind::Block(body), body_span),
                    span: body_span,
                },
                MatchArm {
                    pattern: AstPattern::Wildcard,
                    guard: None,
                    // 模式不再匹配即跳出循环
                    body: AstExpr::new(ExprKind::Break(None), body_span),
                    span: body_span,
                },
            ];
            let inner = AstExpr::new(ExprKind::Match { expr, arms }, body_span);
            let loop_body = AstBlock {
                stmts: Vec::new(),
                final_expr: Some(inner),
                span: body_span,
            };
            let span = self.merge_span(start, body_span);
            return Ok(AstExpr::new(ExprKind::Loop { body: loop_body }, span));
        }
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
