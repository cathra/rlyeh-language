//! macro_：LLVM 发射子模块。
//! （由 mod.rs 的 `impl <'src> Parser<'src>` 拆分而来，保持语义等价）

use super::*;

impl <'src> Parser<'src> {
    pub(super) fn parse_macro_call(&mut self, name: String, start: Span) -> Result<AstExpr, ParseError> {
        self.bump(); // `!`
        let close = if self.check(&Token::LParen) {
            Token::RParen
        } else if self.check(&Token::LBracket) {
            Token::RBracket
        } else if self.check(&Token::LBrace) {
            Token::RBrace
        } else {
            return Err(self.unexpected("宏调用的定界符 '('/'['/'{'"));
        };
        let tokens = self.collect_group_content(&close)?;
        // I3 集合宏（`arr!`/`vec!`/`map!`）：parse 期 desugar 为数组字面量或
        // 块表达式（`Vec::with_capacity` + 逐元素 `push`/`insert`），零新增 IR 节点
        if is_collection_macro(&name) {
            return self.parse_collection_macro(&name, &tokens, start);
        }
        // 内置格式化宏：参数 = 表达式列表
        if is_builtin_macro(&name) {
            let mut wrapped = Vec::with_capacity(tokens.len() + 2);
            wrapped.push(LocatedToken::new(Token::LParen, start));
            for t in tokens {
                wrapped.push(LocatedToken::new(t, start));
            }
            wrapped.push(LocatedToken::new(Token::RParen, start));
            let mut sub = Parser::from_tokens(wrapped, self.macros.clone(), self.macro_depth);
            let (args, _) = sub.parse_call_args()?;
            if !sub.at_eof() {
                return Err(ParseError::Macro {
                    msg: format!("宏 `{name}` 的参数解析后有多余 token"),
                    line: start.line,
                    col: start.col,
                });
            }
            let span = self.span_until_current(start);
            return Ok(AstExpr::new(
                ExprKind::MacroCall {
                    name: format!("{name}!"),
                    args,
                },
                span,
            ));
        }
        // 用户宏：token 流展开 → 递归解析
        if self.macro_depth >= MAX_MACRO_DEPTH {
            return Err(ParseError::Macro {
                msg: format!(
                    "宏 `{name}` 展开超过深度上限 {MAX_MACRO_DEPTH}（疑似无限递归）"
                ),
                line: start.line,
                col: start.col,
            });
        }
        if self.macros.contains_key(&name) {
            self.macro_depth += 1;
            let result = (|| {
                let expanded = expand_macro(&self.macros, &name, &tokens).map_err(|e| {
                    ParseError::Macro {
                        msg: e.0,
                        line: start.line,
                        col: start.col,
                    }
                })?;
                let located: Vec<LocatedToken> = expanded
                    .into_iter()
                    .map(|t| LocatedToken::new(t, start))
                    .collect();
                let mut sub =
                    Parser::from_tokens(located, self.macros.clone(), self.macro_depth);
                let expr = sub.parse_expr()?;
                if !sub.at_eof() {
                    return Err(ParseError::Macro {
                        msg: format!(
                            "宏 `{name}` 展开产物含多余 token（transcriber 应展开为单个表达式）"
                        ),
                        line: start.line,
                        col: start.col,
                    });
                }
                Ok(expr)
            })();
            self.macro_depth -= 1;
            return result;
        }
        Err(ParseError::Macro {
            msg: format!(
                "未定义的宏 `{name}`（内置宏：println!/print!/format!/dbg!/eprintln!/eprint!/arr!/vec!/map!）"
            ),
            line: start.line,
            col: start.col,
        })
    }

    pub(super) fn parse_collection_macro(
        &mut self,
        name: &str,
        tokens: &[Token],
        start: Span,
    ) -> Result<AstExpr, ParseError> {
        let mut wrapped = Vec::with_capacity(tokens.len() + 2);
        wrapped.push(LocatedToken::new(Token::LParen, start));
        for t in tokens {
            wrapped.push(LocatedToken::new(t.clone(), start));
        }
        wrapped.push(LocatedToken::new(Token::RParen, start));
        let mut sub = Parser::from_tokens(wrapped, self.macros.clone(), self.macro_depth);
        let span = self.span_until_current(start);

        let result = match name {
            "arr" => {
                let (elems, _) = sub.parse_call_args()?;
                if !sub.at_eof() {
                    return Err(ParseError::Macro {
                        msg: "宏 `arr!` 参数解析后有多余 token".to_string(),
                        line: start.line,
                        col: start.col,
                    });
                }
                Ok(AstExpr::new(ExprKind::ArrayLit(elems), span))
            }
            "vec" | "map" => {
                let is_map = name == "map";
                // 逐元素解析：`vec!` 元素为单表达式；`map!` 元素为 `k => v`（FatArrow）
                // （先消费包裹的 `(`，与 `parse_call_args` 行为一致）
                let mut elems: Vec<(AstExpr, Option<AstExpr>)> = Vec::new();
                sub.expect(&Token::LParen, "'('")?;
                loop {
                    if sub.check(&Token::RParen) {
                        break; // 空集合 `vec![]` / `map![]`
                    }
                    if sub.at_eof() {
                        return Err(ParseError::Macro {
                            msg: format!("宏 `{name}!` 元素列表未闭合"),
                            line: start.line,
                            col: start.col,
                        });
                    }
                    let key = sub.parse_expr()?;
                    if sub.eat(&Token::FatArrow) {
                        let val = sub.parse_expr()?;
                        elems.push((key, Some(val)));
                    } else if is_map {
                        return Err(ParseError::Macro {
                            msg: "宏 `map!` 元素必须为 `k => v` 键值对（缺少 `=>`）".to_string(),
                            line: start.line,
                            col: start.col,
                        });
                    } else {
                        elems.push((key, None));
                    }
                    if !sub.eat(&Token::Comma) {
                        break;
                    }
                }
                sub.expect(&Token::RParen, "')'")?;
                if !sub.at_eof() {
                    return Err(ParseError::Macro {
                        msg: format!("宏 `{name}!` 参数解析后有多余 token"),
                        line: start.line,
                        col: start.col,
                    });
                }

                // 临时变量名（与 typecheck `fresh_temp` 命名风格一致）
                let tmp_name = if is_map {
                    format!("__map_{}", self.collection_temp_seq)
                } else {
                    format!("__vec_{}", self.collection_temp_seq)
                };
                self.collection_temp_seq += 1;

                let mut stmts = Vec::new();
                // `let mut __tmp = Vec/HashMap::with_capacity(n);`（空 → `new()`）
                let ctor = if elems.is_empty() {
                    ExprKind::Call {
                        callee: AstExpr::new(
                            ExprKind::Ident(if is_map {
                                "HashMap::new".to_string()
                            } else {
                                "Vec::new".to_string()
                            }),
                            span,
                        ),
                        args: Vec::new(),
                        type_args: Vec::new(),
                    }
                } else {
                    let len = AstExpr::new(ExprKind::IntLiteral(elems.len() as i128), span);
                    ExprKind::Call {
                        callee: AstExpr::new(
                            ExprKind::Ident(if is_map {
                                "HashMap::with_capacity".to_string()
                            } else {
                                "Vec::with_capacity".to_string()
                            }),
                            span,
                        ),
                        args: vec![len],
                        type_args: Vec::new(),
                    }
                };
                stmts.push(AstStmt::Let {
                    pattern: AstPattern::Ident(tmp_name.clone()),
                    type_anno: None,
                    init: AstExpr::new(ctor, span),
                    mutable: true,
                });

                // 逐元素 `.push(e)` / `.insert(k, v)`
                let receiver = AstExpr::new(ExprKind::Ident(tmp_name.clone()), span);
                for (k, v) in &elems {
                    let args = if is_map {
                        vec![
                            k.clone(),
                            v.clone().expect("map! 元素 value 必填（已校验）"),
                        ]
                    } else {
                        vec![k.clone()]
                    };
                    stmts.push(AstStmt::Semi(AstExpr::new(
                        ExprKind::MethodCall {
                            receiver: receiver.clone(),
                            method: if is_map {
                                "insert".to_string()
                            } else {
                                "push".to_string()
                            },
                            args,
                            trait_hint: None,
                        },
                        span,
                    )));
                }

                // 块值 = 集合变量
                let block = AstExpr::new(
                    ExprKind::Block(AstBlock {
                        stmts,
                        final_expr: Some(AstExpr::new(ExprKind::Ident(tmp_name), span)),
                        span,
                    }),
                    span,
                );
                Ok(block)
            }
            _ => unreachable!("is_collection_macro 已过滤宏名"),
        };
        result
    }

    pub(super) fn looks_like_struct_ctor(&self) -> bool {
        if !self.check(&Token::LBrace) {
            return false;
        }
        // `..base` 更新语法（无显式字段）：`{ ..` 即结构体构造
        if self.peek_n(1).is_some_and(|t| t.token == Token::Range) {
            return true;
        }
        // `{ Ident :`（冒号后不能是 `:`，避免 `Ident { Enum::Variant` 歧义）
        if !self.peek_n(1).is_some_and(|t| matches!(t.token, Token::Ident(_))) {
            return false;
        }
        if !self.peek_n(2).is_some_and(|t| t.token == Token::Colon) {
            return false;
        }
        !self.peek_n(3).is_some_and(|t| t.token == Token::Colon)
    }

    pub(super) fn looks_like_generic_struct_ctor(&self) -> bool {
        if !self.check(&Token::Lt) {
            return false;
        }
        let mut depth = 0;
        let mut i = 0;
        loop {
            let Some(tok) = self.peek_n(i) else {
                return false;
            };
            match tok.token {
                Token::Lt => depth += 1,
                Token::Gt => {
                    depth -= 1;
                    if depth == 0 {
                        return self
                            .peek_n(i + 1)
                            .is_some_and(|t| t.token == Token::LBrace);
                    }
                }
                Token::RBrace | Token::Eof => return false,
                _ => {}
            }
            i += 1;
        }
    }}
