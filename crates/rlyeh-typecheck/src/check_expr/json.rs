//! 表达式检查子模块：json。
//! （由 macro_serialize.rs 二次拆分而来，保持语义等价）

use super::*;


/// P2（2026-08-28）：`json::try_parse::<T>(s) -> Result<T, JsonError>` 严格解析。
/// 非法输入返回 `Err(JsonError::ParseError(msg))`（替代 `json::parse` 的静默零值/宽松解析）。
/// MVP：标量（i64/bool/String）做严格校验；HashMap/struct 做首尾 `{}` 闭合校验（深层
/// 逐段校验 + 错误定位为后续子步骤）。
pub(crate) fn check_json_try_parse(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "json.try_parse".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    if type_args.len() != 1 {
        return Err(TypeError::Unsupported {
            what: "json.try_parse 需要 1 个类型实参（turbofish `json.try_parse::<T>(s)`）"
                .to_string(),
            span,
        });
    }
    let target = resolve_ast_type(ctx, &type_args[0], span)?;
    let try_ast = json_try_parse_ast(ctx, &target, &args[0], span)?;
    let (hir, ty) = infer_expr(ctx, &try_ast)?;
    Ok((hir, ty))
}

pub(crate) fn check_json_parse(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "json.parse".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    if type_args.len() != 1 {
        return Err(TypeError::Unsupported {
            what: "json.parse 需要 1 个类型实参（turbofish `json.parse::<T>(s)`）".to_string(),
            span,
        });
    }
    let target = resolve_ast_type(ctx, &type_args[0], span)?;
    let parse_ast = json_parse_ast(ctx, &target, &args[0], span)?;
    let (hir, _) = infer_expr(ctx, &parse_ast)?;
    Ok((hir, target))
}

pub(crate) fn json_parse_ast(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    span: Span,
) -> Result<AstExpr, TypeError> {
    // 统一实参转 String（JSON 文本实参可为字符串字面量 / String / &str）
    let s = mk_path_call(
        vec!["String".to_string(), "from".to_string()],
        vec![arg.clone()],
        span,
    );
    match ty {
        // i64 → `string_to_int(s)`（std；JSON 数字文本无引号，MVP 直接解析）
        Type::I64 => Ok(mk_ident_call(
            "string_to_int".to_string(),
            vec![s],
            span,
        )),
        // bool → `if s == "true" { true } else { false }`（String 内容相等 → bytes_eq）
        Type::Bool => {
            let eq = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![s, string_from_lit_ast("true".to_string(), span)],
                    operators: vec![CompareOp::Eq],
                },
                span,
            );
            Ok(AstExpr::new(
                ExprKind::If {
                    cond: eq,
                    then_block: AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(AstExpr::new(ExprKind::BoolLiteral(true), span)),
                        span,
                    },
                    else_block: Some(AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(AstExpr::new(ExprKind::BoolLiteral(false), span)),
                        span,
                    }),
                },
                span,
            ))
        }
        // String → `json_unescape(s)`（引号剥离 + 转义还原，core.rl）
        Type::Named(n, _) if n == "String" => Ok(mk_ident_call(
            "json_unescape".to_string(),
            vec![s],
            span,
        )),
        // HashMap<K, V> → JSON 对象 `{"k":v,...}` 反序列化。
        // desugar 为块表达式 + `for x in vec`（check_for_vec 遍历 split 结果）：
        // `{ let __s = String::from(<arg>);                 // JSON 文本
        //    let __body = __s.substring(1, __s.len() - 1);  // 剥离首尾 { }
        //    let __parts = __body.split(",");               // 逗号分段（键/值含逗号 MVP 限制）
        //    let mut __m: HashMap<K, V> = HashMap::new();   // 注解定型（空对象 {} 亦定型）
        //    for __part in __parts {
        //      let __c = __part.find(":");
        //      if __c >= 0 {
        //        let __kpart = __part.substring(0, __c);
        //        let __vpart = __part.substring(__c + 1, __part.len());
        //        let __k = <键解析>;                          // i64: string_to_int(json_unescape(..))
        //                                                      // String: json_unescape(..)
        //        let __v = <值解析，递归 json_parse_ast>;
        //        __m.insert(__k, __v);
        //      }
        //    }
        //    __m }`
        // 键限 i64 / String（JSON 键恒为带引号字符串，如 `"1"`——先 json_unescape 剥引号，
        // i64 再经 string_to_int 转整数）；值限标量（i64 / bool / String）。嵌套 HashMap 值
        // MVP 显式 Unsupported（split(",") 分段无法正确处理内层逗号）；数组 / Vec / 结构体值
        // 经 json_parse_ast 递归自然落 Unsupported。须置于 struct 分支之前（见 stringify）。
        Type::Named(n, args) if n == "HashMap" && args.len() == 2 => {
            let k_ty = substitute(&args[0], &ctx.generic_subst);
            let v_ty = substitute(&args[1], &ctx.generic_subst);
            if matches!(k_ty, Type::Infer) || matches!(v_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "json.parse：HashMap 键/值类型未确定（turbofish 显式定型，如 `json.parse::<HashMap<i64, i64>>(s)`）".to_string(),
                    span,
                });
            }
            let key_is_i64 = matches!(k_ty, Type::I64);
            let key_is_string = matches!(&k_ty, Type::Named(kn, _) if kn == "String");
            if !key_is_i64 && !key_is_string {
                return Err(TypeError::Unsupported {
                    what: format!("json.parse：HashMap 键类型 `{k_ty}`（MVP 支持 i64 / String）"),
                    span,
                });
            }
            if matches!(&v_ty, Type::Named(vn, _) if vn == "HashMap") {
                return Err(TypeError::Unsupported {
                    what: "json.parse：嵌套 HashMap 值反序列化（MVP 支持标量值 i64 / bool / String）"
                        .to_string(),
                    span,
                });
            }
            // 临时变量：JSON 文本 / 剥离后主体 / 逗号分段 / 结果 map / 段 / 冒号下标 /
            // 键段 / 值段 / 键 / 值
            let s_name = ctx.fresh_temp();
            let body_name = ctx.fresh_temp();
            let parts_name = ctx.fresh_temp();
            let m_name = ctx.fresh_temp();
            let part_name = ctx.fresh_temp();
            let c_name = ctx.fresh_temp();
            let kpart_name = ctx.fresh_temp();
            let vpart_name = ctx.fresh_temp();
            let k_name = ctx.fresh_temp();
            let v_name = ctx.fresh_temp();
            let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
            let body_id = AstExpr::new(ExprKind::Ident(body_name.clone()), span);
            let parts_id = AstExpr::new(ExprKind::Ident(parts_name.clone()), span);
            let m_id = AstExpr::new(ExprKind::Ident(m_name.clone()), span);
            let part_id = AstExpr::new(ExprKind::Ident(part_name.clone()), span);
            let c_id = AstExpr::new(ExprKind::Ident(c_name.clone()), span);
            let kpart_id = AstExpr::new(ExprKind::Ident(kpart_name.clone()), span);
            let vpart_id = AstExpr::new(ExprKind::Ident(vpart_name.clone()), span);
            let k_id = AstExpr::new(ExprKind::Ident(k_name.clone()), span);
            let v_id = AstExpr::new(ExprKind::Ident(v_name.clone()), span);
            // `recv.method(args)` 方法调用 AST
            let mcall = |recv: AstExpr, method: &str, args: Vec<AstExpr>| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: method.to_string(),
                        args,
                        trait_hint: None,
                    },
                    span,
                )
            };
            // 键解析：JSON 键恒带引号 → `json_unescape(__kpart)` 剥引号 + 还原转义；
            // i64 键再经 `string_to_int` 转整数
            let unescaped_key =
                mk_ident_call("json_unescape".to_string(), vec![kpart_id.clone()], span);
            let key_parse = if key_is_i64 {
                mk_ident_call("string_to_int".to_string(), vec![unescaped_key], span)
            } else {
                unescaped_key
            };
            // 值解析（递归；标量 i64 / bool / String，其余落 Unsupported）
            let val_parse = json_parse_ast(ctx, &v_ty, &vpart_id, span)?;
            // if __c >= 0 { ... __m.insert(__k, __v) }
            let c_ge_zero = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![c_id.clone(), AstExpr::new(ExprKind::IntLiteral(0), span)],
                    operators: vec![CompareOp::Ge],
                },
                span,
            );
            let if_parse = AstExpr::new(
                ExprKind::If {
                    cond: c_ge_zero,
                    then_block: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(kpart_name),
                                type_anno: None,
                                init: mcall(
                                    part_id.clone(),
                                    "substring",
                                    vec![
                                        AstExpr::new(ExprKind::IntLiteral(0), span),
                                        c_id.clone(),
                                    ],
                                ),
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(vpart_name),
                                type_anno: None,
                                init: mcall(
                                    part_id.clone(),
                                    "substring",
                                    vec![
                                        AstExpr::new(
                                            ExprKind::Binary {
                                                op: BinaryOp::Add,
                                                left: c_id.clone(),
                                                right: AstExpr::new(
                                                    ExprKind::IntLiteral(1),
                                                    span,
                                                ),
                                            },
                                            span,
                                        ),
                                        mcall(part_id.clone(), "len", Vec::new()),
                                    ],
                                ),
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(k_name),
                                type_anno: None,
                                init: key_parse,
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(v_name),
                                type_anno: None,
                                init: val_parse,
                                mutable: false,
                            },
                            AstStmt::Semi(mcall(
                                m_id.clone(),
                                "insert",
                                vec![k_id, v_id],
                            )),
                        ],
                        final_expr: None,
                        span,
                    },
                    else_block: None,
                },
                span,
            );
            // for __part in __parts { let __c = __part.find(":"); <if 解析+insert> }
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Ident(part_name),
                    iterator: parts_id.clone(),
                    body: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(c_name),
                                type_anno: None,
                                init: mcall(
                                    part_id,
                                    "find",
                                    vec![string_from_lit_ast(":".to_string(), span)],
                                ),
                                mutable: false,
                            },
                            AstStmt::Semi(if_parse),
                        ],
                        final_expr: None,
                        span,
                    },
                },
                span,
            );
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(s_name),
                            type_anno: None,
                            init: mk_path_call(
                                vec!["String".to_string(), "from".to_string()],
                                vec![arg.clone()],
                                span,
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(body_name),
                            type_anno: None,
                            init: mcall(
                                s_id.clone(),
                                "substring",
                                vec![
                                    AstExpr::new(ExprKind::IntLiteral(1), span),
                                    AstExpr::new(
                                        ExprKind::Binary {
                                            op: BinaryOp::Sub,
                                            left: mcall(s_id, "len", Vec::new()),
                                            right: AstExpr::new(ExprKind::IntLiteral(1), span),
                                        },
                                        span,
                                    ),
                                ],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(parts_name),
                            type_anno: None,
                            init: mcall(
                                body_id,
                                "split",
                                vec![string_from_lit_ast(",".to_string(), span)],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(m_name),
                            type_anno: Some(rlyeh_ast::SpannedAstType {
                                ty: AstType::Path(
                                    "HashMap".to_string(),
                                    vec![ty_to_ast(&k_ty), ty_to_ast(&v_ty)],
                                ),
                                span: rlyeh_lexer::Span::dummy(),
                            }),
                            init: mk_path_call(
                                vec!["HashMap".to_string(), "new".to_string()],
                                Vec::new(),
                                span,
                            ),
                            mutable: true,
                        },
                        AstStmt::Semi(for_expr),
                    ],
                    final_expr: Some(m_id),
                    span,
                }),
                span,
            ))
        }
        // 用户 struct → JSON 对象 `{"f0":v0,"f1":v1}` 反序列化（字段名匹配，顺序无关，
        // 缺失字段保持零值，未知字段忽略）。desugar 为块表达式 + `for x in vec`
        // （复用 check_for_vec 遍历 split 结果）：
        // `{ let __s = String::from(<arg>);                 // JSON 文本
        //    let __body = __s.substring(1, __s.len() - 1);  // 剥离首尾 { }
        //    let __parts = __body.split(",");               // 逗号分段
        //    let mut __p: Point = Point { x: 0, y: 0 };     // 零值构造
        //    for __part in __parts {
        //      let __c = __part.find(":");
        //      if __c >= 0 {
        //        let __name = json_unescape(__part.substring(0, __c));  // 字段名（剥引号）
        //        let __val = __part.substring(__c + 1, __part.len());
        //        if __name == "x" { __p.x = <字段 x 值解析>; }   // 递归 json_parse_ast
        //        else if __name == "y" { __p.y = <字段 y 值解析>; }
        //        else { }                                        // 未知字段忽略
        //      }
        //    }
        //    __p }`
        // 字段值限 json_parse_ast 支持类型（i64 / bool / String / 嵌套 struct）；
        // 嵌套 struct / Vec / HashMap 值含逗号经 split(",") 分段错误的 MVP 限制
        // （与 HashMap 分支一致）；泛型 struct（带类型实参）MVP 不支持（零值与字段
        // 解析需按实参定型）。
        Type::Named(n, args) if ctx.lookup_struct(&n).is_some() && args.is_empty() => {
            let def = ctx.lookup_struct(&n).cloned().unwrap();
            if def.fields.is_empty() {
                return Err(TypeError::Unsupported {
                    what: format!("json.parse：空结构体 `{n}` 反序列化"),
                    span,
                });
            }
            // 临时变量：JSON 文本 / 剥离后主体 / 逗号分段 / 结果 struct / 段 /
            // 冒号下标 / 字段名 / 值段
            let s_name = ctx.fresh_temp();
            let body_name = ctx.fresh_temp();
            let parts_name = ctx.fresh_temp();
            let p_name = ctx.fresh_temp();
            let part_name = ctx.fresh_temp();
            let c_name = ctx.fresh_temp();
            let name_name = ctx.fresh_temp();
            let val_name = ctx.fresh_temp();
            let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
            let body_id = AstExpr::new(ExprKind::Ident(body_name.clone()), span);
            let parts_id = AstExpr::new(ExprKind::Ident(parts_name.clone()), span);
            let p_id = AstExpr::new(ExprKind::Ident(p_name.clone()), span);
            let part_id = AstExpr::new(ExprKind::Ident(part_name.clone()), span);
            let c_id = AstExpr::new(ExprKind::Ident(c_name.clone()), span);
            let name_id = AstExpr::new(ExprKind::Ident(name_name.clone()), span);
            let val_id = AstExpr::new(ExprKind::Ident(val_name.clone()), span);
            // `recv.method(args)` 方法调用 AST
            let mcall = |recv: AstExpr, method: &str, args: Vec<AstExpr>| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: method.to_string(),
                        args,
                        trait_hint: None,
                    },
                    span,
                )
            };
            // 字段值解析（递归 json_parse_ast）
            let field_parse = |ctx: &mut TypeContext, fty: &Type| {
                json_parse_ast(ctx, fty, &val_id.clone(), span)
            };
            // 零值构造（缺失字段保持零值）
            let mut zero_fields = Vec::new();
            for (fname, fty) in &def.fields {
                zero_fields.push((fname.clone(), ty_to_zero_ast(ctx, fty, span)?));
            }
            let zero_ctor = AstExpr::new(
                ExprKind::StructCtor {
                    type_name: vec![n.clone()],
                    type_args: Vec::new(),
                    fields: zero_fields,
                },
                span,
            );
            // if-else 链：`if __name == "x" { __p.x = <解析>; } else if ... else { }`
            // 自后向前构建；最内层 else 为空块（未知字段忽略）
            let mut chain: Option<AstExpr> = None;
            for (fname, fty) in def.fields.iter().rev() {
                let name_eq = AstExpr::new(
                    ExprKind::ComparisonChain {
                        elements: vec![
                            name_id.clone(),
                            string_from_lit_ast(fname.clone(), span),
                        ],
                        operators: vec![CompareOp::Eq],
                    },
                    span,
                );
                let inner = chain.take();
                let then_block = AstBlock {
                    stmts: vec![AstStmt::Semi(AstExpr::new(
                        ExprKind::Assign {
                            target: AstExpr::new(
                                ExprKind::FieldAccess {
                                    expr: p_id.clone(),
                                    field: fname.clone(),
                                },
                                span,
                            ),
                            op: AssignOp::Assign,
                            value: field_parse(ctx, fty)?,
                        },
                        span,
                    ))],
                    final_expr: None,
                    span,
                };
                chain = Some(AstExpr::new(
                    ExprKind::If {
                        cond: name_eq,
                        then_block,
                        else_block: Some(AstBlock {
                            stmts: Vec::new(),
                            final_expr: inner,
                            span,
                        }),
                    },
                    span,
                ));
            }
            let if_chain = chain.expect("struct 至少一个字段");
            // `if __c >= 0 { let __name = ...; let __val = ...; <if 链> }`
            let c_ge_zero = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![c_id.clone(), AstExpr::new(ExprKind::IntLiteral(0), span)],
                    operators: vec![CompareOp::Ge],
                },
                span,
            );
            let if_parse = AstExpr::new(
                ExprKind::If {
                    cond: c_ge_zero,
                    then_block: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(name_name),
                                type_anno: None,
                                init: mk_ident_call(
                                    "json_unescape".to_string(),
                                    vec![mcall(
                                        part_id.clone(),
                                        "substring",
                                        vec![
                                            AstExpr::new(ExprKind::IntLiteral(0), span),
                                            c_id.clone(),
                                        ],
                                    )],
                                    span,
                                ),
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(val_name),
                                type_anno: None,
                                init: mcall(
                                    part_id.clone(),
                                    "substring",
                                    vec![
                                        AstExpr::new(
                                            ExprKind::Binary {
                                                op: BinaryOp::Add,
                                                left: c_id.clone(),
                                                right: AstExpr::new(
                                                    ExprKind::IntLiteral(1),
                                                    span,
                                                ),
                                            },
                                            span,
                                        ),
                                        mcall(part_id.clone(), "len", Vec::new()),
                                    ],
                                ),
                                mutable: false,
                            },
                            AstStmt::Semi(if_chain),
                        ],
                        final_expr: None,
                        span,
                    },
                    else_block: None,
                },
                span,
            );
            // `for __part in __parts { let __c = __part.find(":"); <if 解析> }`
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Ident(part_name),
                    iterator: parts_id.clone(),
                    body: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(c_name),
                                type_anno: None,
                                init: mcall(
                                    part_id,
                                    "find",
                                    vec![string_from_lit_ast(":".to_string(), span)],
                                ),
                                mutable: false,
                            },
                            AstStmt::Semi(if_parse),
                        ],
                        final_expr: None,
                        span,
                    },
                },
                span,
            );
            // 顶层块：`{ let __s; let __body; let __parts; let mut __p; <for>; __p }`
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(s_name),
                            type_anno: None,
                            init: mk_path_call(
                                vec!["String".to_string(), "from".to_string()],
                                vec![arg.clone()],
                                span,
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(body_name),
                            type_anno: None,
                            init: mcall(
                                s_id.clone(),
                                "substring",
                                vec![
                                    AstExpr::new(ExprKind::IntLiteral(1), span),
                                    AstExpr::new(
                                        ExprKind::Binary {
                                            op: BinaryOp::Sub,
                                            left: mcall(s_id, "len", Vec::new()),
                                            right: AstExpr::new(
                                                ExprKind::IntLiteral(1),
                                                span,
                                            ),
                                        },
                                        span,
                                    ),
                                ],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(parts_name),
                            type_anno: None,
                            init: mcall(
                                body_id,
                                "split",
                                vec![string_from_lit_ast(",".to_string(), span)],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(p_name),
                            type_anno: Some(rlyeh_ast::SpannedAstType {
                                ty: AstType::Path(n.clone(), Vec::new()),
                                span: rlyeh_lexer::Span::dummy(),
                            }),
                            init: zero_ctor,
                            mutable: true,
                        },
                        AstStmt::Semi(for_expr),
                    ],
                    final_expr: Some(p_id),
                    span,
                }),
                span,
            ))
        }
        other => Err(TypeError::Unsupported {
            what: format!("json.parse：类型 `{other}` 反序列化"),
            span,
        }),
    }
}

/// P2（2026-08-28）：构造 `json.try_parse::<T>(s)` 的严格校验 + Result 包装块 AST：
/// `{ let __s = String::from(<arg>); if <严格校验 __s> { Result::Ok(<json_parse_ast>)
///   } else { Result::Err(JsonError::ParseError(<msg>)) } }`。
/// MVP：标量（i64/bool/String）严格校验；HashMap/struct 首尾 `{}` 闭合校验
/// （深层逐段校验 + 错误定位为后续子步骤）。
pub(crate) fn json_try_parse_ast(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    span: Span,
) -> Result<AstExpr, TypeError> {
    let s_name = ctx.fresh_temp();
    let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
    let mcall = |recv: AstExpr, method: &str, args: Vec<AstExpr>| {
        AstExpr::new(
            ExprKind::MethodCall {
                receiver: recv,
                method: method.to_string(),
                args,
                trait_hint: None,
            },
            span,
        )
    };
    let cmp = |left: AstExpr, right: AstExpr| {
        AstExpr::new(
            ExprKind::ComparisonChain {
                elements: vec![left, right],
                operators: vec![CompareOp::Eq],
            },
            span,
        )
    };
    // 校验条件 + 错误消息
    let (cond, msg): (AstExpr, &str) = match ty {
        Type::I64 => {
            // is_ok 返回 i64（0/1），`if` 条件需 bool → `parse_int_strict(__s).is_ok() == 1`
            let call = mk_ident_call("parse_int_strict".to_string(), vec![s_id.clone()], span);
            let isok = mcall(call, "is_ok", Vec::new());
            (
                cmp(isok, AstExpr::new(ExprKind::IntLiteral(1), span)),
                "invalid integer",
            )
        }
        Type::Bool => {
            let eq_true = cmp(s_id.clone(), string_from_lit_ast("true".to_string(), span));
            let eq_false = cmp(s_id.clone(), string_from_lit_ast("false".to_string(), span));
            (
                AstExpr::new(
                    ExprKind::Binary {
                        op: BinaryOp::Or,
                        left: eq_true,
                        right: eq_false,
                    },
                    span,
                ),
                "invalid bool",
            )
        }
        Type::Named(n, _) if n == "String" => {
            // `json_unescape_checked(__s).is_ok() == 1`（is_ok 返回 i64）
            let call = mk_ident_call(
                "json_unescape_checked".to_string(),
                vec![s_id.clone()],
                span,
            );
            let isok = mcall(call, "is_ok", Vec::new());
            (
                cmp(isok, AstExpr::new(ExprKind::IntLiteral(1), span)),
                "invalid string",
            )
        }
        // HashMap / struct：首尾 `{ }`（123）闭合校验
        Type::Named(..) => {
            let len = mcall(s_id.clone(), "len", Vec::new());
            let len_ge2 = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![len.clone(), AstExpr::new(ExprKind::IntLiteral(2), span)],
                    operators: vec![CompareOp::Ge],
                },
                span,
            );
            let first = cmp(
                mcall(s_id.clone(), "get", vec![AstExpr::new(ExprKind::IntLiteral(0), span)]),
                AstExpr::new(ExprKind::IntLiteral(123), span),
            );
            let last = cmp(
                mcall(
                    s_id.clone(),
                    "get",
                    vec![AstExpr::new(
                        ExprKind::Binary {
                            op: BinaryOp::Sub,
                            left: len,
                            right: AstExpr::new(ExprKind::IntLiteral(1), span),
                        },
                        span,
                    )],
                ),
                AstExpr::new(ExprKind::IntLiteral(123), span),
            );
            let and1 = AstExpr::new(
                ExprKind::Binary {
                    op: BinaryOp::And,
                    left: len_ge2,
                    right: first,
                },
                span,
            );
            let and2 = AstExpr::new(
                ExprKind::Binary {
                    op: BinaryOp::And,
                    left: and1,
                    right: last,
                },
                span,
            );
            (and2, "invalid object")
        }
        other => {
            return Err(TypeError::Unsupported {
                what: format!("json.try_parse：类型 `{other}` 反序列化"),
                span,
            });
        }
    };
    // Result::Ok(<json_parse_ast 对 __s>) / Result::Err(JsonError::ParseError(msg))
    let parse = json_parse_ast(ctx, ty, &s_id, span)?;
    let ok = mk_path_call(
        vec!["Result".to_string(), "Ok".to_string()],
        vec![parse],
        span,
    );
    // 用完整路径 `serde::JsonError::ParseError`（json 为内建模块，用户作用域无裸名 JsonError）
    let err = mk_path_call(
        vec!["Result".to_string(), "Err".to_string()],
        vec![mk_path_call(
            vec![
                "serde".to_string(),
                "JsonError".to_string(),
                "ParseError".to_string(),
            ],
            vec![string_from_lit_ast(msg.to_string(), span)],
            span,
        )],
        span,
    );
    let if_expr = AstExpr::new(
        ExprKind::If {
            cond,
            then_block: AstBlock {
                stmts: Vec::new(),
                final_expr: Some(ok),
                span,
            },
            else_block: Some(AstBlock {
                stmts: Vec::new(),
                final_expr: Some(err),
                span,
            }),
        },
        span,
    );
    Ok(AstExpr::new(
        ExprKind::Block(AstBlock {
            stmts: vec![AstStmt::Let {
                pattern: AstPattern::Ident(s_name),
                type_anno: None,
                init: mk_path_call(
                    vec!["String".to_string(), "from".to_string()],
                    vec![arg.clone()],
                    span,
                ),
                mutable: false,
            }],
            final_expr: Some(if_expr),
            span,
        }),
        span,
    ))
}

pub(crate) fn check_json_to_writer(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 2 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "json.to_writer".to_string(),
            expected: 2,
            found: args.len(),
            span,
        });
    }
    // 首个参数须为 File 写句柄（File / &File / &mut File；名称按短名或路径后缀匹配，
    // 解析后完整名可能为 `io::file::File`）
    let (_w_hir, w_ty) = infer_expr(ctx, &args[0])?;
    let inner = match &w_ty {
        Type::Named(n, _) => Some(n),
        Type::Ref(t, _) => match &**t {
            Type::Named(n, _) => Some(n),
            _ => None,
        },
        _ => None,
    };
    let is_file = inner
        .map(|n| n == "File" || n.ends_with("::File"))
        .unwrap_or(false);
    if !is_file {
        return Err(TypeError::Unsupported {
            what: format!(
                "json.to_writer：首个参数须为 `File`（MVP；TcpStream 留待流式接线），实为 `{w_ty}`"
            ),
            span,
        });
    }
    // 构造 `w.write_all(json.stringify(v))` 调用 AST
    let ser = mk_path_call(
        vec!["json".to_string(), "stringify".to_string()],
        vec![args[1].clone()],
        span,
    );
    let call_ast = AstExpr::new(
        ExprKind::MethodCall {
            receiver: args[0].clone(),
            method: "write_all".to_string(),
            args: vec![ser],
            trait_hint: None,
        },
        span,
    );
    infer_expr(ctx, &call_ast)
}

pub(crate) fn check_json_from_reader(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "json.from_reader".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    if type_args.len() != 1 {
        return Err(TypeError::Unsupported {
            what: "json.from_reader：须显式泛型实参 `json.from_reader::<T>(r)`".to_string(),
            span,
        });
    }
    // 首个参数须为 File 读句柄（名称按短名或路径后缀匹配）
    let (_r_hir, r_ty) = infer_expr(ctx, &args[0])?;
    let inner = match &r_ty {
        Type::Named(n, _) => Some(n),
        Type::Ref(t, _) => match &**t {
            Type::Named(n, _) => Some(n),
            _ => None,
        },
        _ => None,
    };
    let is_file = inner
        .map(|n| n == "File" || n.ends_with("::File"))
        .unwrap_or(false);
    if !is_file {
        return Err(TypeError::Unsupported {
            what: format!(
                "json.from_reader：首个参数须为 `File`（MVP；TcpStream 留待流式接线），实为 `{r_ty}`"
            ),
            span,
        });
    }
    // 构造 `r.read_to_string().unwrap()` → `json.parse::<T>(...)` 调用 AST
    let read_ast = AstExpr::new(
        ExprKind::MethodCall {
            receiver: args[0].clone(),
            method: "read_to_string".to_string(),
            args: Vec::new(),
            trait_hint: None,
        },
        span,
    );
    let unwrap_ast = AstExpr::new(
        ExprKind::MethodCall {
            receiver: read_ast,
            method: "unwrap".to_string(),
            args: Vec::new(),
            trait_hint: None,
        },
        span,
    );
    let parse_ast = AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(
                ExprKind::Path(vec!["json".to_string(), "parse".to_string()]),
                span,
            ),
            args: vec![unwrap_ast],
            type_args: type_args.to_vec(),
        },
        span,
    );
    infer_expr(ctx, &parse_ast)
}
