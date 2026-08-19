//! 表达式类型推断与 HIR 生成。

use zeta_ast::{AstBlock, AstExpr, AstType, BinaryOp, ExprKind, UnaryOp};
use zeta_hir::{HirBinaryOp, HirBlock, HirExpr, HirUnaryOp};
use zeta_lexer::Span;

use crate::comparison;
use crate::context::TypeContext;
use crate::error::TypeError;
use crate::in_expr;
use crate::types::{Mutability, Type};

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
            let ty = ctx.variable_type(name, span)?;
            Ok((HirExpr::Variable(name.clone()), ty))
        }
        ExprKind::Path(_) => Err(TypeError::Unsupported {
            what: "路径表达式（`a::b::c`）".to_string(),
            span,
        }),
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
        ExprKind::InRegion { expr, .. } => {
            // 区域归属：MVP 阶段仅检查内部表达式
            let (hir, ty) = infer_expr(ctx, expr)?;
            Ok((hir, ty))
        }

        ExprKind::Assign { target, value, .. } => {
            let (_, t_ty) = infer_expr(ctx, target)?;
            let (_, v_ty) = infer_expr(ctx, value)?;
            if !t_ty.compatible_with(&v_ty) {
                return Err(TypeError::WrongType {
                    expected: t_ty.to_string(),
                    found: v_ty.to_string(),
                    span,
                });
            }
            Ok((HirExpr::Unit, Type::Unit))
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

        ExprKind::Match { .. } => Err(TypeError::Unsupported {
            what: "match 表达式在 MVP 阶段".to_string(),
            span,
        }),

        ExprKind::For { iterator, .. } => {
            let (_, it_ty) = infer_expr(ctx, iterator)?;
            if it_ty.is_numeric() || matches!(it_ty, Type::Str) {
                // 简化：数值与字符串视为可迭代（MVP）
                Ok((HirExpr::Unit, Type::Unit))
            } else {
                Err(TypeError::ExpectedIterable {
                    found: it_ty.to_string(),
                    span,
                })
            }
        }
        ExprKind::While { cond, .. } => {
            let (_, c_ty) = infer_expr(ctx, cond)?;
            if !c_ty.is_bool() {
                return Err(TypeError::ExpectedBool {
                    found: c_ty.to_string(),
                    span,
                });
            }
            Ok((HirExpr::Unit, Type::Unit))
        }
        ExprKind::Loop { .. } => Ok((HirExpr::Unit, Type::Unit)),

        ExprKind::Region { body, .. } => {
            let (_, ty) = check_block(ctx, body)?;
            Ok((HirExpr::Unit, ty))
        }
        ExprKind::Transfer { expr, .. } => {
            let (hir, ty) = infer_expr(ctx, expr)?;
            Ok((hir, ty))
        }

        ExprKind::Call { callee, args } => check_call(ctx, callee, args, span),
        ExprKind::MethodCall { .. } => Err(TypeError::Unsupported {
            what: "方法调用在 MVP 阶段".to_string(),
            span,
        }),
        ExprKind::FieldAccess { .. } => Err(TypeError::Unsupported {
            what: "字段访问在 MVP 阶段".to_string(),
            span,
        }),
        ExprKind::Index { .. } => Err(TypeError::Unsupported {
            what: "索引访问在 MVP 阶段".to_string(),
            span,
        }),
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
        _ => {
            return Err(TypeError::Unsupported {
                what: "复杂被调用表达式".to_string(),
                span,
            });
        }
    };

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

    // 普通函数调用：查签名并检查实参
    let signature =
        ctx.lookup_fn_signature(&name)
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
            callee: name,
            args: hir_args,
        },
        signature.return_type,
    ))
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
        AstType::Array(inner, _) => {
            let inner = resolve_ast_type(ctx, inner, span)?;
            Ok(Type::Array(Box::new(inner), 0))
        }
        AstType::Fn(_, _) => Err(TypeError::Unsupported {
            what: "函数类型 `fn(A) -> B` 在 MVP 阶段".to_string(),
            span,
        }),
        AstType::Infer => Ok(Type::Infer),
    }
}
