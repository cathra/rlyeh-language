//! 表达式解析（Pratt 解析器）。

use crate::error::ParseError;
use crate::parser::Parser;
use zeta_ast::{AssignOp, AstBlock, AstExpr, BinaryOp, CaptureMode, CompareOp, ExprKind, UnaryOp};
use zeta_lexer::{Span, Token};

/// 运算符优先级（数值越大优先级越高）。
///
/// 层次：赋值(5) < `||`(10) < `&&`(20) < `|`(30) < `^`(35) < `&`(40)
/// < 范围(45) < 比较链(60) < 移位(70) < `+ -`(80)
/// < `* / %`(90) < `as`(100) < 前缀(110) < 后缀(120) < 路径(130)。
mod prec {
    /// 赋值（右结合）
    pub const ASSIGN: u8 = 5;
    /// `||`
    pub const OR: u8 = 10;
    /// `&&`
    pub const AND: u8 = 20;
    /// `|`
    pub const BIT_OR: u8 = 30;
    /// `^`
    pub const BIT_XOR: u8 = 35;
    /// `&`
    pub const BIT_AND: u8 = 40;
    /// `..<` / `...` / `<..`
    pub const RANGE: u8 = 45;
    /// 比较运算符
    pub const COMPARE: u8 = 60;
    /// `<<` `>>`
    pub const SHIFT: u8 = 70;
    /// `+ -`
    pub const ADD: u8 = 80;
    /// `* / %`
    pub const MUL: u8 = 90;
    /// `as`
    pub const CAST: u8 = 100;
}

/// `in`/`not in` 右侧目标
pub(crate) enum InTarget {
    /// 括号集合 `(a, b, c)`（元素可为单值或范围，范围元素语义为离散展开）
    Set(Vec<AstExpr>),
    /// 裸范围 `a..<b` / `a...b` / `a<..b`
    Range(AstExpr),
}

impl<'src> Parser<'src> {
    /// 解析完整表达式
    pub(crate) fn parse_expr(&mut self) -> Result<AstExpr, ParseError> {
        self.parse_expr_prec(0)
    }

    /// 以最小优先级解析表达式（Pratt 主循环）
    fn parse_expr_prec(&mut self, min_prec: u8) -> Result<AstExpr, ParseError> {
        let mut lhs = self.parse_prefix()?;
        loop {
            // 后缀运算符（优先级最高之一）
            if self.check(&Token::Dot) {
                lhs = self.parse_postfix_dot(lhs)?;
                continue;
            }
            if self.check(&Token::LParen) {
                let start = lhs.span;
                let (args, end) = self.parse_call_args()?;
                let span = self.merge_span(start, end);
                lhs = AstExpr::new(ExprKind::Call { callee: lhs, args }, span);
                continue;
            }
            if self.check(&Token::LBracket) {
                let start = lhs.span;
                self.bump();
                let index = self.parse_expr()?;
                let rb = self.expect(&Token::RBracket, "']'")?;
                let span = self.merge_span(start, rb.span);
                lhs = AstExpr::new(ExprKind::Index { expr: lhs, index }, span);
                continue;
            }
            if self.check(&Token::In) {
                let start = lhs.span;
                self.bump();
                if self
                    .current()
                    .is_some_and(|t| matches!(t, Token::Lifetime(_)))
                {
                    let region = self.expect_lifetime()?;
                    let end = lhs.span;
                    lhs = AstExpr::new(
                        ExprKind::InRegion { expr: lhs, region },
                        self.merge_span(start, end),
                    );
                } else {
                    lhs = match self.parse_in_target()? {
                        InTarget::Set(set) => AstExpr::new(
                            ExprKind::InSet {
                                value: lhs,
                                set,
                                negated: false,
                            },
                            self.span_until_current(start),
                        ),
                        InTarget::Range(range) => AstExpr::new(
                            ExprKind::InRange {
                                value: lhs,
                                range,
                                negated: false,
                            },
                            self.span_until_current(start),
                        ),
                    };
                }
                continue;
            }
            if self.check(&Token::NotIn) {
                let start = lhs.span;
                self.bump();
                lhs = match self.parse_in_target()? {
                    InTarget::Set(set) => AstExpr::new(
                        ExprKind::InSet {
                            value: lhs,
                            set,
                            negated: true,
                        },
                        self.span_until_current(start),
                    ),
                    InTarget::Range(range) => AstExpr::new(
                        ExprKind::InRange {
                            value: lhs,
                            range,
                            negated: true,
                        },
                        self.span_until_current(start),
                    ),
                };
                continue;
            }

            // 中缀运算符
            let Some(tok) = self.current().cloned() else {
                break;
            };
            // 旧范围语法 `a..b` / `a..=b` 已废弃，给出明确提示
            if matches!(tok, Token::Range) {
                return Err(self.unexpected("new range syntax `..<` or `...`"));
            }
            let Some((p, right_assoc)) = infix_info(&tok) else {
                break;
            };
            if p < min_prec {
                break;
            }

            // 赋值（右结合）
            if right_assoc {
                let op = assign_op(&tok);
                self.bump();
                let start = lhs.span;
                let value = self.parse_expr_prec(prec::ASSIGN)?;
                let span = self.merge_span(start, value.span);
                lhs = AstExpr::new(
                    ExprKind::Assign {
                        target: lhs,
                        op,
                        value,
                    },
                    span,
                );
                continue;
            }

            // 比较链：收集连续比较运算符
            if p == prec::COMPARE {
                let start = lhs.span;
                let mut elements = vec![lhs];
                let mut operators = Vec::new();
                while let Some(op) = compare_op(self.current()) {
                    self.bump();
                    operators.push(op);
                    let rhs = self.parse_expr_prec(prec::COMPARE + 1)?;
                    elements.push(rhs);
                }
                let end = elements.last().map(|e| e.span).unwrap_or(start);
                lhs = AstExpr::new(
                    ExprKind::ComparisonChain {
                        elements,
                        operators,
                    },
                    self.merge_span(start, end),
                );
                continue;
            }

            // 范围表达式：`a..<b`（左闭右开）/ `a...b`（闭）/ `a<..b`（左开右闭）
            if p == prec::RANGE {
                let (lower_inclusive, upper_inclusive) = match tok {
                    Token::DotDotLt => (true, false),
                    Token::DotDotDot => (true, true),
                    Token::LtDotDot => (false, true),
                    _ => unreachable!("guarded by infix_info"),
                };
                self.bump();
                let start = lhs.span;
                let upper = self.parse_expr_prec(prec::RANGE + 1)?;
                let span = self.merge_span(start, upper.span);
                lhs = AstExpr::new(
                    ExprKind::Range {
                        lower: lhs,
                        upper,
                        lower_inclusive,
                        upper_inclusive,
                    },
                    span,
                );
                continue;
            }

            // as 类型转换
            if p == prec::CAST {
                self.bump();
                let start = lhs.span;
                let target_type = self.parse_type()?;
                let end = self.span_until_current(start);
                lhs = AstExpr::new(
                    ExprKind::Cast {
                        expr: lhs,
                        target_type,
                    },
                    end,
                );
                continue;
            }

            // 普通左结合二元运算
            let op = binary_op(&tok);
            self.bump();
            let start = lhs.span;
            let rhs = self.parse_expr_prec(p + 1)?;
            let span = self.merge_span(start, rhs.span);
            lhs = AstExpr::new(
                ExprKind::Binary {
                    op,
                    left: lhs,
                    right: rhs,
                },
                span,
            );
        }
        Ok(lhs)
    }

    /// 解析 `in`/`not in` 右侧目标：
    /// - `(a, b, c)` → 集合（`InSet`，成员判断，范围元素展开为离散成员）
    /// - `a..<b` / `a...b` / `a<..b` → 裸范围（`InRange`，区间判断）
    ///
    /// 区域目标 `'r`（`InRegion`）由调用方在处理 `Lifetime` 时单独处理。
    fn parse_in_target(&mut self) -> Result<InTarget, ParseError> {
        if self.check(&Token::LParen) {
            return Ok(InTarget::Set(self.parse_set_elements()?));
        }
        // 裸范围：`x in 0..<10`
        let range = self.parse_expr_prec(prec::RANGE)?;
        if !matches!(&*range.kind, ExprKind::Range { .. }) {
            return Err(self.unexpected("集合 `(a, b, c)` 或范围 `a..<b` / `a...b` / `a<..b`"));
        }
        Ok(InTarget::Range(range))
    }

    /// 解析 `(elem, elem, ...)` 集合元素
    fn parse_set_elements(&mut self) -> Result<Vec<AstExpr>, ParseError> {
        self.expect(&Token::LParen, "'('")?;
        let mut set = Vec::new();
        while !self.check(&Token::RParen) {
            if self.at_eof() {
                return Err(self.unexpected("')'"));
            }
            set.push(self.parse_expr()?);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RParen, "')'")?;
        Ok(set)
    }

    /// 解析调用实参 `(a, b, c)`，返回参数列表与结束位置
    fn parse_call_args(&mut self) -> Result<(Vec<AstExpr>, Span), ParseError> {
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

    /// 处理后缀 `.`（字段访问 / 方法调用 / `.await`）
    fn parse_postfix_dot(&mut self, receiver: AstExpr) -> Result<AstExpr, ParseError> {
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
            _ => Err(self.unexpected("field name or 'await' after '.'")),
        }
    }

    /// 解析前缀表达式（字面量 / 标识符 / 路径 / 一元 / 控制流 / 闭包）
    fn parse_prefix(&mut self) -> Result<AstExpr, ParseError> {
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

    /// 标识符开头的前缀：`move` 闭包 / 路径 / 普通标识符
    fn parse_ident_prefix(&mut self, name: String, start: Span) -> Result<AstExpr, ParseError> {
        self.bump();
        // `move |x| ...` / `move || ...` 闭包
        if name == "move" && (self.check(&Token::BitOr) || self.check(&Token::OrOr)) {
            return self.parse_closure(CaptureMode::Move);
        }
        // 路径 `a::b::c`（lexer 将 `::` 拆为两个 `:`，此处合并）
        let mut segments = vec![name];
        while self.eat_colon_colon() {
            let seg = self.expect_ident()?;
            segments.push(seg);
        }
        let span = self.span_until_current(start);
        if segments.len() == 1 {
            let name = segments.pop().expect("non-empty segments");
            Ok(AstExpr::new(ExprKind::Ident(name), span))
        } else {
            Ok(AstExpr::new(ExprKind::Path(segments), span))
        }
    }

    /// 消费 `::`（由两个相邻 `Colon` 组成）
    pub(crate) fn eat_colon_colon(&mut self) -> bool {
        if self.check(&Token::Colon) && self.peek_n(1).is_some_and(|t| t.token == Token::Colon) {
            self.bump();
            self.bump();
            true
        } else {
            false
        }
    }

    /// 括号表达式：空集合 / 集合字面量 / 分组
    ///
    /// 括号内逗号分隔的元素构成集合（`(1, 3, 5)`、`(0...10, 20, 30)`）；
    /// 单元素时若为范围（`(0...10)`）构成单元素集合，否则为分组（`(x + y)`）。
    fn parse_paren_or_set(&mut self) -> Result<AstExpr, ParseError> {
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

    /// if / else if / else 表达式
    fn parse_if_expr(&mut self) -> Result<AstExpr, ParseError> {
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

    /// match 表达式
    fn parse_match_expr(&mut self) -> Result<AstExpr, ParseError> {
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

    /// while 表达式
    fn parse_while_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::While, "'while'")?.span;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        let span = self.merge_span(start, body.span);
        Ok(AstExpr::new(ExprKind::While { cond, body }, span))
    }

    /// loop 表达式
    fn parse_loop_expr(&mut self) -> Result<AstExpr, ParseError> {
        let start = self.expect(&Token::Loop, "'loop'")?.span;
        let body = self.parse_block()?;
        let span = self.merge_span(start, body.span);
        Ok(AstExpr::new(ExprKind::Loop { body }, span))
    }

    /// for 表达式：`for pattern in iterator { body }`
    fn parse_for_expr(&mut self) -> Result<AstExpr, ParseError> {
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

    /// return 表达式
    fn parse_return_expr(&mut self) -> Result<AstExpr, ParseError> {
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

    /// break 表达式
    fn parse_break_expr(&mut self) -> Result<AstExpr, ParseError> {
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

    /// 语句终止符：`;` `}` 或 EOF
    fn stmt_terminator(&self) -> bool {
        self.check(&Token::Semicolon) || self.check(&Token::RBrace) || self.at_eof()
    }

    /// send 表达式：`send actor.method(args)`
    fn parse_send_expr(&mut self) -> Result<AstExpr, ParseError> {
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

    /// 闭包：`|params| body` / `|| body` / `move |params| body` / `move || body`
    fn parse_closure(&mut self, capture: CaptureMode) -> Result<AstExpr, ParseError> {
        let start = match self.peek() {
            Some(lt) => lt.span,
            None => return Err(ParseError::UnexpectedEof),
        };
        // 开 `|`：单个 `|` 后有参数列表，`||` 表示无参数
        let mut params = Vec::new();
        if self.eat(&Token::BitOr) {
            while !self.check(&Token::BitOr) {
                if self.at_eof() {
                    return Err(self.unexpected("'|'"));
                }
                params.push(self.parse_pattern()?);
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
                body,
                capture,
            },
            span,
        ))
    }

    /// 从 `start` 到当前 token 之前构造 span
    pub(crate) fn span_until_current(&self, start: Span) -> Span {
        match self.peek() {
            Some(lt) => Span {
                start: start.start,
                end: lt.span.start,
                line: start.line,
                col: start.col,
            },
            None => start,
        }
    }
}

/// 中缀运算符信息：`(优先级, 是否右结合)`
fn infix_info(tok: &Token) -> Option<(u8, bool)> {
    let p = match tok {
        Token::Assign | Token::PlusEq | Token::MinusEq | Token::StarEq | Token::SlashEq => {
            prec::ASSIGN
        }
        Token::OrOr => prec::OR,
        Token::AndAnd => prec::AND,
        Token::BitOr => prec::BIT_OR,
        Token::BitXor => prec::BIT_XOR,
        Token::BitAnd => prec::BIT_AND,
        Token::DotDotLt | Token::DotDotDot | Token::LtDotDot => prec::RANGE,
        Token::Lt | Token::Le | Token::Gt | Token::Ge | Token::Eq | Token::Ne => prec::COMPARE,
        Token::Shl | Token::Shr => prec::SHIFT,
        Token::Plus | Token::Minus => prec::ADD,
        Token::Star | Token::Slash | Token::Percent => prec::MUL,
        Token::As => prec::CAST,
        _ => return None,
    };
    Some((p, p == prec::ASSIGN))
}

/// token → 比较运算符
fn compare_op(tok: Option<&Token>) -> Option<CompareOp> {
    match tok {
        Some(Token::Lt) => Some(CompareOp::Lt),
        Some(Token::Le) => Some(CompareOp::Le),
        Some(Token::Gt) => Some(CompareOp::Gt),
        Some(Token::Ge) => Some(CompareOp::Ge),
        Some(Token::Eq) => Some(CompareOp::Eq),
        Some(Token::Ne) => Some(CompareOp::Ne),
        _ => None,
    }
}

/// token → 赋值运算符
fn assign_op(tok: &Token) -> AssignOp {
    match tok {
        Token::Assign => AssignOp::Assign,
        Token::PlusEq => AssignOp::AddAssign,
        Token::MinusEq => AssignOp::SubAssign,
        Token::StarEq => AssignOp::MulAssign,
        Token::SlashEq => AssignOp::DivAssign,
        _ => unreachable!("guarded by infix_info"),
    }
}

/// token → 二元运算符
fn binary_op(tok: &Token) -> BinaryOp {
    match tok {
        Token::Plus => BinaryOp::Add,
        Token::Minus => BinaryOp::Sub,
        Token::Star => BinaryOp::Mul,
        Token::Slash => BinaryOp::Div,
        Token::Percent => BinaryOp::Mod,
        Token::BitAnd => BinaryOp::BitAnd,
        Token::BitOr => BinaryOp::BitOr,
        Token::BitXor => BinaryOp::BitXor,
        Token::Shl => BinaryOp::Shl,
        Token::Shr => BinaryOp::Shr,
        Token::AndAnd => BinaryOp::And,
        Token::OrOr => BinaryOp::Or,
        _ => unreachable!("guarded by infix_info"),
    }
}

// 注意：parse_match_arm 定义在 stmt.rs，见 `impl Parser` 的 parse_match_arm。
