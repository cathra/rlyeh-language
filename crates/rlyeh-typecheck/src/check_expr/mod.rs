//! 表达式类型推断与 HIR 生成。

use std::collections::HashMap;

use rlyeh_ast::{AssignOp, AstBlock, AstExpr, AstPattern, AstStmt, AstType, BinaryOp, CaptureMode, CompareOp, ExprKind, RegionStrategy, UnaryOp};
use rlyeh_hir::{
    FieldScalar, HirAssignOp, HirBinaryOp, HirBlock, HirExpr, HirFnDecl, HirItem, HirItemKind,
    HirParam, HirRegionOptions, HirRegionStrategy, HirStmt, HirUnaryOp,
};
use rlyeh_lexer::Span;

use crate::check_item::type_to_extern_name;
use crate::comparison;
use crate::context::{DeferredClosure, FnTemplate, TypeContext};
use crate::error::TypeError;
use crate::in_expr;
use crate::types::{field_scalar_of, type_mono_key, FnSignature, ImplDef, Mutability, Type};

/// 查询内建函数签名；`None` 表示不是内建。
///
/// - `print` / `println`：任意类型参数（`Infer` 与一切兼容）、返回 `()`
/// - `alloc_array(n)`：运行时槽数分配，返回 `[T; 0]`（长度 0 约定 = 动态数组指针）
/// - `array_copy(dst, src, n)` / `array_free(p)`：动态数组缓冲操作
///
/// 与 `rlyeh-lir::lower::BUILTIN_FUNCTIONS`、`rlyeh-codegen` 保持一致。

/// H4 `dyn Trait` 转换：把具体类型的数据指针转成 trait 对象胖指针。
///
/// 生成 HIR 块（vtable 运行时构造 + 2 槽胖指针）：
/// ```text
/// let __vt = Alloc(3 + N);              // N = trait 方法数；槽 0-2 为 drop/size/align（MVP = 0）
/// FieldSet(__vt, 0, 0); FieldSet(__vt, 1, 0); FieldSet(__vt, 2, 0);
/// let __m0 = FnPtr("Circle::area");   FieldSet(__vt, 3, __m0);
/// let __m1 = FnPtr("Circle::describe"); FieldSet(__vt, 4, __m1);
/// let __dyn = Alloc(2);                 // 胖指针：槽 0 = 数据指针，槽 1 = vtable 指针
/// FieldSet(__dyn, 0, data_ptr); FieldSet(__dyn, 1, __vt);
/// final: __dyn
/// ```
///
/// 后续 `dyn_obj.method(args)` 经 `check_method_call` 的 `Type::Dyn` 分支
/// 从 vtable 槽 `3 + 方法索引` 读函数指针并间接调用。
/// MVP 限制：trait 与 impl 均须非泛型；转换源为具体类型（非泛型参数）。

/// H4 辅助：类型是否引用了 `Self`。trait 方法签名含关联类型（`Self`）时，
/// trait 对象调用无法确定具体类型，MVP 报 Unsupported。

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
        // X4：单元类型字面量 `()`（`Result::Ok(())` 的值；空 tuple）
        ExprKind::Unit => Ok((HirExpr::Unit, Type::Unit)),
        ExprKind::TimeLiteral { hour, minute, .. } => {
            // 时间字面量归一化为分钟值，按整数处理（可与整数集合/范围统一比较）
            let minutes = i128::from(*hour) * 60 + i128::from(*minute);
            Ok((HirExpr::IntLiteral(minutes), Type::I64))
        }

        ExprKind::Ident(name) => {
            // 1. 局部变量（U1：HIR 引用用存储槽名——遮蔽变量经 resolve 返回
            //    mangle 槽名，下游按槽名区分变量存储）
            if let Some((slot, ty)) = ctx.resolve_variable(name) {
                return Ok((HirExpr::Variable(slot.to_string()), ty.clone()));
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
            // 3. 函数引用（函数一等值）：`let f = my_func;`。
            // 裸名经模块前缀 / use 别名解析为完整符号名。
            let resolved = resolve_callable(ctx, name);
            if !ctx.fn_templates.contains_key(&resolved) {
                if let Some(sig) = ctx.fn_signatures.get(&resolved).cloned() {
                    return Ok((
                        HirExpr::FnPtr(resolved),
                        Type::Fn(Box::new(sig)),
                    ));
                }
            }
            // 4. 无参枚举变体构造：裸 `None` / `Kind::Variant`（K1 需要
            // `return None;` 形式；与 check_call 的 split_variant_path 兜底一致）
            if let Some((en, vr)) = split_variant_path(ctx, name) {
                return check_variant_construct(ctx, &en, &vr, &[], span);
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
            // 函数引用（跨模块路径）：`math::add` 作为函数值
            if !ctx.fn_templates.contains_key(&resolved) {
                if let Some(sig) = ctx.fn_signatures.get(&resolved).cloned() {
                    return Ok((
                        HirExpr::FnPtr(resolved),
                        Type::Fn(Box::new(sig)),
                    ));
                }
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
            // P8：非切片的裸 Range 须显式边界（省略 `..` 仅切片 `v[..]` 支持）
            let lower = lower.as_ref().ok_or_else(|| TypeError::Unsupported {
                what: "范围表达式缺少下界（省略边界 `..` 仅切片 `v[..]` 支持）".to_string(),
                span,
            })?;
            let upper = upper.as_ref().ok_or_else(|| TypeError::Unsupported {
                what: "范围表达式缺少上界（省略边界 `..` 仅切片 `v[..]` 支持）".to_string(),
                span,
            })?;
            let (_, lo_ty) = infer_expr(ctx, lower)?;
            let (hi_hir, hi_ty) = infer_expr(ctx, upper)?;
            if !lo_ty.compatible_with(&hi_ty) {
                return Err(TypeError::ChainTypeMismatch { span });
            }
            let _ = hi_hir;
            Ok((HirExpr::Unit, lo_ty))
        }

        ExprKind::Binary { op, left, right } => {
            let (mut l_hir, mut l_ty) = infer_expr(ctx, left)?;
            let (mut r_hir, r_ty) = infer_expr(ctx, right)?;
            // V1：裸指针算术 `ptr + n`（迭代器瘦指针推进）——返回同类型裸指针。
            // 元素步长由元素标量种类决定（is_str 场景由 `&s[i]` 取址路径经
            // `Ref{Index}` 特判携带，本路径非字符串）。
            if *op == BinaryOp::Add
                && matches!(&l_ty, Type::RawPtr(_, _))
                && matches!(r_ty, Type::I64)
            {
                let (inner, is_mut) = match &l_ty {
                    Type::RawPtr(inner, is_mut) => (inner.clone(), *is_mut),
                    _ => unreachable!(),
                };
                let elem = field_scalar_of(&inner);
                let ptr_hir = HirExpr::PtrAdd {
                    base: Box::new(l_hir),
                    offset: Box::new(r_hir),
                    elem,
                };
                return Ok((ptr_hir, Type::RawPtr(inner, is_mut)));
            }
            // `a + b`（String + String）→ 拼接（拷贝语义，A3）：
            // `let __s = a.clone(); __s.push_str(b); __s`
            // clone 深拷贝左操作数到全新缓冲，消除共享缓冲别名隐患
            // （拼接结果与左操作数互不影响；push_str/clone 经方法实例化
            // 路径注册函数体）
            let l_str = comparison::is_string_type(ctx, &l_ty)
                || comparison::is_str_view(&l_ty)
                || comparison::is_str_value(&l_ty);
            let r_str = comparison::is_string_type(ctx, &r_ty)
                || comparison::is_str_view(&r_ty)
                || comparison::is_str_value(&r_ty);
            if *op == BinaryOp::Add && l_str && r_str {
                // `str` 值操作数（字符串字面量绑定）升级为 String 对象（编译期
                // 长度展开），使 clone / push_str 按 String 对象解析（槽数匹配）
                if comparison::is_str_value(&l_ty) {
                    let (h, t) = check_string_from(ctx, std::slice::from_ref(left), left.span)?;
                    l_hir = h;
                    l_ty = t;
                }
                if comparison::is_str_value(&r_ty) {
                    let (h, _t) = check_string_from(ctx, std::slice::from_ref(right), right.span)?;
                    r_hir = h;
                }
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
                UnaryOp::Deref => {
                    let inner = match &o_ty {
                        Type::Ref(inner, _) | Type::RawPtr(inner, _) => (**inner).clone(),
                        _ => match heap_wrapper_inner(&o_ty) {
                            Some(t) => t,
                            None => {
                                // Y4b-1（2026-08-28）：自定义 `Deref` trait 解引用——
                                // o_ty 实现了 `deref` 方法时，`*x` 生成 `x.deref()` 调用
                                // （返回 `deref()` 的目标类型）；否则维持内建类型限制报错。
                                if ctx.find_impl_for_method(&o_ty, "deref").is_some() {
                                    let (deref_hir, t) = check_method_call(
                                        ctx, operand, "deref", &[], None, span,
                                    )?;
                                    return Ok((deref_hir, t));
                                }
                                return Err(TypeError::Unsupported {
                                    what: format!(
                                        "解引用 `*` 仅支持引用类型 `&T`、裸指针 `*const T`/`*mut T`、堆装箱 `Box<T>`/`Rc<T>`/`Arc<T>`/`Gc<T>` 或实现 `Deref<T>` trait 的类型，发现 `{o_ty}`"
                                    ),
                                    span,
                                })
                            }
                        },
                    };
                    Ok((
                        HirExpr::Deref {
                            expr: Box::new(heap_ptr_hir(o_hir, &o_ty)),
                            ty: field_scalar_of(&inner),
                        },
                        inner,
                    ))
                }
                UnaryOp::AddrOf | UnaryOp::AddrOfMut => {
                    let is_mut = matches!(op, UnaryOp::AddrOfMut);
                    // U5：`&` / `&mut` 目标放宽为三类——
                    // ① 变量：取变量槽地址；
                    // ② 解引用 `&*p` / `&mut *p`：MIR 折叠直接透传指针（写回原地址）；
                    // ③ 不可变 `&expr`（任意表达式）：求值到临时槽再取址（读语义正确）。
                    // V1 解锁：字段/索引目标（`&obj.field` / `&arr[i]`）——MIR
                    // `AddrOfField`/`PtrAdd`（GEP）取真实槽地址，`&mut` 写回原字段/
                    // 元素生效（替代 U5 时拷贝取址的语义错误路径）。
                    // 保持禁止：`&mut` 对纯表达式目标（临时值不可变借用，Rust 同样
                    // 禁止）；`&&T` 引用再取引用。
                    let ok_target = matches!(*operand.kind, ExprKind::Ident(_))
                        || matches!(
                            *operand.kind,
                            ExprKind::Unary { op: UnaryOp::Deref, .. }
                        )
                        || matches!(
                            *operand.kind,
                            ExprKind::FieldAccess { .. } | ExprKind::Index { .. }
                        )
                        || !is_mut;
                    if !ok_target {
                        return Err(TypeError::Unsupported {
                            what: "`&mut` 仅支持变量、解引用 `&*p`、字段 `&obj.field` 与索引 `&arr[i]` 目标（临时值不可变借用）"
                                .to_string(),
                            span,
                        });
                    }
                    // 引用再取引用（`&&T`）待扩展
                    if matches!(o_ty, Type::Ref(_, _)) {
                        return Err(TypeError::Unsupported {
                            what: "MVP 阶段不支持对引用再取引用（`&&T`）".to_string(),
                            span,
                        });
                    }
                    let m = if is_mut {
                        Mutability::Mutable
                    } else {
                        Mutability::Immutable
                    };
                    let pointee = field_scalar_of(&o_ty);
                    Ok((
                        HirExpr::Ref {
                            expr: Box::new(o_hir),
                            is_mut,
                            pointee,
                        },
                        Type::Ref(Box::new(o_ty), m),
                    ))
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
            // L3 接线：计算被归属对象在区域中的字节大小（slot_count × 8），
            // 供 codegen 生成 `rlyeh_region_alloc` + 值镜像；标量也按 1 槽计，
            // codegen 仅在目标为 Ptr 槽（聚合对象）时接线。
            let size = type_slot_count(ctx, &ty, span)?.saturating_mul(8);
            Ok((
                HirExpr::InRegion {
                    expr: Box::new(hir),
                    region: region.clone(),
                    size,
                },
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
                            check_method_call(ctx, operand, "deref_mut", &[], None, span)?;
                        let (v_hir, v_ty) = infer_expr(ctx, value)?;
                        let inner = match &dm_ty {
                            Type::Ref(inner, _) => (**inner).clone(),
                            _ => dm_ty.clone(),
                        };
                        if !inner.compatible_with(&v_ty) {
                            return Err(TypeError::WrongType {
                                expected: inner.to_string(),
                                found: v_ty.to_string(),
                                span,
                            });
                        }
                        let ty = field_scalar_of(&inner);
                        return Ok((
                            HirExpr::DerefSet {
                                base: Box::new(dm_hir),
                                value: Box::new(v_hir),
                                ty,
                            },
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
                // 解引用赋值：`*p = v` / `*p += v`
                HirExpr::Deref { expr: base, ty } => {
                    if !matches!(op, AssignOp::Assign) {
                        let hir_op = match op {
                            AssignOp::AddAssign => HirBinaryOp::Add,
                            AssignOp::SubAssign => HirBinaryOp::Sub,
                            AssignOp::MulAssign => HirBinaryOp::Mul,
                            AssignOp::DivAssign => HirBinaryOp::Div,
                            AssignOp::Assign => unreachable!(),
                        };
                        return Ok((
                            HirExpr::DerefSet {
                                base: base.clone(),
                                value: Box::new(HirExpr::Binary(
                                    hir_op,
                                    Box::new(HirExpr::Deref {
                                        expr: base,
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
                        HirExpr::DerefSet {
                            base,
                            value: Box::new(v_hir),
                            ty,
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
                HirExpr::Region {
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

        ExprKind::Call {
            callee,
            args,
            type_args,
        } => check_call(ctx, callee, args, type_args, span),
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            trait_hint,
        } => check_method_call(ctx, receiver, method, args, trait_hint.as_deref(), span),
        ExprKind::StructCtor {
            type_name,
            type_args,
            fields,
        } => check_struct_construct(ctx, type_name, type_args, fields, span),
        ExprKind::FieldAccess { expr, field } => {
            let (base_hir, base_ty) = infer_expr(ctx, expr)?;
            check_field_access(ctx, base_hir, base_ty, field, span)
        }
        ExprKind::Index { expr, index } => check_index(ctx, expr, index, span),
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
                    HirExpr::Cast {
                        expr: Box::new(hir),
                        to: dst_ty.to_string(),
                    },
                    dst_ty,
                ))
            } else {
                Ok((hir, dst_ty))
            }
        }

        ExprKind::Await(inner) => infer_expr(ctx, inner),

        ExprKind::Block(block) => {
            let (hir, ty) = check_block(ctx, block)?;
            Ok((HirExpr::Block(Box::new(hir)), ty))
        }
        ExprKind::GcRegion { body } => check_gc_region(ctx, body, span),
        ExprKind::Return(Some(e)) => {
            let (hir, _) = infer_expr(ctx, e)?;
            Ok((HirExpr::Return(Some(Box::new(hir))), Type::Never))
        }
        ExprKind::Return(None) => Ok((HirExpr::Return(None), Type::Never)),
        ExprKind::Question(inner) => check_question(ctx, inner, span),
        ExprKind::Break(Some(e)) => {
            let (hir, _) = infer_expr(ctx, e)?;
            Ok((HirExpr::Break(Some(Box::new(hir))), Type::Never))
        }
        ExprKind::Break(None) => Ok((HirExpr::Break(None), Type::Never)),
        ExprKind::Continue => Ok((HirExpr::Continue, Type::Never)),

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
                            callee: "rlyeh_actor_send".to_string(),
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
        // 内置格式化宏（I2）：`println!` / `print!` / `format!` / `dbg!`
        ExprKind::MacroCall { name, args } => check_macro_call(ctx, name, args, span),
    }
}

/// 模式检查结果：(守卫条件, 绑定语句, 是否为纯绑定(无条件), 绑定变量类型表)
type PatternResult = (Option<HirExpr>, Vec<HirStmt>, bool, Vec<(String, Type)>);

pub(crate) struct FormatSeg {
    text: String,
    is_value: bool,
    /// `{:?}` → true（Q3b：Debug 路径；`{}` → false，Display 路径）
    is_debug: bool,
    /// X4：对齐说明符（`<` 左 / `>` 右 / `^` 居中；无 = `\0`）
    align: char,
    /// X4：宽度（`{:>10}` → 10；无 = 0）
    width: i64,
    /// X4：填充字符字节（`{:*>10}` → `*` = 42；无 = 空格 32）
    fill: u8,
}

/// 解析格式串占位符：`{}`（Display）、`{:?}`（Debug）、`{{`/`}}` 转义。

/// `String::from(字面量)` 调用 AST。

/// 简单标识符调用 AST（`int_to_string(x)` / `json_escape(s)` 等）。

/// 路径调用 AST（`String::from(x)` / `json.stringify(x)`）。

/// `a + b` 拼接 AST。

/// 折叠拼接：`p0 + p1 + ...`。

/// L2 `json.stringify(v)` → JSON 文本（编译器内建，AST 层 desugar，零新增 IR 节点）。
///
/// 支持类型：`i64` / `bool` / `String` / `&str` / 数组 `[T; N]` / `Vec<T>` / 结构体（嵌套递归）；
/// `HashMap<K, V>`（键限 `i64` / `String`，值递归；输出 `{"k":v,...}`，遍历顺序 = 哈希槽序）；
/// `f64` 报 Unsupported（规划）。

/// 递归 JSON 序列化 AST 构建。

/// L2 `json.parse::<T>(s)` → T（编译器内建，AST 层 desugar）。
///
/// MVP 支持：`i64` / `bool` / `String`（引号剥离 + 转义还原）；数组 / Vec / 结构体反序列化规划。

/// 递归 JSON 反序列化 AST 构建（MVP：标量 + String）。

// ===========================================================================
// Q4 `toml` 模块（轻量 MVP）：基础标量 / 嵌套表（内联表）/ 数组 stringify/parse
// ===========================================================================

/// Q4 `toml.to_string(v)` / `toml.stringify(v)` → TOML 文本（编译器内建，AST 层 desugar，
/// 零新增 IR 节点）。MVP 无泛型 trait 约束（`T: Serialize` bound 不支持），签名退化为
/// 无 bound 形式：类型由实参推断。
///
/// 支持类型：`i64` / `bool` / `String` / `&str` / 数组 `[T; N]` / `Vec<T>` / 结构体（嵌套
/// 递归）；`HashMap<K, V>`（键限 `i64` / `String`，值递归）。
///
/// 输出格式（紧凑、无多余空白，与 `toml::from_str` 的 round-trip 对齐）：
///   - 顶层结构体 → 多行：`f1 = v1\nf2 = v2`（每字段一行 `key = value`）
///   - 嵌套结构体字段 → 内联表：`{x = 1, y = 2}`（MVP 用内联表；`[section]` 行式子表规划中）
///   - 数组 / Vec → `[e1, e2]`；HashMap → `{"k" = v, "k2" = v2}`（键带引号，TOML 合法）
///   - 标量：i64 → `int_to_string`；bool → `true` / `false`；String → `"` + json_escape + `"`
///     （TOML 基本转义与 JSON 一致，复用 core.rl `json_escape` / `json_unescape`）

/// 递归 TOML 序列化 AST 构建。`top_level`：顶层结构体输出多行 `key = value`（标准 TOML
/// 顶层键值对），嵌套字段输出内联表 `{ ... }`。

/// Q4 `toml.from_str::<T>(s)` / `toml.parse::<T>(s)` → T（编译器内建，AST 层 desugar）。
///
/// MVP 支持（round-trip 对齐 `toml::to_string` 的紧凑输出，无多余空白）：
/// `i64` / `bool` / `String`（引号剥离 + 转义还原）；`Vec<T>`（数组 `[e1,e2]`，元素限
/// 标量）；`HashMap<K, V>`（内联表 `{"k" = v, ...}`，键/值限标量）；结构体（顶层多行
/// `key = value` / 嵌套内联表 `{ ... }`）。数组类型 `[T; N]` 与 f64 报 Unsupported。

/// 递归 TOML 反序列化 AST 构建。`inline`：结构体目标的文本形态——`false` 顶层多行
/// （`key = value` 行，`\n` 分隔，不剥括号）；`true` 内联表（`{ ... }`，剥首尾 `{ }`，
/// `,` 分隔）。标量 / 数组 / HashMap 分支忽略 `inline`（数组自身剥 `[ ]`，HashMap 恒为
/// 内联表）。

/// Q2b `json.to_writer(w, v)` → `w.write_all(json.stringify(v))`（编译器内建）。
///
/// 参数：`w` 为 `File` / `&File` / `&mut File`（写句柄，方法调用自动剥引用层）；
/// `v` 递归 `check_json_stringify` 序列化。返回 `Result<i64, io::error::IoError>`
/// （`File::write_all` 的返回类型，调用方可 match / `?` 处理）。

/// Q2b `json.from_reader::<T>(r)` → `json.parse::<T>(r.read_to_string().unwrap())`
/// （编译器内建）。
///
/// 参数：`r` 为 `File` / `&mut File`（读句柄）。读取失败经 `Result::unwrap` 死循环
/// （MVP 语义，与 std `Result::unwrap` 一致）；返回 `T`（须经 turbofish 指定）。

/// 字段类型 → 零值 AST（struct 反序列化零值构造用；缺失字段保持零值）。

/// 值 → String 的 AST（`{}` 显示 / `{:?}` Debug；MVP 内建类型走内建转换，
/// 自定义类型经 `Display::fmt` / `Debug::fmt_debug` 方法调用）。

/// 按预推断类型构造值 → String 的 AST（`dbg!` 等场景参数尚未绑定为变量时复用）。

/// `println!` / `print!` / `format!`：`{}` 占位符格式化 → String 拼接 + 打印内建。

/// `dbg!(expr)`：打印 `dbg: <值>` 并返回原值（MVP 无源码文本标签）。

/// 用替换表替换类型中的泛型占位。

/// 剥离引用层（`&T` → `T`）。

/// 堆指针包装 `Box<T>` / `Rc<T>` / `Arc<T>` / `Gc<T>` → 内层类型 `T`。

/// 连续剥离引用与堆指针包装（`&Box<T>` / `&Rc<T>` → `T`）。

/// 堆包装对象表达式 → 堆内 `T` 对象区首槽地址。
///
/// - `Box<T>`（K2）/ `Rc<T>` / `Arc<T>`（K3）/ `Gc<T>`（K4）：栈上 1 槽存堆指针
///   （Box 指向值区首槽；Rc / Gc 指向内层对象，值区自堆首槽起、元数据在尾部），
///   解引用槽 0 即堆首槽地址；
/// - `&T` / 普通聚合对象：求值即对象指针，原样返回。
///
/// 字段访问 / 方法调用 / 索引 / 解引用的 base 统一经此改写为堆对象指针。

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


mod iter;
mod binary;
mod call;
mod resolve;
mod construct;
mod field;
mod heap;
mod index_enum;
mod method;
mod generic;
mod util;
mod macro_ser;
mod json;
mod toml;
mod closure;
mod iterator;
mod json_ser;
mod toml_ser;

use iter::*;
use binary::*;
use call::*;
use resolve::*;
use construct::*;
use field::*;
use heap::*;
use index_enum::*;
use method::*;
use generic::*;
use util::*;

pub(crate) use closure::check_deferred_closure_binding;
pub(crate) use closure::check_closure_value_binding;
pub(crate) use closure::try_closure_value_as_fn;
pub(crate) use closure::fix_deferred_closure_with_sig;
pub(crate) use closure::check_closure_expected;
pub(crate) use resolve::resolve_ast_type;
pub(crate) use construct::check_string_from;
// block/misc 对外 API
pub(crate) use block::{check_block, check_block_inner};
pub(crate) use misc::{builtin_signature, coerce_to_dyn, type_mentions_self};
// 宏/序列化辅助被兄弟子模块调用，显式 re-export 供 `use super::*` 可见
pub(crate) use macro_ser::{check_macro_call, parse_format_string, string_from_lit_ast,
    mk_ident_call, mk_path_call, bin_add, fold_add};
pub(crate) use json::{
    check_json_parse, check_json_try_parse, check_json_to_writer, check_json_from_reader,
};
pub(crate) use toml::{check_toml_parse, check_toml_try_parse};
pub(crate) use json_ser::check_json_stringify;
pub(crate) use toml_ser::check_toml_stringify;
// 闭包辅助被 call.rs 等兄弟子模块调用
pub(crate) use closure::{check_capture_closure_iife, check_closure_value_call};
// 迭代器辅助被 iter/method/json/toml 等兄弟子模块调用
pub(crate) use iterator::{check_for_iterator, try_check_adapter, ty_to_ast};


mod misc;
mod block;
