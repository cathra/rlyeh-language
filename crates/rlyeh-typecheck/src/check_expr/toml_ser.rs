//! 表达式检查子模块：toml_ser。
//! （由 toml.rs 二次拆分而来，保持语义等价）

use super::*;

pub(crate) fn check_toml_stringify(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "toml.stringify".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let (_, ty) = infer_expr(ctx, &args[0])?;
    let text_ast = toml_serialize_ast(ctx, &ty, &args[0], span, true)?;
    let (hir, _) = infer_expr(ctx, &text_ast)?;
    Ok((hir, Type::Named("String".to_string(), Vec::new())))
}

/// [section]（阶段 X / X2）：判断字段类型是否为嵌套 struct（非 Vec/HashMap/String/标量）。
/// 用于顶层 TOML 序列化把嵌套 struct 字段输出为 `[section]` 行式子表；
/// 亦用于反序列化（toml.rs）按 section 分派字段归入对应嵌套 struct。
pub(crate) fn is_nested_struct_type(ctx: &TypeContext, ty: &Type) -> bool {
    if let Type::Named(ftname, fargs) = ty {
        if !fargs.is_empty() {
            return false;
        }
        let full = ctx
            .resolve_full_name(ftname)
            .unwrap_or_else(|| ftname.clone());
        if full == "Vec" || full == "HashMap" || full == "String" {
            return false;
        }
        return ctx.lookup_struct(ftname).is_some();
    }
    false
}

pub(crate) fn toml_serialize_ast(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    span: Span,
    top_level: bool,
) -> Result<AstExpr, TypeError> {
    // X2（多级 [section]，2026-08-27）：`sec_path` 记录当前 [section] 路径栈
    //（顶层嵌套 struct 字段逐层 push，section 头用 `[a.b]` 点号路径）。
    let mut sec_path: Vec<String> = Vec::new();
    toml_serialize_ast_path(ctx, ty, arg, span, top_level, &mut sec_path)
}

/// 带 [section] 路径栈的序列化（多级嵌套支持）。`toml_serialize_ast` 的递归内核。
pub(crate) fn toml_serialize_ast_path(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    span: Span,
    top_level: bool,
    sec_path: &mut Vec<String>,
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
        // X2（2026-08-30）：f64 → `float_to_string(arg)`（裸浮点，无引号）
        Type::F64 => Ok(mk_ident_call("float_to_string".to_string(), vec![arg.clone()], span)),
        // String → 含换行(10)序列化为 TOML 多行字符串 `"""...""`（原始，不转义），
        // 否则 `"` + json_escape(s) + `"`（TOML 基本转义与 JSON 一致，复用）
        Type::Named(n, _) if n == "String" => {
            let quote = |s: &str| string_from_lit_ast(s.to_string(), span);
            let escaped = fold_add(
                vec![
                    quote("\""),
                    mk_ident_call("json_escape".to_string(), vec![arg.clone()], span),
                    quote("\""),
                ],
                span,
            );
            let raw = fold_add(
                vec![quote("\"\"\""), arg.clone(), quote("\"\"\"")],
                span,
            );
            let mcall = |recv: AstExpr, method: &str, args: Vec<AstExpr>| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: method.to_string(),
                        args,
                        protocol_hint: None,
                    },
                    span,
                )
            };
            let has_nl = mcall(
                arg.clone(),
                "find",
                vec![string_from_lit_ast("\n".to_string(), span)],
            );
            let cond = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![has_nl, AstExpr::new(ExprKind::IntLiteral(0), span)],
                    operators: vec![CompareOp::Ge],
                },
                span,
            );
            Ok(AstExpr::new(
                ExprKind::If {
                    cond,
                    then_block: AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(raw),
                        span,
                    },
                    else_block: Some(AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(escaped),
                        span,
                    }),
                },
                span,
            ))
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
                parts.push(toml_serialize_ast_path(ctx, elem, &idx, span, false, sec_path)?);
            }
            parts.push(string_from_lit_ast("]".to_string(), span));
            Ok(fold_add(parts, span))
        }
        // Vec<T> → while 循环构建 `[e0,e1,...]`（须置于 struct 分支之前——Vec 是 std struct）
        Type::Named(n, args) if n == "Vec" && args.len() == 1 => {
            let elem_ty = substitute(&args[0], &ctx.generic_subst);
            if matches!(elem_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "toml.stringify：Vec 元素类型未确定（如 `let v: Vec<i64> = vec![...]` 注解）"
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
            // __o.push_str(<元素递归>)
            let idx = AstExpr::new(
                ExprKind::Index {
                    expr: arg.clone(),
                    index: i_id.clone(),
                },
                span,
            );
            let elem_ast = toml_serialize_ast_path(ctx, &elem_ty, &idx, span, false, sec_path)?;
            loop_stmts.push(AstStmt::Semi(push(out_id.clone(), elem_ast)));
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
        // HashMap<K, V> → 内联表 `{"k" = v, ...}`（键带引号，TOML 合法；值递归）。
        // desugar 为块表达式 + `for (k, v) in m`（check_for_hashmap 槽位遍历）：
        // `{ let mut __o = String::new(); let mut __first = 1; __o.push_str("{");
        //    for (k, v) in m {
        //      if __first > 0 { __first = 0; } else { __o.push_str(","); }
        //      __o.push_str(<键>); __o.push_str(" = "); __o.push_str(<值>);
        //    }
        //    __o.push_str("}"); __o }`
        // 键：i64 → `"` + int_to_string(k) + `"`；String → `"` + json_escape(k) + `"`。
        // 值：递归 `toml_serialize_ast`（top_level=false）。须置于 struct 分支之前（同 json）。
        Type::Named(n, args) if n == "HashMap" && args.len() == 2 => {
            let k_ty = substitute(&args[0], &ctx.generic_subst);
            let v_ty = substitute(&args[1], &ctx.generic_subst);
            if matches!(k_ty, Type::Infer) || matches!(v_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "toml.stringify：HashMap 键/值类型未确定（如 `let m: HashMap<i64, i64> = map![...]` 注解定型）".to_string(),
                    span,
                });
            }
            let key_is_i64 = matches!(k_ty, Type::I64);
            let key_is_string = matches!(&k_ty, Type::Named(kn, _) if kn == "String");
            if !key_is_i64 && !key_is_string {
                return Err(TypeError::Unsupported {
                    what: format!("toml.stringify：HashMap 键类型 `{k_ty}`（MVP 支持 i64 / String）"),
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
            // 键序列化：i64 → `"` + int_to_string(k) + `"`；String → json_escape(k)
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
                // X4（2026-09-21）：String 键须显式加引号。`json_escape` 仅做转义、
                // 不自带引号（此前注释「自带引号」有误），故须与 i64 分支一致地
                // `"` + json_escape(k) + `"`，产出合法内联表键 `{"k" = v}`。
                fold_add(
                    vec![
                        string_from_lit_ast("\"".to_string(), span),
                        mk_ident_call("json_escape".to_string(), vec![k_id.clone()], span),
                        string_from_lit_ast("\"".to_string(), span),
                    ],
                    span,
                )
            };
            let val_ser = toml_serialize_ast_path(ctx, &v_ty, &v_id, span, false, sec_path)?;
            // `if __first > 0 { __first = 0 } else { __o.push_str(",") }`
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
            let mut loop_stmts = vec![AstStmt::Semi(AstExpr::new(
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
            ))];
            // __o.push_str(<键>); __o.push_str(" = "); __o.push_str(<值>)
            loop_stmts.push(AstStmt::Semi(push(
                out_id.clone(),
                key_ser,
            )));
            loop_stmts.push(AstStmt::Semi(push(
                out_id.clone(),
                string_from_lit_ast(" = ".to_string(), span),
            )));
            loop_stmts.push(AstStmt::Semi(push(out_id.clone(), val_ser)));
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Tuple(
                        vec![
                            AstPattern::Ident(k_name),
                            AstPattern::Ident(v_name),
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
            let body_stmts = vec![
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
            ];
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: body_stmts,
                    final_expr: Some(out_id),
                    span,
                }),
                span,
            ))
        }
        // 结构体（嵌套递归）。
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
            if def.fields.is_empty() {
                // 顶层空结构体 → 空文本；嵌套空结构体 → `{}` 内联表
                if top_level {
                    return Ok(mk_path_call(
                        vec!["String".to_string(), "new".to_string()],
                        Vec::new(),
                        span,
                    ));
                }
                return Ok(string_from_lit_ast("{}".to_string(), span));
            }
            // 字段访问 AST：`arg.field`
            let field_arg = |fname: &str| {
                AstExpr::new(
                    ExprKind::FieldAccess {
                        expr: arg.clone(),
                        field: fname.to_string(),
                    },
                    span,
                )
            };
            // 顶层：多行 `f1 = v1\nf2 = v2`（字段序 = 定义序）；嵌套：内联表 `{f1 = v1,f2 = v2}`。
            // X2（2026-08-27）：标准 TOML `key = value`（`=` 两侧空格）；顶层嵌套 struct
            // 字段输出标准 `[section]` 行式子表（`\n[point]\nx = 7\ny = 9`）。
            let mut parts = if top_level {
                Vec::new()
            } else {
                vec![string_from_lit_ast("{".to_string(), span)]
            };
            for (i, (fname, fty_ast)) in def.fields.iter().enumerate() {
                // 顶层 + 字段为嵌套 struct（非 Vec/HashMap/String）→ [section] 行式
                let is_section = top_level && is_nested_struct_type(&*ctx, fty_ast);
                if is_section {
                    // 多级路径：`[a.b]`（递归 push 父字段名，section 头用点号路径）
                    sec_path.push(fname.clone());
                    parts.push(string_from_lit_ast(
                        format!("\n[{}]\n", sec_path.join(".")),
                        span,
                    ));
                    let farg = field_arg(fname);
                    parts.push(toml_serialize_ast_path(ctx, fty_ast, &farg, span, true, sec_path)?);
                    sec_path.pop();
                } else {
                    if i > 0 {
                        let sep = if top_level { "\n" } else { "," };
                        parts.push(string_from_lit_ast(sep.to_string(), span));
                    }
                    // X2（2026-08-27）：标准 TOML `key = value`（`=` 两侧空格）
                    parts.push(string_from_lit_ast(format!("{fname} = "), span));
                    let farg = field_arg(fname);
                    parts.push(toml_serialize_ast_path(ctx, fty_ast, &farg, span, false, sec_path)?);
                }
            }
            if !top_level {
                parts.push(string_from_lit_ast("}".to_string(), span));
            }
            Ok(fold_add(parts, span))
        }
        other => Err(TypeError::Unsupported {
            what: format!("toml.stringify：类型 `{other}` 序列化"),
            span,
        }),
    }
}
