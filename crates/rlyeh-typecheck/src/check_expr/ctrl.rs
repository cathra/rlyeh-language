//! ctrl：表达式推断的控制流 / 调用 / 宏等变体。
//! （由 check_expr/mod.rs 的 `infer_expr` 尾部分支拆分而来，保持语义等价）
//!
//! 覆盖 `ComparisonChain` / `In*` / `Assign` / `If` / `Match` / `For` / `While` /
//! `Loop` / `Region` / `Transfer` / `Call` / `MethodCall` / `Send` / `MacroCall` 等；
//! 字面量与 `Ident` / `Path` / `Binary` / `Unary` 仍留在 `infer_expr`。

use rlyeh_hir::HirExprKind;
use rlyeh_lexer::Span;
use super::*;
use crate::{Warning, WarningKind};

/// B-1（借用简化 RFC）：检测「对引用显式 `*` 解引用后紧接成员访问」的冗余写法
/// `(*r).field` / `(*r).method()` / `(*r)[i]`。引用会自动解引用，提示去掉 `*`。
///
/// 仅当操作数类型为 `&T` / `&mut T` 时告警；裸指针（`*p`）与自定义 `Deref` protocol
/// 解引用仍需 `*`——前者是 `Type::RawPtr`，后者操作数本身不是引用。
fn warn_redundant_deref(ctx: &mut TypeContext, recv: &AstExpr, _span: Span) {
    if let ExprKind::Unary {
        op: UnaryOp::Deref,
        operand,
    } = &*recv.kind
    {
        if let Ok((_, o_ty)) = infer_expr(ctx, operand) {
            if matches!(o_ty, Type::Ref(..)) {
                ctx.emit_warning(Warning {
                    kind: WarningKind::RedundantDeref {
                        suggestion:
                            "引用会自动解引用，去掉 `*`，直接写 `x.field` / `x.method()` / `x[i]`"
                                .to_string(),
                    },
                    span: recv.span,
                });
            }
        }
    }
}

/// `infer_expr` 尾部分支的下沉入口。
///
/// `mod.rs` 的 `infer_expr` 以 `_ =>` 兜底分派到此处；兜底分支不可达
/// （两处分支的并集覆盖全部 `ExprKind`），保留是为分派表被误改时给出明确诊断。
pub(crate) fn infer_expr_tail(
    ctx: &mut TypeContext,
    expr: &AstExpr,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    match &*expr.kind {
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
        ExprKind::InContainer {
            value,
            container,
            negated,
        } => in_expr::check_in_container_expression(ctx, value.clone(), container.clone(), *negated, span),
        ExprKind::InRegion { expr, region } => {
            let (hir, ty) = infer_expr(ctx, expr)?;
            // L3 接线：计算被归属对象在区域中的字节大小（slot_count × 8），
            // 供 codegen 生成 `rlyeh_region_alloc` + 值镜像；标量也按 1 槽计，
            // codegen 仅在目标为 Ptr 槽（聚合对象）时接线。
            let size = type_slot_count(ctx, &ty, span)?.saturating_mul(8);
            Ok((
                HirExpr::new(HirExprKind::InRegion{
                    expr: Box::new(hir),
                    region: region.clone(),
                    size,
                }, Span::dummy()),
                ty,
            ))
        }

        ExprKind::Assign { target, op, value } => {
            // Y4b-4（2026-08-30）：DerefMut 分发——`*guard = v` 经守卫的 `deref_mut`
            // 方法（返回 &mut T）生成 `*(guard.deref_mut()) = v`，复用 DerefSet；
            // 仅纯赋值（`=`）支持，复合赋值 `*g += v` 回落至既有无 Unsupported 路径。
            if matches!(op, AssignOp::Assign) {
                if let ExprKind::Unary {
                    op: UnaryOp::Deref,
                    operand,
                } = &*target.kind
                {
                    let (_, o_ty) = infer_expr(ctx, operand)?;
                    if ctx.find_impl_for_method(&o_ty, "deref_mut").is_some() {
                        let (dm_hir, dm_ty) =
                            check_method_call(ctx, operand, "deref_mut", &[], None, span, 0)?;
                        let (v_hir, v_ty) = infer_expr(ctx, value)?;
                        let inner = match &dm_ty {
                            Type::Ref(inner, _, _) => (**inner).clone(),
                            _ => dm_ty.clone(),
                        };
                        if !inner.compatible_with(&v_ty) {
                            return Err(TypeError::WrongType {
                                expected: inner.to_string(),
                                found: v_ty.to_string(),
                                span,
                                related: vec![],
                            });
                        }
                        let ty = field_scalar_of(&inner);
                        return Ok((
                            HirExpr::new(HirExprKind::DerefSet{
                                base: Box::new(dm_hir),
                                value: Box::new(v_hir),
                                ty,
                            }, Span::dummy()),
                            Type::Unit,
                        ));
                    }
                }
            }
            let (t_hir, t_ty) = infer_expr(ctx, target)?;
            // H4 去虚拟化失效：dyn 变量被重新赋值后绑定源具体类型不再成立，
            // 后续调用回退 vtable 间接分派
            if matches!(op, AssignOp::Assign) {
                if let ExprKind::Ident(var) = &*target.kind {
                    ctx.remove_dyn_concrete(var);
                }
            }
            let (mut v_hir, mut v_ty) = infer_expr(ctx, value)?;
            // U4：字段级联合赋值——目标字段类型为 `A | B`、右值为其中某成员类型时，
            // desugar 为匿名 enum 构造（复用 U2 的 `make_union_ctor`），与结构体
            // 字面量构造（`check_struct_construct`）保持同一语义。仅 `=` 适用：
            // 复合赋值（`obj.field += v`）对联合无意义，不做构造（其操作数
            // 类型检查会在下方自然报错）。
            if matches!(op, AssignOp::Assign) {
                if let Type::Union(us) = &t_ty {
                    if let Some(idx) = us.iter().position(|u| v_ty.compatible_with(u)) {
                        let member_ty = us[idx].clone();
                        v_hir = crate::check_stmt::make_union_ctor(ctx, v_hir, idx, &member_ty);
                        v_ty = t_ty.clone();
                    }
                }
            }
            if !t_ty.compatible_with(&v_ty) {
                // B-2（P0'）：目标类型 `t_ty`、右值类型 `v_ty`；若 `v_ty == &T`
                // 且 `t_ty == T` 且 `T: Copy`，自动解引用取值（`x = r;`）
                match crate::check_expr::try_auto_deref_coerce(ctx, &t_ty, &v_ty, value, span) {
                    Some(Ok((c_hir, _))) => {
                        v_hir = c_hir;
                    }
                    _ => {
                        return Err(TypeError::WrongType {
                            expected: t_ty.to_string(),
                            found: v_ty.to_string(),
                            span,
                            related: vec![],
                        });
                    }
                }
            }
            let target_name = match t_hir.kind {
                HirExprKind::Variable(v) => v,
                // 结构体 / actor 状态字段赋值：`obj.field = value` → FieldSet；
                // 复合赋值 `obj.field += v` → FieldSet(base, idx, Binary(op, FieldGet, v))
                HirExprKind::FieldGet{ base, index, ty } => {
                    if !matches!(op, AssignOp::Assign) {
                        let hir_op = match op {
                            AssignOp::AddAssign => HirBinaryOp::Add,
                            AssignOp::SubAssign => HirBinaryOp::Sub,
                            AssignOp::MulAssign => HirBinaryOp::Mul,
                            AssignOp::DivAssign => HirBinaryOp::Div,
                            AssignOp::Assign => unreachable!(),
                        };
                        return Ok((
                            HirExpr::new(HirExprKind::FieldSet{
                                base: base.clone(),
                                index,
                                value: Box::new(HirExpr::new(HirExprKind::Binary(
                                    hir_op,
                                    Box::new(HirExpr::new(HirExprKind::FieldGet{
                                        base,
                                        index,
                                        ty,
                                    }, Span::dummy())),
                                    Box::new(v_hir),
                                ), Span::dummy())),
                                ty,
                            }, Span::dummy()),
                            Type::Unit,
                        ));
                    }
                    return Ok((
                        HirExpr::new(HirExprKind::FieldSet{
                            base,
                            index,
                            value: Box::new(v_hir),
                            ty,
                        }, Span::dummy()),
                        Type::Unit,
                    ));
                }
                // 索引元素赋值：`arr[i] = value` / `s[i] = ch`（仅纯赋值）
                HirExprKind::Index{
                    base,
                    index,
                    elem,
                    is_str,
                    len: _,
                } => {
                    if !matches!(op, AssignOp::Assign) {
                        return Err(TypeError::Unsupported {
                            what: "复合赋值目标为索引元素在 MVP 阶段（仅支持 `a[i] = ...`）"
                                .to_string(),
                            span,
                        });
                    }
                    return Ok((
                        HirExpr::new(HirExprKind::IndexSet{
                            base,
                            index,
                            value: Box::new(v_hir),
                            elem,
                            is_str,
                            len: None,
                        }, Span::dummy()),
                        Type::Unit,
                    ));
                }
                // 解引用赋值：`*p = v` / `*p += v`
                HirExprKind::Deref{ expr: base, ty } => {
                    if !matches!(op, AssignOp::Assign) {
                        let hir_op = match op {
                            AssignOp::AddAssign => HirBinaryOp::Add,
                            AssignOp::SubAssign => HirBinaryOp::Sub,
                            AssignOp::MulAssign => HirBinaryOp::Mul,
                            AssignOp::DivAssign => HirBinaryOp::Div,
                            AssignOp::Assign => unreachable!(),
                        };
                        return Ok((
                            HirExpr::new(HirExprKind::DerefSet{
                                base: base.clone(),
                                value: Box::new(HirExpr::new(HirExprKind::Binary(
                                    hir_op,
                                    Box::new(HirExpr::new(HirExprKind::Deref{
                                        expr: base,
                                        ty,
                                    }, Span::dummy())),
                                    Box::new(v_hir),
                                ), Span::dummy())),
                                ty,
                            }, Span::dummy()),
                            Type::Unit,
                        ));
                    }
                    return Ok((
                        HirExpr::new(HirExprKind::DerefSet{
                            base,
                            value: Box::new(v_hir),
                            ty,
                        }, Span::dummy()),
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
            // 全局变量赋值：`static`（不可变）禁止赋值；`static mut` 必须位于
            // `unsafe` 块内（与裸指针 / extern 同属 unsafe 边界，SH-P0-1）。
            if let Some((_, is_mut)) = ctx.lookup_global(&target_name) {
                if !is_mut {
                    return Err(TypeError::CannotAssignStatic {
                        name: target_name.clone(),
                        span,
                    });
                }
                if !ctx.in_unsafe {
                    return Err(TypeError::StaticMutOutsideUnsafe {
                        name: target_name.clone(),
                        span,
                    });
                }
            }
            let hir_op = match op {
                AssignOp::Assign => HirAssignOp::Assign,
                AssignOp::AddAssign => HirAssignOp::AddAssign,
                AssignOp::SubAssign => HirAssignOp::SubAssign,
                AssignOp::MulAssign => HirAssignOp::MulAssign,
                AssignOp::DivAssign => HirAssignOp::DivAssign,
            };
            Ok((
                HirExpr::new(HirExprKind::Assign{
                    target: target_name,
                    op: hir_op,
                    value: Box::new(v_hir),
                }, Span::dummy()),
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
                            related: vec![],
                        });
                    }
                }
                _ => t_ty,
            };
            let hir = HirExpr::new(HirExprKind::If{
                cond: Box::new(c_hir),
                then_block: Box::new(t_hir),
                else_block: e_hir.map(Box::new),
            }, Span::dummy());
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
            // EH-6 M1 前置：压入 break 值槽占位——`while` 内的 `break` 不得
            // 写入外层 `loop` 的槽（循环类型仍为 Unit，槽值不使用）。
            ctx.loop_break_types.push(None);
            let (b_hir, _) = check_block(ctx, body)?;
            ctx.loop_break_types.pop();
            Ok((
                HirExpr::new(HirExprKind::While{
                    cond: Box::new(c_hir),
                    body: Box::new(b_hir),
                }, Span::dummy()),
                Type::Unit,
            ))
        }
        ExprKind::Loop { body, .. } => {
            // EH-6 M1 前置：循环类型由 `break` 携带的值决定——
            // 无 `break` → `Never`；`break;` → `Unit`；`break e` → `typeof(e)`。
            ctx.loop_break_types.push(None);
            let (b_hir, _) = check_block(ctx, body)?;
            let break_ty = ctx.loop_break_types.pop().flatten();
            Ok((
                HirExpr::new(HirExprKind::Loop{
                    body: Box::new(b_hir),
                }, Span::dummy()),
                break_ty.unwrap_or(Type::Never),
            ))
        }

        ExprKind::Region {
            name,
            options,
            body,
        } => {
            let (hir_block, ty) = check_block(ctx, body)?;
            // L3 PGO 回灌：`adaptive` 区域优先采用 profile 推荐初始容量
            // （`rlyeh build --profile` 注入，见 driver::compile_with_region_hints）。
            let pgo_size = if options.adaptive {
                name.as_ref()
                    .and_then(|n| ctx.region_hints.get(n))
                    .copied()
            } else {
                None
            };
            Ok((
                HirExpr::new(HirExprKind::Region{
                    name: name.clone(),
                    options: HirRegionOptions {
                        size: pgo_size.or(options.size),
                        allow_growth: options.allow_growth,
                        growth_factor: options.growth_factor,
                        adaptive: options.adaptive,
                        exact: options.exact,
                        strategy: options.strategy.map(|s| match s {
                            RegionStrategy::Bump => HirRegionStrategy::Bump,
                        }),
                    },
                    body: Box::new(hir_block),
                }, Span::dummy()),
                ty,
            ))
        }
        ExprKind::Transfer { expr, region } => {
            let (hir, ty) = infer_expr(ctx, expr)?;
            Ok((
                HirExpr::new(HirExprKind::Transfer{
                    expr: Box::new(hir),
                    region: region.clone(),
                }, Span::dummy()),
                ty,
            ))
        }

        // EH-6 M1（2026-09-21）：`try { .. }` 错误聚合块——desugar 为
        // `loop { <body>; break <Kind>::<OkVariant>(<tail>) }`，块内 `?` 改为
        // 残留式 `break`。块的类型取外层函数的错误位类型。
        ExprKind::TryBlock(block) => check_try_block(ctx, block, span),

        ExprKind::Call {
            callee,
            args,
            type_args,
        } => check_call(ctx, callee, args, type_args, span),
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            protocol_hint,
        } => {
            warn_redundant_deref(ctx, receiver, span);
            check_method_call(ctx, receiver, method, args, protocol_hint.as_deref(), span, 0)
        }
        ExprKind::StructCtor {
            type_name,
            type_args,
            fields,
            base,
        } => check_struct_construct(ctx, type_name, type_args, fields, base, span),
        ExprKind::TupleLit(elems) => check_tuple_construct(ctx, elems, span),
        ExprKind::FieldAccess { expr, field } => {
            warn_redundant_deref(ctx, expr, span);
            check_field_access(ctx, expr, field, span, 0)
        }
        ExprKind::Index { expr, index } => {
            warn_redundant_deref(ctx, expr, span);
            check_index(ctx, expr, index, span, 0)
        }
        ExprKind::ArrayLit(elems) => check_array_lit(ctx, elems, span),
        ExprKind::Closure { .. } => Err(TypeError::Unsupported {
            what: "闭包缺少 fn 类型上下文（H2 无捕获闭包：用作 fn 形参实参，或 `let f: fn(..) = |..| ..` 注解绑定；捕获闭包 H3 规划中）"
                .to_string(),
            span,
        }),

        ExprKind::Cast { expr, target_type } => {
            let (hir, src_ty) = infer_expr(ctx, expr)?;
            let dst_ty = resolve_ast_type(ctx, target_type, span)?;
            // U6 Cast IR：仅对「数值→数值」且源/目标不同的转换产出 Cast 节点
            //（i128/u128 存储非 64 位槽，与指针/引用/聚合转换一并保持擦除）。
            if is_castable_scalar(&src_ty) && is_castable_scalar(&dst_ty) && src_ty != dst_ty {
                Ok((
                    HirExpr::new(HirExprKind::Cast{
                        expr: Box::new(hir),
                        to: dst_ty.to_string(),
                    }, Span::dummy()),
                    dst_ty,
                ))
            } else {
                Ok((hir, dst_ty))
            }
        }

        ExprKind::Await(inner) => infer_expr(ctx, inner),

        ExprKind::Block(block) => {
            let (hir, ty) = check_block(ctx, block)?;
            Ok((HirExpr::new(HirExprKind::Block(Box::new(hir)), Span::dummy()), ty))
        }
        ExprKind::UnsafeBlock(block) => {
            // SH-P0-1：`unsafe { ... }` 块表达式。块内进入受控上下文，
            // extern 函数调用门禁（E3）在块内放行（见 development-plan-0.2.0.md §3.5 / SH-P0-1）。
            let prev = ctx.in_unsafe;
            ctx.in_unsafe = true;
            let (hir, ty) = check_block(ctx, block)?;
            ctx.in_unsafe = prev;
            Ok((HirExpr::new(HirExprKind::UnsafeBlock(Box::new(hir)), Span::dummy()), ty))
        }
        ExprKind::GcRegion { body } => check_gc_region(ctx, body, span),
        ExprKind::Return(Some(e)) => {
            let (hir, _) = infer_expr(ctx, e)?;
            Ok((HirExpr::new(HirExprKind::Return(Some(Box::new(hir))), Span::dummy()), Type::Never))
        }
        ExprKind::Return(None) => Ok((HirExpr::new(HirExprKind::Return(None), Span::dummy()), Type::Never)),
        ExprKind::Question(inner) => check_question(ctx, inner, span),
        ExprKind::Break(Some(e)) => {
            let (hir, ty) = infer_expr(ctx, e)?;
            // EH-6 M1 前置：把值类型登记到最内层循环（首次登记为准；多臂
            // `break` 类型不一致属用户错误，交由下游按变量类型冲突报出）。
            if let Some(slot) = ctx.loop_break_types.last_mut() {
                if slot.is_none() {
                    *slot = Some(ty);
                }
            }
            Ok((HirExpr::new(HirExprKind::Break(Some(Box::new(hir))), Span::dummy()), Type::Never))
        }
        ExprKind::Break(None) => {
            // `break;` 携带单元值——循环类型为 `()`（而非 `Never`）。
            if let Some(slot) = ctx.loop_break_types.last_mut() {
                if slot.is_none() {
                    *slot = Some(Type::Unit);
                }
            }
            // EH-6 M1：若位于 `try` 块的隐式循环层（即当前循环栈深度恰等于
            // try 块入口记录的基准），说明这是**裸 `break`**，会越界跳到隐式
            // 循环而非用户预期的目标——直接拒绝（嵌套的真实循环深度更深，
            // 不受影响，见 check_try_block 的越界拦截）。
            if let Some(&base) = ctx.try_loop_bases.last() {
                if ctx.loop_break_types.len() == base + 1 {
                    return Err(TypeError::BreakOutsideLoop { span });
                }
            }
            Ok((HirExpr::new(HirExprKind::Break(None), Span::dummy()), Type::Never))
        }
        // EH-6 M1：`?` 在 `try` 块内合成的残留式跳出。desugar 为 `break
        // <Err/None>(..)`，但**不登记**循环 break 值类型（`Break(Some)` 会
        // 把 Ok 位未定的 `Result<?, E>` 登记进去，污染 `try` 块类型）——其
        // 值类型由 `check_try_block` 显式组装，此处只负责降低为 `Never`。
        ExprKind::TryBreak(e) => {
            let (hir, _) = infer_expr(ctx, e.as_ref())?;
            Ok((HirExpr::new(HirExprKind::Break(Some(Box::new(hir))), Span::dummy()), Type::Never))
        }
        ExprKind::Continue => {
            // EH-6 M1：裸 `continue` 位于 `try` 块隐式循环层会重启 try 体，
            // 属静默错误，直接拒绝（嵌套真实循环的 continue 不受影响）。
            if let Some(&base) = ctx.try_loop_bases.last() {
                if ctx.loop_break_types.len() == base + 1 {
                    return Err(TypeError::BreakOutsideLoop { span });
                }
            }
            Ok((HirExpr::new(HirExprKind::Continue, Span::dummy()), Type::Never))
        }

        ExprKind::Send { actor, method, args } => {
            // `send actor.method(a, b)` → `rlyeh_actor_send(recv, kind, a, b, 0)`
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
                    let mut call_args = vec![recv_hir, HirExpr::new(HirExprKind::IntLiteral(kind as i128), Span::dummy())];
                    for arg in args {
                        let (h, t) = infer_expr(ctx, arg)?;
                        if !t.compatible_with(&Type::I64) {
                            return Err(TypeError::ArgumentTypeMismatch {
                                name: format!("{self_ty}::{method}"),
                                index: call_args.len() - 2,
                                expected: "i64".to_string(),
                                found: t.to_string(),
                                span: arg.span,
                                related: vec![],
                            });
                        }
                        call_args.push(h);
                    }
                    while call_args.len() < 5 {
                        call_args.push(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy()));
                    }
                    return Ok((
                        HirExpr::new(HirExprKind::Call{
                            callee: "rlyeh_actor_send".to_string(),
                            args: call_args,
                        }, Span::dummy()),
                        Type::I64,
                    ));
                }
            }
            Err(TypeError::Unsupported {
                what: "send 目标不是 Actor 类型".to_string(),
                span,
            })
        }
        // 内置格式化宏（I2）：`println!` / `print!` / `format!` / `dbg!`
        ExprKind::MacroCall { name, args } => check_macro_call(ctx, name, args, span),
        _ => Err(TypeError::Unsupported {
            what: "表达式变体（既未归入 infer_expr 也未归入 infer_expr_tail）".to_string(),
            span,
        }),
    }
}

/// EH-6 M1（2026-09-21）：`try { .. }` 错误聚合块。
///
/// **desugar**：`try { <stmts>; <tail> }` →
/// `loop { <stmts>; break <Kind>::<OkVariant>(<tail>) }`，块内 `?` 在
/// `check_question` 中改为残留式 `break`（见 `ExprKind::TryBreak`）。块的类型 =
/// 外层函数的错误位类型 `Result<T, E_fn>` / `Option<T>`（`E_fn` 取自
/// `current_return_type`，与 `?` 的 `From` 转换目标一致，**无需类型推断**）。
///
/// **块值类型组装**：隐式循环的 break 值类型由 `break Ok(tail)` 登记为
/// `Result<T_tail, ?>`（`?` 残留用 `TryBreak` 不登记，避免 Ok 位未定污染类型），
/// 故循环类型已是 `Result<T_tail, _>`，这里仅把 `_` 用 `E_fn` 显式替换。
///
/// **越界拦截**：进块时记录 `loop_break_types` 深度基准，块内裸 `break` /
/// `continue` 落在该层即拒绝（`infer_expr_tail` 的 `Break`/`Continue` 分支处理）。
pub(super) fn check_try_block(
    ctx: &mut TypeContext,
    block: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 1. 函数须返回 Result / Option（错误位类型来自外层函数）。
    let (rt_name, ok_variant) = match &ctx.current_return_type {
        Some(Type::Named(n, _)) if n.ends_with("Result") => (n.clone(), "Ok"),
        Some(Type::Named(n, _)) if n.ends_with("Option") => (n.clone(), "Some"),
        _ => {
            return Err(TypeError::Unsupported {
                what: "`try` 块必须位于返回 `Result<T, E>` 或 `Option<T>` 的函数内".to_string(),
                span,
            })
        }
    };

    // 2. 组装 desugar AST：`loop { <stmts>; break <Kind>::<OkVariant>(<tail>) }`。
    let tail = block
        .final_expr
        .clone()
        .unwrap_or_else(|| AstExpr::new(ExprKind::Unit, span));
    let ok_callee = AstExpr::new(
        ExprKind::Path(vec![rt_name.clone(), ok_variant.to_string()]),
        span,
    );
    let ok_arg = AstExpr::new(
        ExprKind::Call {
            callee: ok_callee,
            args: vec![tail],
            type_args: vec![],
        },
        span,
    );
    let final_break = AstExpr::new(ExprKind::Break(Some(ok_arg)), span);
    let loop_body = AstBlock {
        stmts: block.stmts.clone(),
        final_expr: Some(final_break),
        span: block.span,
    };
    let loop_ast = AstExpr::new(
        ExprKind::Loop {
            body: loop_body,
        },
        span,
    );

    // 3. 进入隐式循环：压入越界基准，检查（含块内 `?` → `TryBreak`），弹出。
    ctx.try_loop_bases.push(ctx.loop_break_types.len());
    let (hir, loop_ty) = infer_expr(ctx, &loop_ast)?;
    ctx.try_loop_bases.pop();

    // 4. 组装块类型：取循环 break 值的第 0 位作 T，错误位 E 取自外层函数。
    let tail_ty = match &loop_ty {
        Type::Named(_, args) => args.first().cloned().unwrap_or(Type::Unit),
        _ => Type::Unit,
    };
    let result_ty = match &ctx.current_return_type {
        Some(Type::Named(n, args)) if n.ends_with("Result") => Type::Named(
            n.clone(),
            vec![tail_ty, args.get(1).cloned().unwrap_or(Type::Unit)],
        ),
        Some(Type::Named(n, _)) if n.ends_with("Option") => {
            Type::Named(n.clone(), vec![tail_ty])
        }
        _ => unreachable!("rt_name 已保证 Result/Option"),
    };
    Ok((hir, result_ty))
}
