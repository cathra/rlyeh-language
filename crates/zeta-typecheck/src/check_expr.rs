//! 表达式类型推断与 HIR 生成。

use std::collections::HashMap;

use zeta_ast::{AssignOp, AstBlock, AstExpr, AstPattern, AstType, BinaryOp, ExprKind, UnaryOp};
use zeta_hir::{
    FieldScalar, HirAssignOp, HirBinaryOp, HirBlock, HirExpr, HirFnDecl, HirItem, HirItemKind,
    HirParam, HirRegionOptions, HirStmt, HirUnaryOp,
};
use zeta_lexer::Span;

use crate::comparison;
use crate::context::{FnTemplate, TypeContext};
use crate::error::TypeError;
use crate::in_expr;
use crate::types::{field_scalar_of, type_mono_key, FnSignature, ImplDef, Mutability, Type};

/// 查询内建函数签名；`None` 表示不是内建。
///
/// - `print` / `println`：任意类型参数（`Infer` 与一切兼容）、返回 `()`
/// - `alloc_array(n)`：运行时槽数分配，返回 `[T; 0]`（长度 0 约定 = 动态数组指针）
/// - `array_copy(dst, src, n)` / `array_free(p)`：动态数组缓冲操作
///
/// 与 `zeta-lir::lower::BUILTIN_FUNCTIONS`、`zeta-codegen` 保持一致。
pub fn builtin_signature(name: &str) -> Option<(Vec<Type>, Type)> {
    let dyn_arr = || Type::Array(Box::new(Type::Infer), 0);
    match name {
        "print" | "println" => Some((vec![Type::Infer], Type::Unit)),
        "alloc_array" => Some((vec![Type::I64], dyn_arr())),
        "array_copy" => Some((vec![dyn_arr(), dyn_arr(), Type::I64], Type::Unit)),
        "array_free" => Some((vec![dyn_arr()], Type::Unit)),
        // String 动态缓冲（按字节）：
        "alloc_bytes" => Some((vec![Type::I64], Type::Array(Box::new(Type::U8), 0))),
        "copy_bytes" => Some((vec![Type::Infer, Type::Infer, Type::I64], Type::Unit)),
        // String 内容相等：`bytes_eq(a, b, n)` → `memcmp(a, b, n) == 0`
        "bytes_eq" => Some((vec![Type::Infer, Type::Infer, Type::I64], Type::Bool)),
        // String 字典序：`bytes_cmp(a, b, n)` → `memcmp(a, b, n)` 有符号扩展为 i64
        // （负/零/正 → 小于/等于/大于；前缀相等时长度兜底由 desugar 层处理）
        "bytes_cmp" => Some((vec![Type::Infer, Type::Infer, Type::I64], Type::I64)),
        // String 打印：`print_string` / `println_string` 接收 String 对象指针
        "print_string" | "println_string" => Some((vec![Type::Infer], Type::Unit)),
        // HashMap 键散列：Knuth 乘法混合散列（MVP 仅支持整数键），返回非负散列值
        "hash_value" => Some((vec![Type::Infer], Type::I64)),
        _ => None,
    }
}

/// 推断表达式的类型并生成对应 HIR。
pub(crate) fn infer_expr(
    ctx: &mut TypeContext,
    expr: &AstExpr,
) -> Result<(HirExpr, Type), TypeError> {
    let span = expr.span;
    match &*expr.kind {
        ExprKind::IntLiteral(n) => Ok((HirExpr::IntLiteral(*n), Type::I64)),
        ExprKind::FloatLiteral(f) => Ok((HirExpr::FloatLiteral(*f), Type::F64)),
        ExprKind::StringLiteral(s) => Ok((HirExpr::StringLiteral(s.clone()), Type::Str)),
        ExprKind::CharLiteral(c) => Ok((HirExpr::CharLiteral(*c), Type::Char)),
        ExprKind::BoolLiteral(b) => Ok((HirExpr::BoolLiteral(*b), Type::Bool)),
        ExprKind::TimeLiteral { hour, minute, .. } => {
            // 时间字面量归一化为分钟值，按整数处理（可与整数集合/范围统一比较）
            let minutes = i128::from(*hour) * 60 + i128::from(*minute);
            Ok((HirExpr::IntLiteral(minutes), Type::I64))
        }

        ExprKind::Ident(name) => {
            // 1. 局部变量
            if let Some(ty) = ctx.lookup_variable(name).cloned() {
                return Ok((HirExpr::Variable(name.clone()), ty));
            }
            // 2. 常量引用：顶层常量名 → 当前模块内常量（`prefix::name`）
            if let Some((value, ty)) = ctx.lookup_constant(name) {
                return Ok((value.clone(), ty.clone()));
            }
            if !name.contains("::") && !ctx.module_prefix.is_empty() {
                let full = format!("{}::{}", ctx.module_prefix, name);
                if let Some((value, ty)) = ctx.lookup_constant(&full) {
                    return Ok((value.clone(), ty.clone()));
                }
            }
            Err(TypeError::UndefinedVariable {
                name: name.clone(),
                span,
            })
        }
        ExprKind::Path(segments) => {
            let path = segments.join("::");
            let resolved = resolve_callable(ctx, &path);
            // 无参枚举变体构造：`Option::None` / `shape::Kind::None`
            if !ctx.fn_signatures.contains_key(&resolved) {
                if let Some((en, vr)) = split_variant_path(ctx, &resolved) {
                    return check_variant_construct(ctx, &en, &vr, &[], span);
                }
            }
            // 模块常量引用：`math::ORIGIN`
            if let Some((value, ty)) = ctx.lookup_constant(&resolved).cloned() {
                return Ok((value, ty));
            }
            Err(TypeError::Unsupported {
                what: "路径表达式（`a::b::c`）".to_string(),
                span,
            })
        }
        ExprKind::Set(_) => Err(TypeError::Unsupported {
            what: "独立集合字面量（仅允许作为 `in` 右侧）".to_string(),
            span,
        }),
        ExprKind::Range {
            lower,
            upper,
            lower_inclusive: _,
            upper_inclusive: _,
        } => {
            let (_, lo_ty) = infer_expr(ctx, lower)?;
            let (hi_hir, hi_ty) = infer_expr(ctx, upper)?;
            if !lo_ty.compatible_with(&hi_ty) {
                return Err(TypeError::ChainTypeMismatch { span });
            }
            let _ = hi_hir;
            Ok((HirExpr::Unit, lo_ty))
        }

        ExprKind::Binary { op, left, right } => {
            let (l_hir, l_ty) = infer_expr(ctx, left)?;
            let (r_hir, r_ty) = infer_expr(ctx, right)?;
            // `a + b`（String + String）→ 拼接（拷贝语义，A3）：
            // `let __s = a.clone(); __s.push_str(b); __s`
            // clone 深拷贝左操作数到全新缓冲，消除共享缓冲别名隐患
            // （拼接结果与左操作数互不影响；push_str/clone 经方法实例化
            // 路径注册函数体）
            if *op == BinaryOp::Add
                && comparison::is_string_type(ctx, &l_ty)
                && comparison::is_string_type(ctx, &r_ty)
            {
                let s_name = ctx.fresh_temp();
                let clone_fn = {
                    let impl_def = ctx
                        .find_impl_for_method(&l_ty, "clone")
                        .cloned()
                        .ok_or_else(|| TypeError::FunctionNotFound {
                            name: "String::clone".to_string(),
                            span,
                        })?;
                    let method_def = impl_def
                        .methods
                        .iter()
                        .find(|m| m.sig.name == "clone")
                        .cloned()
                        .ok_or_else(|| TypeError::FunctionNotFound {
                            name: "String::clone".to_string(),
                            span,
                        })?;
                    instantiate_impl_method(ctx, &impl_def, &method_def, &HashMap::new(), span)?
                };
                let impl_def = ctx
                    .find_impl_for_method(&l_ty, "push_str")
                    .cloned()
                    .ok_or_else(|| TypeError::FunctionNotFound {
                        name: "String::push_str".to_string(),
                        span,
                    })?;
                let method_def = impl_def
                    .methods
                    .iter()
                    .find(|m| m.sig.name == "push_str")
                    .cloned()
                    .ok_or_else(|| TypeError::FunctionNotFound {
                        name: "String::push_str".to_string(),
                        span,
                    })?;
                let fn_name = instantiate_impl_method(ctx, &impl_def, &method_def, &HashMap::new(), span)?;
                let stmts = vec![
                    HirStmt::Let {
                        name: s_name.clone(),
                        init: HirExpr::Call {
                            callee: clone_fn,
                            args: vec![l_hir],
                        },
                        mutable: true,
                    },
                    HirStmt::Expr(HirExpr::Call {
                        callee: fn_name,
                        args: vec![HirExpr::Variable(s_name.clone()), r_hir],
                    }),
                ];
                let hir = HirExpr::Block(Box::new(HirBlock {
                    stmts,
                    final_expr: Some(HirExpr::Variable(s_name)),
                }));
                return Ok((hir, l_ty));
            }
            let (hir_op, result_ty) = check_binary(*op, &l_ty, &r_ty, span)?;
            let hir = HirExpr::Binary(hir_op, Box::new(l_hir), Box::new(r_hir));
            Ok((hir, result_ty))
        }
        ExprKind::Unary { op, operand } => {
            let (o_hir, o_ty) = infer_expr(ctx, operand)?;
            match op {
                UnaryOp::Neg => {
                    if !o_ty.is_numeric() {
                        return Err(TypeError::ExpectedNumeric {
                            found: o_ty.to_string(),
                            span,
                        });
                    }
                    Ok((HirExpr::Unary(HirUnaryOp::Neg, Box::new(o_hir)), o_ty))
                }
                UnaryOp::Not => {
                    if !o_ty.is_bool() {
                        return Err(TypeError::ExpectedBool {
                            found: o_ty.to_string(),
                            span,
                        });
                    }
                    Ok((HirExpr::Unary(HirUnaryOp::Not, Box::new(o_hir)), Type::Bool))
                }
                UnaryOp::Deref | UnaryOp::AddrOf | UnaryOp::AddrOfMut => {
                    Err(TypeError::Unsupported {
                        what: "引用与解引用（& / *）在 MVP 阶段".to_string(),
                        span,
                    })
                }
            }
        }

        ExprKind::ComparisonChain {
            elements,
            operators,
        } => comparison::check_comparison_chain(ctx, elements.clone(), operators.clone(), span),
        ExprKind::InSet {
            value,
            set,
            negated,
        } => in_expr::check_in_expression(ctx, value.clone(), set.clone(), *negated, span),
        ExprKind::InRange {
            value,
            range,
            negated,
        } => in_expr::check_in_range_expression(ctx, value.clone(), range.clone(), *negated, span),
        ExprKind::InRegion { expr, region } => {
            let (hir, ty) = infer_expr(ctx, expr)?;
            Ok((
                HirExpr::InRegion {
                    expr: Box::new(hir),
                    region: region.clone(),
                },
                ty,
            ))
        }

        ExprKind::Assign { target, op, value } => {
            let (t_hir, t_ty) = infer_expr(ctx, target)?;
            let (v_hir, v_ty) = infer_expr(ctx, value)?;
            if !t_ty.compatible_with(&v_ty) {
                return Err(TypeError::WrongType {
                    expected: t_ty.to_string(),
                    found: v_ty.to_string(),
                    span,
                });
            }
            let target_name = match t_hir {
                HirExpr::Variable(v) => v,
                // 结构体 / actor 状态字段赋值：`obj.field = value` → FieldSet；
                // 复合赋值 `obj.field += v` → FieldSet(base, idx, Binary(op, FieldGet, v))
                HirExpr::FieldGet { base, index, ty } => {
                    if !matches!(op, AssignOp::Assign) {
                        let hir_op = match op {
                            AssignOp::AddAssign => HirBinaryOp::Add,
                            AssignOp::SubAssign => HirBinaryOp::Sub,
                            AssignOp::MulAssign => HirBinaryOp::Mul,
                            AssignOp::DivAssign => HirBinaryOp::Div,
                            AssignOp::Assign => unreachable!(),
                        };
                        return Ok((
                            HirExpr::FieldSet {
                                base: base.clone(),
                                index,
                                value: Box::new(HirExpr::Binary(
                                    hir_op,
                                    Box::new(HirExpr::FieldGet {
                                        base,
                                        index,
                                        ty,
                                    }),
                                    Box::new(v_hir),
                                )),
                                ty,
                            },
                            Type::Unit,
                        ));
                    }
                    return Ok((
                        HirExpr::FieldSet {
                            base,
                            index,
                            value: Box::new(v_hir),
                            ty,
                        },
                        Type::Unit,
                    ));
                }
                // 索引元素赋值：`arr[i] = value` / `s[i] = ch`（仅纯赋值）
                HirExpr::Index {
                    base,
                    index,
                    elem,
                    is_str,
                } => {
                    if !matches!(op, AssignOp::Assign) {
                        return Err(TypeError::Unsupported {
                            what: "复合赋值目标为索引元素在 MVP 阶段（仅支持 `a[i] = ...`）"
                                .to_string(),
                            span,
                        });
                    }
                    return Ok((
                        HirExpr::IndexSet {
                            base,
                            index,
                            value: Box::new(v_hir),
                            elem,
                            is_str,
                        },
                        Type::Unit,
                    ));
                }
                _ => {
                    return Err(TypeError::Unsupported {
                        what: "非变量赋值目标在 MVP 阶段（仅支持 `x = ...`）".to_string(),
                        span,
                    });
                }
            };
            let hir_op = match op {
                AssignOp::Assign => HirAssignOp::Assign,
                AssignOp::AddAssign => HirAssignOp::AddAssign,
                AssignOp::SubAssign => HirAssignOp::SubAssign,
                AssignOp::MulAssign => HirAssignOp::MulAssign,
                AssignOp::DivAssign => HirAssignOp::DivAssign,
            };
            Ok((
                HirExpr::Assign {
                    target: target_name,
                    op: hir_op,
                    value: Box::new(v_hir),
                },
                Type::Unit,
            ))
        }

        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            let (c_hir, c_ty) = infer_expr(ctx, cond)?;
            if !c_ty.is_bool() {
                return Err(TypeError::ExpectedBool {
                    found: c_ty.to_string(),
                    span,
                });
            }
            let (t_hir, t_ty) = check_block(ctx, then_block)?;
            let (e_hir, e_ty) = match else_block {
                Some(b) => {
                    let (h, ty) = check_block(ctx, b)?;
                    (Some(h), Some(ty))
                }
                None => (None, None),
            };
            let result_ty = match (&e_ty, t_ty == Type::Unit) {
                (Some(et), false) if *et != t_ty => {
                    // then 与 else 分支类型不一致：
                    // - 数值类型 → 取合并类型（int/float 提升）；
                    // - 其余类型（聚合/引用等）→ 兼容时取更具体的那个（`_` 占位被具体类型吸收），
                    //   否则报错。注意不能对非数值调用 merge_numeric（会把 Option<i64> 合并成 i64）。
                    if t_ty.is_numeric() && et.is_numeric() {
                        merge_numeric(t_ty.clone(), et.clone())
                    } else if t_ty.compatible_with(et) {
                        if matches!(&t_ty, Type::Infer) {
                            et.clone()
                        } else {
                            t_ty
                        }
                    } else {
                        return Err(TypeError::WrongType {
                            expected: t_ty.to_string(),
                            found: et.to_string(),
                            span,
                        });
                    }
                }
                _ => t_ty,
            };
            let hir = HirExpr::If {
                cond: Box::new(c_hir),
                then_block: Box::new(t_hir),
                else_block: e_hir.map(Box::new),
            };
            Ok((hir, result_ty))
        }

        ExprKind::Match { expr, arms } => check_match(ctx, expr, arms, span),

        ExprKind::For {
            pattern,
            iterator,
            body,
        } => check_for(ctx, pattern, iterator, body, span),
        ExprKind::While { cond, body, .. } => {
            let (c_hir, c_ty) = infer_expr(ctx, cond)?;
            if !c_ty.is_bool() {
                return Err(TypeError::ExpectedBool {
                    found: c_ty.to_string(),
                    span,
                });
            }
            let (b_hir, _) = check_block(ctx, body)?;
            Ok((
                HirExpr::While {
                    cond: Box::new(c_hir),
                    body: Box::new(b_hir),
                },
                Type::Unit,
            ))
        }
        ExprKind::Loop { body, .. } => {
            let (b_hir, _) = check_block(ctx, body)?;
            Ok((
                HirExpr::Loop {
                    body: Box::new(b_hir),
                },
                Type::Never,
            ))
        }

        ExprKind::Region {
            name,
            options,
            body,
        } => {
            let (hir_block, ty) = check_block(ctx, body)?;
            Ok((
                HirExpr::Region {
                    name: name.clone(),
                    options: HirRegionOptions {
                        size: options.size,
                        allow_growth: options.allow_growth,
                        growth_factor: options.growth_factor,
                        adaptive: options.adaptive,
                        exact: options.exact,
                    },
                    body: Box::new(hir_block),
                },
                ty,
            ))
        }
        ExprKind::Transfer { expr, region } => {
            let (hir, ty) = infer_expr(ctx, expr)?;
            Ok((
                HirExpr::Transfer {
                    expr: Box::new(hir),
                    region: region.clone(),
                },
                ty,
            ))
        }

        ExprKind::Call { callee, args } => check_call(ctx, callee, args, span),
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => check_method_call(ctx, receiver, method, args, span),
        ExprKind::StructCtor { type_name, fields } => {
            check_struct_construct(ctx, type_name, fields, span)
        }
        ExprKind::FieldAccess { expr, field } => {
            let (base_hir, base_ty) = infer_expr(ctx, expr)?;
            check_field_access(ctx, base_hir, base_ty, field, span)
        }
        ExprKind::Index { expr, index } => check_index(ctx, expr, index, span),
        ExprKind::ArrayLit(elems) => check_array_lit(ctx, elems, span),
        ExprKind::Closure { .. } => Err(TypeError::Unsupported {
            what: "闭包在 MVP 阶段".to_string(),
            span,
        }),

        ExprKind::Cast { expr, target_type } => {
            let (hir, _) = infer_expr(ctx, expr)?;
            let ty = resolve_ast_type(ctx, target_type, span)?;
            Ok((hir, ty))
        }

        ExprKind::Await(inner) => infer_expr(ctx, inner),

        ExprKind::Block(block) => {
            let (hir, ty) = check_block(ctx, block)?;
            Ok((HirExpr::Block(Box::new(hir)), ty))
        }
        ExprKind::Return(Some(e)) => {
            let (hir, _) = infer_expr(ctx, e)?;
            Ok((HirExpr::Return(Some(Box::new(hir))), Type::Never))
        }
        ExprKind::Return(None) => Ok((HirExpr::Return(None), Type::Never)),
        ExprKind::Break(Some(e)) => {
            let (hir, _) = infer_expr(ctx, e)?;
            Ok((HirExpr::Break(Some(Box::new(hir))), Type::Never))
        }
        ExprKind::Break(None) => Ok((HirExpr::Break(None), Type::Never)),
        ExprKind::Continue => Ok((HirExpr::Continue, Type::Never)),

        ExprKind::Send { actor, method, args } => {
            // `send actor.method(a, b)` → `zeta_actor_send(recv, kind, a, b, 0)`
            // （异步发送不等待结果；参数经消息槽传递）
            let (recv_hir, recv_ty) = infer_expr(ctx, actor)?;
            let self_ty = peel_ref(&recv_ty);
            if let Type::Named(name, _) = &self_ty {
                if let Some(ad) = ctx.lookup_actor(name).cloned() {
                    let kind = ad
                        .methods
                        .iter()
                        .position(|m| m.name == *method)
                        .ok_or_else(|| TypeError::FunctionNotFound {
                            name: format!("{self_ty}::{method}"),
                            span,
                        })?;
                    if args.len() > 3 {
                        return Err(TypeError::UnexpectedArgumentCount {
                            name: format!("{self_ty}::{method}"),
                            expected: 3,
                            found: args.len(),
                            span,
                        });
                    }
                    let mut call_args = vec![recv_hir, HirExpr::IntLiteral(kind as i128)];
                    for arg in args {
                        let (h, t) = infer_expr(ctx, arg)?;
                        if !t.compatible_with(&Type::I64) {
                            return Err(TypeError::ArgumentTypeMismatch {
                                name: format!("{self_ty}::{method}"),
                                index: call_args.len() - 2,
                                expected: "i64".to_string(),
                                found: t.to_string(),
                                span: arg.span,
                            });
                        }
                        call_args.push(h);
                    }
                    while call_args.len() < 5 {
                        call_args.push(HirExpr::IntLiteral(0));
                    }
                    return Ok((
                        HirExpr::Call {
                            callee: "zeta_actor_send".to_string(),
                            args: call_args,
                        },
                        Type::I64,
                    ));
                }
            }
            Err(TypeError::Unsupported {
                what: "send 目标不是 Actor 类型".to_string(),
                span,
            })
        }
    }
}

/// 检查代码块并生成 HIR 块。
pub(crate) fn check_block(
    ctx: &mut TypeContext,
    block: &AstBlock,
) -> Result<(HirBlock, Type), TypeError> {
    let mut stmts = Vec::with_capacity(block.stmts.len());
    for stmt in &block.stmts {
        let (hir_stmt, _) = crate::check_stmt::check_stmt(ctx, stmt)?;
        stmts.push(hir_stmt);
    }
    let mut final_ty = Type::Unit;
    let mut final_expr = None;
    if let Some(e) = &block.final_expr {
        let (hir, ty) = infer_expr(ctx, e)?;
        final_ty = ty;
        final_expr = Some(hir);
    }
    Ok((HirBlock { stmts, final_expr }, final_ty))
}

/// 检查 for 循环：迭代器分派。
///
/// - range 表达式（`lo..<hi` 等）→ [`check_for_range`]（数值递增循环）
/// - `Vec<T>` 容器（`for x in v`）→ [`check_for_vec`]（索引遍历循环）
/// - 其余（HashMap 等，需元组模式解构）在 MVP 阶段待支持
fn check_for(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iterator: &AstExpr,
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if matches!(&*iterator.kind, ExprKind::Range { .. }) {
        return check_for_range(ctx, pattern, iterator, body, span);
    }
    let (iter_hir, iter_ty) = infer_expr(ctx, iterator)?;
    if let Type::Named(n, args) = peel_ref(&iter_ty) {
        let full = ctx.resolve_full_name(&n).unwrap_or_else(|| n.clone());
        if full == "Vec" && ctx.lookup_struct(&full).is_some() {
            return check_for_vec(ctx, pattern, iter_hir, &args, body, span);
        }
        if full == "HashMap" && ctx.lookup_struct(&full).is_some() {
            return check_for_hashmap(ctx, pattern, iter_hir, &args, body, span);
        }
    }
    Err(TypeError::Unsupported {
        what: "非 range / Vec / HashMap 迭代器的 for 循环（集合 / 容器迭代 MVP 阶段仅支持 Vec 与 HashMap）"
            .to_string(),
        span,
    })
}

/// 检查 range 迭代的 for 循环：`for pat in lo..<hi { body }`。
///
/// MVP 阶段支持 range 迭代器（`..<` / `...` / `<..` / `<..<`），
/// 在类型检查层 desugar 为 `loop`：
///
/// ```zeta
/// let __for_lo = lo;
/// let __for_hi = hi;
/// let mut pat = start - 1;    // start = lo（下界闭）或 lo + 1（下界开）
/// loop {
///     pat += 1;               // continue 回跳也执行，避免跳过递增死循环
///     if pat >= hi { break; } // 上界闭区间时为 `>`
///     body
/// }
/// ```
fn check_for_range(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iterator: &AstExpr,
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 1. 迭代器必须为 range 表达式（集合 / 容器迭代待后续支持）
    let (lower, upper, lower_inclusive, upper_inclusive) = match &*iterator.kind {
        ExprKind::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => (lower, upper, *lower_inclusive, *upper_inclusive),
        _ => {
            return Err(TypeError::Unsupported {
                what: "非 range 迭代器的 for 循环（集合 / 容器迭代在 MVP 阶段）".to_string(),
                span,
            })
        }
    };

    // 2. 循环变量必须是标识符
    let name = match pattern {
        AstPattern::Ident(n) => n.clone(),
        _ => {
            return Err(TypeError::Unsupported {
                what: "for 循环复杂模式（元组 / 结构体解构）".to_string(),
                span,
            })
        }
    };

    // 3. 边界表达式类型检查（必须为整数且类型一致）
    let (lo_hir, lo_ty) = infer_expr(ctx, lower)?;
    let (hi_hir, hi_ty) = infer_expr(ctx, upper)?;
    if !lo_ty.is_integer() || !hi_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: lo_ty.to_string(),
            span,
        });
    }
    if !lo_ty.compatible_with(&hi_ty) {
        return Err(TypeError::ChainTypeMismatch { span });
    }

    // 4. 唯一临时名（避免与用户变量冲突）
    let lo_name = format!("__for_lo_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let hi_name = format!("__for_hi_{}", ctx.temp_counter);
    ctx.temp_counter += 1;

    // 5. 起始值：下界开区间时从 lo + 1 开始；
    //    循环变量初始化为 `start - 1`，配合循环体开头的 `pat += 1`，
    //    使第一次迭代 pat == start。
    let start = if lower_inclusive {
        HirExpr::Variable(lo_name.clone())
    } else {
        HirExpr::Binary(
            HirBinaryOp::Add,
            Box::new(HirExpr::Variable(lo_name.clone())),
            Box::new(HirExpr::IntLiteral(1)),
        )
    };
    let init = HirExpr::Binary(
        HirBinaryOp::Sub,
        Box::new(start),
        Box::new(HirExpr::IntLiteral(1)),
    );

    let mut stmts = vec![
        HirStmt::Let {
            name: lo_name.clone(),
            init: lo_hir,
            mutable: false,
        },
        HirStmt::Let {
            name: hi_name.clone(),
            init: hi_hir,
            mutable: false,
        },
        HirStmt::Let {
            name: name.clone(),
            init,
            mutable: true,
        },
    ];

    // 6. 循环体检查（迭代变量在作用域内）
    ctx.insert_variable(lo_name.clone(), lo_ty.clone());
    ctx.insert_variable(hi_name.clone(), hi_ty.clone());
    ctx.insert_variable(name.clone(), lo_ty.clone());
    let (b_hir, _) = check_block(ctx, body)?;
    ctx.variables.remove(&name);
    ctx.variables.remove(&lo_name);
    ctx.variables.remove(&hi_name);

    // 7. 退出条件：`pat >= hi`（上界闭区间为 `pat > hi`）。
    //    注意用 `loop` 而非 `while`：循环体开头的 `pat += 1` 在每次
    //    迭代（含 continue 回跳）时都会执行，保证 continue 不会跳过递增。
    let exit_op = if upper_inclusive {
        HirBinaryOp::Gt
    } else {
        HirBinaryOp::Ge
    };
    let exit_cond = HirExpr::Binary(
        exit_op,
        Box::new(HirExpr::Variable(name.clone())),
        Box::new(HirExpr::Variable(hi_name)),
    );

    // 8. loop 体：`pat += 1` → 退出判断 → 原 body 语句
    let mut loop_body_stmts = vec![
        HirStmt::Expr(HirExpr::Assign {
            target: name.clone(),
            op: HirAssignOp::AddAssign,
            value: Box::new(HirExpr::IntLiteral(1)),
        }),
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(exit_cond),
            then_block: Box::new(HirBlock {
                stmts: vec![HirStmt::Expr(HirExpr::Break(None))],
                final_expr: None,
            }),
            else_block: None,
        }),
    ];
    loop_body_stmts.extend(b_hir.stmts);
    if let Some(fe) = b_hir.final_expr {
        loop_body_stmts.push(HirStmt::Expr(fe));
    }
    let loop_expr = HirExpr::Loop {
        body: Box::new(HirBlock {
            stmts: loop_body_stmts,
            final_expr: None,
        }),
    };

    stmts.push(HirStmt::Expr(loop_expr));
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: None,
        })),
        Type::Unit,
    ))
}

/// 检查 `Vec<T>` 容器迭代的 for 循环：`for pat in v { body }`。
///
/// 在类型检查层 desugar 为索引遍历循环（复用 Vec 3 槽布局：
/// 槽 0 = data 指针、槽 1 = len、槽 2 = cap）：
///
/// ```zeta
/// let __for_v = v;             // 绑定容器（防迭代器重复求值）
/// let __for_len = __for_v.len; // 缓存长度（槽 1）
/// let mut __for_i = 0;
/// loop {
///     if __for_i >= __for_len { break; }
///     let pat = __for_v[__for_i]; // Index：槽 0 data 指针 + 元素步长 8
///     __for_i += 1;               // continue 回跳前已递增，不会死循环
///     body
/// }
/// ```
fn check_for_vec(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iter_hir: HirExpr,
    args: &[Type],
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 1. 元素类型：`Vec<T>` 的类型参数经泛型替换；Infer 无法确定槽标量
    let elem_ty = substitute(args.first().unwrap_or(&Type::Infer), &ctx.generic_subst);
    if matches!(elem_ty, Type::Infer) {
        return Err(TypeError::Unsupported {
            what: "`for ... in` 的 Vec 迭代要求元素类型确定（如 `let v: Vec<i64> = ...`）"
                .to_string(),
            span,
        });
    }

    // 2. 循环变量必须是标识符
    let name = match pattern {
        AstPattern::Ident(n) => n.clone(),
        _ => {
            return Err(TypeError::Unsupported {
                what: "for 循环复杂模式（元组 / 结构体解构）".to_string(),
                span,
            })
        }
    };

    // 3. 唯一临时名（避免与用户变量冲突）
    let v_name = format!("__for_v_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let len_name = format!("__for_len_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let i_name = format!("__for_i_{}", ctx.temp_counter);
    ctx.temp_counter += 1;

    // 4. 前缀语句：绑定容器、缓存长度、初始化计数器
    let mut stmts = vec![
        HirStmt::Let {
            name: v_name.clone(),
            init: iter_hir,
            mutable: false,
        },
        HirStmt::Let {
            name: len_name.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(v_name.clone())),
                index: 1, // Vec 槽 1 = len
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: i_name.clone(),
            init: HirExpr::IntLiteral(0),
            mutable: true,
        },
    ];

    // 5. 循环体检查（容器 / 长度 / 计数器 / 迭代变量在作用域内）
    let vec_ty = Type::Named("Vec".to_string(), vec![elem_ty.clone()]);
    ctx.insert_variable(v_name.clone(), vec_ty);
    ctx.insert_variable(len_name.clone(), Type::I64);
    ctx.insert_variable(i_name.clone(), Type::I64);
    ctx.insert_variable(name.clone(), elem_ty.clone());
    let (b_hir, _) = check_block(ctx, body)?;
    for var in [&name, &i_name, &len_name, &v_name] {
        ctx.variables.remove(var);
    }

    // 6. loop 体：边界检查 → 取元素绑定 → 递增 → 原 body 语句
    let elem_scalar = field_scalar_of(&elem_ty);
    let mut loop_body_stmts = vec![
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Ge,
                Box::new(HirExpr::Variable(i_name.clone())),
                Box::new(HirExpr::Variable(len_name.clone())),
            )),
            then_block: Box::new(HirBlock {
                stmts: vec![HirStmt::Expr(HirExpr::Break(None))],
                final_expr: None,
            }),
            else_block: None,
        }),
        HirStmt::Let {
            name: name.clone(),
            init: HirExpr::Index {
                base: Box::new(HirExpr::FieldGet {
                    base: Box::new(HirExpr::Variable(v_name.clone())),
                    index: 0, // Vec 槽 0 = data 指针
                    ty: FieldScalar::Ptr,
                }),
                index: Box::new(HirExpr::Variable(i_name.clone())),
                elem: elem_scalar,
                is_str: false,
            },
            mutable: false,
        },
        HirStmt::Expr(HirExpr::Assign {
            target: i_name.clone(),
            op: HirAssignOp::AddAssign,
            value: Box::new(HirExpr::IntLiteral(1)),
        }),
    ];
    loop_body_stmts.extend(b_hir.stmts);
    if let Some(fe) = b_hir.final_expr {
        loop_body_stmts.push(HirStmt::Expr(fe));
    }
    let loop_expr = HirExpr::Loop {
        body: Box::new(HirBlock {
            stmts: loop_body_stmts,
            final_expr: None,
        }),
    };

    stmts.push(HirStmt::Expr(loop_expr));
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: None,
        })),
        Type::Unit,
    ))
}

/// 检查 `HashMap<K, V>` 容器迭代的 for 循环：`for (k, v) in m { body }`。
///
/// 在类型检查层 desugar 为索引遍历循环（复用 HashMap 6 槽布局：
/// 槽 0 = keys 指针、槽 1 = vals 指针、槽 2 = states 指针（0=空 1=占用 2=墓碑）、
/// 槽 3 = len、槽 4 = used、槽 5 = cap）。HashMap 是稀疏存储（删除产生墓碑），
/// 遍历必须按容量扫描并跳过 `states[i] != 1` 的空槽 / 墓碑：
///
/// ```zeta
/// let __for_m = m;               // 绑定容器（防迭代器重复求值）
/// let __for_cap = __for_m.cap;   // 容量（槽 5，含墓碑槽）
/// let mut __for_i = 0;
/// loop {
///     if __for_i >= __for_cap { break; }
///     if __for_m.states[__for_i] != 1 { __for_i += 1; continue; } // 跳槽
///     let k = __for_m.keys[__for_i];  // keys 指针（槽 0）+ 步长 8
///     let v = __for_m.vals[__for_i];  // vals 指针（槽 1）+ 步长 8
///     __for_i += 1;                   // continue 回跳前已递增，无死循环
///     body
/// }
/// ```
fn check_for_hashmap(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iter_hir: HirExpr,
    args: &[Type],
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 1. 键值类型：`HashMap<K, V>` 的类型参数经泛型替换；Infer 无法确定槽标量
    let k_ty = substitute(args.first().unwrap_or(&Type::Infer), &ctx.generic_subst);
    let v_ty = substitute(args.get(1).unwrap_or(&Type::Infer), &ctx.generic_subst);
    if matches!(k_ty, Type::Infer) || matches!(v_ty, Type::Infer) {
        return Err(TypeError::Unsupported {
            what: "`for ... in` 的 HashMap 迭代要求键值类型确定（如 `let m: HashMap<i64, i64> = ...`）"
                .to_string(),
            span,
        });
    }

    // 2. 模式必须为 `(k, v)` 二元元组，元素均为标识符
    let (k_name, v_name) = match pattern {
        AstPattern::Tuple(pats) if pats.len() == 2 => {
            match (&pats[0], &pats[1]) {
                (AstPattern::Ident(k), AstPattern::Ident(v)) => (k.clone(), v.clone()),
                _ => {
                    return Err(TypeError::Unsupported {
                        what: "HashMap 迭代模式必须为 `(k, v)` 标识符对".to_string(),
                        span,
                    })
                }
            }
        }
        _ => {
            return Err(TypeError::Unsupported {
                what: "HashMap 迭代必须使用 `for (k, v) in m` 元组模式".to_string(),
                span,
            })
        }
    };

    // 3. 唯一临时名（避免与用户变量冲突）
    let m_name = format!("__for_m_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let cap_name = format!("__for_cap_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let i_name = format!("__for_i_{}", ctx.temp_counter);
    ctx.temp_counter += 1;

    // 4. 前缀语句：绑定容器、缓存容量、初始化计数器
    let mut stmts = vec![
        HirStmt::Let {
            name: m_name.clone(),
            init: iter_hir,
            mutable: false,
        },
        HirStmt::Let {
            name: cap_name.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(m_name.clone())),
                index: 5, // HashMap 槽 5 = cap
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: i_name.clone(),
            init: HirExpr::IntLiteral(0),
            mutable: true,
        },
    ];

    // 5. 循环体检查（容器 / 容量 / 计数器 / k / v 在作用域内）
    let map_ty = Type::Named("HashMap".to_string(), vec![k_ty.clone(), v_ty.clone()]);
    ctx.insert_variable(m_name.clone(), map_ty);
    ctx.insert_variable(cap_name.clone(), Type::I64);
    ctx.insert_variable(i_name.clone(), Type::I64);
    ctx.insert_variable(k_name.clone(), k_ty.clone());
    ctx.insert_variable(v_name.clone(), v_ty.clone());
    let (b_hir, _) = check_block(ctx, body)?;
    for var in [&k_name, &v_name, &i_name, &cap_name, &m_name] {
        ctx.variables.remove(var);
    }

    // 6. loop 体：边界检查 → 跳槽 → 绑定 k/v → 递增 → 原 body 语句
    let k_scalar = field_scalar_of(&k_ty);
    let v_scalar = field_scalar_of(&v_ty);
    let mut loop_body_stmts = vec![
        // if __for_i >= __for_cap { break }
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Ge,
                Box::new(HirExpr::Variable(i_name.clone())),
                Box::new(HirExpr::Variable(cap_name.clone())),
            )),
            then_block: Box::new(HirBlock {
                stmts: vec![HirStmt::Expr(HirExpr::Break(None))],
                final_expr: None,
            }),
            else_block: None,
        }),
        // if __for_m.states[__for_i] != 1 { __for_i += 1; continue; }
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Ne,
                Box::new(HirExpr::Index {
                    base: Box::new(HirExpr::FieldGet {
                        base: Box::new(HirExpr::Variable(m_name.clone())),
                        index: 2, // HashMap 槽 2 = states 指针
                        ty: FieldScalar::Ptr,
                    }),
                    index: Box::new(HirExpr::Variable(i_name.clone())),
                    elem: FieldScalar::Int,
                    is_str: false,
                }),
                Box::new(HirExpr::IntLiteral(1)),
            )),
            then_block: Box::new(HirBlock {
                stmts: vec![
                    HirStmt::Expr(HirExpr::Assign {
                        target: i_name.clone(),
                        op: HirAssignOp::AddAssign,
                        value: Box::new(HirExpr::IntLiteral(1)),
                    }),
                    HirStmt::Expr(HirExpr::Continue),
                ],
                final_expr: None,
            }),
            else_block: None,
        }),
        // let k = __for_m.keys[__for_i]
        HirStmt::Let {
            name: k_name.clone(),
            init: HirExpr::Index {
                base: Box::new(HirExpr::FieldGet {
                    base: Box::new(HirExpr::Variable(m_name.clone())),
                    index: 0, // HashMap 槽 0 = keys 指针
                    ty: FieldScalar::Ptr,
                }),
                index: Box::new(HirExpr::Variable(i_name.clone())),
                elem: k_scalar,
                is_str: false,
            },
            mutable: false,
        },
        // let v = __for_m.vals[__for_i]
        HirStmt::Let {
            name: v_name.clone(),
            init: HirExpr::Index {
                base: Box::new(HirExpr::FieldGet {
                    base: Box::new(HirExpr::Variable(m_name.clone())),
                    index: 1, // HashMap 槽 1 = vals 指针
                    ty: FieldScalar::Ptr,
                }),
                index: Box::new(HirExpr::Variable(i_name.clone())),
                elem: v_scalar,
                is_str: false,
            },
            mutable: false,
        },
        // __for_i += 1
        HirStmt::Expr(HirExpr::Assign {
            target: i_name.clone(),
            op: HirAssignOp::AddAssign,
            value: Box::new(HirExpr::IntLiteral(1)),
        }),
    ];
    loop_body_stmts.extend(b_hir.stmts);
    if let Some(fe) = b_hir.final_expr {
        loop_body_stmts.push(HirStmt::Expr(fe));
    }
    let loop_expr = HirExpr::Loop {
        body: Box::new(HirBlock {
            stmts: loop_body_stmts,
            final_expr: None,
        }),
    };

    stmts.push(HirStmt::Expr(loop_expr));
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: None,
        })),
        Type::Unit,
    ))
}

/// 检查二元运算：运算符与操作数类型。
fn check_binary(
    op: BinaryOp,
    left: &Type,
    right: &Type,
    span: Span,
) -> Result<(HirBinaryOp, Type), TypeError> {
    let hir_op = match op {
        BinaryOp::Add => HirBinaryOp::Add,
        BinaryOp::Sub => HirBinaryOp::Sub,
        BinaryOp::Mul => HirBinaryOp::Mul,
        BinaryOp::Div => HirBinaryOp::Div,
        BinaryOp::Mod => HirBinaryOp::Mod,
        BinaryOp::And | BinaryOp::Or => {
            if !left.is_bool() || !right.is_bool() {
                return Err(TypeError::ExpectedBool {
                    found: left.to_string(),
                    span,
                });
            }
            let hir_op = if op == BinaryOp::And {
                HirBinaryOp::And
            } else {
                HirBinaryOp::Or
            };
            return Ok((hir_op, Type::Bool));
        }
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
            // 位运算：要求整数操作数，结果为整数
            if !left.is_integer() || !right.is_integer() {
                return Err(TypeError::ExpectedInt {
                    found: left.to_string(),
                    span,
                });
            }
            let hir_op = match op {
                BinaryOp::BitAnd => HirBinaryOp::BitAnd,
                BinaryOp::BitOr => HirBinaryOp::BitOr,
                BinaryOp::BitXor => HirBinaryOp::BitXor,
                BinaryOp::Shl => HirBinaryOp::Shl,
                _ => HirBinaryOp::Shr,
            };
            return Ok((hir_op, left.clone()));
        }
    };

    // 算术运算：要求数值类型
    if !left.is_numeric() || !right.is_numeric() {
        return Err(TypeError::ExpectedNumeric {
            found: left.to_string(),
            span,
        });
    }
    if !left.compatible_with(right) {
        return Err(TypeError::WrongType {
            expected: left.to_string(),
            found: right.to_string(),
            span,
        });
    }
    Ok((hir_op, merge_numeric(left.clone(), right.clone())))
}

/// 合并两个兼容的数值类型（浮点优先）。
fn merge_numeric(a: Type, b: Type) -> Type {
    if a.is_float() || b.is_float() {
        Type::F64
    } else {
        Type::I64
    }
}

/// 检查函数 / 宏调用。
fn check_call(
    ctx: &mut TypeContext,
    callee: &AstExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let name = match &*callee.kind {
        ExprKind::Ident(n) => n.clone(),
        // 模块路径调用：`math::add(...)`
        ExprKind::Path(segments) => segments.join("::"),
        _ => {
            return Err(TypeError::Unsupported {
                what: "复杂被调用表达式".to_string(),
                span,
            });
        }
    };

    // 内建函数（`print` / `println` / `alloc_array` 等，由代码生成层映射到运行时）：
    // 按签名检查参数、返回签名类型
    if let Some((params, ret)) = builtin_signature(&name) {
        // `print` / `println` 允许 0..=1 个参数（`println()` 打印空行）
        let is_print = name == "print" || name == "println";
        let max_args = if is_print { 1 } else { params.len() };
        if args.len() > max_args {
            return Err(TypeError::UnexpectedArgumentCount {
                name: name.clone(),
                expected: max_args,
                found: args.len(),
                span,
            });
        }
        // `print(s)` / `println(s)` 参数为 String → 展开为 `print_string` / `println_string`
        // 内建（动态缓冲按 `%.*s` 打印；typecheck 无法在内建签名层表达对象槽读取）
        if is_print && args.len() == 1 {
            let (hir, ty) = infer_expr(ctx, &args[0])?;
            if let Type::Named(n, _) = peel_ref(&ty) {
                let full = ctx
                    .resolve_full_name(&n)
                    .unwrap_or_else(|| n.clone());
                if full == "String" && ctx.lookup_struct(&full).is_some() {
                    let callee = if name == "println" {
                        "println_string"
                    } else {
                        "print_string"
                    };
                    return Ok((
                        HirExpr::Call {
                            callee: callee.to_string(),
                            args: vec![hir],
                        },
                        Type::Unit,
                    ));
                }
            }
        }
        // `hash_value(s)` 参数为 String → 展开为 djb2 内容哈希（逐字节散列，
        // 同一内容字符串恒同哈希，保证 HashMap 探测链正确；字节索引 `s[i]`
        // 步长 1，typecheck 无法在内建签名层表达对象槽读取 + 循环）
        if name == "hash_value" && args.len() == 1 {
            let (hir, ty) = infer_expr(ctx, &args[0])?;
            if let Type::Named(n, _) = peel_ref(&ty) {
                let full = ctx
                    .resolve_full_name(&n)
                    .unwrap_or_else(|| n.clone());
                if full == "String" && ctx.lookup_struct(&full).is_some() {
                    let hash = string_hash_hir(ctx, &hir);
                    return Ok((hash, Type::I64));
                }
            }
        }
        let mut hir_args = Vec::with_capacity(args.len());
        for (i, (a, pty)) in args.iter().zip(&params).enumerate() {
            let (hir, ty) = infer_expr(ctx, a)?;
            if !ty.compatible_with(pty) {
                return Err(TypeError::ArgumentTypeMismatch {
                    name: name.clone(),
                    index: i,
                    expected: pty.to_string(),
                    found: ty.to_string(),
                    span: a.span,
                });
            }
            hir_args.push(hir);
        }
        return Ok((
            HirExpr::Call {
                callee: name,
                args: hir_args,
            },
            ret,
        ));
    }

    // 宏调用（`println!` 等）：检查参数、返回 `()`
    if name.ends_with('!') {
        for a in args {
            let (_, _) = infer_expr(ctx, a)?;
        }
        return Ok((
            HirExpr::Call {
                callee: name,
                args: Vec::new(),
            },
            Type::Unit,
        ));
    }

    // actor 构造函数：`Counter::new()` → `zeta_actor_spawn("<handle>", <state_new>())`；
    // `Counter::new_supervised(strategy)` → `zeta_actor_spawn_supervised("<handle>", "<state_new>", strategy)`
    if let Some((actor_part, seg)) = name.rsplit_once("::") {
        if seg == "new" || seg == "new_supervised" {
            if let Some(actor_full) = ctx.lookup_actor(actor_part).map(|_| {
                ctx.resolve_full_name(actor_part)
                    .unwrap_or_else(|| actor_part.to_string())
            }) {
                let supervised = seg == "new_supervised";
                // 受监督构造必须提供策略参数（i64）：0=OneForOne 1=AllForOne 2=RestartForOne
                let strategy_hir = if supervised {
                    if args.len() != 1 {
                        return Err(TypeError::Unsupported {
                            what: format!("`{actor_part}::new_supervised` 需要 1 个策略参数（i64）"),
                            span,
                        });
                    }
                    let (s_hir, s_ty) = infer_expr(ctx, &args[0])?;
                    if !matches!(s_ty, Type::I64) {
                        return Err(TypeError::Unsupported {
                            what: "actor 监督策略参数必须是 i64".to_string(),
                            span: args[0].span,
                        });
                    }
                    s_hir
                } else {
                    if !args.is_empty() {
                        return Err(TypeError::Unsupported {
                            what: format!("`{actor_part}::new` 不接受参数"),
                            span,
                        });
                    }
                    HirExpr::IntLiteral(0)
                };
                // handler / factory 名必须为 3 槽 String 结构体（data/len/cap）——runtime 侧
                // `cstr()` 按 C 字符串读取。不能传裸字符串字面量（瘦 data 指针，
                // codegen 对 extern `String` 参数会按结构体再解引用一层）。
                // 复用 `String::from` 展开（alloc_bytes + copy_bytes + 三槽构造）。
                let handle = format!("{actor_full}::__handle");
                let (handle_hir, _) = check_string_from(
                    ctx,
                    &[AstExpr {
                        kind: Box::new(ExprKind::StringLiteral(handle)),
                        span,
                    }],
                    span,
                )?;
                let state_new_call = HirExpr::Call {
                    callee: format!("{actor_full}::__state_new"),
                    args: vec![],
                };
                let (callee, args) = if supervised {
                    let factory = format!("{actor_full}::__state_new");
                    let (factory_hir, _) = check_string_from(
                        ctx,
                        &[AstExpr {
                            kind: Box::new(ExprKind::StringLiteral(factory)),
                            span,
                        }],
                        span,
                    )?;
                    (
                        "zeta_actor_spawn_supervised".to_string(),
                        vec![handle_hir, factory_hir, strategy_hir],
                    )
                } else {
                    ("zeta_actor_spawn".to_string(), vec![handle_hir, state_new_call])
                };
                return Ok((
                    HirExpr::Call { callee, args },
                    Type::Named(actor_full, vec![]),
                ));
            }
        }
    }

    // 普通函数调用：先经 use 别名 / 模块路径解析到完整符号名，再查签名
    let resolved = resolve_callable(ctx, &name);

    // 枚举变体构造：`Option::Some(x)`、`shape::Kind::Pair(x, y)` 或裸 `Some(x)`
    // （普通函数同名时优先函数路径）
    if !ctx.fn_signatures.contains_key(&resolved) {
        if let Some((en, vr)) = split_variant_path(ctx, &resolved) {
            return check_variant_construct(ctx, &en, &vr, args, span);
        }
    }

    // `Vec` 构造器特判：`Vec::with_capacity(n)` / `Vec::new()`
    // （泛型 impl 静态方法 MVP 不支持，编译器直接展开为动态数组分配 + 结构体构造）
    if let Some((ty_name, method)) = resolved.split_once("::") {
        let ty_full = ctx
            .resolve_full_name(ty_name)
            .unwrap_or_else(|| ty_name.to_string());
        if ty_full == "Vec" && ctx.lookup_struct(&ty_full).is_some()
            && (method == "with_capacity" || method == "new")
        {
            return check_vec_construct(ctx, method, args, span);
        }
        // `String` 构造器特判：`new()` / `with_capacity(n)` / `from("字面量")`
        if ty_full == "String" && ctx.lookup_struct(&ty_full).is_some() {
            match method {
                "new" | "with_capacity" => {
                    return check_string_construct(ctx, method, args, span);
                }
                "from" => return check_string_from(ctx, args, span),
                _ => {}
            }
        }
        // `HashMap` 构造器特判：`new()` / `with_capacity(n)`（6 槽开放寻址哈希表）
        if ty_full == "HashMap" && ctx.lookup_struct(&ty_full).is_some()
            && (method == "with_capacity" || method == "new")
        {
            return check_hashmap_construct(ctx, method, args, span);
        }
    }

    // 静态方法调用：`Point::origin()`（impl 中无 self 的方法）
    // （仅当 `Type::method` 不是普通函数/泛型模板时才走此路径）
    if !ctx.fn_signatures.contains_key(&resolved) && !ctx.fn_templates.contains_key(&resolved) {
        if let Some((ty_name, method)) = resolved.split_once("::") {
            let ty_full = ctx
                .resolve_full_name(ty_name)
                .unwrap_or_else(|| ty_name.to_string());
            if ctx.lookup_struct(&ty_full).is_some() || ctx.lookup_enum(&ty_full).is_some() {
                return check_static_method_call(ctx, &ty_full, method, args, span);
            }
        }
    }

    // 泛型函数模板：调用点按实参类型实例化
    if ctx.fn_templates.contains_key(&resolved) {
        return check_generic_call(ctx, &resolved, args, span);
    }

    let signature =
        ctx.lookup_fn_signature(&resolved)
            .cloned()
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: name.clone(),
                span,
            })?;
    if args.len() != signature.params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name,
            expected: signature.params.len(),
            found: args.len(),
            span,
        });
    }
    let mut hir_args = Vec::with_capacity(args.len());
    for (i, (arg, param_ty)) in args.iter().zip(&signature.params).enumerate() {
        let (hir, ty) = infer_expr(ctx, arg)?;
        if !ty.compatible_with(param_ty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: name.clone(),
                index: i,
                expected: param_ty.to_string(),
                found: ty.to_string(),
                span: arg.span,
            });
        }
        hir_args.push(hir);
    }
    Ok((
        HirExpr::Call {
            callee: resolved,
            args: hir_args,
        },
        signature.return_type,
    ))
}

/// 将调用名解析为完整符号名（use 导入别名 / 模块路径 → 目标符号）。
fn resolve_callable(ctx: &TypeContext, name: &str) -> String {
    if ctx.fn_signatures.contains_key(name) {
        return name.to_string();
    }
    // 模块内函数引用：裸名回退到 `prefix::name`
    if !name.contains("::") && !ctx.module_prefix.is_empty() {
        let full = format!("{}::{}", ctx.module_prefix, name);
        if ctx.fn_signatures.contains_key(&full) {
            return full;
        }
    }
    ctx.use_aliases
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

/// 解析枚举变体路径：`Enum::Variant` / `mod::Enum::Variant` / 裸 `Variant`。
///
/// 从**右侧**拆分最后一个 `::`（`rsplit_once`），使多段模块路径
/// `shape::Kind::None` 正确拆为枚举 `shape::Kind` + 变体 `None`；
/// 2 段 `Kind::None` 与 1 段裸 `None` 也统一处理。枚举名经
/// [`TypeContext::resolve_full_name`] 解析（use 导入别名 → 完整符号名）。
fn split_variant_path(ctx: &TypeContext, resolved: &str) -> Option<(String, String)> {
    if let Some((en, vr)) = resolved.rsplit_once("::") {
        let en_full = ctx
            .resolve_full_name(en)
            .unwrap_or_else(|| en.to_string());
        if ctx.enum_defs.contains_key(&en_full) {
            return Some((en_full, vr.to_string()));
        }
    }
    ctx.resolve_variant(None, resolved)
}

/// 将 AST 类型解析为内部类型表示。
pub(crate) fn resolve_ast_type(
    ctx: &TypeContext,
    ty: &AstType,
    span: Span,
) -> Result<Type, TypeError> {
    match ty {
        AstType::Path(name, args) => {
            if args.is_empty() {
                ctx.resolve_named_type(name, span)
            } else {
                let mut resolved = Vec::with_capacity(args.len());
                for a in args {
                    resolved.push(resolve_ast_type(ctx, a, span)?);
                }
                Ok(Type::Named(name.clone(), resolved))
            }
        }
        AstType::Ref(inner, is_mut) => {
            let inner = resolve_ast_type(ctx, inner, span)?;
            let m = if *is_mut {
                Mutability::Mutable
            } else {
                Mutability::Immutable
            };
            Ok(Type::Ref(Box::new(inner), m))
        }
        AstType::Tuple(ts) => {
            let mut resolved = Vec::with_capacity(ts.len());
            for t in ts {
                resolved.push(resolve_ast_type(ctx, t, span)?);
            }
            Ok(Type::Tuple(resolved))
        }
        AstType::Array(inner, size) => {
            let inner = resolve_ast_type(ctx, inner, span)?;
            // MVP：数组大小仅支持整数字面量（`[T; N]`）
            let n = match size {
                Some(e) => match *e.kind {
                    ExprKind::IntLiteral(v) if v >= 0 => v as usize,
                    _ => {
                        return Err(TypeError::Unsupported {
                            what: "数组大小非常量整数字面量在 MVP 阶段".to_string(),
                            span,
                        })
                    }
                },
                None => {
                    return Err(TypeError::Unsupported {
                        what: "数组类型缺少大小 `[T; N]`".to_string(),
                        span,
                    })
                }
            };
            Ok(Type::Array(Box::new(inner), n))
        }
        AstType::Fn(_, _) => Err(TypeError::Unsupported {
            what: "函数类型 `fn(A) -> B` 在 MVP 阶段".to_string(),
            span,
        }),
        AstType::Infer => Ok(Type::Infer),
    }
}

// ===========================================================================
// 聚合类型支持：结构体构造/字段访问、枚举变体构造、match 表达式、
// impl 方法调用、泛型实例化
// ===========================================================================

/// 结构体字面量构造：`Point { x: 3, y: 4 }` → 堆对象 `Alloc + 字段槽` 序列。
///
/// 与枚举不同，结构体无判别槽（槽 0 起即字段）。返回对象指针（HIR 块）
/// 与结构体类型 `Named(name, [])`。
fn check_struct_construct(
    ctx: &mut TypeContext,
    type_name: &[String],
    fields: &[(String, AstExpr)],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let struct_name = type_name.join("::");
    // 支持 use 导入别名与模块路径（如 std 拆分后 `time::Instant { ... }`）：
    // 裸名构造 `Duration { ... }` 经别名解析为完整符号名 `time::Duration`。
    let struct_name = ctx
        .resolve_full_name(&struct_name)
        .unwrap_or(struct_name);
    let def = ctx.lookup_struct(&struct_name).cloned().ok_or_else(|| {
        TypeError::UndefinedType {
            name: struct_name.clone(),
            span,
        }
    })?;

    // 未知字段校验
    for (fname, fval) in fields {
        if !def.fields.iter().any(|(n, _)| n == fname) {
            return Err(TypeError::UnknownField {
                struct_name: struct_name.clone(),
                field: fname.clone(),
                span: fval.span,
            });
        }
    }

    // 展开为 Alloc + 字段槽
    let base = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: base.clone(),
        init: HirExpr::Alloc {
            slots: def.fields.len(),
        },
        mutable: false,
    }];
    for (i, (fname, fty)) in def.fields.iter().enumerate() {
        let init = fields
            .iter()
            .find(|(n, _)| n == fname)
            .map(|(_, e)| e)
            .ok_or_else(|| TypeError::MissingField {
                struct_name: struct_name.clone(),
                field: fname.clone(),
                span,
            })?;
        let (hir, arg_ty) = infer_expr(ctx, init)?;
        if !arg_ty.compatible_with(fty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: format!("struct `{struct_name}` field `{fname}`"),
                index: i,
                expected: fty.to_string(),
                found: arg_ty.to_string(),
                span: init.span,
            });
        }
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: i,
            value: Box::new(hir),
            ty: field_scalar_of(fty),
        }));
    }

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named(struct_name, Vec::new()),
    ))
}

/// `Vec::with_capacity(cap)` / `Vec::new()`：编译器直接展开。
///
/// 展开为 `data = alloc_array(cap)` + `Vec` 结构体构造
/// （槽 0 = data 指针，槽 1 = len = 0，槽 2 = cap），返回 `Vec<Infer>`，
/// 类型参数由上下文（如 `let v: Vec<i64> = ...` 注解）统一。
fn check_vec_construct(
    ctx: &mut TypeContext,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let cap = if method == "new" {
        AstExpr::new(ExprKind::IntLiteral(4), span)
    } else if args.len() == 1 {
        args[0].clone()
    } else {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("Vec::{method}"),
            expected: 1,
            found: args.len(),
            span,
        });
    };
    let (cap_hir, cap_ty) = infer_expr(ctx, &cap)?;
    if !cap_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: cap_ty.to_string(),
            span,
        });
    }

    // 展开为 Alloc + 三个字段槽（data / len / cap）
    let data_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let cap_for_alloc = cap_hir.clone();
    let stmts = vec![
        HirStmt::Let {
            name: data_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_for_alloc],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: base.clone(),
            init: HirExpr::Alloc { slots: 3 },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(data_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 2,
            value: Box::new(cap_hir),
            ty: FieldScalar::Int,
        }),
    ];

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named("Vec".to_string(), vec![Type::Infer]),
    ))
}

/// `String::with_capacity(cap)` / `String::new()`：编译器直接展开。
///
/// 与 `Vec` 构造相同的三槽布局（data 指针 / len / cap），但缓冲按**字节**
/// 分配（`alloc_bytes`），默认容量 8 字节。返回 `String`（非泛型）。
fn check_string_construct(
    ctx: &mut TypeContext,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let cap = if method == "new" {
        AstExpr::new(ExprKind::IntLiteral(8), span)
    } else if args.len() == 1 {
        args[0].clone()
    } else {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("String::{method}"),
            expected: 1,
            found: args.len(),
            span,
        });
    };
    let (cap_hir, cap_ty) = infer_expr(ctx, &cap)?;
    if !cap_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: cap_ty.to_string(),
            span,
        });
    }

    let data_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let cap_for_alloc = cap_hir.clone();
    let stmts = vec![
        HirStmt::Let {
            name: data_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_bytes".to_string(),
                args: vec![cap_for_alloc],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: base.clone(),
            init: HirExpr::Alloc { slots: 3 },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(data_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 2,
            value: Box::new(cap_hir),
            ty: FieldScalar::Int,
        }),
    ];

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named("String".to_string(), vec![]),
    ))
}

/// `HashMap::with_capacity(cap)` / `HashMap::new()`：编译器直接展开。
///
/// 与 `HashMap` 结构体字段顺序一致（6 槽）：槽 0 = keys 指针（`[K; 0]`）、
/// 槽 1 = vals 指针（`[V; 0]`）、槽 2 = states 指针（`[i64; 0]`）、
/// 槽 3 = len = 0、槽 4 = used = 0、槽 5 = cap。三个动态数组均经
/// `alloc_array` 分配（8 字节步长），默认容量 8。返回 `HashMap<Infer, Infer>`，
/// 类型参数由上下文（如 `let m: HashMap<i64, i64> = ...` 注解）统一。
fn check_hashmap_construct(
    ctx: &mut TypeContext,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let cap = if method == "new" {
        AstExpr::new(ExprKind::IntLiteral(8), span)
    } else if args.len() == 1 {
        args[0].clone()
    } else {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("HashMap::{method}"),
            expected: 1,
            found: args.len(),
            span,
        });
    };
    let (cap_hir, cap_ty) = infer_expr(ctx, &cap)?;
    if !cap_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: cap_ty.to_string(),
            span,
        });
    }

    let keys_tmp = ctx.fresh_temp();
    let vals_tmp = ctx.fresh_temp();
    let states_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let cap_for_alloc = cap_hir.clone();
    let stmts = vec![
        HirStmt::Let {
            name: keys_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_for_alloc],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: vals_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_hir.clone()],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: states_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_hir.clone()],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: base.clone(),
            init: HirExpr::Alloc { slots: 6 },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(keys_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(HirExpr::Variable(vals_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 2,
            value: Box::new(HirExpr::Variable(states_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 3,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 4,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 5,
            value: Box::new(cap_hir),
            ty: FieldScalar::Int,
        }),
    ];

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named("HashMap".to_string(), vec![Type::Infer, Type::Infer]),
    ))
}

/// `String::from("字面量")`：把字符串字面量拷贝到动态字节缓冲。
///
/// MVP 限制：参数必须是字符串字面量（编译器已知字节长度；非字面量 Str
/// 的运行时长度表达后续版本支持）。展开为
/// `data = alloc_bytes(len)` + `copy_bytes(data, s, len)` + 三槽构造（cap = len）。
fn check_string_from(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "String::from".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let (s_hir, s_ty) = infer_expr(ctx, &args[0])?;
    if s_ty != Type::Str {
        return Err(TypeError::WrongType {
            expected: "string literal".to_string(),
            found: s_ty.to_string(),
            span: args[0].span,
        });
    }
    // 字面量直用；`let s = "..."` 绑定的变量经 local_inits 表追踪回字面量，
    // 其余非字面量 Str（运行期才确定内容的字符串）暂不支持（长度表达未实现）
    let s = match &s_hir {
        HirExpr::StringLiteral(s) => Some(s.clone()),
        HirExpr::Variable(name) => match ctx.lookup_local_init(name) {
            Some(HirExpr::StringLiteral(s)) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
    .ok_or_else(|| TypeError::Unsupported {
        what: "String::from 暂仅支持字符串字面量（或绑定字面量的变量）；非字面量 Str 的长度表达未实现"
            .to_string(),
        span: args[0].span,
    })?;
    let len = s.len() as i128;
    // 分配 len+1 字节并连 LLVM 字符串常量自带的 \00 一起拷入：
    // runtime 侧按 C 字符串（NUL 结尾）读取（如 actor 的 handle/factory 符号名
    // 经 dlsym 前由 CStr 扫描），缓冲末尾必须补 NUL，否则读超到相邻堆内存。
    let alloc_len = len + 1;

    let data_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let stmts = vec![
        HirStmt::Let {
            name: data_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_bytes".to_string(),
                args: vec![HirExpr::IntLiteral(alloc_len)],
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::Call {
            callee: "copy_bytes".to_string(),
            args: vec![
                HirExpr::Variable(data_tmp.clone()),
                HirExpr::StringLiteral(s.clone()),
                HirExpr::IntLiteral(alloc_len),
            ],
        }),
        HirStmt::Let {
            name: base.clone(),
            init: HirExpr::Alloc { slots: 3 },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(data_tmp)),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(HirExpr::IntLiteral(len)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 2,
            value: Box::new(HirExpr::IntLiteral(len)),
            ty: FieldScalar::Int,
        }),
    ];

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named("String".to_string(), vec![]),
    ))
}

/// 结构体字段访问：`point.x` → `FieldGet(base, index)`。
///
/// 接收者可为结构体值或引用（`&Point` / `&mut Point`），字段类型按定义返回。
fn check_field_access(
    ctx: &mut TypeContext,
    base_hir: HirExpr,
    base_ty: Type,
    field: &str,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let Type::Named(name, _) = peel_ref(&base_ty) else {
        return Err(TypeError::ExpectedStruct {
            found: base_ty.to_string(),
            span,
        });
    };

    // actor 状态字段访问（方法体内 `self.value`）：状态 = 槽数组，FieldGet 槽索引
    if let Some(ad) = ctx.lookup_actor(&name) {
        let idx = ad
            .fields
            .iter()
            .position(|f| f.name == field)
            .ok_or_else(|| TypeError::UnknownField {
                struct_name: name.clone(),
                field: field.to_string(),
                span,
            })?;
        let fty = resolve_ast_type(ctx, &ad.fields[idx].type_, span)?;
        return Ok((
            HirExpr::FieldGet {
                base: Box::new(base_hir),
                index: idx,
                ty: field_scalar_of(&fty),
            },
            fty,
        ));
    }

    let def = ctx.lookup_struct(&name).cloned().ok_or_else(|| {
        TypeError::UndefinedType {
            name: name.clone(),
            span,
        }
    })?;
    let (idx, fty) = def
        .fields
        .iter()
        .enumerate()
        .find(|(_, (n, _))| n == field)
        .map(|(i, (_, t))| (i, t.clone()))
        .ok_or_else(|| TypeError::UnknownField {
            struct_name: name.clone(),
            field: field.to_string(),
            span,
        })?;
    // 字段类型经泛型替换（泛型方法实例化时 `T` → 具体类型），
    // 与 match 模式解构（`substitute(fty, &ctx.generic_subst)`）保持一致
    let fty_sub = substitute(&fty, &ctx.generic_subst);
    Ok((
        HirExpr::FieldGet {
            base: Box::new(base_hir),
            index: idx,
            ty: field_scalar_of(&fty_sub),
        },
        fty_sub,
    ))
}

/// 范围切片：`s[lo..<hi]` / `s[lo...hi]` / `s[lo<..hi]`（MVP 仅 String）。
///
/// desugar 为 `String::substring` 方法调用（typecheck 层特判展开，经
/// `instantiate_impl_method` 注册函数体，复用纯 Zeta `substring`）：
/// - `s[lo..<hi]` → `s.substring(lo, hi)`（左闭右开）
/// - `s[lo...hi]` → `s.substring(lo, hi + 1)`（闭区间 → 半开）
/// - `s[lo<..hi]` → `s.substring(lo + 1, hi + 1)`（左开右闭 → 半开）
///
/// 数组/Vec 动态切片（结果长度运行时确定）暂报 Unsupported。
fn check_slice(
    ctx: &mut TypeContext,
    expr: &AstExpr,
    lower: &AstExpr,
    upper: &AstExpr,
    lower_inclusive: bool,
    upper_inclusive: bool,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let (b_hir, b_ty) = infer_expr(ctx, expr)?;
    // 支持 String / Vec<T> / 数组 [T; N] 三类切片对象：
    // - String → 方法实例化 substring（现有路径）
    // - Vec<T> → 方法实例化 slice（std 泛型方法，越界 clamp）
    // - 数组 [T; N] → 展开为 Vec 拷贝循环（动态切片，边界 clamp 到 [0, N]）
    let is_vec = matches!(&b_ty, Type::Named(n, _) if n == "Vec");
    let is_arr = matches!(&b_ty, Type::Array(_, _));
    if !comparison::is_string_type(ctx, &b_ty) && !is_vec && !is_arr {
        return Err(TypeError::Unsupported {
            what: "范围切片（`s[lo..<hi]`）暂仅支持 String / Vec / 数组对象".to_string(),
            span,
        });
    }
    let (lo_hir, lo_ty) = infer_expr(ctx, lower)?;
    let (hi_hir, hi_ty) = infer_expr(ctx, upper)?;
    if !lo_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: lo_ty.to_string(),
            span: lower.span,
        });
    }
    if !hi_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: hi_ty.to_string(),
            span: upper.span,
        });
    }
    // 区间 → substring 的半开参数 [start, end)：
    // `..<` 含下界不含上界；`...` 双闭（end + 1）；`<..` 不含下界（start + 1）
    let start = if lower_inclusive {
        lo_hir
    } else {
        HirExpr::Binary(
            HirBinaryOp::Add,
            Box::new(lo_hir),
            Box::new(HirExpr::IntLiteral(1)),
        )
    };
    let end = if upper_inclusive {
        HirExpr::Binary(
            HirBinaryOp::Add,
            Box::new(hi_hir),
            Box::new(HirExpr::IntLiteral(1)),
        )
    } else {
        hi_hir
    };
    if comparison::is_string_type(ctx, &b_ty) {
        // String → substring（非泛型，subst 为空）
        let impl_def = ctx
            .find_impl_for_method(&b_ty, "substring")
            .cloned()
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: "String::substring".to_string(),
                span,
            })?;
        let method_def = impl_def
            .methods
            .iter()
            .find(|m| m.sig.name == "substring")
            .cloned()
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: "String::substring".to_string(),
                span,
            })?;
        let subst: HashMap<String, Type> = HashMap::new();
        let fn_name = instantiate_impl_method(ctx, &impl_def, &method_def, &subst, span)?;
        Ok((
            HirExpr::Call {
                callee: fn_name,
                args: vec![b_hir, start, end],
            },
            b_ty,
        ))
    } else if let Type::Named(_, args) = &b_ty {
        // Vec<T> → slice（泛型，subst 把 impl 类型参数替换为具体类型参数）
        let impl_def = ctx
            .find_impl_for_method(&b_ty, "slice")
            .cloned()
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: "Vec::slice".to_string(),
                span,
            })?;
        let method_def = impl_def
            .methods
            .iter()
            .find(|m| m.sig.name == "slice")
            .cloned()
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: "Vec::slice".to_string(),
                span,
            })?;
        let mut subst: HashMap<String, Type> = HashMap::new();
        for (tp, arg) in impl_def.type_params.iter().zip(args.iter()) {
            subst.insert(tp.clone(), substitute(arg, &ctx.generic_subst));
        }
        let fn_name = instantiate_impl_method(ctx, &impl_def, &method_def, &subst, span)?;
        Ok((
            HirExpr::Call {
                callee: fn_name,
                args: vec![b_hir, start, end],
            },
            b_ty,
        ))
    } else if let Type::Array(elem, n) = &b_ty {
        // 数组 [T; N] → 展开为 Vec 拷贝循环：
        //   let __base = <数组>;
        //   let __data = alloc_array(4); let __out = Alloc(3);
        //   __out.0 = __data; __out.1 = 0; __out.2 = 4;      // Vec::new()（cap 4）
        //   let __i = start; let __hi = end;
        //   if __i < 0 { __i = 0 }                            // clamp 下界
        //   if __hi > N { __hi = N }                          // clamp 上界
        //   while __i < __hi { __out.push(__base[__i]); __i += 1 }
        //   __out
        let elem_sub = substitute(elem, &ctx.generic_subst);
        let is_byte = matches!(elem_sub, Type::U8);
        let elem_scalar = field_scalar_of(&elem_sub);
        let vec_ty = Type::Named("Vec".to_string(), vec![elem_sub.clone()]);
        // Vec<elem>::push 实例化（std 泛型方法，subst = {T: elem}）
        let impl_def = ctx
            .find_impl_for_method(&vec_ty, "push")
            .cloned()
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: "Vec::push".to_string(),
                span,
            })?;
        let method_def = impl_def
            .methods
            .iter()
            .find(|m| m.sig.name == "push")
            .cloned()
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: "Vec::push".to_string(),
                span,
            })?;
        let push_subst = HashMap::from([("T".to_string(), elem_sub)]);
        let push_name = instantiate_impl_method(ctx, &impl_def, &method_def, &push_subst, span)?;

        // 临时名
        let base_name = format!("__slice_base_{}", ctx.temp_counter);
        ctx.temp_counter += 1;
        let out_name = format!("__slice_out_{}", ctx.temp_counter);
        ctx.temp_counter += 1;
        let i_name = format!("__slice_i_{}", ctx.temp_counter);
        ctx.temp_counter += 1;
        let hi_name = format!("__slice_hi_{}", ctx.temp_counter);
        ctx.temp_counter += 1;
        let data_name = format!("__slice_data_{}", ctx.temp_counter);
        ctx.temp_counter += 1;

        // 前缀语句：绑定数组、构造 Vec::new（cap 4 三槽）、计数器、上界、clamp
        let mut stmts = vec![
            HirStmt::Let {
                name: base_name.clone(),
                init: b_hir,
                mutable: false,
            },
            HirStmt::Let {
                name: data_name.clone(),
                init: HirExpr::Call {
                    callee: "alloc_array".to_string(),
                    args: vec![HirExpr::IntLiteral(4)],
                },
                mutable: false,
            },
            HirStmt::Let {
                name: out_name.clone(),
                init: HirExpr::Alloc { slots: 3 },
                mutable: false,
            },
            HirStmt::Semi(HirExpr::FieldSet {
                base: Box::new(HirExpr::Variable(out_name.clone())),
                index: 0,
                value: Box::new(HirExpr::Variable(data_name)),
                ty: FieldScalar::Ptr,
            }),
            HirStmt::Semi(HirExpr::FieldSet {
                base: Box::new(HirExpr::Variable(out_name.clone())),
                index: 1,
                value: Box::new(HirExpr::IntLiteral(0)),
                ty: FieldScalar::Int,
            }),
            HirStmt::Semi(HirExpr::FieldSet {
                base: Box::new(HirExpr::Variable(out_name.clone())),
                index: 2,
                value: Box::new(HirExpr::IntLiteral(4)),
                ty: FieldScalar::Int,
            }),
            HirStmt::Let {
                name: i_name.clone(),
                init: start,
                mutable: true,
            },
            HirStmt::Let {
                name: hi_name.clone(),
                init: end,
                mutable: true,
            },
            // clamp 下界：if __i < 0 { __i = 0 }
            HirStmt::Expr(HirExpr::If {
                cond: Box::new(HirExpr::Binary(
                    HirBinaryOp::Lt,
                    Box::new(HirExpr::Variable(i_name.clone())),
                    Box::new(HirExpr::IntLiteral(0)),
                )),
                then_block: Box::new(HirBlock {
                    stmts: vec![HirStmt::Expr(HirExpr::Assign {
                        target: i_name.clone(),
                        op: HirAssignOp::Assign,
                        value: Box::new(HirExpr::IntLiteral(0)),
                    })],
                    final_expr: None,
                }),
                else_block: None,
            }),
            // clamp 上界：if __hi > N { __hi = N }
            HirStmt::Expr(HirExpr::If {
                cond: Box::new(HirExpr::Binary(
                    HirBinaryOp::Gt,
                    Box::new(HirExpr::Variable(hi_name.clone())),
                    Box::new(HirExpr::IntLiteral(*n as i128)),
                )),
                then_block: Box::new(HirBlock {
                    stmts: vec![HirStmt::Expr(HirExpr::Assign {
                        target: hi_name.clone(),
                        op: HirAssignOp::Assign,
                        value: Box::new(HirExpr::IntLiteral(*n as i128)),
                    })],
                    final_expr: None,
                }),
                else_block: None,
            }),
        ];
        // 循环：while __i < __hi { __out.push(__base[__i]); __i += 1 }
        stmts.push(HirStmt::Expr(HirExpr::While {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Lt,
                Box::new(HirExpr::Variable(i_name.clone())),
                Box::new(HirExpr::Variable(hi_name.clone())),
            )),
            body: Box::new(HirBlock {
                stmts: vec![
                    HirStmt::Expr(HirExpr::Call {
                        callee: push_name,
                        args: vec![
                            HirExpr::Variable(out_name.clone()),
                            HirExpr::Index {
                                base: Box::new(HirExpr::Variable(base_name.clone())),
                                index: Box::new(HirExpr::Variable(i_name.clone())),
                                elem: elem_scalar,
                                is_str: is_byte,
                            },
                        ],
                    }),
                    HirStmt::Expr(HirExpr::Assign {
                        target: i_name.clone(),
                        op: HirAssignOp::AddAssign,
                        value: Box::new(HirExpr::IntLiteral(1)),
                    }),
                ],
                final_expr: None,
            }),
        }));
        Ok((
            HirExpr::Block(Box::new(HirBlock {
                stmts,
                final_expr: Some(HirExpr::Variable(out_name)),
            })),
            vec_ty,
        ))
    } else {
        unreachable!("切片对象类型已被前置判断过滤")
    }
}

/// 索引访问：`arr[i]` / `s[i]`。
///
/// 支持数组 `[T; N]`（元素类型 `T`）与字符串 `Str`（元素为 `Char`）。
/// 索引表达式必须为整数类型；结果为元素值（可读，也可作为赋值目标）。
fn check_index(
    ctx: &mut TypeContext,
    expr: &AstExpr,
    index: &AstExpr,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 范围切片 `s[lo..<hi]` / `s[lo...hi]` / `s[lo<..hi]`：索引表达式为
    // Range 时改走切片路径（`infer_expr` 对 Range 仅返回 Unit，须先行特判）
    if let ExprKind::Range {
        lower,
        upper,
        lower_inclusive,
        upper_inclusive,
        ..
    } = &*index.kind
    {
        return check_slice(
            ctx,
            expr,
            lower,
            upper,
            *lower_inclusive,
            *upper_inclusive,
            span,
        );
    }
    let (b_hir, b_ty) = infer_expr(ctx, expr)?;
    let (i_hir, i_ty) = infer_expr(ctx, index)?;
    if !i_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: i_ty.to_string(),
            span: index.span,
        });
    }
    match peel_ref(&b_ty) {
        Type::Array(elem_ty, _) => {
            // 元素类型经泛型替换（泛型方法实例化时 `T` → 具体类型）
            let elem_sub = substitute(&elem_ty, &ctx.generic_subst);
            // `u8` 字节数组按字节存储（步长 1，is_str=true）；其余元素步长 8
            let is_byte = matches!(elem_sub, Type::U8);
            Ok((
                HirExpr::Index {
                    base: Box::new(b_hir),
                    index: Box::new(i_hir),
                    elem: field_scalar_of(&elem_sub),
                    is_str: is_byte,
                },
                elem_sub,
            ))
        }
        Type::Str => Ok((
            HirExpr::Index {
                base: Box::new(b_hir),
                index: Box::new(i_hir),
                elem: FieldScalar::Char,
                is_str: true,
            },
            Type::Char,
        )),
        Type::Named(n, args) => {
            let full = ctx.resolve_full_name(&n).unwrap_or_else(|| n.clone());
            // `s[i]`：String 对象按字节索引（步长 1），base 取槽 0 的 data 指针
            if full == "String" && ctx.lookup_struct(&full).is_some() {
                Ok((
                    HirExpr::Index {
                        base: Box::new(HirExpr::FieldGet {
                            base: Box::new(b_hir),
                            index: 0,
                            ty: FieldScalar::Ptr,
                        }),
                        index: Box::new(i_hir),
                        elem: FieldScalar::Int,
                        is_str: true,
                    },
                    Type::U8,
                ))
            } else if full == "Vec" && ctx.lookup_struct(&full).is_some() {
                // `v[i]`：Vec 动态数组按元素索引（步长 8），base 取槽 0 的 data 指针；
                // 元素类型取 `Vec<T>` 的类型参数并经泛型替换
                let elem_ty = args.first().cloned().unwrap_or(Type::Infer);
                let elem_sub = substitute(&elem_ty, &ctx.generic_subst);
                Ok((
                    HirExpr::Index {
                        base: Box::new(HirExpr::FieldGet {
                            base: Box::new(b_hir),
                            index: 0,
                            ty: FieldScalar::Ptr,
                        }),
                        index: Box::new(i_hir),
                        elem: field_scalar_of(&elem_sub),
                        is_str: false,
                    },
                    elem_sub,
                ))
            } else {
                Err(TypeError::WrongType {
                    expected: "array or string".to_string(),
                    found: b_ty.to_string(),
                    span,
                })
            }
        }
        other => Err(TypeError::WrongType {
            expected: "array or string".to_string(),
            found: other.to_string(),
            span,
        }),
    }
}

/// 数组字面量 `[a, b, c]`：元素类型统一，展开为
/// `Alloc + 逐元素 FieldSet` 块（数组值为槽区指针）。
///
/// MVP 限制：空数组 `[]` 需要元素类型标注，暂不支持。
fn check_array_lit(
    ctx: &mut TypeContext,
    elems: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if elems.is_empty() {
        return Err(TypeError::Unsupported {
            what: "空数组字面量 `[]` 在 MVP 阶段（无法推断元素类型）".to_string(),
            span,
        });
    }
    let mut hir_elems = Vec::with_capacity(elems.len());
    let mut elem_ty: Option<Type> = None;
    for e in elems {
        let (hir, ty) = infer_expr(ctx, e)?;
        if let Some(prev) = &elem_ty {
            if !ty.compatible_with(prev) {
                return Err(TypeError::WrongType {
                    expected: prev.to_string(),
                    found: ty.to_string(),
                    span: e.span,
                });
            }
        } else {
            elem_ty = Some(ty);
        }
        hir_elems.push(hir);
    }
    let elem_ty = elem_ty.expect("non-empty array elements");
    let elem_scalar = field_scalar_of(&elem_ty);
    // 展开为 Alloc + 逐元素 FieldSet（数组值为槽区指针）
    let base = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: base.clone(),
        init: HirExpr::Alloc {
            slots: elems.len(),
        },
        mutable: false,
    }];
    for (i, h) in hir_elems.into_iter().enumerate() {
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: i,
            value: Box::new(h),
            ty: elem_scalar,
        }));
    }
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Array(Box::new(elem_ty), elems.len()),
    ))
}

/// 枚举变体构造：`Option::Some(x)` → 堆对象 `Alloc + tag 槽 + 字段槽` 序列。
///
/// 返回对象指针（HIR 块）与枚举类型 `Named(enum, 泛型实参)`。
fn check_variant_construct(
    ctx: &mut TypeContext,
    enum_name: &str,
    variant_name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let enum_def = ctx
        .lookup_enum(enum_name)
        .cloned()
        .ok_or_else(|| TypeError::UndefinedType {
            name: enum_name.to_string(),
            span,
        })?;
    let variant = enum_def
        .variants
        .iter()
        .find(|v| v.name == variant_name)
        .cloned()
        .ok_or_else(|| TypeError::FunctionNotFound {
            name: format!("{enum_name}::{variant_name}"),
            span,
        })?;
    if args.len() != variant.fields.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{enum_name}::{variant_name}"),
            expected: variant.fields.len(),
            found: args.len(),
            span,
        });
    }

    // 由实参类型推断枚举泛型参数（如 `Option::Some(x: i64)` → `Option<i64>`）
    let mut subst: HashMap<String, Type> = HashMap::new();
    for ((_, fty), arg) in variant.fields.iter().zip(args) {
        let (_, arg_ty) = infer_expr(ctx, arg)?;
        unify(fty, &arg_ty, &mut subst)?;
    }

    // 展开为 Alloc + tag 槽 + 字段槽
    let base = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: base.clone(),
        init: HirExpr::Alloc {
            slots: enum_def.slot_count,
        },
        mutable: false,
    }];
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(base.clone())),
        index: 0,
        value: Box::new(HirExpr::IntLiteral(variant.tag as i128)),
        ty: FieldScalar::Int,
    }));
    for (i, (arg, (_, fty))) in args.iter().zip(&variant.fields).enumerate() {
        let (hir, arg_ty) = infer_expr(ctx, arg)?;
        let fty = substitute(fty, &subst);
        if !arg_ty.compatible_with(&fty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: format!("{enum_name}::{variant_name}"),
                index: i,
                expected: fty.to_string(),
                found: arg_ty.to_string(),
                span: arg.span,
            });
        }
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1 + i,
            value: Box::new(hir),
            ty: field_scalar_of(&fty),
        }));
    }

    let ty = Type::Named(
        enum_name.to_string(),
        enum_def
            .type_params
            .iter()
            .map(|tp| {
                let t = substitute(&Type::Generic(tp.clone()), &subst);
                // 实参无法确定泛型参数（如 `Option::None`）→ 用 `_` 占位，
                // 由后续方法调用/比较上下文推断
                if matches!(t, Type::Generic(_)) {
                    Type::Infer
                } else {
                    t
                }
            })
            .collect(),
    );
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        ty,
    ))
}

/// match 表达式展开：绑定 scrutinee 为临时变量，按模式生成判别条件
/// 与字段绑定的 if-else 链。
fn check_match(
    ctx: &mut TypeContext,
    expr: &AstExpr,
    arms: &[zeta_ast::MatchArm],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let (s_hir, s_ty) = infer_expr(ctx, expr)?;
    // 引用类型的 scrutinee（`match self`，`self: &T`）：模式匹配针对被引用
    // 的聚合类型；MIR 层 `&T` 与 `T` 均为对象指针，无需显式解引用。
    let pat_ty = peel_ref(&s_ty);
    let tmp = ctx.fresh_temp();
    let tmp_var = HirExpr::Variable(tmp.clone());
    let stmts = vec![HirStmt::Let {
        name: tmp.clone(),
        init: s_hir,
        mutable: true,
    }];

    // 从最后一个 arm 开始反向构建 if-else 链
    let mut else_hir: Option<HirExpr> = None;
    let mut result_ty = Type::Unit;
    for arm in arms.iter().rev() {
        let (cond, binds, is_binding, bound_tys) =
            check_pattern(ctx, &arm.pattern, &pat_ty, tmp_var.clone(), span)?;
        // 注册模式绑定变量（arm body / guard 内引用），求值后恢复作用域。
        // 注意：克隆保存而非清空，arm body 仍可见外层变量（函数参数、外层 let）。
        let saved_vars = ctx.variables.clone();
        for (n, t) in &bound_tys {
            ctx.variables.insert(n.clone(), t.clone());
        }
        let arm_result: Result<(Option<HirExpr>, HirExpr, Type), TypeError> = (|| {
            // 守卫条件（`pattern if guard => body`）：与模式条件 And 合并
            let cond = match (&arm.guard, cond) {
                (Some(guard), Some(c)) => {
                    let (g_hir, _) = infer_expr(ctx, guard)?;
                    Some(HirExpr::Binary(
                        HirBinaryOp::And,
                        Box::new(c),
                        Box::new(g_hir),
                    ))
                }
                (None, c) => c,
                (Some(_), None) => {
                    return Err(TypeError::Unsupported {
                        what: "对兜底模式使用守卫条件".to_string(),
                        span,
                    })
                }
            };
            let (body_hir, body_ty) = infer_expr(ctx, &arm.body)?;
            Ok((cond, body_hir, body_ty))
        })();
        ctx.variables = saved_vars;
        let (cond, body_hir, body_ty) = arm_result?;
        // match 各 arm 返回类型必须一致（Never 表示不返回，跳过）
        if else_hir.is_some()
            && body_ty != Type::Never
            && result_ty != Type::Never
            && !body_ty.compatible_with(&result_ty)
        {
            return Err(TypeError::WrongType {
                expected: result_ty.to_string(),
                found: body_ty.to_string(),
                span,
            });
        }
        result_ty = body_ty;
        let then_block = HirBlock {
            stmts: binds,
            final_expr: Some(body_hir),
        };
        else_hir = Some(if is_binding {
            // 兜底模式（标识符 / 通配符）：直接作为 else 分支
            HirExpr::Block(Box::new(then_block))
        } else {
            HirExpr::If {
                cond: Box::new(cond.ok_or_else(|| TypeError::Unsupported {
                    what: "无条件的非兜底 match 模式".to_string(),
                    span,
                })?),
                then_block: Box::new(then_block),
                else_block: else_hir.map(|e| {
                    Box::new(HirBlock {
                        stmts: Vec::new(),
                        final_expr: Some(e),
                    })
                }),
            }
        });
    }

    let final_expr = else_hir.unwrap_or(HirExpr::Unit);
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(final_expr),
        })),
        result_ty,
    ))
}

/// 解析 match 模式为 (判别条件, 绑定语句, 是否为兜底模式)。
///
/// 对枚举模式生成 `FieldGet(scrutinee, 0) == tag` 条件，并递归展开子模式
/// 的字段绑定（槽 `1 + 字段下标`）。
/// 模式检查结果：(守卫条件, 绑定语句, 是否为纯绑定(无条件), 绑定变量类型表)
type PatternResult = (Option<HirExpr>, Vec<HirStmt>, bool, Vec<(String, Type)>);

#[allow(clippy::too_many_arguments)]
fn check_pattern(
    ctx: &mut TypeContext,
    pat: &zeta_ast::AstPattern,
    pat_ty: &Type,
    scrutinee: HirExpr,
    span: Span,
) -> Result<PatternResult, TypeError> {
    use zeta_ast::AstPattern;
    match pat {
        AstPattern::Ident(name) => Ok((
            None,
            vec![HirStmt::Let {
                name: name.clone(),
                init: scrutinee,
                mutable: false,
            }],
            true,
            vec![(name.clone(), pat_ty.clone())],
        )),
        AstPattern::Wildcard => Ok((None, Vec::new(), true, Vec::new())),
        AstPattern::Literal(lit) => {
            let lit_hir = literal_to_hir(lit, span)?;
            let cond = HirExpr::Binary(
                HirBinaryOp::Eq,
                Box::new(scrutinee),
                Box::new(lit_hir),
            );
            Ok((Some(cond), Vec::new(), false, Vec::new()))
        }
        AstPattern::Enum(variant, sub_pats) => {
            let Type::Named(en, _) = pat_ty else {
                return Err(TypeError::Unsupported {
                    what: format!("对非枚举类型 `{pat_ty}` 使用枚举模式 `{variant}`"),
                    span,
                });
            };
            let enum_def = ctx.lookup_enum(en).cloned().ok_or_else(|| {
                TypeError::UndefinedType {
                    name: en.clone(),
                    span,
                }
            })?;
            let variant_def = enum_def
                .variants
                .iter()
                .find(|v| v.name == *variant)
                .cloned()
                .ok_or_else(|| TypeError::FunctionNotFound {
                    name: format!("{en}::{variant}"),
                    span,
                })?;
            if sub_pats.len() != variant_def.fields.len() {
                return Err(TypeError::UnexpectedArgumentCount {
                    name: format!("{en}::{variant}"),
                    expected: variant_def.fields.len(),
                    found: sub_pats.len(),
                    span,
                });
            }
            // 主条件：tag == 变体序号
            let tag_cond = HirExpr::Binary(
                HirBinaryOp::Eq,
                Box::new(HirExpr::FieldGet {
                    base: Box::new(scrutinee.clone()),
                    index: 0,
                    ty: FieldScalar::Int,
                }),
                Box::new(HirExpr::IntLiteral(variant_def.tag as i128)),
            );
            // 子模式：字段槽 1+i，条件用 And 合并。
            // 字段类型经泛型替换：优先合并当前 generic_subst（泛型方法体内
            // `T` → 具体类型）；再从具体实例化 `pat_ty` 的类型参数推导枚举
            // 泛型映射——用户级 match（非泛型方法体，generic_subst 为空）时，
            // `match (o: Option<String>)` 需把 `T` 解析为 `String`。
            let mut subst = ctx.generic_subst.clone();
            if let Type::Named(_, pat_args) = pat_ty {
                if !pat_args.is_empty() && pat_args.len() == enum_def.type_params.len() {
                    for (tp, arg) in enum_def.type_params.iter().zip(pat_args) {
                        subst.insert(tp.clone(), arg.clone());
                    }
                }
            }
            let mut binds = Vec::new();
            let mut bound_tys = Vec::new();
            let mut cond = tag_cond;
            for (i, (sub, (_, fty))) in sub_pats.iter().zip(&variant_def.fields).enumerate() {
                let fty_sub = substitute(fty, &subst);
                let (sub_cond, sub_binds, _, sub_tys) = check_pattern(
                    ctx,
                    sub,
                    &fty_sub,
                    HirExpr::FieldGet {
                        base: Box::new(scrutinee.clone()),
                        index: 1 + i,
                        ty: field_scalar_of(&fty_sub),
                    },
                    span,
                )?;
                binds.extend(sub_binds);
                bound_tys.extend(sub_tys);
                if let Some(sc) = sub_cond {
                    cond = HirExpr::Binary(
                        HirBinaryOp::And,
                        Box::new(cond),
                        Box::new(sc),
                    );
                }
            }
            Ok((Some(cond), binds, false, bound_tys))
        }
        AstPattern::EnumPath(segments, sub_pats) => {
            // 路径模式取最后一段为变体名，复用 Enum 分支逻辑
            //（`lib::Option::Some(x)` → variant = "Some"）
            let variant = segments.last().cloned().ok_or_else(|| {
                TypeError::Unsupported {
                    what: "空路径枚举模式".to_string(),
                    span,
                }
            })?;
            let pat = AstPattern::Enum(variant, sub_pats.clone());
            check_pattern(ctx, &pat, pat_ty, scrutinee, span)
        }
        AstPattern::Tuple(_) | AstPattern::Struct(..) => Err(TypeError::Unsupported {
            what: "元组 / 结构体模式在 MVP 阶段".to_string(),
            span,
        }),
        AstPattern::Range { .. } => Err(TypeError::Unsupported {
            what: "范围模式在 MVP 阶段".to_string(),
            span,
        }),
        AstPattern::Ref(inner, _) => check_pattern(ctx, inner, pat_ty, scrutinee, span),
    }
}

/// 将字面量值转为 HIR 字面量。
fn literal_to_hir(lit: &zeta_ast::LiteralValue, span: Span) -> Result<HirExpr, TypeError> {
    use zeta_ast::LiteralValue;
    Ok(match lit {
        LiteralValue::Int(v) => HirExpr::IntLiteral(*v),
        LiteralValue::Float(v) => HirExpr::FloatLiteral(*v),
        LiteralValue::Str(s) => HirExpr::StringLiteral(s.clone()),
        LiteralValue::Char(c) => HirExpr::CharLiteral(*c),
        LiteralValue::Bool(b) => HirExpr::BoolLiteral(*b),
        LiteralValue::Time { .. } => {
            return Err(TypeError::Unsupported {
                what: "时间字面量模式".to_string(),
                span,
            })
        }
    })
}

/// 静态方法调用：`Point::origin(args)` → impl 块中无 `self` 的方法 →
/// desugar 为顶层函数调用 `Type::method(args...)`（无接收者）。
fn check_static_method_call(
    ctx: &mut TypeContext,
    ty_name: &str,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let self_ty = Type::Named(ty_name.to_string(), Vec::new());
    let impl_def = ctx
        .find_impl_for_method(&self_ty, method)
        .cloned()
        .ok_or_else(|| TypeError::FunctionNotFound {
            name: format!("{self_ty}::{method}"),
            span,
        })?;
    let method_def = impl_def
        .methods
        .iter()
        .find(|m| m.sig.name == method)
        .cloned()
        .ok_or_else(|| TypeError::FunctionNotFound {
            name: format!("{self_ty}::{method}"),
            span,
        })?;

    // 静态方法必须无 `self` 参数
    if let Some(body) = &method_def.body {
        if body.params.first().map(|p| p.name == "self") == Some(true) {
            return Err(TypeError::FunctionNotFound {
                name: format!("{self_ty}::{method}"),
                span,
            });
        }
    }

    // 泛型 impl 的静态方法：MVP 不支持（self 类型无法由调用确定类型参数）
    if !impl_def.type_params.is_empty() {
        return Err(TypeError::Unsupported {
            what: format!("泛型 impl `{ty_name}` 的静态方法 `{method}`"),
            span,
        });
    }

    let subst: HashMap<String, Type> = HashMap::new();
    let expected: Vec<Type> = method_def
        .sig
        .params
        .iter()
        .map(|p| substitute(p, &subst))
        .collect();
    let ret_ty = substitute(&method_def.sig.return_type, &subst);

    let base_fn = format!("{ty_name}::{method}");
    let fn_name = instantiate_impl_method(ctx, &impl_def, &method_def, &subst, span)?;

    if args.len() != expected.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: base_fn.clone(),
            expected: expected.len(),
            found: args.len(),
            span,
        });
    }
    let mut hir_args = Vec::with_capacity(args.len());
    for (i, (arg, pty)) in args.iter().zip(&expected).enumerate() {
        let (hir, ty) = infer_expr(ctx, arg)?;
        if !ty.compatible_with(pty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: base_fn.clone(),
                index: i,
                expected: pty.to_string(),
                found: ty.to_string(),
                span: arg.span,
            });
        }
        hir_args.push(hir);
    }
    Ok((
        HirExpr::Call {
            callee: fn_name,
            args: hir_args,
        },
        ret_ty,
    ))
}

/// 方法调用：`recv.method(args)` → 解析到 impl 块 → desugar 为
/// 顶层函数调用 `Type::method(recv, args...)`。
fn check_method_call(
    ctx: &mut TypeContext,
    receiver: &AstExpr,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let (recv_hir, recv_ty) = infer_expr(ctx, receiver)?;
    let self_ty = peel_ref(&recv_ty);
    if matches!(self_ty, Type::Unit) {
        return Err(TypeError::Unsupported {
            what: format!("对单元类型调用方法 `{method}`"),
            span,
        });
    }

    // actor 方法调用：`counter.method(a, b)` → `zeta_actor_ask(recv, kind, a, b, 0)`
    // （MVP 同步语义，`.await` 仅为可选语法标记；参数经消息槽传递）
    if let Type::Named(name, _) = &self_ty {
        if let Some(ad) = ctx.lookup_actor(name).cloned() {
            let kind = ad
                .methods
                .iter()
                .position(|m| m.name == method)
                .ok_or_else(|| TypeError::FunctionNotFound {
                    name: format!("{self_ty}::{method}"),
                    span,
                })?;
            if args.len() > 3 {
                return Err(TypeError::UnexpectedArgumentCount {
                    name: format!("{self_ty}::{method}"),
                    expected: 3,
                    found: args.len(),
                    span,
                });
            }
            let mut call_args = vec![recv_hir, HirExpr::IntLiteral(kind as i128)];
            for arg in args {
                let (h, t) = infer_expr(ctx, arg)?;
                if !t.compatible_with(&Type::I64) {
                    return Err(TypeError::ArgumentTypeMismatch {
                        name: format!("{self_ty}::{method}"),
                        index: call_args.len() - 2,
                        expected: "i64".to_string(),
                        found: t.to_string(),
                        span: arg.span,
                    });
                }
                call_args.push(h);
            }
            while call_args.len() < 5 {
                call_args.push(HirExpr::IntLiteral(0));
            }
            return Ok((
                HirExpr::Call {
                    callee: "zeta_actor_ask".to_string(),
                    args: call_args,
                },
                Type::I64,
            ));
        }
    }

    // 查找含该方法的 impl 块（inherent 优先，trait 次之）
    let impl_def = ctx
        .find_impl_for_method(&self_ty, method)
        .cloned()
        .ok_or_else(|| TypeError::FunctionNotFound {
            name: format!("{self_ty}::{method}"),
            span,
        })?;
    let method_def = impl_def
        .methods
        .iter()
        .find(|m| m.sig.name == method)
        .cloned()
        .ok_or_else(|| TypeError::FunctionNotFound {
            name: format!("{self_ty}::{method}"),
            span,
        })?;

    // 由接收者类型统一 impl 泛型参数
    let mut subst: HashMap<String, Type> = HashMap::new();
    unify(&impl_def.self_type, &self_ty, &mut subst)?;

    // 参数类型（`self` 之后的显式参数）
    let mut expected: Vec<Type> = method_def
        .sig
        .params
        .iter()
        .skip(1)
        .map(|p| substitute(p, &subst))
        .collect();
    // 显式泛型参数的方法（如 `fn f<T>(...)`）由实参类型推断
    if method_def.body.as_ref().is_some_and(|b| !b.generics.is_empty()) {
        for (pty, a) in expected.iter().zip(args.iter()) {
            let (_, arg_ty) = infer_expr(ctx, a)?;
            unify(pty, &arg_ty, &mut subst)?;
        }
        expected = method_def
            .sig
            .params
            .iter()
            .skip(1)
            .map(|p| substitute(p, &subst))
            .collect();
    }

    // 参数类型推断 + Infer 回填：期望类型含未定型 `_`（如裸 `Result::Err(7)`
    // 的 `unwrap_or(default: T)`，T 经接收者 unified 后仍为 Infer）时，用实参
    // 类型定型，使返回类型不再泄漏 `_`。
    let mut hir_args = vec![recv_hir];
    let mut arg_tys = Vec::with_capacity(args.len());
    for (pty, a) in expected.iter().zip(args.iter()) {
        let (hir, ty) = infer_expr(ctx, a)?;
        arg_tys.push(ty.clone());
        if contains_infer(pty) {
            unify(pty, &ty, &mut subst)?;
        }
        hir_args.push(hir);
    }
    // 回填可能定型类型参数，重算签名（返回类型必须用定型后的 subst）
    expected = method_def
        .sig
        .params
        .iter()
        .skip(1)
        .map(|p| substitute(p, &subst))
        .collect();
    let ret_ty = substitute(&method_def.sig.return_type, &subst);

    // 方法函数名：inherent/trait 方法统一 `Type::method`，泛型实例化追加后缀
    let Type::Named(base_name, _) = &impl_def.self_type else {
        return Err(TypeError::Unsupported {
            what: "impl 目标类型必须为具名类型".to_string(),
            span,
        });
    };
    let base_fn = format!("{base_name}::{method}");
    // 无论是否泛型，都在调用点实例化方法体（非泛型为无后缀的 `Type::method`）
    let fn_name = instantiate_impl_method(ctx, &impl_def, &method_def, &subst, span)?;

    // 检查参数并组装调用（self 为接收者）
    if args.len() != expected.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: base_fn,
            expected: expected.len(),
            found: args.len(),
            span,
        });
    }
    for (i, (ty, pty)) in arg_tys.iter().zip(&expected).enumerate() {
        if !ty.compatible_with(pty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: base_fn.clone(),
                index: i + 1,
                expected: pty.to_string(),
                found: ty.to_string(),
                span: args[i].span,
            });
        }
    }
    Ok((
        HirExpr::Call {
            callee: fn_name,
            args: hir_args,
        },
        ret_ty,
    ))
}

/// 泛型函数调用：由实参类型推断类型参数并实例化。
fn check_generic_call(
    ctx: &mut TypeContext,
    resolved: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let template = ctx
        .fn_templates
        .get(resolved)
        .cloned()
        .ok_or_else(|| TypeError::FunctionNotFound {
            name: resolved.to_string(),
            span,
        })?;
    if args.len() != template.sig.params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: resolved.to_string(),
            expected: template.sig.params.len(),
            found: args.len(),
            span,
        });
    }

    // 由实参类型推断类型参数
    let mut subst: HashMap<String, Type> = HashMap::new();
    let mut hir_args = Vec::with_capacity(args.len());
    for (arg, pty) in args.iter().zip(&template.sig.params) {
        let (hir, ty) = infer_expr(ctx, arg)?;
        unify(pty, &ty, &mut subst)?;
        hir_args.push(hir);
    }

    // 实例化（或命中缓存）得到具体函数名与替换后的签名
    let (fn_name, signature) = instantiate_generic_fn(ctx, resolved, &template, &subst, span)?;
    if signature.params.len() != hir_args.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: resolved.to_string(),
            expected: signature.params.len(),
            found: hir_args.len(),
            span,
        });
    }
    for (i, (arg, pty)) in args.iter().zip(&signature.params).enumerate() {
        let (_, ty) = infer_expr(ctx, arg)?;
        if !ty.compatible_with(pty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: resolved.to_string(),
                index: i,
                expected: pty.to_string(),
                found: ty.to_string(),
                span: arg.span,
            });
        }
    }
    Ok((
        HirExpr::Call {
            callee: fn_name,
            args: hir_args,
        },
        signature.return_type,
    ))
}

/// 实例化泛型函数：克隆模板、替换类型参数、检查 body，输出为具体函数项。
///
/// 实例键 = `名称#T1,T2`；重复实例化命中缓存。实例函数名 =
/// `名称__T1_T2`。
fn instantiate_generic_fn(
    ctx: &mut TypeContext,
    resolved: &str,
    template: &FnTemplate,
    subst: &HashMap<String, Type>,
    span: Span,
) -> Result<(String, FnSignature), TypeError> {
    let key = mono_key(resolved, template, subst);
    if let Some(existing) = ctx.mono_instances.get(&key) {
        let sig = ctx.lookup_fn_signature(existing).cloned().ok_or_else(|| {
            TypeError::FunctionNotFound {
                name: existing.clone(),
                span,
            }
        })?;
        return Ok((existing.clone(), sig));
    }

    // 实例函数名：`名称__T1_T2`（可读的稳定后缀）
    let suffix = template
        .type_params
        .iter()
        .map(|tp| {
            subst
                .get(tp)
                .map(type_mono_key)
                .unwrap_or_else(|| tp.clone())
        })
        .collect::<Vec<_>>()
        .join("_");
    let mono_name = format!("{resolved}__{suffix}");

    // 克隆 AST 并替换类型参数后检查（body 内 `T` 经 generic_subst 解析）
    let mut cloned = template.ast.clone();
    cloned.name = mono_name.clone();
    cloned.generics.clear();

    // 先注册实例键，防止 body 内递归调用自身导致无限实例化
    ctx.mono_instances.insert(key, mono_name.clone());

    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = template.type_params.clone();
    ctx.generic_subst = subst.clone();

    let sig = crate::check_item::fn_signature_with_self(ctx, &cloned, None, span)?;
    let body = crate::check_item::check_fn_body_with_self(ctx, &cloned, None)?;

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;

    let params = cloned
        .params
        .iter()
        .map(|p| HirParam {
            name: p.name.clone(),
        })
        .collect();
    ctx.mono_items.push(HirItem {
        name: mono_name.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params,
            body,
            is_extern: false,
            extern_sig: None,
        }),
    });
    ctx.insert_fn_signature(mono_name.clone(), sig.clone());
    Ok((mono_name, sig))
}

/// 实例化泛型 impl 方法：函数名 = `Type::method__T1_T2`。
fn instantiate_impl_method(
    ctx: &mut TypeContext,
    impl_def: &ImplDef,
    method_def: &crate::types::ImplMethod,
    subst: &HashMap<String, Type>,
    span: Span,
) -> Result<String, TypeError> {
    let Type::Named(base_name, _) = &impl_def.self_type else {
        return Err(TypeError::Unsupported {
            what: "impl 目标类型必须为具名类型".to_string(),
            span,
        });
    };
    let base_fn = format!("{base_name}::{}", method_def.sig.name);
    let suffix = impl_def
        .type_params
        .iter()
        .map(|tp| {
            subst
                .get(tp)
                .map(type_mono_key)
                .unwrap_or_else(|| tp.clone())
        })
        .collect::<Vec<_>>()
        .join("_");
    let mono_name = if suffix.is_empty() {
        base_fn.clone()
    } else {
        format!("{base_fn}__{suffix}")
    };
    let key = format!("{base_fn}#{suffix}");
    if ctx.mono_instances.contains_key(&key) {
        return Ok(mono_name);
    }

    let body_ast = method_def.body.clone().ok_or_else(|| TypeError::Unsupported {
        what: "无函数体的抽象方法被调用".to_string(),
        span,
    })?;
    let mut cloned = body_ast.clone();
    cloned.name = mono_name.clone();
    cloned.generics.clear();

    ctx.mono_instances.insert(key, mono_name.clone());

    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    ctx.type_params = impl_def.type_params.clone();
    ctx.generic_subst = subst.clone();

    // self 参数类型：impl 方法签名的首个参数（`&self` 层级已含）经替换。
    // 静态方法（无 self 参数）传 None。
    let self_param = method_def.sig.params.first().map(|p| substitute(p, subst));
    let sig = crate::check_item::fn_signature_with_self(ctx, &cloned, self_param.as_ref(), span)?;
    let body = crate::check_item::check_fn_body_with_self(ctx, &cloned, self_param.as_ref())?;

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;

    let params = cloned
        .params
        .iter()
        .map(|p| HirParam {
            name: p.name.clone(),
        })
        .collect();
    ctx.mono_items.push(HirItem {
        name: mono_name.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params,
            body,
            is_extern: false,
            extern_sig: None,
        }),
    });
    ctx.insert_fn_signature(mono_name.clone(), sig);
    Ok(mono_name)
}

/// 泛型实例化键：`名称#T1,T2`。
fn mono_key(
    resolved: &str,
    template: &FnTemplate,
    subst: &HashMap<String, Type>,
) -> String {
    let args = template
        .type_params
        .iter()
        .map(|tp| {
            subst
                .get(tp)
                .map(type_mono_key)
                .unwrap_or_else(|| tp.clone())
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{resolved}#{args}")
}

/// 类型参数统一：将参数类型中的泛型占位与实参类型绑定。
fn unify(
    param: &Type,
    arg: &Type,
    subst: &mut HashMap<String, Type>,
) -> Result<(), TypeError> {
    match param {
        Type::Generic(tp) => {
            subst.insert(tp.clone(), arg.clone());
            Ok(())
        }
        Type::Named(pn, ps) => {
            if let Type::Named(an, as_) = arg {
                if pn == an {
                    for (p, a) in ps.iter().zip(as_.iter()) {
                        unify(p, a, subst)?;
                    }
                }
            }
            Ok(())
        }
        Type::Ref(inner, _) => {
            if let Type::Ref(ainner, _) = arg {
                unify(inner, ainner, subst)?;
            }
            Ok(())
        }
        Type::Tuple(ts) => {
            if let Type::Tuple(ats) = arg {
                for (p, a) in ts.iter().zip(ats.iter()) {
                    unify(p, a, subst)?;
                }
            }
            Ok(())
        }
        // 未定型类型参数（`_`）：用实参类型替换 subst 中所有 Infer 条目。
        // 场景：裸 `Result::Err(7).unwrap_or(100)` —— 接收者 unified 后
        // `T → Infer`、`E → i64`，实参 100 将 T 定型为 i64。
        Type::Infer => {
            for v in subst.values_mut() {
                if matches!(v, Type::Infer) {
                    *v = arg.clone();
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// 类型中是否含未定型 `_`（Infer）。
fn contains_infer(ty: &Type) -> bool {
    match ty {
        Type::Infer => true,
        Type::Named(_, ps) => ps.iter().any(contains_infer),
        Type::Ref(inner, _) => contains_infer(inner),
        Type::Tuple(ts) => ts.iter().any(contains_infer),
        Type::Array(inner, _) => contains_infer(inner),
        _ => false,
    }
}

/// 用替换表替换类型中的泛型占位。
fn substitute(ty: &Type, subst: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Generic(tp) => subst.get(tp).cloned().unwrap_or_else(|| ty.clone()),
        Type::Named(n, ps) => Type::Named(
            n.clone(),
            ps.iter().map(|p| substitute(p, subst)).collect(),
        ),
        Type::Ref(inner, m) => Type::Ref(Box::new(substitute(inner, subst)), *m),
        Type::Tuple(ts) => Type::Tuple(ts.iter().map(|t| substitute(t, subst)).collect()),
        Type::Array(inner, n) => Type::Array(Box::new(substitute(inner, subst)), *n),
        _ => ty.clone(),
    }
}

/// 剥离引用层（`&T` → `T`）。
fn peel_ref(ty: &Type) -> Type {
    match ty {
        Type::Ref(inner, _) => (**inner).clone(),
        _ => ty.clone(),
    }
}

/// `hash_value(s)`（s: String）→ djb2 内容哈希 HIR：
///
/// ```text
/// let __s = <expr>;
/// let __data = __s.data;   // 槽 0 字节指针
/// let __len = __s.len;     // 槽 1 长度
/// let mut __h = 5381;      // djb2 初始值
/// let mut __i = 0;
/// loop {
///     if __i >= __len { break }
///     let __b = __data[__i];   // 字节（u8，LIR 层 zext 为 i64）
///     __h = __h * 33 + __b;
///     __i = __i + 1;
/// }
/// __h
/// ```
///
/// 同一内容字符串恒得相同哈希（HashMap 探测链正确性）；乘法按 LLVM `mul`
/// wrapping 语义回绕。操作数绑定唯一临时变量，防止重复求值。
fn string_hash_hir(ctx: &mut TypeContext, s: &HirExpr) -> HirExpr {
    let s_name = ctx.fresh_temp();
    let data_name = ctx.fresh_temp();
    let len_name = ctx.fresh_temp();
    let h_name = ctx.fresh_temp();
    let i_name = ctx.fresh_temp();
    let b_name = ctx.fresh_temp();

    let s_var = HirExpr::Variable(s_name.clone());
    let data_field = HirExpr::FieldGet {
        base: Box::new(s_var.clone()),
        index: 0,
        ty: FieldScalar::Ptr,
    };
    let len_field = HirExpr::FieldGet {
        base: Box::new(s_var),
        index: 1,
        ty: FieldScalar::Int,
    };

    let loop_body = HirBlock {
        stmts: vec![
            // if __i >= __len { break }
            HirStmt::Expr(HirExpr::If {
                cond: Box::new(HirExpr::Binary(
                    HirBinaryOp::Ge,
                    Box::new(HirExpr::Variable(i_name.clone())),
                    Box::new(HirExpr::Variable(len_name.clone())),
                )),
                then_block: Box::new(HirBlock {
                    stmts: vec![HirStmt::Expr(HirExpr::Break(None))],
                    final_expr: None,
                }),
                else_block: None,
            }),
            // let __b = __data[__i]
            HirStmt::Let {
                name: b_name.clone(),
                init: HirExpr::Index {
                    base: Box::new(HirExpr::Variable(data_name.clone())),
                    index: Box::new(HirExpr::Variable(i_name.clone())),
                    elem: FieldScalar::Int,
                    is_str: true,
                },
                mutable: false,
            },
            // __h = __h * 33 + __b
            HirStmt::Expr(HirExpr::Assign {
                target: h_name.clone(),
                op: HirAssignOp::Assign,
                value: Box::new(HirExpr::Binary(
                    HirBinaryOp::Add,
                    Box::new(HirExpr::Binary(
                        HirBinaryOp::Mul,
                        Box::new(HirExpr::Variable(h_name.clone())),
                        Box::new(HirExpr::IntLiteral(33)),
                    )),
                    Box::new(HirExpr::Variable(b_name)),
                )),
            }),
            // __i = __i + 1
            HirStmt::Expr(HirExpr::Assign {
                target: i_name.clone(),
                op: HirAssignOp::Assign,
                value: Box::new(HirExpr::Binary(
                    HirBinaryOp::Add,
                    Box::new(HirExpr::Variable(i_name.clone())),
                    Box::new(HirExpr::IntLiteral(1)),
                )),
            }),
        ],
        final_expr: None,
    };

    let stmts = vec![
        HirStmt::Let {
            name: s_name,
            init: s.clone(),
            mutable: false,
        },
        HirStmt::Let {
            name: data_name,
            init: data_field,
            mutable: false,
        },
        HirStmt::Let {
            name: len_name,
            init: len_field,
            mutable: false,
        },
        HirStmt::Let {
            name: h_name.clone(),
            init: HirExpr::IntLiteral(5381),
            mutable: true,
        },
        HirStmt::Let {
            name: i_name.clone(),
            init: HirExpr::IntLiteral(0),
            mutable: true,
        },
        HirStmt::Expr(HirExpr::Loop {
            body: Box::new(loop_body),
        }),
    ];

    HirExpr::Block(Box::new(HirBlock {
        stmts,
        final_expr: Some(HirExpr::Variable(h_name)),
    }))
}
