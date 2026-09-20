//! 表达式检查子模块：macro_ser。
//! （由 macro_serialize.rs 二次拆分而来，保持语义等价）

use super::*;

pub(crate) fn check_macro_call(
    ctx: &mut TypeContext,
    name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    match name {
        "println!" | "print!" | "format!" | "eprintln!" | "eprint!" => {
            check_format_macro(ctx, name, args, span)
        }
        "dbg!" => check_dbg_macro(ctx, args, span),
        // W（SH-P2-10）：断言与 panic 宏——desugar 为对内置 `panic` 的调用。
        "panic!" => check_panic_macro(ctx, args, span),
        "unreachable!" | "todo!" => check_unreachable_macro(ctx, name, span),
        "assert!" => check_assert_like(ctx, args, AssertKind::Plain, span),
        "assert_eq!" => check_assert_like(ctx, args, AssertKind::Eq, span),
        "assert_ne!" => check_assert_like(ctx, args, AssertKind::Ne, span),
        _ => Err(TypeError::Unsupported {
            what: format!("未实现的宏 `{name}`"),
            span,
        }),
    }
}

/// `panic!` 系列 desugar 的目标内建（codegen 输出到 stderr 后 `abort`）。
enum AssertKind {
    /// `assert!(cond, msg?)`：条件为假时 panic。
    Plain,
    /// `assert_eq!(a, b, msg?)`：等价于 `assert!(a == b)`。
    Eq,
    /// `assert_ne!(a, b, msg?)`：等价于 `assert!(a != b)`。
    Ne,
}

fn empty_str(span: Span) -> AstExpr {
    AstExpr::new(ExprKind::StringLiteral(String::new()), span)
}

/// `panic!(msg?)` → 调用内置 `panic(msg)`（缺省空串）。
fn check_panic_macro(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let msg = args.first().cloned().unwrap_or_else(|| empty_str(span));
    let call = mk_ident_call("panic".to_string(), vec![msg], span);
    infer_expr(ctx, &call)
}

/// `unreachable!()` / `todo!()` → `panic!("<提示>")`。
fn check_unreachable_macro(
    ctx: &mut TypeContext,
    name: &str,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let lit = AstExpr::new(ExprKind::StringLiteral(format!("{name} 不可达")), span);
    let call = mk_ident_call("panic".to_string(), vec![lit], span);
    infer_expr(ctx, &call)
}

/// `assert!` / `assert_eq!` / `assert_ne!` desugar：
/// `if !(cond) { panic!(msg) } else { () }`。
fn check_assert_like(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    kind: AssertKind,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 构造被断言成立的条件表达式 `cond`。
    let cmp_eq = |a: AstExpr, b: AstExpr| -> AstExpr {
        AstExpr::new(
            ExprKind::ComparisonChain {
                elements: vec![a, b],
                operators: vec![CompareOp::Eq],
            },
            span,
        )
    };
    let cond = match kind {
        AssertKind::Plain => args[0].clone(),
        AssertKind::Eq => cmp_eq(args[0].clone(), args[1].clone()),
        AssertKind::Ne => AstExpr::new(
            ExprKind::Unary {
                op: UnaryOp::Not,
                operand: cmp_eq(args[0].clone(), args[1].clone()),
            },
            span,
        ),
    };
    // `if !cond { panic!(msg) } else { () }`
    let not_cond = AstExpr::new(
        ExprKind::Unary {
            op: UnaryOp::Not,
            operand: cond,
        },
        span,
    );
    let msg_idx = match kind {
        AssertKind::Plain => 1,
        AssertKind::Eq | AssertKind::Ne => 2,
    };
    let msg = args.get(msg_idx).cloned().unwrap_or_else(|| empty_str(span));
    let panic_call = mk_ident_call("panic".to_string(), vec![msg], span);
    let then_block = AstBlock {
        stmts: vec![AstStmt::Semi(panic_call)],
        final_expr: None,
        span,
    };
    let else_block = AstBlock {
        stmts: vec![],
        final_expr: Some(AstExpr::new(ExprKind::Unit, span)),
        span,
    };
    let if_expr = AstExpr::new(
        ExprKind::If {
            cond: not_cond,
            then_block,
            else_block: Some(else_block),
        },
        span,
    );
    infer_expr(ctx, &if_expr)
}

pub(crate) fn parse_format_string(s: &str, span: Span) -> Result<Vec<FormatSeg>, TypeError> {
    let mut segs = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    cur.push('{');
                } else {
                    // `{}`（display）、`{:?}`（debug）、或带格式说明符
                    // （X4：`{:>10}` / `{:<5}` / `{:^8}` / `{:*>10}`，`fill align width`）。
                    let mut is_debug = false;
                    // X4 格式说明符（默认：无对齐、宽度 0、填充空格）
                    let mut align: char = '\0';
                    let mut width: i64 = 0;
                    let mut fill: u8 = b' ';
                    if chars.peek() == Some(&':') {
                        chars.next();
                        if chars.peek() == Some(&'?') {
                            chars.next();
                            is_debug = true;
                        } else {
                            // 解析 `fill align width`：可选填充字符 + 可选对齐符 + 可选宽度
                            if let Some(&c) = chars.peek() {
                                if c != '}' && !c.is_ascii_digit() && c != '<' && c != '>' && c != '^'
                                {
                                    // 第一个非对齐/非数字字符作为填充字符
                                    if c.is_ascii() {
                                        fill = c as u8;
                                        chars.next();
                                    }
                                }
                            }
                            if let Some(&c) = chars.peek() {
                                if c == '<' || c == '>' || c == '^' {
                                    align = c;
                                    chars.next();
                                }
                            }
                            // 数字宽度
                            let mut w = String::new();
                            while let Some(&c) = chars.peek() {
                                if c.is_ascii_digit() {
                                    w.push(c);
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            if !w.is_empty() {
                                width = w.parse::<i64>().unwrap_or(0);
                            }
                        }
                    }
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        if !cur.is_empty() {
                            segs.push(FormatSeg {
                                text: std::mem::take(&mut cur),
                                is_value: false,
                                is_debug: false,
                                align: '\0',
                                width: 0,
                                fill: b' ',
                            });
                        }
                        segs.push(FormatSeg {
                            text: String::new(),
                            is_value: true,
                            is_debug,
                            align,
                            width,
                            fill,
                        });
                    } else {
                        return Err(TypeError::Unsupported {
                            what: "格式串中的 `{` 未闭合（转义应写作 `{{`）".into(),
                            span,
                        });
                    }
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    cur.push('}');
                } else {
                    return Err(TypeError::Unsupported {
                        what: "格式串中的 `}` 未配对（转义应写作 `}}`）".into(),
                        span,
                    });
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        segs.push(FormatSeg {
            text: cur,
            is_value: false,
            is_debug: false,
            align: '\0',
            width: 0,
            fill: b' ',
        });
    }
    if segs.is_empty() {
        // 空格式串（或纯转义）→ 单个空字面量段
        segs.push(FormatSeg {
            text: String::new(),
            is_value: false,
            is_debug: false,
            align: '\0',
            width: 0,
            fill: b' ',
        });
    }
    Ok(segs)
}

pub(crate) fn string_from_lit_ast(s: String, span: Span) -> AstExpr {
    let callee = AstExpr::new(
        ExprKind::Path(vec!["String".to_string(), "from".to_string()]),
        span,
    );
    let arg = AstExpr::new(ExprKind::StringLiteral(s), span);
    AstExpr::new(ExprKind::Call { callee, args: vec![arg], type_args: Vec::new() }, span)
}

pub(crate) fn mk_ident_call(name: String, args: Vec<AstExpr>, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(ExprKind::Ident(name), span),
            args,
            type_args: Vec::new(),
        },
        span,
    )
}

pub(crate) fn mk_path_call(segments: Vec<String>, args: Vec<AstExpr>, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(ExprKind::Path(segments), span),
            args,
            type_args: Vec::new(),
        },
        span,
    )
}

pub(crate) fn bin_add(left: AstExpr, right: AstExpr, span: Span) -> AstExpr {
    AstExpr::new(ExprKind::Binary { op: BinaryOp::Add, left, right }, span)
}

pub(crate) fn fold_add(parts: Vec<AstExpr>, span: Span) -> AstExpr {
    parts
        .into_iter()
        .reduce(|a, b| bin_add(a, b, span))
        .expect("parts 非空")
}
