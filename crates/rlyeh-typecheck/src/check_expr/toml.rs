//! 表达式检查子模块：toml。
//! （由 macro_serialize.rs 二次拆分而来，保持语义等价）

use super::*;

// 入口检查 / 严格解析与字段分派构造已按簇下沉到子模块
// （文件大小约束：单个文件 ≤1000 行）。`check_toml_*` 对外路径经此 re-export 不变。
mod build;
mod try_parse;

pub(crate) use try_parse::{check_toml_parse, check_toml_try_parse};

use build::*;


pub(crate) fn toml_parse_ast(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    span: Span,
    inline: bool,
) -> Result<AstExpr, TypeError> {
    // 统一实参转 String（TOML 文本实参可为字符串字面量 / String / &str）
    let s = mk_path_call(
        vec!["String".to_string(), "from".to_string()],
        vec![arg.clone()],
        span,
    );
    match ty {
        // i64 → `string_to_int(s)`（std；数字文本无引号，MVP 直接解析）
        Type::I64 => Ok(mk_ident_call(
            "string_to_int".to_string(),
            vec![s],
            span,
        )),
        // bool → `if s == "true" { true } else { false }`
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
        // String → `json_unescape(s)`（引号剥离 + 转义还原，标准库预置）
        // 字符串 → 多行字符串 `"""` 感知（`toml_string_value_ast`）
        Type::Named(n, _) if n == "String" => Ok(toml_string_value_ast(&s, span)),
        // X2（2026-08-30）：f64 → `string_to_float(s)`
        Type::F64 => Ok(mk_ident_call("string_to_float".to_string(), vec![s], span)),
        // X2（2026-08-30）：固定数组 `[T; N]` → 生成 `[parse(parts[0]), ..., parse(parts[N-1])]`
        // （N 编译期已知；parts = 剥 `[]` 后引号感知分段，复用 split_quoted 处理元素含逗号字符串）
        Type::Array(elem_ty, n) => {
            let elem_ty = substitute(&elem_ty, &ctx.generic_subst);
            if matches!(elem_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "toml.parse：固定数组元素类型未确定（如 `toml::from_str::<[i64; 3]>(s)` 需标注类型）".to_string(),
                    span,
                });
            }
            if *n == 0 {
                return Err(TypeError::Unsupported {
                    what: "toml.parse：空固定数组 `[T; 0]` 反序列化未支持".to_string(),
                    span,
                });
            }
            let s_name = ctx.fresh_temp();
            let body_name = ctx.fresh_temp();
            let parts_name = ctx.fresh_temp();
            let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
            let body_id = AstExpr::new(ExprKind::Ident(body_name.clone()), span);
            let parts_id = AstExpr::new(ExprKind::Ident(parts_name.clone()), span);
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
            let mut elems: Vec<AstExpr> = Vec::with_capacity(*n);
            for i in 0..*n {
                let part_i = AstExpr::new(
                    ExprKind::Index {
                        expr: parts_id.clone(),
                        index: AstExpr::new(ExprKind::IntLiteral(i as i128), span),
                    },
                    span,
                );
                let elem_ast = toml_parse_ast(ctx, &elem_ty, &part_i, span, false)?;
                elems.push(elem_ast);
            }
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(s_name),
                            type_anno: None,
                            init: s,
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
                                            left: mcall(s_id.clone(), "len", Vec::new()),
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
                            init: mk_ident_call(
                                "split_quoted".to_string(),
                                vec![body_id, AstExpr::new(ExprKind::IntLiteral(44), span)],
                                span,
                            ),
                            mutable: false,
                        },
                    ],
                    final_expr: Some(AstExpr::new(ExprKind::ArrayLit(elems), span)),
                    span,
                }),
                span,
            ))
        }
        // Vec<T> → 数组 `[e1,e2]` 反序列化（元素限标量 i64 / bool / String）。
        // desugar 为块表达式 + `for x in vec`：
        // `{ let __s = String::from(<arg>);                 // TOML 文本
        //    let __body = __s.substring(1, __s.len() - 1);  // 剥离首尾 [ ]
        //    let __parts = __body.split(",");               // 逗号分段
        //    let mut __v: Vec<T> = Vec::new();              // 注解定型（空数组 [] 亦定型）
        //    for __part in __parts {
        //      let __e = <元素递归 toml_parse_ast>;
        //      __v.push(__e);
        //    }
        //    __v }`
        Type::Named(n, args) if n == "Vec" && args.len() == 1 => {
            let elem_ty = substitute(&args[0], &ctx.generic_subst);
            if matches!(elem_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "toml.parse：Vec 元素类型未确定（turbofish 显式定型，如 `toml.parse::<Vec<i64>>(s)`）".to_string(),
                    span,
                });
            }
            let s_name = ctx.fresh_temp();
            let body_name = ctx.fresh_temp();
            let parts_name = ctx.fresh_temp();
            let v_name = ctx.fresh_temp();
            let part_name = ctx.fresh_temp();
            let e_name = ctx.fresh_temp();
            let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
            let body_id = AstExpr::new(ExprKind::Ident(body_name.clone()), span);
            let parts_id = AstExpr::new(ExprKind::Ident(parts_name.clone()), span);
            let v_id = AstExpr::new(ExprKind::Ident(v_name.clone()), span);
            let part_id = AstExpr::new(ExprKind::Ident(part_name.clone()), span);
            let e_id = AstExpr::new(ExprKind::Ident(e_name.clone()), span);
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
            let elem_parse = toml_parse_ast(ctx, &elem_ty, &part_id.clone(), span, false)?;
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Ident(part_name),
                    iterator: parts_id.clone(),
                    body: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(e_name),
                                type_anno: None,
                                init: elem_parse,
                                mutable: false,
                            },
                            AstStmt::Semi(AstExpr::new(
                                ExprKind::MethodCall {
                                    receiver: v_id.clone(),
                                    method: "push".to_string(),
                                    args: vec![e_id],
                                    protocol_hint: None,
                                },
                                span,
                            )),
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
                            init: s,
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
                            // X2（2026-08-27）：数组用引号感知分段（元素含逗号字符串不分割）
                            init: mk_ident_call(
                                "split_quoted".to_string(),
                                vec![
                                    body_id,
                                    AstExpr::new(ExprKind::IntLiteral(44), span), // ','
                                ],
                                span,
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(v_name),
                            type_anno: Some(rlyeh_ast::SpannedAstType {
                                ty: AstType::Path(
                                    "Vec".to_string(),
                                    vec![ty_to_ast(&elem_ty)],
                                ),
                                span: rlyeh_lexer::Span::dummy(),
                            }),
                            init: mk_path_call(
                                vec!["Vec".to_string(), "new".to_string()],
                                Vec::new(),
                                span,
                            ),
                            mutable: true,
                        },
                        AstStmt::Semi(for_expr),
                    ],
                    final_expr: Some(v_id),
                    span,
                }),
                span,
            ))
        }
        // HashMap<K, V> → 内联表 `{"k" = v, ...}` 反序列化（键/值限标量）。
        // 与 json.parse 的 HashMap 分支同构，仅分隔符 `:` → ` = ` 与键剥引号路径一致：
        // `{ let __s = String::from(<arg>);
        //    let __body = __s.substring(1, __s.len() - 1);  // 剥离首尾 { }
        //    let __parts = __body.split(",");
        //    let mut __m: HashMap<K, V> = HashMap::new();
        //    for __part in __parts {
        //      let __c = __part.find(" = ");               // 等号分隔（键/值含等号 MVP 限制）
        //      if __c >= 0 {
        //        let __kpart = __part.substring(0, __c);
        //        let __vpart = __part.substring(__c + 1, __part.len());
        //        let __k = <键解析>;                        // i64: string_to_int(json_unescape(kpart))
        //                                                    // String: json_unescape(kpart)
        //        let __v = <值解析，递归标量>;
        //        __m.insert(__k, __v);
        //      }
        //    }
        //    __m }`
        // 注意：stringify 键为带引号（`"1"` / `"a"`）——键段先 json_unescape 剥引号，
        // i64 再经 string_to_int 转整数。须置于 struct 分支之前（同 stringify）。
        Type::Named(n, args) if n == "HashMap" && args.len() == 2 => {
            let k_ty = substitute(&args[0], &ctx.generic_subst);
            let v_ty = substitute(&args[1], &ctx.generic_subst);
            if matches!(k_ty, Type::Infer) || matches!(v_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "toml.parse：HashMap 键/值类型未确定（turbofish 显式定型，如 `toml.parse::<HashMap<i64, i64>>(s)`）".to_string(),
                    span,
                });
            }
            let key_is_i64 = matches!(k_ty, Type::I64);
            let key_is_string = matches!(&k_ty, Type::Named(kn, _) if kn == "String");
            if !key_is_i64 && !key_is_string {
                return Err(TypeError::Unsupported {
                    what: format!("toml.parse：HashMap 键类型 `{k_ty}`（MVP 支持 i64 / String）"),
                    span,
                });
            }
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
            // 键解析：i64 → `string_to_int(json_unescape(kpart))`；String → `json_unescape(kpart)`
            let k_unescaped = mk_ident_call("json_unescape".to_string(), vec![kpart_id.clone()], span);
            let key_parse = if key_is_i64 {
                mk_ident_call("string_to_int".to_string(), vec![k_unescaped], span)
            } else {
                k_unescaped
            };
            let val_parse = toml_parse_ast(ctx, &v_ty, &vpart_id.clone(), span, false)?;
            let if_parse = AstExpr::new(
                ExprKind::If {
                    cond: AstExpr::new(
                        ExprKind::ComparisonChain {
                            elements: vec![c_id.clone(), AstExpr::new(ExprKind::IntLiteral(0), span)],
                            operators: vec![CompareOp::Ge],
                        },
                        span,
                    ),
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
                                                right: AstExpr::new(ExprKind::IntLiteral(1), span),
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
                            AstStmt::Semi(AstExpr::new(
                                ExprKind::MethodCall {
                                    receiver: m_id.clone(),
                                    method: "insert".to_string(),
                                    args: vec![k_id, v_id],
                                    protocol_hint: None,
                                },
                                span,
                            )),
                        ],
                        final_expr: None,
                        span,
                    },
                    else_block: None,
                },
                span,
            );
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
                                    vec![string_from_lit_ast("=".to_string(), span)],
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
                            init: s,
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
                            // X2（2026-08-27）：HashMap 内联表用引号感知分段（值含逗号字符串不分割）
                            init: mk_ident_call(
                                "split_quoted".to_string(),
                                vec![
                                    body_id,
                                    AstExpr::new(ExprKind::IntLiteral(44), span), // ','
                                ],
                                span,
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
        // 结构体 → 顶层多行 `key = value` / 嵌套内联表 `{ ... }` 反序列化。
        // 字段序 = 定义序；缺失字段保持零值，未知字段忽略。desugar 为块表达式 +
        // `for x in vec`（复用 check_for_vec 遍历 split 结果）：
        // `{ let __s = String::from(<arg>);
        //    let __body = <inline ? __s.substring(1, __s.len() - 1) : __s>;  // 内联表剥 { }
        //    let __parts = __body.split(<inline ? "," : "\n">);
        //    let mut __p: Point = Point { x: 0, y: 0 };     // 零值构造
        //    for __part in __parts {
        //      let __c = __part.find("=");
        //      if __c >= 0 {
        //        let __name = __part.substring(0, __c);      // 裸键，直接比较
        //        let __val = __part.substring(__c + 1, __part.len());
        //        if __name == "x" { __p.x = <解析>; } else if ... else { }
        //      }
        //    }
        //    __p }`
        // 字段值限 toml_parse_ast 支持类型（i64 / bool / String / Vec / 嵌套 struct /
        // HashMap）；嵌套 struct / HashMap 值为内联表 `{...}`（含逗号经 split 分段错误
        // 的 MVP 限制与 json 一致）；泛型 struct MVP 不支持（与 json 一致）。
        Type::Named(n, args) if ctx.lookup_struct(&n).is_some() && args.is_empty() => {
            let def = ctx.lookup_struct(&n).cloned().unwrap();
            if def.fields.is_empty() {
                return Err(TypeError::Unsupported {
                    what: format!("toml.parse：空结构体 `{n}` 反序列化"),
                    span,
                });
            }
            let s_name = ctx.fresh_temp();
            let body_name = ctx.fresh_temp();
            let parts_name = ctx.fresh_temp();
            let p_name = ctx.fresh_temp();
            let part_name = ctx.fresh_temp();
            let c_name = ctx.fresh_temp();
            let name_name = ctx.fresh_temp();
            let val_name = ctx.fresh_temp();
            let cur_sec_name = ctx.fresh_temp();
            let cur_sec_id = AstExpr::new(ExprKind::Ident(cur_sec_name.clone()), span);
            let cbr_name = ctx.fresh_temp();
            let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
            let body_id = AstExpr::new(ExprKind::Ident(body_name.clone()), span);
            let parts_id = AstExpr::new(ExprKind::Ident(parts_name.clone()), span);
            let p_id = AstExpr::new(ExprKind::Ident(p_name.clone()), span);
            let part_id = AstExpr::new(ExprKind::Ident(part_name.clone()), span);
            let c_id = AstExpr::new(ExprKind::Ident(c_name.clone()), span);
            let name_id = AstExpr::new(ExprKind::Ident(name_name.clone()), span);
            let val_id = AstExpr::new(ExprKind::Ident(val_name.clone()), span);
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
            // 字段值解析（递归 toml_parse_ast，inline=true——嵌套值可能为内联表）
            let field_parse = |ctx: &mut TypeContext, fty: &Type| {
                toml_parse_ast(ctx, fty, &val_id.clone(), span, true)
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
                    base: None,
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
            // X2（[section] 完整实现，2026-08-27）：按 `__cur_sec` 分派字段——
            // 顶层（`__cur_sec.len() == 0`）走 `if_chain`；每个嵌套 struct 字段
            // 一个 `else if __cur_sec == "point"` 分支，内部再做该 struct 的字段分派。
            // 仅顶层多行（非内联）需要 section 分派；内联表走原 `if_chain`。
            let dispatch: AstExpr = if inline {
                if_chain
            } else {
                // 多级 [section]：递归生成所有嵌套 struct 路径的 section 分支
                let mut else_chain = build_section_branches(
                    ctx, &def.fields, &[], &p_id, &cur_sec_id, &name_id, &val_id, span, None,
                );
                // 顶层：if __cur_sec.len() == 0 { if_chain } else { <section 分支> }
                AstExpr::new(
                    ExprKind::If {
                        cond: AstExpr::new(
                            ExprKind::ComparisonChain {
                                elements: vec![
                                    AstExpr::new(
                                        ExprKind::MethodCall {
                                            receiver: cur_sec_id.clone(),
                                            method: "len".to_string(),
                                            args: Vec::new(),
                                            protocol_hint: None,
                                        },
                                        span,
                                    ),
                                    AstExpr::new(ExprKind::IntLiteral(0), span),
                                ],
                                operators: vec![CompareOp::Eq],
                            },
                            span,
                        ),
                        then_block: AstBlock {
                            stmts: vec![AstStmt::Semi(if_chain)],
                            final_expr: None,
                            span,
                        },
                        else_block: else_chain.take().or_else(|| Some(AstBlock {
                            stmts: Vec::new(),
                            final_expr: None,
                            span,
                        })),
                    },
                    span,
                )
            };
            // `if __c >= 0 { let __name = ...; let __val = ...; <if 链> }`
            let if_parse = AstExpr::new(
                ExprKind::If {
                    cond: AstExpr::new(
                        ExprKind::ComparisonChain {
                            elements: vec![
                                c_id.clone(),
                                AstExpr::new(ExprKind::IntLiteral(0), span),
                            ],
                            operators: vec![CompareOp::Ge],
                        },
                        span,
                    ),
                    then_block: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(name_name),
                                type_anno: None,
                                init: mcall(
                                    mcall(
                                        part_id.clone(),
                                        "substring",
                                        vec![
                                            AstExpr::new(ExprKind::IntLiteral(0), span),
                                            c_id.clone(),
                                        ],
                                    ),
                                    // X2（2026-08-27）：trim 剥离 `key = value` 键两侧空格
                                    "trim",
                                    Vec::new(),
                                ),
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(val_name),
                                type_anno: None,
                                init: mcall(
                                    mcall(
                                        part_id.clone(),
                                        "substring",
                                        vec![
                                            AstExpr::new(
                                                ExprKind::Binary {
                                                    op: BinaryOp::Add,
                                                    left: c_id.clone(),
                                                    right: AstExpr::new(ExprKind::IntLiteral(1), span),
                                                },
                                                span,
                                            ),
                                            mcall(part_id.clone(), "len", Vec::new()),
                                        ],
                                    ),
                                    // X2（2026-08-27）：trim 剥离 `key = value` 值两侧空格
                                    "trim",
                                    Vec::new(),
                                ),
                                mutable: false,
                            },
                            AstStmt::Semi(dispatch),
                        ],
                        final_expr: None,
                        span,
                    },
                    else_block: None,
                },
                span,
            );
            // X2（[section]，2026-08-27）：`[section]` 头检测——若 `__part` 以 `[` 开头，
            // 提取 `[..]` 名到 `__cur_sec`（`let __cbr = __part.find("]"); __cur_sec = __part.substring(1, __cbr)`）。
            // 仅顶层多行（非内联）需要 [section] 状态机。
            let section_detect: Option<AstStmt> = if inline {
                None
            } else {
                let set_sec = AstExpr::new(
                    ExprKind::Assign {
                        target: cur_sec_id.clone(),
                        op: AssignOp::Assign,
                        value: mcall(
                            mcall(part_id.clone(), "substring", vec![
                                AstExpr::new(ExprKind::IntLiteral(1), span),
                                AstExpr::new(ExprKind::Ident(cbr_name.clone()), span),
                            ]),
                            "trim",
                            Vec::new(),
                        ),
                    },
                    span,
                );
                let inner_block = AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(cbr_name),
                            type_anno: None,
                            init: mcall(
                                part_id.clone(),
                                "find",
                                vec![string_from_lit_ast("]".to_string(), span)],
                            ),
                            mutable: false,
                        },
                        AstStmt::Semi(set_sec),
                    ],
                    final_expr: None,
                    span,
                };
                Some(AstStmt::Semi(AstExpr::new(
                    ExprKind::If {
                        cond: AstExpr::new(
                            ExprKind::ComparisonChain {
                                elements: vec![
                                    mcall(part_id.clone(), "find", vec![string_from_lit_ast("[".to_string(), span)]),
                                    AstExpr::new(ExprKind::IntLiteral(0), span),
                                ],
                                operators: vec![CompareOp::Eq],
                            },
                            span,
                        ),
                        then_block: inner_block,
                        else_block: None,
                    },
                    span,
                )))
            };
            // `for __part in __parts { <[section] 检测> let __c = __part.find("="); <if 解析> }`
            let mut for_stmts = Vec::new();
            if let Some(sd) = section_detect {
                for_stmts.push(sd);
            }
            for_stmts.push(AstStmt::Let {
                pattern: AstPattern::Ident(c_name),
                type_anno: None,
                init: mcall(
                    part_id,
                    "find",
                    vec![string_from_lit_ast("=".to_string(), span)],
                ),
                mutable: false,
            });
            for_stmts.push(AstStmt::Semi(if_parse));
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Ident(part_name),
                    iterator: parts_id.clone(),
                    body: AstBlock {
                        stmts: for_stmts,
                        final_expr: None,
                        span,
                    },
                },
                span,
            );
            // 主体：inline → `substring(1, len-1)`（剥 { }）；否则原文本
            let body_init = if inline {
                mcall(
                    s_id.clone(),
                    "substring",
                    vec![
                        AstExpr::new(ExprKind::IntLiteral(1), span),
                        AstExpr::new(
                            ExprKind::Binary {
                                op: BinaryOp::Sub,
                                left: mcall(s_id.clone(), "len", Vec::new()),
                                right: AstExpr::new(ExprKind::IntLiteral(1), span),
                            },
                            span,
                        ),
                    ],
                )
            } else {
                s_id.clone()
            };
            let sep = if inline { "," } else { "\n" };
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(s_name),
                            type_anno: None,
                            init: s,
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(body_name),
                            type_anno: None,
                            init: body_init,
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(parts_name),
                            type_anno: None,
                            // X2（2026-08-27）：内联表用引号感知分段 `split_quoted`
                            //（值含逗号的字符串不分割）；顶层多行用 `split("\n")`。
                            init: if inline {
                                mk_ident_call(
                                    "split_quoted".to_string(),
                                    vec![
                                        body_id,
                                        AstExpr::new(ExprKind::IntLiteral(44), span), // ','
                                    ],
                                    span,
                                )
                            } else {
                                mcall(
                                    body_id,
                                    "split",
                                    vec![string_from_lit_ast(sep.to_string(), span)],
                                )
                            },
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
                        // X2（[section] 完整实现，2026-08-27）：`__cur_sec` 记录当前 section
                        //（顶层为空串；`[fname]` 行设置）。for 循环内按 section 分派字段。
                        AstStmt::Let {
                            pattern: AstPattern::Ident(cur_sec_name),
                            type_anno: None,
                            init: mk_path_call(
                                vec!["String".to_string(), "new".to_string()],
                                Vec::new(),
                                span,
                            ),
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
            what: format!("toml.parse：类型 `{other}` 反序列化"),
            span,
        }),
    }
}

