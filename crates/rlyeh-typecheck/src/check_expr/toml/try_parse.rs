//! toml/try_parse：`toml::parse` / `toml::try_parse` 的入口检查与严格解析 AST。
//! （由 toml.rs 拆分而来，保持语义等价）

use super::*;

/// P3（2026-08-28）：`toml::try_parse::<T>(s) -> Result<T, TomlError>` 严格解析。
/// 非法输入返回 `Err(TomlError::ParseError(msg))`。MVP：标量（i64/bool/String）严格
/// 校验；Vec/内联表/struct 首尾 `[]`/`{}` 闭合校验（深层逐段校验 + 错误定位为后续）。
pub(crate) fn check_toml_try_parse(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "toml.try_parse".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    if type_args.len() != 1 {
        return Err(TypeError::Unsupported {
            what: "toml.try_parse 需要 1 个类型实参（turbofish `toml.try_parse::<T>(s)`）"
                .to_string(),
            span,
        });
    }
    let target = resolve_ast_type(ctx, &type_args[0], span)?;
    let try_ast = toml_try_parse_ast(ctx, &target, &args[0], span)?;
    let (hir, ty) = infer_expr(ctx, &try_ast)?;
    Ok((hir, ty))
}

pub(crate) fn check_toml_parse(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "toml.parse".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    if type_args.len() != 1 {
        return Err(TypeError::Unsupported {
            what: "toml.parse 需要 1 个类型实参（turbofish `toml.parse::<T>(s)`）".to_string(),
            span,
        });
    }
    let target = resolve_ast_type(ctx, &type_args[0], span)?;
    let parse_ast = toml_parse_ast(ctx, &target, &args[0], span, false)?;
    let (hir, _) = infer_expr(ctx, &parse_ast)?;
    Ok((hir, target))
}

/// P3（2026-08-28）：构造 `toml.try_parse::<T>(s)` 的严格校验 + Result 包装块 AST：
/// `{ let __s = String::from(<arg>); if <严格校验 __s> { Result::Ok(<toml_parse_ast>)
///   } else { Result::Err(TomlError::ParseError(<msg>)) } }`。
/// MVP：标量（i64/bool/String）严格校验；Vec 首尾 `[]`、HashMap/struct 首尾 `{}` 闭合
/// 校验（深层逐段校验 + 错误定位为后续子步骤）。
pub(crate) fn toml_try_parse_ast(
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
    let brace_check = |open: i64, msg: &'static str| -> (AstExpr, &'static str) {
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
            AstExpr::new(ExprKind::IntLiteral(open as i128), span),
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
            AstExpr::new(ExprKind::IntLiteral((open + 2) as i128), span),
        );
        let and1 = AstExpr::new(
            ExprKind::Binary { op: BinaryOp::And, left: len_ge2, right: first },
            span,
        );
        let and2 = AstExpr::new(
            ExprKind::Binary { op: BinaryOp::And, left: and1, right: last },
            span,
        );
        (and2, msg)
    };
    // 校验条件 + 错误消息
    let (cond, msg): (AstExpr, &str) = match ty {
        Type::I64 => {
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
                    ExprKind::Binary { op: BinaryOp::Or, left: eq_true, right: eq_false },
                    span,
                ),
                "invalid bool",
            )
        }
        Type::Named(n, _) if n == "String" => {
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
        // Vec<T> → 首尾 `[]`（91 / 93）闭合校验
        Type::Named(n, _) if n == "Vec" => brace_check(91, "invalid array"),
        // HashMap / struct → 首尾 `{}`（123 / 125）闭合校验
        Type::Named(..) => brace_check(123, "invalid object"),
        // X2（2026-08-30）：f64 → `parse_float_strict` 严格校验（合法十进制浮点 Ok，否则 Err）
        Type::F64 => {
            let call = mk_ident_call("parse_float_strict".to_string(), vec![s_id.clone()], span);
            let isok = mcall(call, "is_ok", Vec::new());
            (
                cmp(isok, AstExpr::new(ExprKind::IntLiteral(1), span)),
                "invalid float",
            )
        }
        // X2（2026-08-30）：固定数组 `[T; N]` → 首尾 `[]` 闭合校验（与 Vec 同）
        Type::Array(..) => brace_check(91, "invalid array"),
        other => {
            return Err(TypeError::Unsupported {
                what: format!("toml.try_parse：类型 `{other}` 反序列化"),
                span,
            });
        }
    };
    // Result::Ok(<toml_parse_ast 对 __s，inline=false>) / Result::Err(TomlError::ParseError(msg))
    let parse = toml_parse_ast(ctx, ty, &s_id, span, false)?;
    let ok = mk_path_call(
        vec!["Result".to_string(), "Ok".to_string()],
        vec![parse],
        span,
    );
    let err = mk_path_call(
        vec!["Result".to_string(), "Err".to_string()],
        vec![mk_path_call(
            vec![
                "serde".to_string(),
                "TomlError".to_string(),
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
