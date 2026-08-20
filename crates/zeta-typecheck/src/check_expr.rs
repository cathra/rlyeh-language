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

/// 内建函数（由代码生成层映射到运行时，无需用户声明）。
///
/// 与 `zeta-lir::lower::BUILTIN_FUNCTIONS`、`zeta-codegen` 保持一致。
pub const BUILTIN_FUNCTIONS: &[&str] = &["print", "println"];

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
                // 结构体字段赋值：`obj.field = value` → FieldSet（仅纯赋值）
                HirExpr::FieldGet { base, index, ty } => {
                    if !matches!(op, AssignOp::Assign) {
                        return Err(TypeError::Unsupported {
                            what: "复合赋值目标为结构体字段在 MVP 阶段（仅支持 `field = ...`）"
                                .to_string(),
                            span,
                        });
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
                    // then 与 else 分支类型不一致：取较大的（数值）或报错
                    if t_ty.compatible_with(et) {
                        merge_numeric(t_ty.clone(), et.clone())
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

        ExprKind::Send { .. } => Err(TypeError::Unsupported {
            what: "Actor 消息发送在 MVP 阶段".to_string(),
            span,
        }),
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

/// 检查 for 循环：`for pat in lo..<hi { body }`。
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
fn check_for(
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
            if !left.is_integer() || !right.is_integer() {
                return Err(TypeError::ExpectedInt {
                    found: left.to_string(),
                    span,
                });
            }
            let hir_op = match op {
                BinaryOp::BitAnd => HirBinaryOp::Mod, // placeholder 不会命中
                BinaryOp::BitOr => HirBinaryOp::Mod,
                BinaryOp::BitXor => HirBinaryOp::Mod,
                BinaryOp::Shl => HirBinaryOp::Mod,
                _ => HirBinaryOp::Mod,
            };
            let _ = hir_op;
            return Err(TypeError::Unsupported {
                what: "位运算在 MVP 阶段".to_string(),
                span,
            });
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

    // 内建函数（`print` / `println` 等，由代码生成层映射到运行时）：
    // 接受任意类型参数、返回 `()`
    if BUILTIN_FUNCTIONS.contains(&name.as_str()) {
        let mut hir_args = Vec::with_capacity(args.len());
        for a in args {
            let (hir, _) = infer_expr(ctx, a)?;
            hir_args.push(hir);
        }
        return Ok((
            HirExpr::Call {
                callee: name,
                args: hir_args,
            },
            Type::Unit,
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

    // 普通函数调用：先经 use 别名 / 模块路径解析到完整符号名，再查签名
    let resolved = resolve_callable(ctx, &name);

    // 枚举变体构造：`Option::Some(x)`、`shape::Kind::Pair(x, y)` 或裸 `Some(x)`
    // （普通函数同名时优先函数路径）
    if !ctx.fn_signatures.contains_key(&resolved) {
        if let Some((en, vr)) = split_variant_path(ctx, &resolved) {
            return check_variant_construct(ctx, &en, &vr, args, span);
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
    Ok((
        HirExpr::FieldGet {
            base: Box::new(base_hir),
            index: idx,
            ty: field_scalar_of(&fty),
        },
        fty,
    ))
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
    let (b_hir, b_ty) = infer_expr(ctx, expr)?;
    let (i_hir, i_ty) = infer_expr(ctx, index)?;
    if !i_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: i_ty.to_string(),
            span: index.span,
        });
    }
    match peel_ref(&b_ty) {
        Type::Array(elem_ty, _) => Ok((
            HirExpr::Index {
                base: Box::new(b_hir),
                index: Box::new(i_hir),
                elem: field_scalar_of(&elem_ty),
                is_str: false,
            },
            elem_ty.as_ref().clone(),
        )),
        Type::Str => Ok((
            HirExpr::Index {
                base: Box::new(b_hir),
                index: Box::new(i_hir),
                elem: FieldScalar::Char,
                is_str: true,
            },
            Type::Char,
        )),
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
            // 字段类型经泛型替换（impl 方法实例化时 `T` → 具体类型）。
            let mut binds = Vec::new();
            let mut bound_tys = Vec::new();
            let mut cond = tag_cond;
            for (i, (sub, (_, fty))) in sub_pats.iter().zip(&variant_def.fields).enumerate() {
                let fty_sub = substitute(fty, &ctx.generic_subst);
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
    let mut hir_args = vec![recv_hir];
    for (i, (arg, pty)) in args.iter().zip(&expected).enumerate() {
        let (hir, ty) = infer_expr(ctx, arg)?;
        if !ty.compatible_with(pty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: base_fn.clone(),
                index: i + 1,
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
        kind: HirItemKind::Fn(HirFnDecl { params, body }),
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
        kind: HirItemKind::Fn(HirFnDecl { params, body }),
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
        _ => Ok(()),
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
