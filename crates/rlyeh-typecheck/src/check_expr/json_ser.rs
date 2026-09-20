//! 表达式检查子模块：json_ser。
//! （由 json.rs 二次拆分而来，保持语义等价）

use super::*;

pub(crate) fn check_json_stringify(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "json.stringify".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    // 预推断参数类型（供递归 desugar 使用）
    let (_, ty) = infer_expr(ctx, &args[0])?;
    let text_ast = json_serialize_ast(ctx, &ty, &args[0], span)?;
    let (hir, _) = infer_expr(ctx, &text_ast)?;
    Ok((hir, Type::Named("String".to_string(), Vec::new())))
}

pub(crate) fn json_serialize_ast(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    span: Span,
) -> Result<AstExpr, TypeError> {
    match ty {
        // i64 → `int_to_string(x)`（std）
        Type::I64 => Ok(mk_ident_call(
            "int_to_string".to_string(),
            vec![arg.clone()],
            span,
        )),
        // bool → `if b { "true" } else { "false" }`
        Type::Bool => {
            let mk = |s: &str| string_from_lit_ast(s.to_string(), span);
            Ok(AstExpr::new(
                ExprKind::If {
                    cond: arg.clone(),
                    then_block: AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(mk("true")),
                        span,
                    },
                    else_block: Some(AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(mk("false")),
                        span,
                    }),
                },
                span,
            ))
        }
        // String → `"` + json_escape(s) + `"`（json_escape 在标准库预置）
        Type::Named(n, _) if n == "String" => {
            let quote = |s: &str| string_from_lit_ast(s.to_string(), span);
            let esc = mk_ident_call("json_escape".to_string(), vec![arg.clone()], span);
            Ok(fold_add(vec![quote("\""), esc, quote("\"")], span))
        }
        // &str / 字符串字面量 → `"` + json_escape(String::from(arg)) + `"`
        Type::Str => {
            let quote = |s: &str| string_from_lit_ast(s.to_string(), span);
            let sf = mk_path_call(
                vec!["String".to_string(), "from".to_string()],
                vec![arg.clone()],
                span,
            );
            let esc = mk_ident_call("json_escape".to_string(), vec![sf], span);
            Ok(fold_add(vec![quote("\""), esc, quote("\"")], span))
        }
        // 数组 `[T; N]`：静态展开 `[e0,e1,...]`（长度编译期已知）
        Type::Array(elem, len) => {
            let mut parts = vec![string_from_lit_ast("[".to_string(), span)];
            for i in 0..*len {
                if i > 0 {
                    parts.push(string_from_lit_ast(",".to_string(), span));
                }
                let idx = AstExpr::new(
                    ExprKind::Index {
                        expr: arg.clone(),
                        index: AstExpr::new(ExprKind::IntLiteral(i as i128), span),
                    },
                    span,
                );
                parts.push(json_serialize_ast(ctx, elem, &idx, span)?);
            }
            parts.push(string_from_lit_ast("]".to_string(), span));
            Ok(fold_add(parts, span))
        }
        // Vec<T> → while 循环构建 `[e0,e1,...]`：
        // 注意：必须置于 struct 分支之前——`Vec` 本身是 std struct（data/len/cap 字段），
        // 若先命中 lookup_struct 分支会被误序列化为 `{"data":...,"len":...,"cap":...}`。
        // `{ let mut __o = String::new(); __o.push_str("["); let mut __i = 0;
        //    while __i < v.len() { if __i > 0 { __o.push_str(","); }
        //                           __o.push_str(json.stringify(v[__i])); __i = __i + 1; }
        //    __o.push_str("]"); __o }`
        Type::Named(n, args) if n == "Vec" && args.len() == 1 => {
            // 元素类型定型：`vec![...]` 字面量绑定后为 `Vec<Infer>`（push 不反向精化接收者
            // 类型），Infer 无法确定元素序列化路径——与 check_for_vec 一致，要求上下文 /
            // 显式注解定型（如 `let v: Vec<i64> = vec![1, 2, 3]`）。
            let elem_ty = substitute(&args[0], &ctx.generic_subst);
            if matches!(elem_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "json.stringify：Vec 元素类型未确定（如 `let v: Vec<i64> = vec![...]` 注解）"
                        .to_string(),
                    span,
                });
            }
            let out_name = ctx.fresh_temp();
            let i_name = ctx.fresh_temp();
            let out_id = AstExpr::new(ExprKind::Ident(out_name.clone()), span);
            let i_id = AstExpr::new(ExprKind::Ident(i_name.clone()), span);
            let push = |recv: AstExpr, val: AstExpr| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: "push_str".to_string(),
                        args: vec![val],
                        protocol_hint: None,
                    },
                    span,
                )
            };
            let mut body_stmts = vec![
                AstStmt::Let {
                    pattern: AstPattern::Ident(out_name),
                    type_anno: None,
                    init: mk_path_call(
                        vec!["String".to_string(), "new".to_string()],
                        Vec::new(),
                        span,
                    ),
                    mutable: true,
                },
                AstStmt::Let {
                    pattern: AstPattern::Ident(i_name),
                    type_anno: None,
                    init: AstExpr::new(ExprKind::IntLiteral(0), span),
                    mutable: true,
                },
                AstStmt::Semi(push(out_id.clone(), string_from_lit_ast("[".to_string(), span))),
            ];
            // while __i < v.len()
            let len_call = AstExpr::new(
                ExprKind::MethodCall {
                    receiver: arg.clone(),
                    method: "len".to_string(),
                    args: Vec::new(),
                    protocol_hint: None,
                },
                span,
            );
            let cond = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![i_id.clone(), len_call],
                    operators: vec![CompareOp::Lt],
                },
                span,
            );
            let mut loop_stmts = Vec::new();
            // if __i > 0 { __o.push_str(",") }
            let gt_zero = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![
                        i_id.clone(),
                        AstExpr::new(ExprKind::IntLiteral(0), span),
                    ],
                    operators: vec![CompareOp::Gt],
                },
                span,
            );
            loop_stmts.push(AstStmt::Semi(AstExpr::new(
                ExprKind::If {
                    cond: gt_zero,
                    then_block: AstBlock {
                        stmts: vec![AstStmt::Semi(push(
                            out_id.clone(),
                            string_from_lit_ast(",".to_string(), span),
                        ))],
                        final_expr: None,
                        span,
                    },
                    else_block: None,
                },
                span,
            )));
            // __o.push_str(json.stringify(v[__i]))
            let idx = AstExpr::new(
                ExprKind::Index {
                    expr: arg.clone(),
                    index: i_id.clone(),
                },
                span,
            );
            let json_elem = mk_path_call(
                vec!["json".to_string(), "stringify".to_string()],
                vec![idx],
                span,
            );
            loop_stmts.push(AstStmt::Semi(push(out_id.clone(), json_elem)));
            // __i = __i + 1
            loop_stmts.push(AstStmt::Semi(AstExpr::new(
                ExprKind::Assign {
                    target: i_id.clone(),
                    op: AssignOp::Assign,
                    value: bin_add(
                        i_id.clone(),
                        AstExpr::new(ExprKind::IntLiteral(1), span),
                        span,
                    ),
                },
                span,
            )));
            body_stmts.push(AstStmt::Semi(AstExpr::new(
                ExprKind::While {
                    cond,
                    body: AstBlock {
                        stmts: loop_stmts,
                        final_expr: None,
                        span,
                    },
                },
                span,
            )));
            body_stmts.push(AstStmt::Semi(push(
                out_id.clone(),
                string_from_lit_ast("]".to_string(), span),
            )));
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: body_stmts,
                    final_expr: Some(out_id),
                    span,
                }),
                span,
            ))
        }
        // HashMap<K, V> → `{"k":v,...}`（键转 JSON 字符串键；值递归）。
        // desugar 为块表达式 + `for (k, v) in m`（check_for_hashmap 槽位遍历）：
        // `{ let mut __o = String::new(); let mut __first = 1; __o.push_str("{");
        //    for (k, v) in m {
        //      if __first > 0 { __first = 0; } else { __o.push_str(","); }
        //      __o.push_str(<键>); __o.push_str(":"); __o.push_str(<值>);
        //    }
        //    __o.push_str("}"); __o }`
        // 键：i64 → `"` + int_to_string(k) + `"`；String → `json.stringify(k)`（自带引号 + 转义）。
        // 值：递归 `json_serialize_ast`（支持嵌套 HashMap / Vec / struct / 数组）。
        // 注意：须置于 struct 分支之前——HashMap 本身是 std struct（keys/vals/states 槽），
        // 若先命中 lookup_struct 分支会被误序列化为 `{"keys":...,"vals":...}`。
        Type::Named(n, args) if n == "HashMap" && args.len() == 2 => {
            let k_ty = substitute(&args[0], &ctx.generic_subst);
            let v_ty = substitute(&args[1], &ctx.generic_subst);
            if matches!(k_ty, Type::Infer) || matches!(v_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "json.stringify：HashMap 键/值类型未确定（如 `let m: HashMap<i64, i64> = map![...]` 注解定型）".to_string(),
                    span,
                });
            }
            let key_is_i64 = matches!(k_ty, Type::I64);
            let key_is_string = matches!(&k_ty, Type::Named(kn, _) if kn == "String");
            if !key_is_i64 && !key_is_string {
                return Err(TypeError::Unsupported {
                    what: format!("json.stringify：HashMap 键类型 `{k_ty}`（MVP 支持 i64 / String）"),
                    span,
                });
            }
            let out_name = ctx.fresh_temp();
            let first_name = ctx.fresh_temp();
            let k_name = ctx.fresh_temp();
            let v_name = ctx.fresh_temp();
            let out_id = AstExpr::new(ExprKind::Ident(out_name.clone()), span);
            let first_id = AstExpr::new(ExprKind::Ident(first_name.clone()), span);
            let k_id = AstExpr::new(ExprKind::Ident(k_name.clone()), span);
            let v_id = AstExpr::new(ExprKind::Ident(v_name.clone()), span);
            let push = |recv: AstExpr, val: AstExpr| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: "push_str".to_string(),
                        args: vec![val],
                        protocol_hint: None,
                    },
                    span,
                )
            };
            // 键序列化：i64 → `"` + int_to_string(k) + `"`；String → `json.stringify(k)`
            let key_ser = if key_is_i64 {
                fold_add(
                    vec![
                        string_from_lit_ast("\"".to_string(), span),
                        mk_ident_call("int_to_string".to_string(), vec![k_id.clone()], span),
                        string_from_lit_ast("\"".to_string(), span),
                    ],
                    span,
                )
            } else {
                mk_path_call(
                    vec!["json".to_string(), "stringify".to_string()],
                    vec![k_id.clone()],
                    span,
                )
            };
            // 值序列化（递归）
            let val_ser = json_serialize_ast(ctx, &v_ty, &v_id, span)?;
            let mut loop_stmts = Vec::new();
            // if __first > 0 { __first = 0 } else { __o.push_str(",") }
            let first_gt_zero = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![
                        first_id.clone(),
                        AstExpr::new(ExprKind::IntLiteral(0), span),
                    ],
                    operators: vec![CompareOp::Gt],
                },
                span,
            );
            loop_stmts.push(AstStmt::Semi(AstExpr::new(
                ExprKind::If {
                    cond: first_gt_zero,
                    then_block: AstBlock {
                        stmts: vec![AstStmt::Semi(AstExpr::new(
                            ExprKind::Assign {
                                target: first_id.clone(),
                                op: AssignOp::Assign,
                                value: AstExpr::new(ExprKind::IntLiteral(0), span),
                            },
                            span,
                        ))],
                        final_expr: None,
                        span,
                    },
                    else_block: Some(AstBlock {
                        stmts: vec![AstStmt::Semi(push(
                            out_id.clone(),
                            string_from_lit_ast(",".to_string(), span),
                        ))],
                        final_expr: None,
                        span,
                    }),
                },
                span,
            )));
            // __o.push_str(<键>); __o.push_str(":"); __o.push_str(<值>)
            loop_stmts.push(AstStmt::Semi(push(out_id.clone(), key_ser)));
            loop_stmts.push(AstStmt::Semi(push(
                out_id.clone(),
                string_from_lit_ast(":".to_string(), span),
            )));
            loop_stmts.push(AstStmt::Semi(push(out_id.clone(), val_ser)));
            // for (k, v) in m { ... }
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Tuple(
                        vec![
                            AstPattern::Ident(k_name.clone()),
                            AstPattern::Ident(v_name.clone()),
                        ],
                        Span::dummy(),
                    ),
                    iterator: arg.clone(),
                    body: AstBlock {
                        stmts: loop_stmts,
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
                            pattern: AstPattern::Ident(out_name),
                            type_anno: None,
                            init: mk_path_call(
                                vec!["String".to_string(), "new".to_string()],
                                Vec::new(),
                                span,
                            ),
                            mutable: true,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(first_name),
                            type_anno: None,
                            init: AstExpr::new(ExprKind::IntLiteral(1), span),
                            mutable: true,
                        },
                        AstStmt::Semi(push(
                            out_id.clone(),
                            string_from_lit_ast("{".to_string(), span),
                        )),
                        AstStmt::Semi(for_expr),
                        AstStmt::Semi(push(
                            out_id.clone(),
                            string_from_lit_ast("}".to_string(), span),
                        )),
                    ],
                    final_expr: Some(out_id),
                    span,
                }),
                span,
            ))
        }
        // 结构体 → `{"f1":v1,"f2":v2}`（字段序 = 定义序）。
        // guard 排除 Vec / HashMap（两者是 std struct 但各有专用分支，须先于本分支命中）。
        Type::Named(name, _)
            if ctx.lookup_struct(name).is_some()
                && ctx
                    .resolve_full_name(name)
                    .unwrap_or_else(|| name.clone())
                    != "Vec"
                && ctx
                    .resolve_full_name(name)
                    .unwrap_or_else(|| name.clone())
                    != "HashMap" =>
        {
            let def = ctx.lookup_struct(name).cloned().ok_or_else(|| {
                TypeError::UndefinedType {
                    name: name.clone(),
                    span,
                }
            })?;
            let mut parts = Vec::new();
            for (i, (fname, fty_ast)) in def.fields.iter().enumerate() {
                let prefix = if i == 0 {
                    format!("{{\"{}\":", fname)
                } else {
                    format!(",\"{}\":", fname)
                };
                parts.push(string_from_lit_ast(prefix, span));
                let farg = AstExpr::new(
                    ExprKind::FieldAccess {
                        expr: arg.clone(),
                        field: fname.clone(),
                    },
                    span,
                );
                parts.push(json_serialize_ast(ctx, fty_ast, &farg, span)?);
            }
            parts.push(string_from_lit_ast("}".to_string(), span));
            Ok(fold_add(parts, span))
        }
        other => Err(TypeError::Unsupported {
            what: format!("json.stringify：类型 `{other}` 序列化"),
            span,
        }),
    }
}
