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
        _ => Err(TypeError::Unsupported {
            what: format!("未实现的宏 `{name}`"),
            span,
        }),
    }
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
                    // `{:?}`（debug）或 `{}`（display）
                    let mut is_debug = false;
                    if chars.peek() == Some(&':') {
                        chars.next();
                        if chars.peek() == Some(&'?') {
                            chars.next();
                            is_debug = true;
                        }
                    }
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        if !cur.is_empty() {
                            segs.push(FormatSeg {
                                text: std::mem::take(&mut cur),
                                is_value: false,
                                is_debug: false,
                            });
                        }
                        segs.push(FormatSeg {
                            text: String::new(),
                            is_value: true,
                            is_debug,
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
        });
    }
    if segs.is_empty() {
        // 空格式串（或纯转义）→ 单个空字面量段
        segs.push(FormatSeg {
            text: String::new(),
            is_value: false,
            is_debug: false,
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
