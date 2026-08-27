//! primary：LLVM 发射子模块。
//! （由 mod.rs 的 `impl <'src> Parser<'src>` 拆分而来，保持语义等价）

use super::*;

impl <'src> Parser<'src> {
    pub(super) fn parse_prefix(&mut self) -> Result<AstExpr, ParseError> {
        let start = match self.peek() {
            Some(lt) => lt.span,
            None => return Err(ParseError::UnexpectedEof),
        };
        match self.current().cloned() {
            Some(Token::IntLiteral(v)) => {
                self.bump();
                Ok(AstExpr::new(
                    ExprKind::IntLiteral(v),
                    self.span_until_current(start),
                ))
            }
            Some(Token::FloatLiteral(f)) => {
                self.bump();
                Ok(AstExpr::new(
                    ExprKind::FloatLiteral(f),
                    self.span_until_current(start),
                ))
            }
            Some(Token::StringLiteral(s)) => {
                self.bump();
                Ok(AstExpr::new(
                    ExprKind::StringLiteral(s),
                    self.span_until_current(start),
                ))
            }
            Some(Token::CharLiteral(c)) => {
                self.bump();
                Ok(AstExpr::new(
                    ExprKind::CharLiteral(c),
                    self.span_until_current(start),
                ))
            }
            Some(Token::BoolLiteral(b)) => {
                self.bump();
                Ok(AstExpr::new(
                    ExprKind::BoolLiteral(b),
                    self.span_until_current(start),
                ))
            }
            Some(Token::True) => {
                self.bump();
                Ok(AstExpr::new(
                    ExprKind::BoolLiteral(true),
                    self.span_until_current(start),
                ))
            }
            Some(Token::False) => {
                self.bump();
                Ok(AstExpr::new(
                    ExprKind::BoolLiteral(false),
                    self.span_until_current(start),
                ))
            }
            Some(Token::TimeLiteral {
                hour,
                minute,
                is_pm,
            }) => {
                self.bump();
                Ok(AstExpr::new(
                    ExprKind::TimeLiteral {
                        hour,
                        minute,
                        is_pm,
                    },
                    self.span_until_current(start),
                ))
            }
            Some(Token::LBracket) => {
                self.bump();
                self.parse_array_lit(start)
            }
            Some(Token::Ident(name)) => self.parse_ident_prefix(name, start),
            Some(Token::SelfKw) => {
                self.bump();
                Ok(AstExpr::new(
                    ExprKind::Ident("self".to_string()),
                    self.span_until_current(start),
                ))
            }
            Some(Token::LParen) => self.parse_paren_or_set(),
            Some(Token::LBrace) => {
                let span = self.peek().expect("non-eof").span;
                let block = self.parse_block()?;
                let span = self.merge_span(span, block.span);
                Ok(AstExpr::new(ExprKind::Block(block), span))
            }
            Some(Token::Minus) => {
                self.bump();
                // 一元操作数排除 `as` 及更低优先级中缀，使 `-x as T` 解析为 `(-x) as T`
                let operand = self.parse_expr_prec(prec::CAST + 1)?;
                let span = self.merge_span(start, operand.span);
                Ok(AstExpr::new(
                    ExprKind::Unary {
                        op: UnaryOp::Neg,
                        operand,
                    },
                    span,
                ))
            }
            Some(Token::NotNot) | Some(Token::Not) => {
                self.bump();
                // 一元操作数排除 `as` 及更低优先级中缀，使 `-x as T` 解析为 `(-x) as T`
                let operand = self.parse_expr_prec(prec::CAST + 1)?;
                let span = self.merge_span(start, operand.span);
                Ok(AstExpr::new(
                    ExprKind::Unary {
                        op: UnaryOp::Not,
                        operand,
                    },
                    span,
                ))
            }
            Some(Token::Star) => {
                self.bump();
                // 一元操作数排除 `as` 及更低优先级中缀，使 `-x as T` 解析为 `(-x) as T`
                let operand = self.parse_expr_prec(prec::CAST + 1)?;
                let span = self.merge_span(start, operand.span);
                Ok(AstExpr::new(
                    ExprKind::Unary {
                        op: UnaryOp::Deref,
                        operand,
                    },
                    span,
                ))
            }
            Some(Token::BitAnd) => {
                self.bump();
                let is_mut = self.eat(&Token::Mut);
                // 一元操作数排除 `as` 及更低优先级中缀，使 `-x as T` 解析为 `(-x) as T`
                let operand = self.parse_expr_prec(prec::CAST + 1)?;
                let span = self.merge_span(start, operand.span);
                let op = if is_mut {
                    UnaryOp::AddrOfMut
                } else {
                    UnaryOp::AddrOf
                };
                Ok(AstExpr::new(ExprKind::Unary { op, operand }, span))
            }
            Some(Token::If) => self.parse_if_expr(),
            Some(Token::Match) => self.parse_match_expr(),
            Some(Token::While) => self.parse_while_expr(),
            Some(Token::Loop) => self.parse_loop_expr(),
            Some(Token::For) => self.parse_for_expr(),
            Some(Token::Region) => self.parse_region_expr(),
            Some(Token::GcRegion) => self.parse_gc_region_expr(),
            Some(Token::Transfer) => self.parse_transfer_expr(),
            Some(Token::Return) => self.parse_return_expr(),
            Some(Token::Break) => self.parse_break_expr(),
            Some(Token::Continue) => {
                self.bump();
                Ok(AstExpr::new(
                    ExprKind::Continue,
                    self.span_until_current(start),
                ))
            }
            Some(Token::Send) => self.parse_send_expr(),
            Some(Token::BitOr) => self.parse_closure(CaptureMode::Borrow),
            _ => Err(self.unexpected("expression")),
        }
    }

    pub(super) fn parse_array_lit(&mut self, start: Span) -> Result<AstExpr, ParseError> {
        let mut elems = Vec::new();
        while !self.check(&Token::RBracket) {
            if self.at_eof() {
                return Err(self.unexpected("']'"));
            }
            elems.push(self.parse_expr()?);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        let rbracket = self.expect(&Token::RBracket, "']'")?;
        let span = self.merge_span(start, rbracket.span);
        Ok(AstExpr::new(ExprKind::ArrayLit(elems), span))
    }

    pub(super) fn parse_ident_prefix(&mut self, name: String, start: Span) -> Result<AstExpr, ParseError> {
        self.bump();
        // `move |x| ...` / `move || ...` 闭包
        if name == "move" && (self.check(&Token::BitOr) || self.check(&Token::OrOr)) {
            return self.parse_closure(CaptureMode::Move);
        }
        // 路径 `a::b::c`（lexer 将 `::` 拆为两个 `:`，此处合并）
        let mut segments = vec![name];
        loop {
            // turbofish 检测：`a::b::<T>(...)` 的 `::<` 停止路径解析，由 Call 后缀处理
            if self.check(&Token::Colon)
                && self.peek_n(1).is_some_and(|t| t.token == Token::Colon)
                && self.peek_n(2).is_some_and(|t| t.token == Token::Lt)
            {
                break;
            }
            if !self.eat_colon_colon() {
                break;
            }
            let seg = self.expect_ident()?;
            segments.push(seg);
        }
        let span = self.span_until_current(start);
        // 宏调用：`name!`（内置格式化宏 / 用户 macro_rules!）。
        // `!` 为 not 一元运算符（前缀形式），此处是标识符后的 postfix 位置，
        // 二者不冲突：`!x` 走一元分支，`foo!(...)` 走本分支。
        if segments.len() == 1 && self.check(&Token::NotNot) {
            let name = segments.pop().expect("non-empty segments");
            return self.parse_macro_call(name, start);
        }
        // 结构体字面量构造：`Point { x: 3, y: 4 }`
        // （需 lookahead 确认，避免与 `match s { ... }` 的 scrutinee 块歧义）
        if self.looks_like_struct_ctor() {
            self.bump();
            let mut fields = Vec::new();
            while !self.check(&Token::RBrace) {
                if self.at_eof() {
                    return Err(self.unexpected("'}'"));
                }
                let fname = self.expect_ident()?;
                self.expect(&Token::Colon, "':'")?;
                let value = self.parse_expr()?;
                fields.push((fname, value));
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            let rbrace = self.expect(&Token::RBrace, "'}'")?;
            let span = self.merge_span(span, rbrace.span);
            return Ok(AstExpr::new(
                ExprKind::StructCtor {
                    type_name: segments,
                    type_args: Vec::new(),
                    fields,
                },
                span,
            ));
        }
        // U8：泛型结构体构造：`Pair<i64> { x: 3, y: 4 }`
        // （lookahead：`<类型...>{` 模式，避免与 `<` 比较歧义）
        if self.looks_like_generic_struct_ctor() {
            self.bump(); // `<`
            let mut type_args = Vec::new();
            loop {
                let t = self.parse_type()?;
                type_args.push(t);
                if self.eat(&Token::Comma) {
                    continue;
                }
                break;
            }
            self.expect(&Token::Gt, "'>'")?;
            self.bump(); // `{`
            let mut fields = Vec::new();
            while !self.check(&Token::RBrace) {
                if self.at_eof() {
                    return Err(self.unexpected("'}'"));
                }
                let fname = self.expect_ident()?;
                self.expect(&Token::Colon, "':'")?;
                let value = self.parse_expr()?;
                fields.push((fname, value));
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            let rbrace = self.expect(&Token::RBrace, "'}'")?;
            let span = self.merge_span(span, rbrace.span);
            return Ok(AstExpr::new(
                ExprKind::StructCtor {
                    type_name: segments,
                    type_args,
                    fields,
                },
                span,
            ));
        }
        if segments.len() == 1 {
            let name = segments.pop().expect("non-empty segments");
            Ok(AstExpr::new(ExprKind::Ident(name), span))
        } else {
            Ok(AstExpr::new(ExprKind::Path(segments), span))
        }
    }

    pub(super) fn parse_paren_or_set(&mut self) -> Result<AstExpr, ParseError> {
        let lp = self.expect(&Token::LParen, "'('")?;
        if self.eat(&Token::RParen) {
            return Ok(AstExpr::new(
                ExprKind::Set(Vec::new()),
                self.merge_span(lp.span, lp.span),
            ));
        }
        let first = self.parse_expr()?;
        if self.eat(&Token::Comma) {
            let mut elems = vec![first];
            while !self.check(&Token::RParen) {
                if self.at_eof() {
                    return Err(self.unexpected("')'"));
                }
                elems.push(self.parse_expr()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            let rp = self.expect(&Token::RParen, "')'")?;
            let span = self.merge_span(lp.span, rp.span);
            Ok(AstExpr::new(ExprKind::Set(elems), span))
        } else {
            let rp = self.expect(&Token::RParen, "')'")?;
            let span = self.merge_span(lp.span, rp.span);
            // 单元素范围（`(0...10)`）构成集合，其余为分组
            if matches!(first.kind.as_ref(), ExprKind::Range { .. }) {
                Ok(AstExpr::new(ExprKind::Set(vec![first]), span))
            } else {
                Ok(AstExpr::new(first.kind.as_ref().clone(), span))
            }
        }
    }

    pub(super) fn parse_call_args(&mut self) -> Result<(Vec<AstExpr>, Span), ParseError> {
        self.expect(&Token::LParen, "'('")?;
        let mut args = Vec::new();
        while !self.check(&Token::RParen) {
            if self.at_eof() {
                return Err(self.unexpected("')'"));
            }
            args.push(self.parse_expr()?);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        let rp = self.expect(&Token::RParen, "')'")?;
        Ok((args, rp.span))
    }

    pub(super) fn parse_postfix_dot(&mut self, receiver: AstExpr) -> Result<AstExpr, ParseError> {
        let start = receiver.span;
        self.bump(); // `.`
        match self.current().cloned() {
            Some(Token::Ident(name)) => {
                self.bump();
                if self.check(&Token::LParen) {
                    let (args, end) = self.parse_call_args()?;
                    let span = self.merge_span(start, end);
                    Ok(AstExpr::new(
                        ExprKind::MethodCall {
                            receiver,
                            method: name,
                            args,
                        },
                        span,
                    ))
                } else {
                    let span = self.span_until_current(start);
                    Ok(AstExpr::new(
                        ExprKind::FieldAccess {
                            expr: receiver,
                            field: name,
                        },
                        span,
                    ))
                }
            }
            Some(Token::Await) => {
                self.bump();
                let span = self.span_until_current(start);
                Ok(AstExpr::new(ExprKind::Await(Box::new(receiver)), span))
            }
            // P 阶段：`send`/`recv` 保留字允许作方法名/字段名（`.send()`/`.recv()`，
            // 与 actor 前缀 `send x.m(...)` 语法不冲突——后者仅在表达式开头识别）
            Some(Token::Send) | Some(Token::Recv) => {
                let name = match self.current() {
                    Some(Token::Send) => "send",
                    _ => "recv",
                }
                .to_string();
                self.bump();
                if self.check(&Token::LParen) {
                    let (args, end) = self.parse_call_args()?;
                    let span = self.merge_span(start, end);
                    Ok(AstExpr::new(
                        ExprKind::MethodCall {
                            receiver,
                            method: name,
                            args,
                        },
                        span,
                    ))
                } else {
                    let span = self.span_until_current(start);
                    Ok(AstExpr::new(
                        ExprKind::FieldAccess {
                            expr: receiver,
                            field: name,
                        },
                        span,
                    ))
                }
            }
            _ => Err(self.unexpected("field name or 'await' after '.'")),
        }
    }}
