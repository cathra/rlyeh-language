//! 表达式检查子模块：工具函数。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use super::*;

pub(super) fn ty_to_zero_ast(ctx: &TypeContext, ty: &Type, span: Span) -> Result<AstExpr, TypeError> {
    use Type::*;
    match ty {
        I64 | U8 => Ok(AstExpr::new(ExprKind::IntLiteral(0), span)),
        F64 => Ok(AstExpr::new(ExprKind::FloatLiteral(0.0), span)),
        Bool => Ok(AstExpr::new(ExprKind::BoolLiteral(false), span)),
        Named(n, _) if n == "String" => Ok(string_from_lit_ast(String::new(), span)),
        Named(n, _) if n == "Vec" || n == "HashMap" => Ok(mk_path_call(
            vec![n.clone(), "new".to_string()],
            Vec::new(),
            span,
        )),
        Named(n, args) if ctx.lookup_struct(&n).is_some() && args.is_empty() => {
            let def = ctx.lookup_struct(&n).expect("lookup_struct 已验证");
            let mut fields = Vec::new();
            for (fname, fty) in &def.fields {
                fields.push((fname.clone(), ty_to_zero_ast(ctx, fty, span)?));
            }
            Ok(AstExpr::new(
                ExprKind::StructCtor {
                    type_name: vec![n.clone()],
                    type_args: Vec::new(),
                    fields,
                },
                span,
            ))
        }
        other => Err(TypeError::Unsupported {
            what: format!(
                "json.parse：字段类型 `{other}` 的零值构造（MVP 支持 i64 / u8 / f64 / bool / String / Vec / HashMap / 嵌套 struct）"
            ),
            span,
        }),
    }
}

/// X4：对齐格式占位符应用（`{:>10}` 右 / `{:<5}` 左 / `{:^8}` 居中）。
/// 用 std `pad_start`（左填充=右对齐）/ `pad_end`（右填充=左对齐）实现；
/// 居中经 `let __s = base; (width - __s.len())/2` 计算左填充量后两次 pad。
pub(super) fn align_string_ast(
    ctx: &mut TypeContext,
    base: AstExpr,
    align: char,
    width: i64,
    fill: u8,
    span: Span,
) -> Result<AstExpr, TypeError> {
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
    let fill_lit = AstExpr::new(ExprKind::IntLiteral(fill as i128), span);
    let width_lit = AstExpr::new(ExprKind::IntLiteral(width as i128), span);
    match align {
        '>' => Ok(mcall(base, "pad_start", vec![width_lit, fill_lit])),
        '<' => Ok(mcall(base, "pad_end", vec![width_lit, fill_lit])),
        '^' => {
            // 居中：左 pad = (width - len) / 2
            let s_name = ctx.fresh_temp();
            let len_name = ctx.fresh_temp();
            let left_name = ctx.fresh_temp();
            let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
            let len_id = AstExpr::new(ExprKind::Ident(len_name.clone()), span);
            let left_id = AstExpr::new(ExprKind::Ident(left_name.clone()), span);
            // __s.pad_start(len + left, fill).pad_end(width, fill)
            let right_end = mcall(
                mcall(
                    s_id.clone(),
                    "pad_start",
                    vec![
                        AstExpr::new(
                            ExprKind::Binary {
                                op: BinaryOp::Add,
                                left: len_id.clone(),
                                right: left_id.clone(),
                            },
                            span,
                        ),
                        fill_lit.clone(),
                    ],
                ),
                "pad_end",
                vec![width_lit.clone(), fill_lit.clone()],
            );
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(s_name),
                            type_anno: None,
                            init: base,
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(len_name),
                            type_anno: None,
                            init: mcall(s_id.clone(), "len", Vec::new()),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(left_name),
                            type_anno: None,
                            init: AstExpr::new(
                                ExprKind::Binary {
                                    op: BinaryOp::Div,
                                    left: AstExpr::new(
                                        ExprKind::Binary {
                                            op: BinaryOp::Sub,
                                            left: width_lit.clone(),
                                            right: len_id,
                                        },
                                        span,
                                        ),
                                        right: AstExpr::new(ExprKind::IntLiteral(2), span),
                                },
                                span,
                            ),
                            mutable: false,
                        },
                    ],
                    final_expr: Some(right_end),
                    span,
                }),
                span,
            ))
        }
        _ => Ok(base),
    }
}

pub(super) fn value_to_string_ast(
    ctx: &mut TypeContext,
    arg: &AstExpr,
    debug: bool,
    span: Span,
) -> Result<AstExpr, TypeError> {
    let (_, ty) = infer_expr(ctx, arg)?;
    value_to_string_for_ty(ctx, &ty, arg, debug, span)
}

pub(super) fn value_to_string_for_ty(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    debug: bool,
    span: Span,
) -> Result<AstExpr, TypeError> {
    let base = peel_ref(ty);
    // String 对象 / &str 视图：`String::from(x)`（视图深拷贝；String 值即对象）
    if crate::comparison::is_string_type(ctx, &base) {
        if matches!(ty, Type::Ref(..)) {
            return Ok(AstExpr::new(
                ExprKind::Call {
                    callee: AstExpr::new(
                        ExprKind::Path(vec!["String".to_string(), "from".to_string()]),
                        span,
                    ),
                    args: vec![arg.clone()],
                    type_args: Vec::new(),
                },
                span,
            ));
        }
        return Ok(arg.clone());
    }
    // 字符串字面量 Str：`String::from(arg)`
    if matches!(ty, Type::Str) {
        return Ok(AstExpr::new(
            ExprKind::Call {
                callee: AstExpr::new(
                    ExprKind::Path(vec!["String".to_string(), "from".to_string()]),
                    span,
                ),
                args: vec![arg.clone()],
                type_args: Vec::new(),
            },
            span,
        ));
    }
    // i64 → `int_to_string(x)`（std）
    if matches!(ty, Type::I64) {
        let callee = AstExpr::new(ExprKind::Ident("int_to_string".to_string()), span);
        return Ok(AstExpr::new(
            ExprKind::Call {
                callee,
                args: vec![arg.clone()],
                type_args: Vec::new(),
            },
            span,
        ));
    }
    // bool → `if b { "true" } else { "false" }`
    if matches!(ty, Type::Bool) {
        let mk = |s: &str| string_from_lit_ast(s.to_string(), span);
        let then_block = AstBlock {
            stmts: Vec::new(),
            final_expr: Some(mk("true")),
            span,
        };
        let else_block = AstBlock {
            stmts: Vec::new(),
            final_expr: Some(mk("false")),
            span,
        };
        return Ok(AstExpr::new(
            ExprKind::If {
                cond: arg.clone(),
                then_block,
                else_block: Some(else_block),
            },
            span,
        ));
    }
    // Q3b：自定义类型走 Display / Debug trait 方法
    // （X4 完整化：`{}` → `fmt::Display::fmt`，`{:?}` → `fmt::Debug::fmt`；
    // 同名 `fmt` 经 impl 查找按 trait 名区分——`find_impl_for_trait_method`）。
    // `fmt` 返回 `Result<(), FmtError>`（写缓冲），调用后取 `Formatter::result()`。
    let method = "fmt".to_string();
    let trait_name = if debug { "fmt::Debug" } else { "fmt::Display" };
    let has_impl = ctx
        .find_impl_for_trait_method(&base, trait_name, &method)
        .is_some();
    if has_impl {
        // 块表达式：
        //   { let mut __fmt_q3 = Formatter::new(); x.fmt(&mut __fmt_q3); __fmt_q3.result() }
        // （MVP `&mut` 仅支持变量目标，临时值 `&mut Formatter::new()` 不可用）
        let tmp = "__fmt_q3".to_string();
        let f_new = AstExpr::new(
            ExprKind::Call {
                callee: AstExpr::new(
                    ExprKind::Path(vec!["Formatter".to_string(), "new".to_string()]),
                    span,
                ),
                args: Vec::new(),
                type_args: Vec::new(),
            },
            span,
        );
        let bind = AstStmt::Let {
            pattern: AstPattern::Ident(tmp.clone()),
            type_anno: None,
            init: f_new,
            mutable: true,
        };
        let f_ref = AstExpr::new(
            ExprKind::Unary {
                op: UnaryOp::AddrOfMut,
                operand: AstExpr::new(ExprKind::Ident(tmp.clone()), span),
            },
            span,
        );
        let call = AstExpr::new(
            ExprKind::MethodCall {
                receiver: arg.clone(),
                method: method.clone(),
                args: vec![f_ref],
                // X4：引擎生成的 fmt 调用按 trait 分派（Display::fmt / Debug::fmt 同名）
                trait_hint: Some(trait_name.to_string()),
            },
            span,
        );
        // `x.fmt(&mut __fmt_q3)` 作为表达式语句（返回 Result 忽略）
        let call_stmt = AstStmt::Semi(call);
        // `__fmt_q3.result()`：取回拼接结果 String
        let result_call = AstExpr::new(
            ExprKind::MethodCall {
                receiver: AstExpr::new(ExprKind::Ident(tmp.clone()), span),
                method: "result".to_string(),
                args: Vec::new(),
                trait_hint: None,
            },
            span,
        );
        return Ok(AstExpr::new(
            ExprKind::Block(AstBlock {
                stmts: vec![bind, call_stmt],
                final_expr: Some(result_call),
                span,
            }),
            span,
        ));
    }
    let placeholder = if debug { "{:?}" } else { "{}" };
    Err(TypeError::Unsupported {
        what: format!(
            "`{placeholder}` 占位符不支持类型 `{ty}`（MVP 支持 i64 / bool / String / &str / 字符串字面量；自定义类型须 `impl {placeholder}`）",
            placeholder = placeholder,
            ty = ty,
        ),
        span,
    })
}

pub(super) fn check_format_macro(
    ctx: &mut TypeContext,
    name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // `println!()` / `print!()`：无参数（空行 / 无输出）
    if args.is_empty() {
        if name == "format!" {
            return Err(TypeError::UnexpectedArgumentCount {
                name: name.to_string(),
                expected: 1,
                found: 0,
                span,
            });
        }
        let inner = name.trim_end_matches('!').to_string();
        let call = AstExpr::new(
            ExprKind::Call {
                callee: AstExpr::new(ExprKind::Ident(inner), span),
                args: Vec::new(),
                type_args: Vec::new(),
            },
            span,
        );
        let (hir, _) = infer_expr(ctx, &call)?;
        return Ok((hir, Type::Unit));
    }
    // 格式串：第 1 参数须为字符串字面量
    let fmt = match &*args[0].kind {
        ExprKind::StringLiteral(s) => s.clone(),
        _ => {
            return Err(TypeError::Unsupported {
                what: format!("{name} 的第 1 个参数须为字符串字面量格式串"),
                span: args[0].span,
            });
        }
    };
    let segs = parse_format_string(&fmt, args[0].span)?;
    let value_count = segs.iter().filter(|s| s.is_value).count();
    if value_count != args.len() - 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{name} 占位符"),
            expected: value_count,
            found: args.len() - 1,
            span,
        });
    }
    // 逐段拼接：`String::from(字面量) + to_string(值) + ...`
    // （`+` 在 Binary 分支 desugar 为 clone + push_str 深拷贝，A3 语义）
    let mut acc: Option<AstExpr> = None;
    let mut value_idx = 1usize;
    for seg in &segs {
        let seg_ast = if seg.is_value {
            let arg = &args[value_idx];
            value_idx += 1;
            let base = value_to_string_ast(ctx, arg, seg.is_debug, span)?;
            // X4：对齐格式占位符（`{:>10}` / `{:<5}` / `{:^8}`）——按宽度对齐。
            if seg.width > 0 && seg.align != '\0' {
                align_string_ast(ctx, base, seg.align, seg.width, seg.fill, span)?
            } else {
                base
            }
        } else {
            string_from_lit_ast(seg.text.clone(), span)
        };
        acc = Some(match acc {
            None => seg_ast,
            Some(prev) => AstExpr::new(
                ExprKind::Binary {
                    op: BinaryOp::Add,
                    left: prev,
                    right: seg_ast,
                },
                span,
            ),
        });
    }
    let concat = acc.expect("格式串至少有一段");
    if name == "format!" {
        let (hir, ty) = infer_expr(ctx, &concat)?;
        return Ok((hir, ty));
    }
    // println! / print!：`println(concat)` → 内建 print/println（自动 desugar println_string）
    let inner = name.trim_end_matches('!').to_string();
    let call = AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(ExprKind::Ident(inner), span),
            args: vec![concat],
            type_args: Vec::new(),
        },
        span,
    );
    let (hir, _) = infer_expr(ctx, &call)?;
    Ok((hir, Type::Unit))
}

pub(super) fn check_dbg_macro(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "dbg!".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    // 先推断参数类型（此时 `__tmp` 尚未绑定，必须用预推断类型构造转换）
    let (_, arg_ty) = infer_expr(ctx, &args[0])?;
    let tmp = ctx.fresh_temp();
    // `let __tmp = <expr>; println("dbg: " + to_string(__tmp)); __tmp`
    let prefix = string_from_lit_ast("dbg: ".to_string(), span);
    // `dbg!` 用 Debug 格式（`{:?}` 语义）：自定义类型走 `fmt_debug`
    let value = value_to_string_for_ty(
        ctx,
        &arg_ty,
        &AstExpr::new(ExprKind::Ident(tmp.clone()), span),
        true,
        span,
    )?;
    let concat = AstExpr::new(
        ExprKind::Binary {
            op: BinaryOp::Add,
            left: prefix,
            right: value,
        },
        span,
    );
    let print_call = AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(ExprKind::Ident("println".to_string()), span),
            args: vec![concat],
            type_args: Vec::new(),
        },
        span,
    );
    let block = AstBlock {
        stmts: vec![
            AstStmt::Let {
                pattern: AstPattern::Ident(tmp.clone()),
                type_anno: None,
                init: args[0].clone(),
                mutable: false,
            },
            AstStmt::Expr(print_call),
        ],
        final_expr: Some(AstExpr::new(ExprKind::Ident(tmp), span)),
        span,
    };
    let (hir, ty) = check_block(ctx, &block)?;
    Ok((HirExpr::new(HirExprKind::Block(Box::new(hir)), Span::dummy()), ty))
}

pub(crate) fn substitute(ty: &Type, subst: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Generic(tp) => subst.get(tp).cloned().unwrap_or_else(|| ty.clone()),
        Type::Named(n, ps) => Type::Named(
            n.clone(),
            ps.iter().map(|p| substitute(p, subst)).collect(),
        ),
        Type::Ref(inner, m) => Type::Ref(Box::new(substitute(inner, subst)), *m),
        Type::RawPtr(inner, m) => Type::RawPtr(Box::new(substitute(inner, subst)), *m),
        Type::Tuple(ts) => Type::Tuple(ts.iter().map(|t| substitute(t, subst)).collect()),
        Type::Array(inner, n) => Type::Array(Box::new(substitute(inner, subst)), *n),
        // T1a：函数类型递归替换（fn(T, T) -> i64 中 T 经 impl 泛型统一后须替换为
        // 具体类型——此前 `_` 兜底不深入 Fn 内部，泛型方法 fn 参数（如
        // `Vec::sort_by(cmp: fn(T, T) -> i64)`）调用时形参保持 `fn(T, T)`，
        // 函数名 / 闭包实参均无法 unify（expects fn(T, T) -> i64 错误）。
        Type::Fn(sig) => Type::Fn(Box::new(FnSignature {
            params: sig.params.iter().map(|p| substitute(p, subst)).collect(),
            param_spans: sig.param_spans.clone(),
            return_type: substitute(&sig.return_type, subst),
        })),
        Type::AssocProjection { base, assoc } => Type::AssocProjection {
            base: Box::new(substitute(base, subst)),
            assoc: assoc.clone(),
        },
        _ => ty.clone(),
    }
}

/// 将编译期 [`Type`] 反向转为 [`rlyeh_ast::AstType`]（用于构造 turbofish /
/// 类型注解 AST，如 `From::<E1>::from(__e)` 的 `::<E1>`）。P6c（2026-08-29）。
pub(crate) fn type_to_ast(ty: &Type) -> rlyeh_ast::AstType {
    use rlyeh_ast::AstType;
    use Mutability::*;
    match ty {
        Type::Named(n, args) => {
            AstType::Path(n.clone(), args.iter().map(type_to_ast).collect())
        }
        Type::Ref(inner, m) => AstType::Ref(Box::new(type_to_ast(inner)), matches!(m, Mutable)),
        Type::RawPtr(inner, m) => AstType::RawPtr(Box::new(type_to_ast(inner)), *m),
        Type::Tuple(ts) => AstType::Tuple(ts.iter().map(type_to_ast).collect()),
        Type::Array(inner, _) => AstType::Array(Box::new(type_to_ast(inner)), None),
        Type::Fn(sig) => AstType::Fn(
            sig.params.iter().map(type_to_ast).collect(),
            Box::new(type_to_ast(&sig.return_type)),
        ),
        Type::Dyn(t) => AstType::Dyn(t.clone()),
        Type::Generic(g) => AstType::Path(g.clone(), Vec::new()),
        Type::Str => AstType::Path("string".to_string(), Vec::new()),
        Type::Bool => AstType::Path("bool".to_string(), Vec::new()),
        Type::Char => AstType::Path("char".to_string(), Vec::new()),
        Type::Unit => AstType::Path("()".to_string(), Vec::new()),
        Type::I8 => AstType::Path("i8".to_string(), Vec::new()),
        Type::I16 => AstType::Path("i16".to_string(), Vec::new()),
        Type::I32 => AstType::Path("i32".to_string(), Vec::new()),
        Type::I64 => AstType::Path("i64".to_string(), Vec::new()),
        Type::I128 => AstType::Path("i128".to_string(), Vec::new()),
        Type::ISize => AstType::Path("isize".to_string(), Vec::new()),
        Type::U8 => AstType::Path("u8".to_string(), Vec::new()),
        Type::U16 => AstType::Path("u16".to_string(), Vec::new()),
        Type::U32 => AstType::Path("u32".to_string(), Vec::new()),
        Type::U64 => AstType::Path("u64".to_string(), Vec::new()),
        Type::U128 => AstType::Path("u128".to_string(), Vec::new()),
        Type::USize => AstType::Path("usize".to_string(), Vec::new()),
        Type::F32 => AstType::Path("f32".to_string(), Vec::new()),
        Type::F64 => AstType::Path("f64".to_string(), Vec::new()),
        // Never / Infer / Closure / AssocProjection 不作为 turbofish 实参；兜底为名表达
        _ => AstType::Path(ty.to_string(), Vec::new()),
    }
}

pub(super) fn peel_ref(ty: &Type) -> Type {
    match ty {
        Type::Ref(inner, _) => (**inner).clone(),
        _ => ty.clone(),
    }
}

pub(super) fn heap_wrapper_inner(ty: &Type) -> Option<Type> {
    match ty {
        Type::Named(n, args)
            if matches!(n.as_str(), "Box" | "Rc" | "Arc" | "Gc") && args.len() == 1 =>
        {
            Some(args[0].clone())
        }
        _ => None,
    }
}

pub(super) fn peel_refs_and_heap(ty: &Type) -> Type {
    let mut t = ty.clone();
    loop {
        let next = match &t {
            Type::Ref(inner, _) => (**inner).clone(),
            _ => match heap_wrapper_inner(&t) {
                Some(inner) => inner,
                None => return t,
            },
        };
        t = next;
    }
}

pub(super) fn heap_ptr_hir(hir: HirExpr, ty: &Type) -> HirExpr {
    match peel_ref(ty) {
        Type::Named(n, args)
            if matches!(n.as_str(), "Box" | "Rc" | "Arc" | "Gc") && args.len() == 1 =>
        {
            HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(hir),
                index: 0,
                ty: FieldScalar::Ptr,
            }, Span::dummy())
        }
        _ => hir,
    }
}

pub(super) fn string_hash_hir(ctx: &mut TypeContext, s: &HirExpr) -> HirExpr {
    let s_name = ctx.fresh_temp();
    let data_name = ctx.fresh_temp();
    let len_name = ctx.fresh_temp();
    let h_name = ctx.fresh_temp();
    let i_name = ctx.fresh_temp();
    let b_name = ctx.fresh_temp();

    let s_var = HirExpr::new(HirExprKind::Variable(s_name.clone()), Span::dummy());
    let data_field = HirExpr::new(HirExprKind::FieldGet{
        base: Box::new(s_var.clone()),
        index: 0,
        ty: FieldScalar::Ptr,
    }, Span::dummy());
    let len_field = HirExpr::new(HirExprKind::FieldGet{
        base: Box::new(s_var),
        index: 1,
        ty: FieldScalar::Int,
    }, Span::dummy());

    let loop_body = HirBlock { span: Span::dummy(),
        stmts: vec![
            // if __i >= __len { break }
            HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::If{
                cond: Box::new(HirExpr::new(HirExprKind::Binary(
                    HirBinaryOp::Ge,
                    Box::new(HirExpr::new(HirExprKind::Variable(i_name.clone()), Span::dummy())),
                    Box::new(HirExpr::new(HirExprKind::Variable(len_name.clone()), Span::dummy())),
                ), Span::dummy())),
                then_block: Box::new(HirBlock { span: Span::dummy(),
                    stmts: vec![HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Break(None), Span::dummy())), Span::dummy())],
                    final_expr: None,
                }),
                else_block: None,
            }, Span::dummy())), Span::dummy()),
            // let __b = __data[__i]
            HirStmt::new(HirStmtKind::Let{
                name: b_name.clone(),
                init: HirExpr::new(HirExprKind::Index{
                    base: Box::new(HirExpr::new(HirExprKind::Variable(data_name.clone()), Span::dummy())),
                    index: Box::new(HirExpr::new(HirExprKind::Variable(i_name.clone()), Span::dummy())),
                    elem: FieldScalar::Int,
                    is_str: true,
                }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            // __h = __h * 33 + __b
            HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Assign{
                target: h_name.clone(),
                op: HirAssignOp::Assign,
                value: Box::new(HirExpr::new(HirExprKind::Binary(
                    HirBinaryOp::Add,
                    Box::new(HirExpr::new(HirExprKind::Binary(
                        HirBinaryOp::Mul,
                        Box::new(HirExpr::new(HirExprKind::Variable(h_name.clone()), Span::dummy())),
                        Box::new(HirExpr::new(HirExprKind::IntLiteral(33), Span::dummy())),
                    ), Span::dummy())),
                    Box::new(HirExpr::new(HirExprKind::Variable(b_name), Span::dummy())),
                ), Span::dummy())),
            }, Span::dummy())), Span::dummy()),
            // __i = __i + 1
            HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Assign{
                target: i_name.clone(),
                op: HirAssignOp::Assign,
                value: Box::new(HirExpr::new(HirExprKind::Binary(
                    HirBinaryOp::Add,
                    Box::new(HirExpr::new(HirExprKind::Variable(i_name.clone()), Span::dummy())),
                    Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
                ), Span::dummy())),
            }, Span::dummy())), Span::dummy()),
        ],
        final_expr: None,
    };

    let stmts = vec![
        HirStmt::new(HirStmtKind::Let{
            name: s_name,
            init: s.clone(),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: data_name,
            init: data_field,
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: len_name,
            init: len_field,
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: h_name.clone(),
            init: HirExpr::new(HirExprKind::IntLiteral(5381), Span::dummy()),
            mutable: true,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: i_name.clone(),
            init: HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy()),
            mutable: true,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Loop{
            body: Box::new(loop_body),
        }, Span::dummy())), Span::dummy()),
    ];

    HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
        stmts,
        final_expr: Some(HirExpr::new(HirExprKind::Variable(h_name), Span::dummy())),
    })), Span::dummy())
}

/// 收集模式绑定的**源码层变量名**（SH-P0-7 P-M3，或模式一致性校验用）。
///
/// Rust 要求 `A | B` 的各备选绑定**同名同序**的变量集（`Some(x) | Some(y)`
/// 非法）。校验须基于源码名而非 `insert_variable` 生成的槽名——槽名会随
/// 作用域深度 mangle，同一源码名在不同作用域内检查会得到不同槽名。
pub(super) fn pattern_bind_names(pat: &rlyeh_ast::AstPattern) -> Vec<String> {
    use rlyeh_ast::AstPattern;
    match pat {
        AstPattern::Ident(name) => vec![name.clone()],
        AstPattern::Wildcard | AstPattern::Literal(_) | AstPattern::Range { .. } => Vec::new(),
        AstPattern::Tuple(subs, _) | AstPattern::Enum(_, subs) | AstPattern::EnumPath(_, subs) => {
            subs.iter().flat_map(pattern_bind_names).collect()
        }
        AstPattern::EnumStructPath(_, subs) => {
            subs.iter().flat_map(|(_, p)| pattern_bind_names(p)).collect()
        }
        AstPattern::Struct(_, fields) => fields
            .iter()
            .flat_map(|(_, p)| pattern_bind_names(p))
            .collect(),
        AstPattern::Ref(inner, _) => pattern_bind_names(inner),
        AstPattern::Or(alts) => alts.iter().flat_map(pattern_bind_names).collect(),
    }
}
