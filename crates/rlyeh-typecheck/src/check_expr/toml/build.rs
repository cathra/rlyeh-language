//! toml/build：嵌套 section / 字段分派链与 TOML 字符串字面量 AST 构造。
//! （由 toml.rs 拆分而来，保持语义等价）

use super::*;

/// X2（[section] 完整实现，2026-08-27）：生成嵌套 struct 字段的内部字段分派链。
/// `[point]` section 内 `x = 7` → `if __name == "x" { __p.point.x = <parse __val> }`。
/// 遍历嵌套 struct 的字段，`__name` 匹配则递归 `toml_parse_ast` 解析 `__val` 赋值。
pub(crate) fn build_inner_field_dispatch(
    ctx: &mut TypeContext,
    path: &[String],
    p_id: &AstExpr,
    name_id: &AstExpr,
    val_id: &AstExpr,
    sfty: &Type,
    span: Span,
) -> AstExpr {
    let inner_def = match sfty {
        Type::Named(inner_name, _) => ctx.lookup_struct(inner_name).cloned(),
        _ => None,
    };
    let mut inner_chain: Option<AstExpr> = None;
    if let Some(idef) = inner_def {
        for (ifname, ifty) in idef.fields.iter().rev() {
            let name_eq = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![
                        name_id.clone(),
                        string_from_lit_ast(ifname.clone(), span),
                    ],
                    operators: vec![CompareOp::Eq],
                },
                span,
            );
            let inner = inner_chain.take();
            // 多级 target：`__p.<path[0]>.<path[1]>...<ifname>`（递归 FieldAccess）
            let mut base = p_id.clone();
            for seg in path {
                base = AstExpr::new(
                    ExprKind::FieldAccess { expr: base, field: seg.clone() },
                    span,
                );
            }
            let target = AstExpr::new(
                ExprKind::FieldAccess { expr: base, field: ifname.clone() },
                span,
            );
            // 字段值解析：`toml_parse_ast(ctx, ifty, val_id, span, false)`；
            // 解析失败（如字段类型不受支持）回退为直接引用 `__val`（保留文本）。
            let val_ast = match toml_parse_ast(ctx, ifty, val_id, span, false) {
                Ok(v) => v,
                Err(_) => val_id.clone(),
            };
            let then_block = AstBlock {
                stmts: vec![AstStmt::Semi(AstExpr::new(
                    ExprKind::Assign {
                        target,
                        op: AssignOp::Assign,
                        value: val_ast,
                    },
                    span,
                ))],
                final_expr: None,
                span,
            };
            inner_chain = Some(AstExpr::new(
                ExprKind::If {
                    cond: name_eq,
                    then_block,
                    else_block: inner.map(|b| AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(b),
                        span,
                    }),
                },
                span,
            ));
        }
    }
    match inner_chain {
        Some(c) => c,
        None => AstExpr::new(
            ExprKind::Block(AstBlock {
                stmts: Vec::new(),
                final_expr: None,
                span,
            }),
            span,
        ),
    }
}

/// X2（多级 [section]，2026-08-27）：递归生成所有嵌套 struct 的 `[section]` 分派分支。
/// 从 `path_prefix` 出发遍历 `fields` 的嵌套 struct 字段，为每个路径（`inner`、
/// `inner.p`）生成 `else if __cur_sec == <路径>` 分支；再递归子 struct 的嵌套字段。
/// `else_chain` 为已构建的后续分支（自后向前），返回含本层分支的链。
pub(crate) fn build_section_branches(
    ctx: &mut TypeContext,
    fields: &[(String, Type)],
    path_prefix: &[String],
    p_id: &AstExpr,
    cur_sec_id: &AstExpr,
    name_id: &AstExpr,
    val_id: &AstExpr,
    span: Span,
    mut else_chain: Option<AstBlock>,
) -> Option<AstBlock> {
    for (sfname, sfty) in fields.iter().rev() {
        if !crate::check_expr::toml_ser::is_nested_struct_type(&*ctx, sfty) {
            continue;
        }
        // 当前路径 = 前缀 + 字段名
        let mut path = path_prefix.to_vec();
        path.push(sfname.clone());
        // 先递归子 struct 的嵌套字段（更深路径分支）
        if let Type::Named(inner_name, _) = sfty {
            if let Some(idef) = ctx.lookup_struct(inner_name).cloned() {
                else_chain = build_section_branches(
                    ctx, &idef.fields, &path, p_id, cur_sec_id, name_id, val_id, span, else_chain,
                );
            }
        }
        // 当前路径分支：`else if __cur_sec == "inner.p" { <内部字段分派> }`
        let path_str = path.join(".");
        let inner_chain = build_inner_field_dispatch(ctx, &path, p_id, name_id, val_id, sfty, span);
        let sec_eq = AstExpr::new(
            ExprKind::ComparisonChain {
                elements: vec![
                    cur_sec_id.clone(),
                    string_from_lit_ast(path_str, span),
                ],
                operators: vec![CompareOp::Eq],
            },
            span,
        );
        let new_if = AstExpr::new(
            ExprKind::If {
                cond: sec_eq,
                then_block: AstBlock {
                    stmts: vec![AstStmt::Semi(inner_chain)],
                    final_expr: None,
                    span,
                },
                else_block: else_chain.take(),
            },
            span,
        );
        else_chain = Some(AstBlock {
            stmts: Vec::new(),
            final_expr: Some(new_if),
            span,
        });
    }
    else_chain
}

/// X2（2026-08-30）：TOML 字符串值反序列化 AST——多行字符串 `"""..."""` 走原始
/// 剥离（首 3 字节与末 3 字节均为 `"`(34) 且长度 ≥ 6 时 `substring(3, len-3)`），
/// 否则走 `json_unescape`（JSON 风格单引号 + 转义还原）。
pub(crate) fn toml_string_value_ast(s: &AstExpr, span: Span) -> AstExpr {
    // X2（2026-08-30）：先 trim 去除首尾空白（struct 字段 `key = """...""` 值含前导空格），
    // 再判定多行字符串 `"""..."""` 或 JSON 风格转义字符串。
    let st = mk_path_call(
        vec!["String".to_string(), "from".to_string()],
        vec![AstExpr::new(
            ExprKind::MethodCall {
                receiver: s.clone(),
                method: "trim".to_string(),
                args: Vec::new(),
                trait_hint: None,
            },
            span,
        )],
        span,
    );
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
    let eq = |e: AstExpr| {
        AstExpr::new(
            ExprKind::ComparisonChain {
                elements: vec![e, AstExpr::new(ExprKind::IntLiteral(34), span)],
                operators: vec![CompareOp::Eq],
            },
            span,
        )
    };
    let len = mcall(st.clone(), "len", Vec::new());
    let get = |idx: AstExpr| mcall(st.clone(), "get", vec![idx]);
    let off = |o: i64| {
        AstExpr::new(
            ExprKind::Binary {
                op: BinaryOp::Sub,
                left: len.clone(),
                right: AstExpr::new(ExprKind::IntLiteral(o as i128), span),
            },
            span,
        )
    };
    let first3 = AstExpr::new(
        ExprKind::Binary {
            op: BinaryOp::And,
            left: AstExpr::new(
                ExprKind::Binary {
                    op: BinaryOp::And,
                    left: eq(get(AstExpr::new(ExprKind::IntLiteral(0), span))),
                    right: eq(get(AstExpr::new(ExprKind::IntLiteral(1), span))),
                },
                span,
            ),
            right: eq(get(AstExpr::new(ExprKind::IntLiteral(2), span))),
        },
        span,
    );
    let last3 = AstExpr::new(
        ExprKind::Binary {
            op: BinaryOp::And,
            left: AstExpr::new(
                ExprKind::Binary {
                    op: BinaryOp::And,
                    left: eq(get(off(3))),
                    right: eq(get(off(2))),
                },
                span,
            ),
            right: eq(get(off(1))),
        },
        span,
    );
    let ge6 = AstExpr::new(
        ExprKind::ComparisonChain {
            elements: vec![len.clone(), AstExpr::new(ExprKind::IntLiteral(6), span)],
            operators: vec![CompareOp::Ge],
        },
        span,
    );
    let is_ml = AstExpr::new(
        ExprKind::Binary {
            op: BinaryOp::And,
            left: AstExpr::new(
                ExprKind::Binary {
                    op: BinaryOp::And,
                    left: first3,
                    right: last3,
                },
                span,
            ),
            right: ge6,
        },
        span,
    );
    let raw = mcall(
        st.clone(),
        "substring",
        vec![AstExpr::new(ExprKind::IntLiteral(3), span), off(3)],
    );
    let esc = mk_ident_call("json_unescape".to_string(), vec![st.clone()], span);
    AstExpr::new(
        ExprKind::If {
            cond: is_ml,
            then_block: AstBlock {
                stmts: Vec::new(),
                final_expr: Some(raw),
                span,
            },
            else_block: Some(AstBlock {
                stmts: Vec::new(),
                final_expr: Some(esc),
                span,
            }),
        },
        span,
    )
}

