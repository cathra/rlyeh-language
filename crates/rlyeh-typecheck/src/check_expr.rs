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
pub fn builtin_signature(name: &str) -> Option<(Vec<Type>, Type)> {
    let dyn_arr = || Type::Array(Box::new(Type::Infer), 0);
    match name {
        "print" | "println" | "eprint" | "eprintln" => Some((vec![Type::Infer], Type::Unit)),
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
pub(crate) fn coerce_to_dyn(
    ctx: &mut TypeContext,
    data_ptr: HirExpr,
    concrete: &Type,
    trait_name: &str,
    span: Span,
) -> Result<HirExpr, TypeError> {
    let trait_def = ctx.trait_defs.get(trait_name).cloned().ok_or_else(|| {
        TypeError::UndefinedType {
            name: trait_name.to_string(),
            span,
        }
    })?;
    if !trait_def.type_params.is_empty() {
        return Err(TypeError::Unsupported {
            what: format!("`dyn {trait_name}`：泛型 trait 实例化（trait 对象泛型参数规划中）"),
            span,
        });
    }
    // 找 `impl Trait for 具体类型`（MVP：trait/impl 均非泛型，直接按 self_type 精确匹配）
    let impl_def = ctx
        .impl_defs
        .iter()
        .find(|d| d.trait_name.as_deref() == Some(trait_name) && d.self_type == *concrete)
        .cloned()
        .ok_or_else(|| TypeError::Unsupported {
            what: format!(
                "类型 `{concrete}` 未实现 trait `{trait_name}`，无法转换为 `dyn {trait_name}`"
            ),
            span,
        })?;
    if !impl_def.type_params.is_empty() {
        return Err(TypeError::Unsupported {
            what: format!("`dyn {trait_name}`：泛型 impl（`impl<T> {trait_name} for ...`）规划中"),
            span,
        });
    }
    let n = trait_def.methods.len();
    let mut stmts = Vec::new();
    // 1) vtable 数组：3 元槽（drop/size/align，MVP = 0）+ N 方法槽
    let vt = ctx.fresh_temp();
    stmts.push(HirStmt::Let {
        name: vt.clone(),
        init: HirExpr::Alloc {
            slots: 3 + n,
            by_value: false,
        },
        mutable: false,
    });
    for i in 0..3 {
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(vt.clone())),
            index: i,
            value: Box::new(HirExpr::IntLiteral(0)),
            ty: FieldScalar::Int,
        }));
    }
    // 2) 方法表：按 trait 方法声明顺序填充具体 impl 方法函数指针
    let subst = HashMap::new();
    for (i, m) in trait_def.methods.iter().enumerate() {
        let impl_method = impl_def.methods.iter().find(|im| im.sig.name == m.name).ok_or_else(|| {
            TypeError::Unsupported {
                what: format!(
                    "`{trait_name}` 的 impl for `{concrete}` 缺少方法 `{}`",
                    m.name
                ),
                span,
            }
        })?;
        let fn_name = instantiate_impl_method(ctx, &impl_def, impl_method, &subst, span)?;
        let m_var = ctx.fresh_temp();
        stmts.push(HirStmt::Let {
            name: m_var.clone(),
            init: HirExpr::FnPtr(fn_name),
            mutable: false,
        });
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(vt.clone())),
            index: 3 + i,
            value: Box::new(HirExpr::Variable(m_var)),
            ty: FieldScalar::Ptr,
        }));
    }
    // 3) 胖指针：槽 0 = 数据指针，槽 1 = vtable 指针
    let dyn_var = ctx.fresh_temp();
    stmts.push(HirStmt::Let {
        name: dyn_var.clone(),
        init: HirExpr::Alloc {
            slots: 2,
            by_value: false,
        },
        mutable: false,
    });
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(dyn_var.clone())),
        index: 0,
        value: Box::new(data_ptr),
        ty: FieldScalar::Ptr,
    }));
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(dyn_var.clone())),
        index: 1,
        value: Box::new(HirExpr::Variable(vt)),
        ty: FieldScalar::Ptr,
    }));
    Ok(HirExpr::Block(Box::new(HirBlock {
        stmts,
        final_expr: Some(HirExpr::Variable(dyn_var)),
    })))
}

/// H4 辅助：类型是否引用了 `Self`。trait 方法签名含关联类型（`Self`）时，
/// trait 对象调用无法确定具体类型，MVP 报 Unsupported。
fn type_mentions_self(ty: &Type) -> bool {
    match ty {
        // `Self::Item` 关联类型占位（trait 声明收集时无 impl 上下文，
        // 退化为 `Generic("Self::Item")`）同样视为 `Self` 提及。
        Type::Generic(n) => n == "Self" || n.starts_with("Self::"),
        Type::Ref(t, _) | Type::RawPtr(t, _) | Type::Array(t, _) => type_mentions_self(t),
        Type::Named(_, ps) | Type::Tuple(ps) => ps.iter().any(type_mentions_self),
        _ => false,
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
                                return Err(TypeError::Unsupported {
                                    what: format!(
                                        "解引用 `*` 仅支持引用类型 `&T`、裸指针 `*const T`/`*mut T` 或堆装箱 `Box<T>`/`Rc<T>`/`Arc<T>`/`Gc<T>`，发现 `{o_ty}`"
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
            let (t_hir, t_ty) = infer_expr(ctx, target)?;
            // H4 去虚拟化失效：dyn 变量被重新赋值后绑定源具体类型不再成立，
            // 后续调用回退 vtable 间接分派
            if matches!(op, AssignOp::Assign) {
                if let ExprKind::Ident(var) = &*target.kind {
                    ctx.remove_dyn_concrete(var);
                }
            }
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
        } => check_method_call(ctx, receiver, method, args, span),
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

/// 检查代码块并生成 HIR 块。
pub(crate) fn check_block(
    ctx: &mut TypeContext,
    block: &AstBlock,
) -> Result<(HirBlock, Type), TypeError> {
    check_block_with_expected_final(ctx, block, None)
}

/// 带预期 final 表达式类型的块检查。
///
/// `expected_final` 为 `Some(exp)` 且 final 表达式为闭包、`exp` 为 `Type::Fn(_)`
/// 时，按 H2 无捕获闭包签名检查（而非 `infer_expr` 报缺 fn 上下文），用于
/// `fn make() -> fn(i64) -> i64 { |x| x + 1 }` 返回闭包的函数。
pub(crate) fn check_block_with_expected_final(
    ctx: &mut TypeContext,
    block: &AstBlock,
    expected_final: Option<&Type>,
) -> Result<(HirBlock, Type), TypeError> {
    // U1：块级作用域（查找穿透外层——块内可见外层变量；块内 let 随弹出消失，
    // 与块外同名变量遮蔽时 mangle 槽名，互不干扰）。
    ctx.push_scope(false);
    let result = check_block_inner(ctx, block, expected_final);
    ctx.pop_scope();
    result
}

pub(crate) fn check_block_inner(
    ctx: &mut TypeContext,
    block: &AstBlock,
    expected_final: Option<&Type>,
) -> Result<(HirBlock, Type), TypeError> {
    let mut stmts = Vec::with_capacity(block.stmts.len());
    for stmt in &block.stmts {
        let (hir_stmt, _) = crate::check_stmt::check_stmt(ctx, stmt)?;
        stmts.push(hir_stmt);
    }
    let mut final_ty = Type::Unit;
    let mut final_expr = None;
    if let Some(e) = &block.final_expr {
        let (hir, ty) = if let Some(exp) = expected_final {
            if matches!(exp, Type::Fn(_)) && matches!(&*e.kind, ExprKind::Closure { .. }) {
                check_closure_expected(ctx, e, exp, e.span)?
            } else {
                infer_expr(ctx, e)?
            }
        } else {
            infer_expr(ctx, e)?
        };
        final_ty = ty;
        final_expr = Some(hir);
    }
    Ok((HirBlock { stmts, final_expr }, final_ty))
}

/// `gc_region { ... }`（K4 追踪 GC 生命周期作用域）。
///
/// desugar 为普通 HIR 块（零新增 IR 节点）：
/// ```text
/// {
///   rlyeh_gc_region_begin();              // 快照外层活跃对象（root 栈压栈）
///   <body 语句>...;
///   [let __gc_escape = <final_expr>;     // 块返回值
///    rlyeh_gc_escape(*__gc_escape);]      // 逃逸 Gc 对象登记为 root（解包装取对象 base）
///   rlyeh_gc_collect();                   // 标记-清除（未标记对象回收）
///   [__gc_escape]
/// }
/// ```
///
/// `Gc<T>` 变量 = 栈槽 → 堆 1-槽包装（`Alloc{slots:1}`，槽 0 存 `GcInner` base）。
/// 运行时 `rlyeh_gc_alloc` 以对象 base 注册块表，`mark` 按 `header.base == root.base`
/// 线性查找，故 escape 必须传 **对象 base**（`Deref` 剥包装层），而非包装指针，
/// 否则标记失配、对象被误回收（悬垂读取）。
/// 逃逸：仅 final_expr 类型为 `Gc<T>`（剥引用层后）时登记逃逸对象（存活）；
/// 标量 / 单元 / 非 Gc 聚合返回值直接经收集（无 GC 引用，安全）。
/// MVP 限制（memory-model.md §5）：聚合返回值内含 `Gc` 字段的逃逸不受保护；
/// `gc_region` 结束后块内 Gc 对象失效（逃逸限制，无悬空指针防护）。
fn check_gc_region(
    ctx: &mut TypeContext,
    body: &AstBlock,
    _span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let (mut hir, ty) = check_block(ctx, body)?;
    let mut stmts = std::mem::take(&mut hir.stmts);
    // 块入口：快照外层活跃对象
    stmts.insert(
        0,
        HirStmt::Semi(HirExpr::Call {
            callee: "rlyeh_gc_region_begin".to_string(),
            args: vec![],
        }),
    );
    if let Some(f) = hir.final_expr.take() {
        // 逃逸值保护：仅直接 `Gc<T>` 类型登记（标量 / () / 非 Gc 聚合直接收集）
        if matches!(peel_ref(&ty), Type::Named(n, _) if n == "Gc") {
            let esc = ctx.fresh_temp();
            stmts.push(HirStmt::Let {
                name: esc.clone(),
                init: f,
                mutable: false,
            });
            stmts.push(HirStmt::Semi(HirExpr::Call {
                callee: "rlyeh_gc_escape".to_string(),
                // Gc<T> 变量 = 栈槽 → 堆 1-槽包装（槽 0 存 GcInner base）；
                // heap_ptr_hir 解包装槽 0 得对象 base（与 rlyeh_gc_alloc 注册一致），
                // 传包装指针会导致 mark 线性查找失配、对象被误回收（悬垂读取）。
                args: vec![heap_ptr_hir(HirExpr::Variable(esc.clone()), &ty)],
            }));
            stmts.push(HirStmt::Semi(HirExpr::Call {
                callee: "rlyeh_gc_collect".to_string(),
                args: vec![],
            }));
            return Ok((
                HirExpr::Block(Box::new(HirBlock {
                    stmts,
                    final_expr: Some(HirExpr::Variable(esc)),
                })),
                ty,
            ));
        }
        stmts.push(HirStmt::Semi(HirExpr::Call {
            callee: "rlyeh_gc_collect".to_string(),
            args: vec![],
        }));
        Ok((
            HirExpr::Block(Box::new(HirBlock {
                stmts,
                final_expr: Some(f),
            })),
            ty,
        ))
    } else {
        stmts.push(HirStmt::Semi(HirExpr::Call {
            callee: "rlyeh_gc_collect".to_string(),
            args: vec![],
        }));
        Ok((
            HirExpr::Block(Box::new(HirBlock {
                stmts,
                final_expr: None,
            })),
            Type::Unit,
        ))
    }
}

/// 检查 for 循环：迭代器分派。
///
/// - range 表达式（`lo..<hi` 等）→ [`check_for_range`]（数值递增循环）
/// - `Vec<T>` 容器（`for x in v`）→ [`check_for_vec`]（索引遍历循环）
/// - `[T; N]` 数组（`for x in arr`）→ [`check_for_array`]（索引遍历循环，J1）
/// - `HashMap<K, V>` 容器（`for (k, v) in m`）→ [`check_for_hashmap`]（索引遍历循环）
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
    if let Type::Array(elem, n) = peel_ref(&iter_ty) {
        return check_for_array(ctx, pattern, iter_hir, elem.as_ref(), n, body, span);
    }
    // 自定义迭代器（J2）：接收者类型存在 `next() -> Option<Item>` 方法
    // （inherent 或 trait impl）→ 经 check_for_iterator 接入 for 循环。
    let concrete = peel_ref(&iter_ty);
    if let Some(imp) = ctx.find_impl_for_method(&concrete, "next").cloned() {
        if let Some(md) = imp.methods.iter().find(|m| m.sig.name == "next") {
            let mut subst: HashMap<String, Type> = HashMap::new();
            if unify(&imp.self_type, &concrete, &mut subst).is_ok() {
                let ret = substitute(&md.sig.return_type, &subst);
                let named = match peel_ref(&ret) {
                    Type::Named(n, _) => ctx.resolve_full_name(n.as_str()).unwrap_or_else(|| n.clone()),
                    _ => String::new(),
                };
                if named == "Option" {
                    return check_for_iterator(ctx, pattern, iterator, body, span);
                }
            }
        }
    }
    Err(TypeError::Unsupported {
        what: "非 range / Vec / HashMap / 数组 / 迭代器（含 next() 方法的类型）的 for 循环（集合 / 容器迭代 MVP 阶段仅支持 Vec、HashMap、数组与自定义迭代器）"
            .to_string(),
        span,
    })
}

/// 检查 range 迭代的 for 循环：`for pat in lo..<hi { body }`。
///
/// MVP 阶段支持 range 迭代器（`..<` / `...` / `<..` / `<..<`），
/// 在类型检查层 desugar 为 `loop`：
///
/// ```rlyeh
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

    // 5. 进入循环作用域（迭代变量 / 临时变量在作用域内；结束弹出）。
    //    存储槽名：循环变量被外层同名遮蔽时 mangle，Let/引用/Assign 全部用槽名。
    ctx.push_scope(false);
    let stored_lo = ctx.insert_variable(lo_name.clone(), lo_ty.clone());
    let stored_hi = ctx.insert_variable(hi_name.clone(), hi_ty.clone());
    let stored_name = ctx.insert_variable(name.clone(), lo_ty.clone());

    // 6. 起始值：下界开区间时从 lo + 1 开始；
    //    循环变量初始化为 `start - 1`，配合循环体开头的 `pat += 1`，
    //    使第一次迭代 pat == start。
    let start = if lower_inclusive {
        HirExpr::Variable(stored_lo.clone())
    } else {
        HirExpr::Binary(
            HirBinaryOp::Add,
            Box::new(HirExpr::Variable(stored_lo.clone())),
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
            name: stored_lo.clone(),
            init: lo_hir,
            mutable: false,
        },
        HirStmt::Let {
            name: stored_hi.clone(),
            init: hi_hir,
            mutable: false,
        },
        HirStmt::Let {
            name: stored_name.clone(),
            init,
            mutable: true,
        },
    ];

    // 7. 循环体检查
    let (b_hir, _) = check_block(ctx, body)?;
    ctx.pop_scope();

    // 8. 退出条件：`pat >= hi`（上界闭区间为 `pat > hi`）。
    //    注意用 `loop` 而非 `while`：循环体开头的 `pat += 1` 在每次
    //    迭代（含 continue 回跳）时都会执行，保证 continue 不会跳过递增。
    let exit_op = if upper_inclusive {
        HirBinaryOp::Gt
    } else {
        HirBinaryOp::Ge
    };
    let exit_cond = HirExpr::Binary(
        exit_op,
        Box::new(HirExpr::Variable(stored_name.clone())),
        Box::new(HirExpr::Variable(stored_hi)),
    );

    // 9. loop 体：`pat += 1` → 退出判断 → 原 body 语句
    let mut loop_body_stmts = vec![
        HirStmt::Expr(HirExpr::Assign {
            target: stored_name.clone(),
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
/// ```rlyeh
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

    // 4. 进入循环作用域 + 绑定临时变量（结束弹出）。
    //    存储槽名：循环变量被外层同名遮蔽时 mangle，Let/引用/Assign 全部用槽名。
    ctx.push_scope(false);
    let vec_ty = Type::Named("Vec".to_string(), vec![elem_ty.clone()]);
    let stored_v = ctx.insert_variable(v_name.clone(), vec_ty);
    let stored_len = ctx.insert_variable(len_name.clone(), Type::I64);
    let stored_i = ctx.insert_variable(i_name.clone(), Type::I64);
    let stored_name = ctx.insert_variable(name.clone(), elem_ty.clone());

    // 5. 前缀语句：绑定容器、缓存长度、初始化计数器
    let mut stmts = vec![
        HirStmt::Let {
            name: stored_v.clone(),
            init: iter_hir,
            mutable: false,
        },
        HirStmt::Let {
            name: stored_len.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(stored_v.clone())),
                index: 1, // Vec 槽 1 = len
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: stored_i.clone(),
            init: HirExpr::IntLiteral(0),
            mutable: true,
        },
    ];

    // 7. 循环体检查
    let (b_hir, _) = check_block(ctx, body)?;
    ctx.pop_scope();

    // 8. loop 体：边界检查 → 取元素绑定 → 递增 → 原 body 语句
    let elem_scalar = field_scalar_of(&elem_ty);
    let mut loop_body_stmts = vec![
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Ge,
                Box::new(HirExpr::Variable(stored_i.clone())),
                Box::new(HirExpr::Variable(stored_len.clone())),
            )),
            then_block: Box::new(HirBlock {
                stmts: vec![HirStmt::Expr(HirExpr::Break(None))],
                final_expr: None,
            }),
            else_block: None,
        }),
        HirStmt::Let {
            name: stored_name.clone(),
            init: HirExpr::Index {
                base: Box::new(HirExpr::FieldGet {
                    base: Box::new(HirExpr::Variable(stored_v.clone())),
                    index: 0, // Vec 槽 0 = data 指针
                    ty: FieldScalar::Ptr,
                }),
                index: Box::new(HirExpr::Variable(stored_i.clone())),
                elem: elem_scalar,
                is_str: false,
            },
            mutable: false,
        },
        HirStmt::Expr(HirExpr::Assign {
            target: stored_i.clone(),
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

/// 检查 `[T; N]` 数组迭代的 for 循环：`for pat in arr { body }`。
///
/// 在类型检查层 desugar 为索引遍历循环（数组以指针存储、长度编译期已知，
/// 复用数组索引 `arr[i]` 的 HirExpr::Index 语义，步长 8 / u8 按字节）：
///
/// ```rlyeh
/// let __for_arr = <数组>;         // 绑定数组（防迭代器重复求值）
/// let mut __for_i = 0;
/// loop {
///     if __for_i >= N { break; }  // N 为编译期长度
///     let pat = __for_arr[__for_i]; // 数组索引读取
///     __for_i += 1;               // continue 回跳前已递增，不会死循环
///     body
/// }
/// ```
fn check_for_array(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iter_hir: HirExpr,
    elem_ty: &Type,
    n: usize,
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 1. 元素类型经泛型替换；Infer 无法确定槽标量
    let elem_ty = substitute(elem_ty, &ctx.generic_subst);
    if matches!(elem_ty, Type::Infer) {
        return Err(TypeError::Unsupported {
            what: "`for ... in` 的数组迭代要求元素类型确定（如 `let arr: [i64; N] = ...`）"
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
    let arr_name = format!("__for_arr_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let i_name = format!("__for_i_{}", ctx.temp_counter);
    ctx.temp_counter += 1;

    // 4. 进入循环作用域 + 绑定临时变量（结束弹出）。
    //    存储槽名：循环变量被外层同名遮蔽时 mangle，Let/引用/Assign 全部用槽名。
    ctx.push_scope(false);
    let arr_ty = Type::Array(Box::new(elem_ty.clone()), n);
    let stored_arr = ctx.insert_variable(arr_name.clone(), arr_ty);
    let stored_i = ctx.insert_variable(i_name.clone(), Type::I64);
    let stored_name = ctx.insert_variable(name.clone(), elem_ty.clone());

    // 5. 前缀语句：绑定数组、初始化计数器
    let mut stmts = vec![
        HirStmt::Let {
            name: stored_arr.clone(),
            init: iter_hir,
            mutable: false,
        },
        HirStmt::Let {
            name: stored_i.clone(),
            init: HirExpr::IntLiteral(0),
            mutable: true,
        },
    ];

    // 6. 循环体检查
    let (b_hir, _) = check_block(ctx, body)?;
    ctx.pop_scope();

    // 7. loop 体：边界检查 → 取元素绑定 → 递增 → 原 body 语句
    let elem_scalar = field_scalar_of(&elem_ty);
    let is_byte = matches!(elem_ty, Type::U8);
    let mut loop_body_stmts = vec![
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Ge,
                Box::new(HirExpr::Variable(stored_i.clone())),
                Box::new(HirExpr::IntLiteral(n as i128)),
            )),
            then_block: Box::new(HirBlock {
                stmts: vec![HirStmt::Expr(HirExpr::Break(None))],
                final_expr: None,
            }),
            else_block: None,
        }),
        HirStmt::Let {
            name: stored_name.clone(),
            init: HirExpr::Index {
                base: Box::new(HirExpr::Variable(stored_arr.clone())),
                index: Box::new(HirExpr::Variable(stored_i.clone())),
                elem: elem_scalar,
                is_str: is_byte,
            },
            mutable: false,
        },
        HirStmt::Expr(HirExpr::Assign {
            target: stored_i.clone(),
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

/// 检查自定义迭代器接入 for 的循环（J2）：`for pat in it { body }`。
///
/// 当 `it` 类型存在 `next() -> Option<Item>` 方法（inherent 或 trait impl，
/// 方法调用链复用 check_method_call）时，构造 AST 循环：
///
/// ```rlyeh
/// let mut __for_it = it;                 // 迭代器按值绑定（next(&mut self)）
/// loop {
///     match __for_it.next() {
///         Some(__elem) => { let pat = __elem; body }
///         None => break,
///     }
/// }
/// ```
///
/// 说明：`it` 仅经最终 AST 求值一次（`let __for_it = it;` 绑定中）；循环体外
/// 的初次 infer 仅为分派检测，其结果被丢弃（typecheck 无副作用，不重复执行）。
fn check_for_iterator(
    ctx: &mut TypeContext,
    pattern: &AstPattern,
    iterator: &AstExpr,
    body: &AstBlock,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let it_name = format!("__for_it_{}", ctx.temp_counter);
    ctx.temp_counter += 1;
    let elem_tmp = format!("__for_elem_{}", ctx.temp_counter);
    ctx.temp_counter += 1;

    // 绑定迭代器：`let mut __for_it = iterator;`（`next(&mut self)` 需可变）
    let bind = AstStmt::Let {
        pattern: AstPattern::Ident(it_name.clone()),
        type_anno: None,
        init: iterator.clone(),
        mutable: true,
    };

    // 循环体：`match __for_it.next() { Some(__elem) => .., None => break }`
    let next_call = AstExpr::new(
        ExprKind::MethodCall {
            receiver: AstExpr::new(ExprKind::Ident(it_name.clone()), span),
            method: "next".to_string(),
            args: vec![],
        },
        span,
    );
    // Some 臂：`{ let pat = __elem; body }`
    let mut arm_stmts = vec![AstStmt::Let {
        pattern: pattern.clone(),
        type_anno: None,
        init: AstExpr::new(ExprKind::Ident(elem_tmp.clone()), span),
        mutable: false,
    }];
    arm_stmts.extend(body.stmts.clone());
    let some_body = AstExpr::new(
        ExprKind::Block(AstBlock {
            stmts: arm_stmts,
            final_expr: body.final_expr.clone(),
            span,
        }),
        span,
    );
    // None 臂：`break`
    let none_body = AstExpr::new(ExprKind::Break(None), span);
    let match_expr = AstExpr::new(
        ExprKind::Match {
            expr: next_call,
            arms: vec![
                rlyeh_ast::MatchArm {
                    pattern: AstPattern::Enum("Some".to_string(), vec![AstPattern::Ident(elem_tmp)]),
                    guard: None,
                    body: some_body,
                    span,
                },
                rlyeh_ast::MatchArm {
                    pattern: AstPattern::Enum("None".to_string(), vec![]),
                    guard: None,
                    body: none_body,
                    span,
                },
            ],
        },
        span,
    );
    let loop_expr = AstExpr::new(
        ExprKind::Loop {
            body: AstBlock {
                stmts: vec![AstStmt::Semi(match_expr)],
                final_expr: None,
                span,
            },
        },
        span,
    );

    let block_ast = AstExpr::new(
        ExprKind::Block(AstBlock {
            stmts: vec![bind, AstStmt::Semi(loop_expr)],
            final_expr: None,
            span,
        }),
        span,
    );
    let (hir, _) = infer_expr(ctx, &block_ast)?;
    Ok((hir, Type::Unit))
}

/// 仅推断无捕获闭包的返回类型（J3 适配器预推断用）：参数类型由调用方给出，
/// 闭包体以这些参数类型检查，返回闭包体实际类型（不注入匿名函数项）。
///
/// 与 [`check_closure_expected`] 共享"无捕获"语义：保存并清空外层变量环境，
/// body 引用外部变量报捕获闭包 Unsupported（H3 规划）。
fn closure_return_ty(
    ctx: &mut TypeContext,
    closure: &AstExpr,
    param_tys: &[Type],
    span: Span,
) -> Result<Type, TypeError> {
    let ExprKind::Closure { params, param_types: _, body, capture: _ } = &*closure.kind else {
        return Err(TypeError::Unsupported {
            what: "适配器期望闭包参数".to_string(),
            span,
        });
    };
    if params.len() != param_tys.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "<闭包>".to_string(),
            expected: param_tys.len(),
            found: params.len(),
            span,
        });
    }
    let mut names = Vec::with_capacity(params.len());
    for (i, p) in params.iter().enumerate() {
        match p {
            AstPattern::Ident(n) => names.push(n.clone()),
            AstPattern::Wildcard => names.push(format!("__arg{i}")),
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（仅支持简单标识符）"),
                    span,
                })
            }
        }
    }
    ctx.push_scope(true);
    for (nm, ty) in names.iter().zip(param_tys.iter().cloned()) {
        ctx.insert_variable(nm.clone(), ty);
    }
    let body_result = infer_expr(ctx, body);
    ctx.pop_scope();
    let (_, body_ty) = match body_result {
        Ok(v) => v,
        Err(TypeError::UndefinedVariable { name, span }) => {
            return Err(TypeError::Unsupported {
                what: format!(
                    "闭包捕获外部变量 `{name}`（捕获闭包 H3 规划；H2 无捕获闭包仅可用参数与字面量）"
                ),
                span,
            })
        }
        Err(e) => return Err(e),
    };
    Ok(body_ty)
}

/// 将类型转为 AST 类型注解（J3 适配器构造 `Vec<U>` 注解用）。
fn ty_to_ast(ty: &Type) -> AstType {
    use Type::*;
    match ty {
        I64 => AstType::Path("i64".into(), vec![]),
        U8 => AstType::Path("u8".into(), vec![]),
        F64 => AstType::Path("f64".into(), vec![]),
        Bool => AstType::Path("bool".into(), vec![]),
        Unit => AstType::Path("()".into(), vec![]),
        Named(n, args) => AstType::Path(n.clone(), args.iter().map(ty_to_ast).collect()),
        _ => AstType::Path("()".into(), vec![]),
    }
}

/// J3 迭代器适配器入口：`map` / `filter` / `fold` / `collect` / `take` / `skip`。
///
/// 仅当接收者类型为数组 `[T; N]` 或自定义迭代器（存在 `next() -> Option<T>`
/// 方法，inherent / trait impl）时启用（适配器特判优先于通用方法解析）；
/// 其余情况返回 `Ok(None)` 走通用方法调用路径。
fn try_check_adapter(
    ctx: &mut TypeContext,
    receiver: &AstExpr,
    self_ty: &Type,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<Option<(HirExpr, Type)>, TypeError> {
    if !matches!(method, "map" | "filter" | "fold" | "collect" | "take" | "skip") {
        return Ok(None);
    }
    // 元素类型 T：数组直接取；`Vec<T>` 容器取类型参数；否则检测
    // `next() -> Option<T>` 方法的返回项（自定义迭代器）
    let elem_ty = match self_ty {
        Type::Array(elem, _) => substitute(elem, &ctx.generic_subst),
        Type::Named(n, targs) => {
            let full = ctx.resolve_full_name(n.as_str()).unwrap_or_else(|| n.clone());
            if full == "Vec" && ctx.lookup_struct(&full).is_some() {
                let Some(t) = targs.first().cloned() else {
                    return Ok(None);
                };
                substitute(&t, &ctx.generic_subst)
            } else {
                let Some(imp) = ctx.find_impl_for_method(self_ty, "next").cloned() else {
                    return Ok(None);
                };
                let Some(md) = imp.methods.iter().find(|m| m.sig.name == "next") else {
                    return Ok(None);
                };
                let mut subst: HashMap<String, Type> = HashMap::new();
                if unify(&imp.self_type, self_ty, &mut subst).is_err() {
                    return Ok(None);
                }
                let ret = substitute(&md.sig.return_type, &subst);
                let Type::Named(nn, rtargs) = peel_ref(&ret) else {
                    return Ok(None);
                };
                if ctx
                    .resolve_full_name(nn.as_str())
                    .unwrap_or_else(|| nn.clone())
                    != "Option"
                {
                    return Ok(None);
                }
                let Some(inner) = rtargs.first().cloned() else {
                    return Ok(None);
                };
                substitute(&inner, &ctx.generic_subst)
            }
        }
        other => {
            let Some(imp) = ctx.find_impl_for_method(other, "next").cloned() else {
                return Ok(None);
            };
            let Some(md) = imp.methods.iter().find(|m| m.sig.name == "next") else {
                return Ok(None);
            };
            let mut subst: HashMap<String, Type> = HashMap::new();
            if unify(&imp.self_type, other, &mut subst).is_err() {
                return Ok(None);
            }
            let ret = substitute(&md.sig.return_type, &subst);
            let Type::Named(n, targs) = peel_ref(&ret) else {
                return Ok(None);
            };
            if ctx.resolve_full_name(n.as_str()).unwrap_or_else(|| n.clone()) != "Option" {
                return Ok(None);
            }
            let Some(inner) = targs.first() else {
                return Ok(None);
            };
            substitute(inner, &ctx.generic_subst)
        }
    };
    if matches!(elem_ty, Type::Infer) {
        return Ok(None);
    }
    let (hir, ty) = check_iterator_adapter(ctx, receiver, self_ty, method, args, &elem_ty, span)?;
    Ok(Some((hir, ty)))
}

/// J3 迭代器适配器 desugar（数组或自定义迭代器接收者）。
///
/// 闭包经 [`check_closure_expected`] 按 `fn(T...) -> U` 预期签名注入匿名函数
/// （H2 全链路，返回函数指针），绑定为 `__adp_f` 变量；收集容器为带类型注解
/// 的 `Vec<U>`（`let mut __out: Vec<U> = Vec::new();`，避免 Infer 元素类型）；
/// 循环复用现有分派：
///
/// - 数组：`for __x in <recv> { apply }`（J1 `check_for_array` 索引遍历）
/// - 迭代器：`let mut __it = <recv>; loop { match __it.next() { Some(__x) => apply, None => break } }`
///
/// 各适配器的 apply（循环体内语句）：
/// - `map`:     `let __u = __f(__x); __out.push(__u);`      → Vec<U>
/// - `filter`:  `if __f(__x) { __out.push(__x); }`          → Vec<T>
/// - `fold`:    `__acc = __f(__acc, __x);`                  → 返回 __acc（T_acc）
/// - `take(n)`: `if __n < n { __n += 1; __out.push(__x); } else { break; }` → Vec<T>
/// - `skip(n)`: `if __n < n { __n += 1; } else { __out.push(__x); }`         → Vec<T>
/// - `collect`: `__out.push(__x);`                          → Vec<T>
fn check_iterator_adapter(
    ctx: &mut TypeContext,
    receiver: &AstExpr,
    self_ty: &Type,
    method: &str,
    args: &[AstExpr],
    elem_ty: &Type,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // 数组 `[T; N]` 与 `Vec<T>` 容器经 `for ... in` 分派（J1 / check_for_vec）；
    // 自定义迭代器经 `next()` 循环。
    let is_for_loop = match self_ty {
        Type::Array(_, _) => true,
        Type::Named(n, _) => {
            ctx.resolve_full_name(n.as_str()).unwrap_or_else(|| n.clone()) == "Vec"
        }
        _ => false,
    };
    let mk_ident = |name: &str| AstExpr::new(ExprKind::Ident(name.to_string()), span);
    let mk_block = |stmts: Vec<AstStmt>, final_expr: Option<AstExpr>| AstBlock {
        stmts,
        final_expr,
        span,
    };

    // —— 参数校验 ——
    let expect_args = match method {
        "collect" => 0,
        "fold" => 2,
        _ => 1,
    };
    if args.len() != expect_args {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("<{method}>"),
            expected: expect_args,
            found: args.len(),
            span,
        });
    }

    // —— 唯一临时名 ——
    let f_name = format!("__adp_f_{}", ctx.temp_counter);
    let x_name = format!("__adp_x_{}", ctx.temp_counter);
    let out_name = format!("__adp_out_{}", ctx.temp_counter);
    let it_name = format!("__adp_it_{}", ctx.temp_counter);
    let acc_name = format!("__adp_acc_{}", ctx.temp_counter);
    let n_name = format!("__adp_n_{}", ctx.temp_counter);
    ctx.temp_counter += 6;

    // —— 闭包签名与返回类型 ——
    // u_ty：收集容器元素类型（map/fold 取闭包返回类型；其余取元素类型 T）
    // closure_binding：`__adp_f` 函数指针绑定（map/filter/fold 有，其余无）
    let (u_ty, closure_binding): (Type, Option<(HirExpr, Type)>) = match method {
        // 闭包参数必须为闭包字面量（H2 无捕获闭包）
        "map" | "filter" => {
            let c = &args[0];
            if !matches!(&*c.kind, ExprKind::Closure { .. }) {
                return Err(TypeError::Unsupported {
                    what: format!("`{method}` 的参数必须是闭包 `|x| ..`"),
                    span: c.span,
                });
            }
            let u = closure_return_ty(ctx, c, std::slice::from_ref(elem_ty), c.span)?;
            let fn_sig = FnSignature {
                params: vec![elem_ty.clone()],
                return_type: u.clone(),
            };
            let binding =
                check_closure_expected(ctx, c, &Type::Fn(Box::new(fn_sig)), span)?;
            (u, Some(binding))
        }
        "fold" => {
            let c = &args[1];
            if !matches!(&*c.kind, ExprKind::Closure { .. }) {
                return Err(TypeError::Unsupported {
                    what: "`fold` 的第二个参数必须是闭包 `|acc, x| ..`".to_string(),
                    span: c.span,
                });
            }
            // init 先推断（acc 类型），再推断闭包返回类型
            let (_, acc_ty) = infer_expr(ctx, &args[0])?;
            if matches!(acc_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "`fold` 的初始值要求类型确定（如 `0` / `String::from(\"\")`）".to_string(),
                    span: args[0].span,
                });
            }
            let u = closure_return_ty(ctx, c, &[acc_ty.clone(), elem_ty.clone()], c.span)?;
            let fn_sig = FnSignature {
                params: vec![acc_ty.clone(), elem_ty.clone()],
                return_type: u.clone(),
            };
            let binding =
                check_closure_expected(ctx, c, &Type::Fn(Box::new(fn_sig)), span)?;
            (u, Some(binding))
        }
        _ => {
            // take / skip / collect：无闭包
            (elem_ty.clone(), None)
        }
    };
    if let Some((_, fn_ty)) = &closure_binding {
        ctx.insert_variable(f_name.clone(), fn_ty.clone());
    }

    // —— 收集容器类型 ——
    let result_ty = if method == "fold" {
        // acc 类型：init 的变量类型
        let (_, acc_ty) = infer_expr(ctx, &args[0])?;
        acc_ty
    } else if method == "map" {
        Type::Named("Vec".to_string(), vec![u_ty.clone()])
    } else {
        Type::Named("Vec".to_string(), vec![elem_ty.clone()])
    };
    let vec_ast_ty = match &result_ty {
        Type::Named(n, inner) => AstType::Path(n.clone(), inner.iter().map(ty_to_ast).collect()),
        _ => AstType::Path("Vec".to_string(), vec![ty_to_ast(&result_ty)]),
    };

    // —— apply 语句（循环体内）——
    let push = |out: &str, val: &str| {
        AstExpr::new(
            ExprKind::MethodCall {
                receiver: mk_ident(out),
                method: "push".to_string(),
                args: vec![mk_ident(val)],
            },
            span,
        )
    };
    let apply_stmts: Vec<AstStmt> = match method {
        "map" => {
            let u_name = format!("__adp_u_{}", ctx.temp_counter);
            ctx.temp_counter += 1;
            let call = AstExpr::new(
                ExprKind::Call {
                    callee: mk_ident(&f_name),
                    args: vec![mk_ident(&x_name)],
                    type_args: Vec::new(),
                },
                span,
            );
            vec![
                AstStmt::Let {
                    pattern: AstPattern::Ident(u_name.clone()),
                    type_anno: None,
                    init: call,
                    mutable: false,
                },
                AstStmt::Semi(push(&out_name, &u_name)),
            ]
        }
        "filter" => {
            let cond = AstExpr::new(
                ExprKind::Call {
                    callee: mk_ident(&f_name),
                    args: vec![mk_ident(&x_name)],
                    type_args: Vec::new(),
                },
                span,
            );
            let if_expr = AstExpr::new(
                ExprKind::If {
                    cond,
                    then_block: mk_block(vec![AstStmt::Semi(push(&out_name, &x_name))], None),
                    else_block: None,
                },
                span,
            );
            vec![AstStmt::Semi(if_expr)]
        }
        "fold" => {
            let call = AstExpr::new(
                ExprKind::Call {
                    callee: mk_ident(&f_name),
                    args: vec![mk_ident(&acc_name), mk_ident(&x_name)],
                    type_args: Vec::new(),
                },
                span,
            );
            vec![AstStmt::Semi(AstExpr::new(
                ExprKind::Assign {
                    target: mk_ident(&acc_name),
                    op: AssignOp::Assign,
                    value: call,
                },
                span,
            ))]
        }
        "take" => {
            // if __n < n { __n += 1; __out.push(__x); } else { break }
            let cond = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![mk_ident(&n_name), args[0].clone()],
                    operators: vec![rlyeh_ast::CompareOp::Lt],
                },
                span,
            );
            let inc = AstExpr::new(
                ExprKind::Assign {
                    target: mk_ident(&n_name),
                    op: AssignOp::AddAssign,
                    value: AstExpr::new(ExprKind::IntLiteral(1), span),
                },
                span,
            );
            let then_block = mk_block(
                vec![
                    AstStmt::Semi(inc),
                    AstStmt::Semi(push(&out_name, &x_name)),
                ],
                None,
            );
            let else_block = mk_block(vec![AstStmt::Semi(AstExpr::new(ExprKind::Break(None), span))], None);
            let if_expr = AstExpr::new(
                ExprKind::If {
                    cond,
                    then_block,
                    else_block: Some(else_block),
                },
                span,
            );
            vec![AstStmt::Semi(if_expr)]
        }
        "skip" => {
            // if __n < n { __n += 1 } else { __out.push(__x); }
            let cond = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![mk_ident(&n_name), args[0].clone()],
                    operators: vec![rlyeh_ast::CompareOp::Lt],
                },
                span,
            );
            let inc = AstExpr::new(
                ExprKind::Assign {
                    target: mk_ident(&n_name),
                    op: AssignOp::AddAssign,
                    value: AstExpr::new(ExprKind::IntLiteral(1), span),
                },
                span,
            );
            let if_expr = AstExpr::new(
                ExprKind::If {
                    cond,
                    then_block: mk_block(vec![AstStmt::Semi(inc)], None),
                    else_block: Some(mk_block(
                        vec![AstStmt::Semi(push(&out_name, &x_name))],
                        None,
                    )),
                },
                span,
            );
            vec![AstStmt::Semi(if_expr)]
        }
        _ => {
            // collect：__out.push(__x)
            vec![AstStmt::Semi(push(&out_name, &x_name))]
        }
    };

    // —— 循环构造 ——
    let loop_expr = if is_for_loop {
        AstExpr::new(
            ExprKind::For {
                pattern: AstPattern::Ident(x_name.clone()),
                iterator: receiver.clone(),
                body: mk_block(apply_stmts, None),
            },
            span,
        )
    } else {
        // let mut __it = <recv>;
        // loop { match __it.next() { Some(__x) => { apply }, None => break } }
        let next_call = AstExpr::new(
            ExprKind::MethodCall {
                receiver: mk_ident(&it_name),
                method: "next".to_string(),
                args: vec![],
            },
            span,
        );
        let match_expr = AstExpr::new(
            ExprKind::Match {
                expr: next_call,
                arms: vec![
                    rlyeh_ast::MatchArm {
                        pattern: AstPattern::Enum(
                            "Some".to_string(),
                            vec![AstPattern::Ident(x_name.clone())],
                        ),
                        guard: None,
                        body: AstExpr::new(ExprKind::Block(mk_block(apply_stmts, None)), span),
                        span,
                    },
                    rlyeh_ast::MatchArm {
                        pattern: AstPattern::Enum("None".to_string(), vec![]),
                        guard: None,
                        body: AstExpr::new(ExprKind::Break(None), span),
                        span,
                    },
                ],
            },
            span,
        );
        let loop_ast = AstExpr::new(
            ExprKind::Loop {
                body: mk_block(vec![AstStmt::Semi(match_expr)], None),
            },
            span,
        );
        // 迭代器绑定前缀并入外层块
        loop_ast
    };

    // —— 外层块：`let mut __out: Vec<U> = Vec::new();` (+ fold acc / take n / 迭代器绑定) + 循环 ——
    let mut ast_stmts = vec![AstStmt::Let {
        pattern: AstPattern::Ident(out_name.clone()),
        type_anno: Some(vec_ast_ty),
        init: AstExpr::new(
            ExprKind::Call {
                callee: AstExpr::new(ExprKind::Path(vec!["Vec".to_string(), "new".to_string()]), span),
                args: vec![],
                type_args: Vec::new(),
            },
            span,
        ),
        mutable: true,
    }];
    if method == "fold" {
        ast_stmts.push(AstStmt::Let {
            pattern: AstPattern::Ident(acc_name.clone()),
            type_anno: None,
            init: args[0].clone(),
            mutable: true,
        });
    }
    if matches!(method, "take" | "skip") {
        ast_stmts.push(AstStmt::Let {
            pattern: AstPattern::Ident(n_name.clone()),
            type_anno: None,
            init: AstExpr::new(ExprKind::IntLiteral(0), span),
            mutable: true,
        });
    }
    if !is_for_loop {
        ast_stmts.push(AstStmt::Let {
            pattern: AstPattern::Ident(it_name.clone()),
            type_anno: None,
            init: receiver.clone(),
            mutable: true,
        });
    }
    ast_stmts.push(AstStmt::Semi(loop_expr));
    let final_expr = if method == "fold" {
        mk_ident(&acc_name)
    } else {
        mk_ident(&out_name)
    };
    let block_ast = AstExpr::new(
        ExprKind::Block(mk_block(ast_stmts, Some(final_expr))),
        span,
    );
    let (loop_hir, block_ty) = infer_expr(ctx, &block_ast)?;

    // —— 前缀 HIR：闭包函数指针绑定（map/filter/fold）——
    let mut hir_stmts = vec![];
    if let Some((fnptr_hir, _)) = &closure_binding {
        hir_stmts.push(HirStmt::Let {
            name: f_name.clone(),
            init: fnptr_hir.clone(),
            mutable: false,
        });
    }
    hir_stmts.push(HirStmt::Expr(loop_hir));
    let hir = HirExpr::Block(Box::new(HirBlock {
        stmts: hir_stmts,
        final_expr: Some(if method == "fold" {
            HirExpr::Variable(acc_name)
        } else {
            HirExpr::Variable(out_name)
        }),
    }));
    Ok((hir, block_ty))
}

/// 检查 `HashMap<K, V>` 容器迭代的 for 循环：`for (k, v) in m { body }`。
///
/// 在类型检查层 desugar 为索引遍历循环（复用 HashMap 7 槽布局：
/// 槽 0 = keys 指针、槽 1 = vals 指针、槽 2 = states 指针（0=空 1=占用 2=墓碑）、
/// 槽 3 = len、槽 4 = used、槽 5 = cap、槽 6 = dist 距离数组）。HashMap 是稀疏存储（删除产生墓碑），
/// 遍历必须按容量扫描并跳过 `states[i] != 1` 的空槽 / 墓碑：
///
/// ```rlyeh
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

    // 4. 进入循环作用域 + 绑定临时变量（结束弹出）。
    //    存储槽名：k/v 被外层同名遮蔽时 mangle，Let/引用/Assign 全部用槽名。
    ctx.push_scope(false);
    let map_ty = Type::Named("HashMap".to_string(), vec![k_ty.clone(), v_ty.clone()]);
    let stored_m = ctx.insert_variable(m_name.clone(), map_ty);
    let stored_cap = ctx.insert_variable(cap_name.clone(), Type::I64);
    let stored_i = ctx.insert_variable(i_name.clone(), Type::I64);
    let stored_k = ctx.insert_variable(k_name.clone(), k_ty.clone());
    let stored_v = ctx.insert_variable(v_name.clone(), v_ty.clone());

    // 5. 前缀语句：绑定容器、缓存容量、初始化计数器
    let mut stmts = vec![
        HirStmt::Let {
            name: stored_m.clone(),
            init: iter_hir,
            mutable: false,
        },
        HirStmt::Let {
            name: stored_cap.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(stored_m.clone())),
                index: 5, // HashMap 槽 5 = cap
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: stored_i.clone(),
            init: HirExpr::IntLiteral(0),
            mutable: true,
        },
    ];

    // 6. 循环体检查
    let (b_hir, _) = check_block(ctx, body)?;
    ctx.pop_scope();

    // 7. loop 体：边界检查 → 跳槽 → 绑定 k/v → 递增 → 原 body 语句
    let k_scalar = field_scalar_of(&k_ty);
    let v_scalar = field_scalar_of(&v_ty);
    let mut loop_body_stmts = vec![
        // if __for_i >= __for_cap { break }
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Ge,
                Box::new(HirExpr::Variable(stored_i.clone())),
                Box::new(HirExpr::Variable(stored_cap.clone())),
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
                        base: Box::new(HirExpr::Variable(stored_m.clone())),
                        index: 2, // HashMap 槽 2 = states 指针
                        ty: FieldScalar::Ptr,
                    }),
                    index: Box::new(HirExpr::Variable(stored_i.clone())),
                    elem: FieldScalar::Int,
                    is_str: false,
                }),
                Box::new(HirExpr::IntLiteral(1)),
            )),
            then_block: Box::new(HirBlock {
                stmts: vec![
                    HirStmt::Expr(HirExpr::Assign {
                        target: stored_i.clone(),
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
            name: stored_k.clone(),
            init: HirExpr::Index {
                base: Box::new(HirExpr::FieldGet {
                    base: Box::new(HirExpr::Variable(stored_m.clone())),
                    index: 0, // HashMap 槽 0 = keys 指针
                    ty: FieldScalar::Ptr,
                }),
                index: Box::new(HirExpr::Variable(stored_i.clone())),
                elem: k_scalar,
                is_str: false,
            },
            mutable: false,
        },
        // let v = __for_m.vals[__for_i]
        HirStmt::Let {
            name: stored_v.clone(),
            init: HirExpr::Index {
                base: Box::new(HirExpr::FieldGet {
                    base: Box::new(HirExpr::Variable(stored_m.clone())),
                    index: 1, // HashMap 槽 1 = vals 指针
                    ty: FieldScalar::Ptr,
                }),
                index: Box::new(HirExpr::Variable(stored_i.clone())),
                elem: v_scalar,
                is_str: false,
            },
            mutable: false,
        },
        // __for_i += 1
        HirStmt::Expr(HirExpr::Assign {
            target: stored_i.clone(),
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

/// 是否可参与 U6 数值转换（`as`）的标量类型：
/// ≤64 位整族 + 浮点 + bool + char。
/// `i128`/`u128`（128 位存储非 64 位槽）与指针/引用/聚合不支持。
fn is_castable_scalar(t: &Type) -> bool {
    matches!(
        t,
        Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::ISize
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::USize
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::Char
    )
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
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let name = match &*callee.kind {
        ExprKind::Ident(n) => n.clone(),
        // 模块路径调用：`math::add(...)`
        ExprKind::Path(segments) => segments.join("::"),
        _ => {
            // H3 捕获闭包 IIFE：`(|x, y| body)(args)` 立即调用——
            // callee 为闭包表达式时走捕获闭包路径（body 中引用的外层变量
            // 按值捕获，desugar 为匿名函数 + 捕获变量前置调用）
            if let ExprKind::Closure { params, param_types: _, body, capture } = &*callee.kind {
                return check_capture_closure_iife(ctx, params, body, capture, args, span);
            }
            // 函数值调用（函数指针）：`fns[i](x)` / `get_fn()(x)`
            let (callee_hir, callee_ty) = infer_expr(ctx, callee)?;
            let signature = match &callee_ty {
                Type::Fn(sig) => (**sig).clone(),
                other => {
                    return Err(TypeError::Unsupported {
                        what: format!("复杂被调用表达式（类型 `{other}` 不可调用）"),
                        span,
                    });
                }
            };
            return check_indirect_call(ctx, callee_hir, signature, args, span);
        }
    };

    // L2 编译器内建 JSON 序列化/反序列化（`json.stringify(v)` / `json.parse::<T>(s)`）：
    // AST 层 desugar 为 String 构建 / 解析表达式，零新增 IR 节点。
    // Q2a 泛型 API 别名：`json.to_string(v)` ≡ `json.stringify(v)`；
    // `json.from_str::<T>(s)` ≡ `json.parse::<T>(s)`。MVP 无泛型 trait 约束
    // （`T: Serialize` / `T: Deserialize` bound 不支持），签名退化为无 bound
    // turbofish 形式：序列化类型由实参推断，反序列化经 turbofish 指定。
    if name == "json::stringify" || name == "json::to_string" {
        return check_json_stringify(ctx, args, span);
    }
    if name == "json::parse" || name == "json::from_str" {
        return check_json_parse(ctx, args, type_args, span);
    }
    // Q4 `toml` 模块（轻量 MVP）：`toml.to_string`/`toml.stringify` 序列化（基础标量 /
    // 嵌套表（内联表）/ 数组），`toml.from_str`/`toml.parse` 反序列化（round-trip 对齐
    // stringify 的紧凑输出）。MVP 无泛型 trait 约束（`T: Serialize` / `T: Deserialize`
    // bound 不支持），签名退化为无 bound turbofish 形式（同 Q2 json）。
    if name == "toml::stringify" || name == "toml::to_string" {
        return check_toml_stringify(ctx, args, span);
    }
    if name == "toml::parse" || name == "toml::from_str" {
        return check_toml_parse(ctx, args, type_args, span);
    }
    // Q2b 流式 writer/reader（目标 `File`；TcpStream 留待流式 read_all 方法化）：
    // `json.to_writer(w, v)` → `w.write_all(json.stringify(v))`，返回
    // `Result<i64, io::error::IoError>`；`json.from_reader::<T>(r)` →
    // `json.parse::<T>(r.read_to_string().unwrap())`，读失败经 `unwrap` 死循环
    // （MVP 语义，与 std `Result::unwrap` 一致）。
    if name == "json::to_writer" {
        return check_json_to_writer(ctx, args, span);
    }
    if name == "json::from_reader" {
        return check_json_from_reader(ctx, args, type_args, span);
    }

    // 内建函数（`print` / `println` / `alloc_array` 等，由代码生成层映射到运行时）：
    // 按签名检查参数、返回签名类型
    if let Some((params, ret)) = builtin_signature(&name) {
        // `print` / `println` / `eprint` / `eprintln` 允许 0..=1 个参数
        // （`println()` 打印空行；`eprint*` 输出到 stderr）
        let is_console = name == "print"
            || name == "println"
            || name == "eprint"
            || name == "eprintln";
        let max_args = if is_console { 1 } else { params.len() };
        if args.len() > max_args {
            return Err(TypeError::UnexpectedArgumentCount {
                name: name.clone(),
                expected: max_args,
                found: args.len(),
                span,
            });
        }
        // `print(s)` / `println(s)` / `eprint(s)` / `eprintln(s)` 参数为 String →
        // 展开为 `print_string` / `println_string` / `eprint_string` / `eprintln_string`
        // 内建（动态缓冲按 `%.*s` 打印；typecheck 无法在内建签名层表达对象槽读取）
        if is_console && args.len() == 1 {
            let (mut hir, mut ty) = infer_expr(ctx, &args[0])?;
            // 引用参数自动剥一层（G1 剥层语义覆盖 print/println 内建：
            // `println(r)` 打印解引用值而非地址，与字段访问 / 方法调用剥层一致）
            if let Type::Ref(inner, _) = &ty {
                hir = HirExpr::Deref {
                    expr: Box::new(hir),
                    ty: field_scalar_of(inner),
                };
                ty = (**inner).clone();
            }
            if let Type::Named(n, _) = peel_ref(&ty) {
                let full = ctx
                    .resolve_full_name(&n)
                    .unwrap_or_else(|| n.clone());
                if full == "String" && ctx.lookup_struct(&full).is_some() {
                    let callee = match name.as_str() {
                        "println" => "println_string",
                        "print" => "print_string",
                        "eprintln" => "eprintln_string",
                        _ => "eprint_string",
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
            // 非 String 参数：直接生成 print / println / eprint / eprintln 调用
            // （引用已剥层，避免落入下方通用路径时对原始实参重新 infer 而丢失剥层结果）
            return Ok((
                HirExpr::Call {
                    callee: name.clone(),
                    args: vec![hir],
                },
                Type::Unit,
            ));
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

    // actor 构造函数：`Counter::new()` → `rlyeh_actor_spawn("<handle>", <state_new>())`；
    // `Counter::new_supervised(strategy)` → `rlyeh_actor_spawn_supervised("<handle>", "<state_new>", strategy)`
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
                        "rlyeh_actor_spawn_supervised".to_string(),
                        vec![handle_hir, factory_hir, strategy_hir],
                    )
                } else {
                    ("rlyeh_actor_spawn".to_string(), vec![handle_hir, state_new_call])
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
        // `HashMap` 构造器特判：`new()` / `with_capacity(n)`（7 槽 Robin Hood 哈希表）
        if ty_full == "HashMap" && ctx.lookup_struct(&ty_full).is_some()
            && (method == "with_capacity" || method == "new")
        {
            return check_hashmap_construct(ctx, method, args, span);
        }
        // `Iter` / `IterMut` 构造器特判（V1 瘦指针迭代器，2026-08）：
        // `Iter::new(data, len)`（2 槽）/ `IterMut::new(data, cur, len)`（3 槽）
        if matches!(ty_full.as_str(), "Iter" | "IterMut")
            && ctx.lookup_struct(&ty_full).is_some()
            && method == "new"
        {
            return check_iter_construct(ctx, &ty_full, args, span);
        }
        // `Box` 构造器特判：`Box::new(v)`（K2 堆分配装箱）
        // （Box 为编译器内建智能指针，无需 std 结构体定义）
        if ty_full == "Box" && method == "new" {
            return check_box_new(ctx, args, span);
        }
        // `Rc` / `Arc` 构造器特判：`Rc::new(v)` / `Arc::new(v)`（K3 引用计数装箱）
        if matches!(ty_full.as_str(), "Rc" | "Arc") && method == "new" {
            return check_rc_new(ctx, &ty_full, args, span);
        }
        // `Gc` 构造器特判：`Gc::new(v)`（K4 追踪 GC 装箱）
        // （Gc 为编译器内建智能指针，分配经 rlyeh-gc-runtime 注册块表）
        if ty_full == "Gc" && method == "new" {
            return check_gc_new(ctx, args, span);
        }
        // `Box::leak` 特判（T3a）：泄漏堆对象，返回指向堆 `T` 的裸指针
        // `*mut T`（G3 语义，`*p` 读写可用），不再释放。
        // 目标签名 `fn leak(self) -> &'static mut T`——MVP 退化：返回裸指针
        // 而非引用（值语义等价——`&*b` 经 MIR 折叠（U5）取 Box 槽 0 指针值，
        // 与 `FieldGet(b, 0, Ptr)` 同一地址；裸指针规避 borrowck 引用逃逸检查；
        // `'static` 生命周期标注宽松丢弃（G4））。
        if ty_full == "Box" && method == "leak" {
            if args.len() != 1 {
                return Err(TypeError::UnexpectedArgumentCount {
                    name: "Box::leak".to_string(),
                    expected: 1,
                    found: args.len(),
                    span,
                });
            }
            let (b_hir, b_ty) = infer_expr(ctx, &args[0])?;
            let inner = peel_refs_and_heap(&b_ty);
            let ptr = HirExpr::FieldGet {
                base: Box::new(b_hir),
                index: 0,
                ty: FieldScalar::Ptr,
            };
            return Ok((ptr, Type::RawPtr(Box::new(inner), true)));
        }
        // `Weak` 升级特判：`Weak::upgrade(w)`（K3 弱引用升级为强引用）
        if ty_full == "Weak" && method == "upgrade" {
            if args.len() != 1 {
                return Err(TypeError::UnexpectedArgumentCount {
                    name: "Weak::upgrade".to_string(),
                    expected: 1,
                    found: args.len(),
                    span,
                });
            }
            let (w_hir, w_ty) = infer_expr(ctx, &args[0])?;
            match peel_ref(&w_ty) {
                Type::Named(n, ps) if n == "Weak" && ps.len() == 1 => {
                    return weak_upgrade(ctx, ps[0].clone(), w_hir, &[], span);
                }
                found => {
                    return Err(TypeError::ArgumentTypeMismatch {
                        name: "Weak::upgrade".to_string(),
                        index: 0,
                        expected: "Weak<T>".to_string(),
                        found: found.to_string(),
                        span,
                    });
                }
            }
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

    let signature = match ctx.lookup_fn_signature(&resolved).cloned() {
        Some(s) => s,
        None => {
            // 闭包值对象调用兜底：callee 为闭包值变量（`let f = |x: i64| ..; f(1)`）。
            if let Some(ty) = ctx.lookup_variable(&name).cloned() {
                if let Type::Closure { .. } = &ty {
                    return check_closure_value_call(ctx, &name, &ty, args, span);
                }
            }
            // 函数指针调用兜底：callee 为函数值表达式（如 `let f = add; f(1, 2)`）。
            // 推断失败（未定义变量等）时保留原有 FunctionNotFound 诊断。
            if let Ok((callee_hir, Type::Fn(sig))) = infer_expr(ctx, callee).as_ref() {
                return check_indirect_call(ctx, callee_hir.clone(), (**sig).clone(), args, span);
            }
            return Err(TypeError::FunctionNotFound {
                name: name.clone(),
                span,
            });
        }
    };
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
        // H2 无捕获闭包实参：形参为 fn 类型且实参为闭包 → 按预期签名检查
        // （闭包参数无类型注解，无法脱离 fn 上下文推断参数类型）
        let (hir, ty) =
            if matches!(param_ty, Type::Fn(_)) && matches!(&*arg.kind, ExprKind::Closure { .. }) {
                check_closure_expected(ctx, arg, param_ty, arg.span)?
            } else {
                infer_expr(ctx, arg)?
            };
        // Str 值实参 → 非 Str 形参自动升级（`fn f(s: String)` 传 `f("hi")`）
        let (hir, ty) = upgrade_str_arg(ctx, hir, ty, param_ty, arg)?;
        // 闭包值实参 → fn 形参（H5 补全）：
        // - 未固化延迟闭包值（`let f = |x| x + 1; apply(f, 41);`）：按 fn
        //   形参签名固化参数类型，无捕获时降级为函数指针（捕获非空报 Unsupported）；
        // - 已固化无捕获闭包值（`let f = |x: i64| x + 1; apply(f, 41);`）：
        //   直接降级为函数指针；
        // - 有捕获闭包值保持闭包对象类型 → 与 fn 形参不兼容，报 ArgumentTypeMismatch。
        let (hir, ty) = if let Type::Fn(sig) = param_ty {
            if matches!(&ty, Type::Closure { fn_name, .. } if fn_name.is_empty()) {
                if let ExprKind::Ident(n) = &*arg.kind {
                    fix_deferred_closure_with_sig(ctx, n, sig, arg.span)?
                } else {
                    (hir, ty)
                }
            } else {
                try_closure_value_as_fn(&ty).unwrap_or((hir, ty))
            }
        } else {
            (hir, ty)
        };
        // 函数指针实参 → i64 形参（S0 线程入口地址整数化）：
        // `__rlyeh_thread_spawn(f, 0)` 中 f 为 `fn() -> i64` 函数指针值，extern 形参
        // 是 i64——函数指针按地址整数传递，codegen 在 extern 调用点做 ptrtoint。
        let (hir, ty) = if matches!(param_ty, Type::I64) && matches!(&ty, Type::Fn(_)) {
            (hir, Type::I64)
        } else {
            (hir, ty)
        };
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

/// 迭代检查闭包体并收集捕获变量（H3 IIFE / 闭包值对象共用）。
///
/// 逐轮检查闭包体：未定义变量若在外层变量环境中有类型，即为捕获变量
/// （按值捕获类型快照）；否则报真未定义。每轮至少新增一个捕获变量，
/// 循环收敛。参数（含类型）先插入环境。
///
/// 返回 `(body_hir, body_ty, outer_capture_slots, capture_tys, closure_capture_slots, param_slots)`：
/// - `outer_capture_slots`：捕获源变量在**闭包定义处外层**的存储槽名——调用实参 /
///   闭包对象捕获字段按此引用；
/// - `closure_capture_slots` / `param_slots`：捕获/参数在**闭包 fn 层**实际 insert 的
///   存储槽名（遮蔽时 mangle 为 `name$N`，捕获源必在外层同名，故总是二次 mangle）——
///   闭包函数签名参数名与 body 内 HIR 引用（resolve 槽名）必须一致，emit 使用这些槽名。
fn check_closure_body_with_captures(
    ctx: &mut TypeContext,
    param_pairs: &[(String, Type)],
    body: &AstExpr,
    span: Span,
) -> Result<
    (
        HirExpr,
        Type,
        Vec<String>,
        Vec<Type>,
        Vec<String>,
        Vec<String>,
    ),
    TypeError,
> {
    let mut captures: Vec<String> = Vec::new();
    let mut outer_capture_slots: Vec<String> = Vec::new();
    let mut capture_tys: Vec<Type> = Vec::new();
    let body_result: Result<(HirExpr, Type, Vec<String>, Vec<String>), TypeError> = loop {
        ctx.push_scope(true);
        // 闭包 fn 层实际存储槽名（捕获/参数可能被二次 mangle）
        let mut closure_capture_slots = Vec::with_capacity(captures.len());
        for (nm, ty) in captures.iter().zip(capture_tys.clone()) {
            let s = ctx.insert_variable(nm.clone(), ty);
            closure_capture_slots.push(s);
        }
        let mut param_slots = Vec::with_capacity(param_pairs.len());
        for (nm, ty) in param_pairs.iter() {
            let s = ctx.insert_variable(nm.clone(), ty.clone());
            param_slots.push(s);
        }
        let r = infer_expr(ctx, body);
        ctx.pop_scope();
        match r {
            Ok((h, t)) => break Ok((h, t, closure_capture_slots, param_slots)),
            Err(TypeError::UndefinedVariable { name, .. }) if !captures.contains(&name) => {
                // 环境已恢复为外层（闭包 fn 层已弹出）——在外层变量环境中
                // 能找到类型者即为捕获变量（原名用于检测去重；外层槽名供调用
                // 实参 / 闭包对象捕获字段引用；类型快照供捕获参数绑定）
                if let Some((slot, ty)) = ctx.resolve_variable(&name) {
                    captures.push(name);
                    outer_capture_slots.push(slot.to_string());
                    capture_tys.push(ty.clone());
                    continue;
                }
                break Err(TypeError::UndefinedVariable { name, span });
            }
            Err(e) => break Err(e),
        }
    };
    let (body_hir, body_ty, closure_capture_slots, param_slots) = body_result?;
    Ok((
        body_hir,
        body_ty,
        outer_capture_slots,
        capture_tys,
        closure_capture_slots,
        param_slots,
    ))
}

/// 生成闭包匿名函数：参数 = [捕获变量（外层原名）..., 闭包参数...]。
///
/// 注入函数签名与 `mono_items`，返回匿名函数名。捕获变量参数名保留外层
/// 原名（闭包体内按名字引用）。零新增 IR 节点。
fn emit_closure_fn(
    ctx: &mut TypeContext,
    captures: &[String],
    capture_tys: &[Type],
    param_names: &[String],
    param_tys: &[Type],
    body_hir: HirExpr,
    body_ty: Type,
) -> String {
    let name = format!("__closure_{}", ctx.closure_seq);
    ctx.closure_seq += 1;
    let mut fn_params = capture_tys.to_vec();
    fn_params.extend(param_tys.iter().cloned());
    let mut fn_names = captures.to_vec();
    fn_names.extend(param_names.iter().cloned());
    ctx.insert_fn_signature(
        name.clone(),
        FnSignature {
            params: fn_params,
            return_type: body_ty.clone(),
        },
    );
    ctx.mono_items.push(HirItem {
        name: name.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params: fn_names
                .iter()
                .map(|n| HirParam { name: n.clone() })
                .collect(),
            body: Some(HirBlock {
                stmts: vec![],
                final_expr: Some(body_hir),
            }),
            is_extern: false,
            extern_sig: None,
        }),
    });
    name
}

/// H3 捕获闭包（IIFE）：`(|x, y| body)(args)` 立即调用。
///
/// 捕获闭包不能表示为 fn 指针（无捕获环境），MVP 支持**立即调用**与
/// **闭包值**（`let f = |x: i64| ..; f(..)`，见 [`check_closure_value_binding`]）：
/// 闭包体中引用的外层变量按值捕获，desugar 为匿名函数
/// `__closure_N(cap1, cap2, x, y) -> ret { body }`（捕获变量作为前置参数，
/// 参数名保留外层原名），调用点展开为普通函数调用
/// `__closure_N(cap1, cap2, arg1, arg2)`。零新增 IR 节点。
///
/// MVP 约束：
/// 1. 立即调用；或绑定为闭包值对象（参数须带类型注解）；无注解闭包不能
///    作为 fn 指针值。
/// 2. 按值捕获（捕获变量类型快照）；`move` 关键字 MVP 忽略（所有权宽松）。
/// 3. IIFE 闭包参数无类型注解，类型从实参推断（IIFE 无 fn 类型上下文）；
///    闭包值对象要求参数带类型注解。
/// 4. 捕获变量名与闭包参数同名时参数遮蔽捕获（Rust 语义）。
/// 5. 嵌套捕获闭包（闭包体内再捕获）不支持。
fn check_capture_closure_iife(
    ctx: &mut TypeContext,
    params: &[AstPattern],
    body: &AstExpr,
    _capture: &CaptureMode,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if params.len() != args.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "<捕获闭包>".to_string(),
            expected: params.len(),
            found: args.len(),
            span,
        });
    }
    // 闭包参数名（Ident / Wildcard）
    let mut names = Vec::with_capacity(params.len());
    for (i, p) in params.iter().enumerate() {
        match p {
            AstPattern::Ident(n) => names.push(n.clone()),
            AstPattern::Wildcard => names.push(format!("__arg{i}")),
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（H3 仅支持简单标识符参数）"),
                    span,
                })
            }
        }
    }
    // 实参检查（外层环境）：闭包参数类型从实参推断
    let mut arg_hirs = Vec::with_capacity(args.len());
    let mut arg_tys = Vec::with_capacity(args.len());
    for a in args {
        let (h, t) = infer_expr(ctx, a)?;
        if matches!(t, Type::Str) {
            // 字符串字面量实参升级为 String 语义（与 `let s = "..."` 绑定一致）：
            // 展开 `String::from` 深拷贝（data/len/cap 三槽），使闭包体内
            // 拼接 / 方法调用按 String 对象解析（槽数匹配）
            let (h2, t2) = check_string_from(ctx, std::slice::from_ref(a), a.span)?;
            arg_hirs.push(h2);
            arg_tys.push(t2);
        } else {
            arg_hirs.push(h);
            arg_tys.push(t);
        }
    }
    // 迭代检查闭包体 + 收集捕获（外层槽名供调用实参、闭包层槽名供 emit 签名）
    let pairs: Vec<(String, Type)> = names.iter().cloned().zip(arg_tys.clone()).collect();
    let (body_hir, body_ty, outer_capture_slots, capture_tys, closure_capture_slots, param_slots) =
        check_closure_body_with_captures(ctx, &pairs, body, span)?;
    // 匿名函数生成（参数 = [捕获变量, 闭包参数]）
    let name = emit_closure_fn(
        ctx,
        &closure_capture_slots,
        &capture_tys,
        &param_slots,
        &arg_tys,
        body_hir,
        body_ty.clone(),
    );
    // 调用：捕获变量（闭包定义处外层槽名，按名引用）+ 实参
    let mut call_args: Vec<HirExpr> = outer_capture_slots
        .iter()
        .map(|c| HirExpr::Variable(c.clone()))
        .collect();
    call_args.extend(arg_hirs);
    Ok((
        HirExpr::Call {
            callee: name,
            args: call_args,
        },
        body_ty,
    ))
}

/// 非注解闭包值绑定（H5 补全，延迟）：`let f = |x| body;`。
///
/// 绑定处闭包参数类型未知，无法立即检查闭包体（如 `x + 1` 的 `+` 重载
/// 需要参数类型）。注册延迟绑定记录（`ctx.deferred_closures`），绑定变量
/// 类型为未固化 `Type::Closure`（`fn_name` 为空），HIR 占位为
/// `Alloc{slots:0}`（变量槽为 Ptr，供后续闭包值调用路径 `FieldGet` 使用）；
/// 首次调用点 `f(args)` 由实参类型推断参数类型后固化（见
/// [`check_deferred_closure_call`]），真实捕获对象经调用点块内 `let f = ..`
/// 重新绑定覆盖占位。
///
/// MVP 限制：闭包从未被调用时闭包体不检查（惰性，错误延迟暴露）。
pub(crate) fn check_deferred_closure_binding(
    ctx: &mut TypeContext,
    closure: &AstExpr,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let ExprKind::Closure { params, param_types: _, body: _, capture: _ } = &*closure.kind else {
        return Err(TypeError::Unsupported {
            what: "延迟闭包绑定需要闭包表达式".to_string(),
            span,
        });
    };
    // 参数模式校验（Ident / Wildcard；无注解参数允许，类型由首次调用点推断）
    for p in params.iter() {
        match p {
            AstPattern::Ident(_) | AstPattern::Wildcard => {}
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（仅支持简单标识符参数）"),
                    span,
                })
            }
        }
    }
    // 注册延迟绑定（变量名由 check_stmt 绑定 Ident 分支填充）
    ctx.deferred_closures.push(DeferredClosure {
        var_name: String::new(),
        closure: closure.clone(),
        span,
    });
    // 未固化闭包类型：fn_name 为空标记延迟（params/ret 为占位，固化时回写）
    let ty = Type::Closure {
        captures: vec![],
        params: vec![],
        ret: Box::new(Type::Unit),
        fn_name: String::new(),
    };
    Ok((HirExpr::Alloc {
        slots: 0,
        by_value: false,
    }, ty))
}

/// 闭包值对象：`let f = |x: i64| body;` 绑定后 `f(args)` 调用。
///
/// desugar（H3 规划的"匿名结构体 + call 方法"的 MVP 形态）：
/// - 闭包匿名函数 `__closure_N(cap1, cap2, x) -> ret { body }`（与 IIFE 相同）；
/// - 闭包值 = 捕获聚合对象（每捕获一槽）：`__cv_N = Alloc { slots }` + 逐槽
///   `FieldSet`（按值拷贝捕获变量），`f` 绑定为该对象指针；
/// - 调用点 `f(args)` desugar 为 `__closure_N(FieldGet(f, 0, cap0)..., args...)`。
///
/// 限制（MVP）：
/// - 闭包参数须带类型注解（`|x: i64|`），否则参数类型由首次调用点推断
///   （非注解绑定走 [`check_deferred_closure_binding`]）；
/// - 仅按值捕获；闭包值仅存在于局部变量环境，不跨函数边界传递；
/// - 闭包体内捕获其它闭包值不支持（与 H3 一致）。
pub(crate) fn check_closure_value_binding(
    ctx: &mut TypeContext,
    closure: &AstExpr,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let ExprKind::Closure { params, param_types, body, capture: _ } = &*closure.kind else {
        return Err(TypeError::Unsupported {
            what: "闭包值绑定需要闭包表达式".to_string(),
            span,
        });
    };
    // 参数名 + 类型（要求全注解）
    let mut names = Vec::with_capacity(params.len());
    let mut param_tys = Vec::with_capacity(params.len());
    for (i, (p, anno)) in params.iter().zip(param_types.iter()).enumerate() {
        let nm = match p {
            AstPattern::Ident(n) => n.clone(),
            AstPattern::Wildcard => format!("__arg{i}"),
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（仅支持简单标识符参数）"),
                    span,
                })
            }
        };
        let Some(a) = anno else {
            return Err(TypeError::Unsupported {
                what: format!(
                    "闭包参数 `{nm}` 缺少类型注解（闭包值对象需要 `|x: i64|` 形式；无注解闭包可用 IIFE 或 fn 类型上下文）"
                ),
                span,
            });
        };
        let ty = resolve_ast_type(ctx, a, span)?;
        names.push(nm);
        param_tys.push(ty);
    }
    // 迭代检查闭包体 + 收集捕获（按值捕获类型快照；
    // 外层槽名供对象字段引用、闭包层槽名供 emit 签名）
    let pairs: Vec<(String, Type)> = names.iter().cloned().zip(param_tys.clone()).collect();
    let (body_hir, body_ty, captures, capture_tys, closure_capture_slots, param_slots) =
        check_closure_body_with_captures(ctx, &pairs, body, span)?;
    // 匿名函数生成（参数名用闭包 fn 层 insert 的槽名）
    let fn_name = emit_closure_fn(
        ctx,
        &closure_capture_slots,
        &capture_tys,
        &param_slots,
        &param_tys,
        body_hir,
        body_ty.clone(),
    );
    // 闭包值构造：聚合对象（每捕获一槽）+ 逐槽写入捕获变量（按值拷贝）
    let cv = format!("__cv_{}", ctx.closure_seq - 1);
    let mut stmts = vec![HirStmt::Let {
        name: cv.clone(),
        init: HirExpr::Alloc {
            slots: captures.len(),
            by_value: false,
        },
        mutable: false,
    }];
    for (i, c) in captures.iter().enumerate() {
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(cv.clone())),
            index: i,
            value: Box::new(HirExpr::Variable(c.clone())),
            ty: field_scalar_of(&capture_tys[i]),
        }));
    }
    let init = HirExpr::Block(Box::new(HirBlock {
        stmts,
        final_expr: Some(HirExpr::Variable(cv)),
    }));
    Ok((
        init,
        Type::Closure {
            captures: capture_tys,
            params: param_tys,
            ret: Box::new(body_ty),
            fn_name,
        },
    ))
}

/// 闭包值对象调用：`f(args)`（f 的类型为 [`Type::Closure`]）。
///
/// 展开为 `__closure_N(FieldGet(f, 0, cap0_ty)..., 实参...)`：从闭包值
/// 聚合对象读取捕获字段，与实参一起传给闭包匿名函数。
fn check_closure_value_call(
    ctx: &mut TypeContext,
    name: &str,
    closure_ty: &Type,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let Type::Closure { captures, params, ret, fn_name } = closure_ty else {
        unreachable!("check_closure_value_call 仅接受闭包值类型");
    };
    // 未固化延迟闭包（非注解绑定 `let f = |x| ..; f(..)`）：
    // 绑定处参数类型未知，首次调用点由实参类型推断参数类型后固化。
    if fn_name.is_empty() {
        return check_deferred_closure_call(ctx, name, args, span);
    }
    if args.len() != params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("<闭包值 {name}>"),
            expected: params.len(),
            found: args.len(),
            span,
        });
    }
    // 实参检查（String 字面量升级语义与 IIFE 一致）
    let mut arg_hirs = Vec::with_capacity(args.len());
    for (i, a) in args.iter().enumerate() {
        let (h, t) = infer_expr(ctx, a)?;
        let expected = &params[i];
        let (h, t) = if matches!(t, Type::Str) && !matches!(expected, Type::Str) {
            let (h2, t2) = check_string_from(ctx, std::slice::from_ref(a), a.span)?;
            (h2, t2)
        } else {
            (h, t)
        };
        if !t.compatible_with(expected) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: format!("<闭包值 {name}>"),
                index: i,
                expected: expected.to_string(),
                found: t.to_string(),
                span,
            });
        }
        arg_hirs.push(h);
    }
    // 捕获字段读取 + 实参组装
    let mut call_args: Vec<HirExpr> = captures
        .iter()
        .enumerate()
        .map(|(i, cap_ty)| HirExpr::FieldGet {
            base: Box::new(HirExpr::Variable(name.to_string())),
            index: i,
            ty: field_scalar_of(cap_ty),
        })
        .collect();
    call_args.extend(arg_hirs);
    Ok((
        HirExpr::Call {
            callee: fn_name.to_string(),
            args: call_args,
        },
        (**ret).clone(),
    ))
}

/// 无捕获闭包值 → 函数指针（H5 补全）。
///
/// 零捕获闭包值对象在调用点展开为空字段读取，与 fn 指针等价；当目标
/// 位置为 `Type::Fn`（fn 形参 / 函数返回类型）且签名匹配时，降级为
/// `HirExpr::FnPtr` + `Type::Fn`（复用 H1 函数指针的 MIR/LIR 路径）。
///
/// 有捕获闭包值（`captures` 非空）与未固化延迟闭包（`fn_name` 为空）
/// 不降级：前者捕获对象不跨函数边界，后者匿名函数尚未生成。
pub(crate) fn try_closure_value_as_fn(ty: &Type) -> Option<(HirExpr, Type)> {
    let Type::Closure { captures, params, ret, fn_name } = ty else {
        return None;
    };
    if !captures.is_empty() || fn_name.is_empty() {
        return None;
    }
    let sig = FnSignature {
        params: params.clone(),
        return_type: (**ret).clone(),
    };
    Some((
        HirExpr::FnPtr(fn_name.clone()),
        Type::Fn(Box::new(sig)),
    ))
}

/// 未固化延迟闭包调用（H5 补全）：`let f = |x| body; f(41);`。
///
/// 绑定处（`check_deferred_closure_binding`）参数类型未知，仅记录 AST；
/// 首次调用点由**实参类型**推断闭包参数类型，随后走与闭包值对象绑定
/// 相同的流程：迭代检查闭包体 + 收集捕获 → 生成匿名函数 `__closure_N`
/// → 捕获聚合对象。捕获对象构造内联到调用点块，并把真实对象重新绑定
/// 到变量名（覆盖绑定处占位 `Alloc{slots:0}`），后续 `f(args)` 按常规
/// 闭包值调用路径读取 `f` 的捕获槽。固化的完整类型回写 `variables`。
///
/// MVP 限制：闭包体内捕获其它未固化延迟闭包不支持（捕获类型为未固化
/// 占位，匿名函数生成时槽类型失真）。
fn check_deferred_closure_call(
    ctx: &mut TypeContext,
    name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let Some(idx) = ctx
        .deferred_closures
        .iter()
        .position(|d| d.var_name == name)
    else {
        return Err(TypeError::Unsupported {
            what: format!("闭包值 `{name}` 的延迟绑定记录不存在（内部错误）"),
            span,
        });
    };
    let binding = ctx.deferred_closures.remove(idx);
    let ExprKind::Closure { params, param_types, body, capture: _ } = &*binding.closure.kind else {
        unreachable!("延迟闭包绑定仅接受闭包表达式");
    };
    if params.len() != args.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("<闭包值 {name}>"),
            expected: params.len(),
            found: args.len(),
            span,
        });
    }
    // 闭包参数名（Ident / Wildcard）
    let mut names = Vec::with_capacity(params.len());
    for (i, p) in params.iter().enumerate() {
        match p {
            AstPattern::Ident(n) => names.push(n.clone()),
            AstPattern::Wildcard => names.push(format!("__arg{i}")),
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（仅支持简单标识符参数）"),
                    span,
                })
            }
        }
    }
    // 实参检查（外层环境）。参数类型确定规则（半注解语义）：
    // - 有类型注解的参数：用注解类型，实参须与之兼容（`|x: i64, y|` + `f("s", 2)` 报错）；
    // - 无类型注解的参数：由实参推断（String 字面量升级语义与 IIFE 一致）。
    let mut arg_hirs = Vec::with_capacity(args.len());
    let mut arg_tys = Vec::with_capacity(args.len());
    for (i, a) in args.iter().enumerate() {
        let (h, t) = infer_expr(ctx, a)?;
        let (h, t) = if matches!(t, Type::Str) {
            check_string_from(ctx, std::slice::from_ref(a), a.span)?
        } else {
            (h, t)
        };
        arg_hirs.push(h);
        if let Some(anno) = &param_types[i] {
            let at = resolve_ast_type(ctx, anno, a.span)?;
            if !t.compatible_with(&at) {
                return Err(TypeError::ArgumentTypeMismatch {
                    name: format!("<闭包值 {name}>"),
                    index: i,
                    expected: at.to_string(),
                    found: t.to_string(),
                    span: a.span,
                });
            }
            arg_tys.push(at);
        } else {
            arg_tys.push(t);
        }
    }
    // 迭代检查闭包体 + 收集捕获（按值捕获类型快照；错误定位到绑定处闭包体；
    // 外层槽名供对象字段引用、闭包层槽名供 emit 签名）
    let pairs: Vec<(String, Type)> = names.iter().cloned().zip(arg_tys.clone()).collect();
    let (body_hir, body_ty, captures, capture_tys, closure_capture_slots, param_slots) =
        check_closure_body_with_captures(ctx, &pairs, body, binding.span)?;
    // 匿名函数生成（参数名用闭包 fn 层 insert 的槽名）
    let fn_name = emit_closure_fn(
        ctx,
        &closure_capture_slots,
        &capture_tys,
        &param_slots,
        &arg_tys,
        body_hir,
        body_ty.clone(),
    );
    // 捕获聚合对象构造（内联到调用点块）+ 真实对象重新绑定到变量名
    let cv = format!("__cv_{}", ctx.closure_seq - 1);
    let mut stmts = vec![HirStmt::Let {
        name: cv.clone(),
        init: HirExpr::Alloc {
            slots: captures.len(),
            by_value: false,
        },
        mutable: false,
    }];
    for (i, c) in captures.iter().enumerate() {
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(cv.clone())),
            index: i,
            value: Box::new(HirExpr::Variable(c.clone())),
            ty: field_scalar_of(&capture_tys[i]),
        }));
    }
    // 覆盖绑定处占位（`Alloc{slots:0}`）：后续 `f(args)` 常规路径读 f 捕获槽。
    // U1：Let 绑定名用绑定处 insert 的存储槽名（遮蔽时 mangle），与引用一致。
    let slot_name = ctx
        .resolve_variable(&name)
        .map(|(s, _)| s.to_string())
        .unwrap_or_else(|| name.to_string());
    stmts.push(HirStmt::Let {
        name: slot_name.clone(),
        init: HirExpr::Variable(cv),
        mutable: false,
    });
    // 调用：捕获字段读取（base 为槽名 f）+ 实参
    let mut call_args: Vec<HirExpr> = captures
        .iter()
        .enumerate()
        .map(|(i, _)| HirExpr::FieldGet {
            base: Box::new(HirExpr::Variable(slot_name.clone())),
            index: i,
            ty: field_scalar_of(&capture_tys[i]),
        })
        .collect();
    call_args.extend(arg_hirs);
    let call = HirExpr::Call {
        callee: fn_name.clone(),
        args: call_args,
    };
    // 固化变量类型（后续调用按常规闭包值调用路径检查）
    ctx.insert_variable(
        name.to_string(),
        Type::Closure {
            captures: capture_tys,
            params: arg_tys,
            ret: Box::new(body_ty.clone()),
            fn_name,
        },
    );
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(call),
        })),
        body_ty,
    ))
}

/// 未固化延迟闭包值在 **fn 签名上下文** 中固化（H5 补全）：
/// `let f = |x| x + 1; apply(f, 41);` 与 `fn make() -> fn(..) { let f = |x| ..; f }`。
///
/// 与 [`check_deferred_closure_call`]（由调用点实参类型推断参数类型）不同，
/// 此处闭包参数类型由目标 fn 签名给出。固化后若闭包**捕获非空**报
/// `Unsupported`（捕获闭包值不能跨函数边界，MVP 限制）；成功时返回
/// fn 指针表达式，完整闭包类型回写变量表（后续 `f(..)` 调用走常规
/// 闭包值调用路径）。
pub(crate) fn fix_deferred_closure_with_sig(
    ctx: &mut TypeContext,
    name: &str,
    sig: &FnSignature,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let Some(idx) = ctx
        .deferred_closures
        .iter()
        .position(|d| d.var_name == name)
    else {
        return Err(TypeError::Unsupported {
            what: format!("闭包值 `{name}` 的延迟绑定记录不存在（内部错误）"),
            span,
        });
    };
    let binding = ctx.deferred_closures.remove(idx);
    let ExprKind::Closure { params, param_types, body, capture: _ } = &*binding.closure.kind else {
        unreachable!("延迟闭包绑定仅接受闭包表达式");
    };
    if params.len() != sig.params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("<闭包值 {name}>"),
            expected: sig.params.len(),
            found: params.len(),
            span,
        });
    }
    let mut names = Vec::with_capacity(params.len());
    for (i, p) in params.iter().enumerate() {
        match p {
            AstPattern::Ident(n) => names.push(n.clone()),
            AstPattern::Wildcard => names.push(format!("__arg{i}")),
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（仅支持简单标识符参数）"),
                    span,
                })
            }
        }
    }
    // 参数类型：有注解用注解类型（须与 fn 签名兼容），无注解用签名类型
    let mut param_tys = Vec::with_capacity(sig.params.len());
    for (i, s) in sig.params.iter().enumerate() {
        if let Some(anno) = &param_types[i] {
            let at = resolve_ast_type(ctx, anno, span)?;
            if !at.compatible_with(s) {
                return Err(TypeError::ArgumentTypeMismatch {
                    name: format!("<闭包值 {name}>"),
                    index: i,
                    expected: s.to_string(),
                    found: at.to_string(),
                    span,
                });
            }
            param_tys.push(at);
        } else {
            param_tys.push(s.clone());
        }
    }
    // 迭代检查闭包体 + 收集捕获（参数类型来自 fn 签名 / 注解；
    // 无捕获时闭包层槽名与外层槽名相同，emit 用闭包层槽名）
    let pairs: Vec<(String, Type)> = names.iter().cloned().zip(param_tys.clone()).collect();
    let (body_hir, body_ty, captures, capture_tys, closure_capture_slots, param_slots) =
        check_closure_body_with_captures(ctx, &pairs, body, binding.span)?;
    if !captures.is_empty() {
        return Err(TypeError::Unsupported {
            what: format!(
                "闭包值 `{name}` 捕获 {} 个变量，捕获闭包值不能经 fn 签名传递（MVP 限制：捕获对象不跨函数边界）",
                captures.len()
            ),
            span,
        });
    }
    let fn_name = emit_closure_fn(
        ctx,
        &closure_capture_slots,
        &capture_tys,
        &param_slots,
        &param_tys,
        body_hir,
        body_ty.clone(),
    );
    // 固化变量类型（无捕获 → 完整闭包类型；后续 `f(..)` 调用按常规路径展开）
    ctx.insert_variable(
        name.to_string(),
        Type::Closure {
            captures: vec![],
            params: param_tys,
            ret: Box::new(body_ty),
            fn_name: fn_name.clone(),
        },
    );
    Ok((
        HirExpr::FnPtr(fn_name),
        Type::Fn(Box::new(sig.clone())),
    ))
}

/// 检查通过函数值（函数指针）间接调用：`let f = add; f(1, 2)`。
///
/// 参数 / 返回类型按 [`type_to_extern_name`] 序列化为类型名字符串透传到
/// LIR 层，由 `parse_extern_type` 还原为 `LirType`（与 extern 签名同一约定）。
fn check_indirect_call(
    ctx: &mut TypeContext,
    callee_hir: HirExpr,
    signature: FnSignature,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != signature.params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "<函数指针>".to_string(),
            expected: signature.params.len(),
            found: args.len(),
            span,
        });
    }
    let mut hir_args = Vec::with_capacity(args.len());
    for (i, (arg, param_ty)) in args.iter().zip(&signature.params).enumerate() {
        // H2 无捕获闭包实参：形参为 fn 类型且实参为闭包 → 按预期签名检查
        // （闭包参数无类型注解，无法脱离 fn 上下文推断参数类型）
        let (hir, ty) =
            if matches!(param_ty, Type::Fn(_)) && matches!(&*arg.kind, ExprKind::Closure { .. }) {
                check_closure_expected(ctx, arg, param_ty, arg.span)?
            } else {
                infer_expr(ctx, arg)?
            };
        // Str 值实参 → 非 Str 形参自动升级（函数指针调用 `f("a", "b")`）
        let (hir, ty) = upgrade_str_arg(ctx, hir, ty, param_ty, arg)?;
        // 无捕获闭包值实参 → fn 形参：降级为函数指针（H5 补全，
        // `let f = |x: i64| x + 1; apply(f, 41);`）
        let (hir, ty) = if matches!(param_ty, Type::Fn(_)) {
            try_closure_value_as_fn(&ty).unwrap_or((hir, ty))
        } else {
            (hir, ty)
        };
        if !ty.compatible_with(param_ty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: "<函数指针>".to_string(),
                index: i,
                expected: param_ty.to_string(),
                found: ty.to_string(),
                span: arg.span,
            });
        }
        hir_args.push(hir);
    }
    let param_names = signature.params.iter().map(type_to_extern_name).collect();
    Ok((
        HirExpr::CallIndirect {
            callee: Box::new(callee_hir),
            args: hir_args,
            param_names,
            ret_name: type_to_extern_name(&signature.return_type),
        },
        signature.return_type,
    ))
}

/// H2 无捕获闭包：`|x, y| expr` 按预期 fn 签名检查，desugar 为匿名函数 + 函数指针。
///
/// 零运行时开销：匿名函数注册进 `fn_signatures` 并作为普通 `HirItem::Fn` 注入
/// 全局 items（与泛型实例化同一机制），闭包表达式求值为复用 H1 的 `HirExpr::FnPtr`
/// 函数指针（codegen `i8*` 槽 + 按签名 `bitcast` + 间接 `call`）。
///
/// MVP 约束：
/// 1. 无捕获：闭包体内仅允许引用参数与字面量/内建/模块常量；引用外部变量
///    （包括外层函数参数与局部 let）报 Unsupported（捕获闭包 H3 规划）。
/// 2. 闭包参数无类型注解，参数类型取预期 fn 签名的参数类型（需 fn 类型上下文，
///    如作为 fn 形参实参、`let f: fn(..) = |..| ..` 注解绑定）。
/// 3. 参数模式仅支持简单标识符（`|x, y|`）与 `_`；不支持返回闭包的函数
///    （`fn make() -> fn(..) { |x| .. }`，可先 `let f: fn(..) = |x| ..; f` 转接）。
pub(crate) fn check_closure_expected(
    ctx: &mut TypeContext,
    closure: &AstExpr,
    expected: &Type,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let ExprKind::Closure { params, param_types: _, body, capture: _ } = &*closure.kind else {
        unreachable!("check_closure_expected 仅接受闭包表达式");
    };
    let FnSignature {
        params: sig_params,
        return_type,
    } = match expected {
        Type::Fn(sig) => (**sig).clone(),
        _ => {
            return Err(TypeError::Unsupported {
                what: "闭包需要 fn 类型上下文（H2 无捕获闭包：用作 fn 形参实参，或 `let f: fn(..) = |..| ..` 注解绑定）"
                    .to_string(),
                span,
            })
        }
    };
    if params.len() != sig_params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "<闭包>".to_string(),
            expected: sig_params.len(),
            found: params.len(),
            span,
        });
    }
    // 闭包参数名（AstPattern::Ident / Wildcard；其余模式 MVP 不支持）
    let mut names = Vec::with_capacity(params.len());
    for (i, p) in params.iter().enumerate() {
        match p {
            AstPattern::Ident(n) => names.push(n.clone()),
            AstPattern::Wildcard => names.push(format!("__arg{i}")),
            other => {
                return Err(TypeError::Unsupported {
                    what: format!("闭包参数模式 `{other:?}`（H2 仅支持简单标识符参数）"),
                    span,
                })
            }
        }
    }
    // 匿名函数名：全局唯一（`__closure_` 前缀不会与用户符号冲突）
    let name = format!("__closure_{}", ctx.closure_seq);
    ctx.closure_seq += 1;

    // 闭包体检查：函数边界作用域（隔离，body 引用外部变量将报 UndefinedVariable
    // → 转为捕获闭包 Unsupported，H3 规划）。
    ctx.push_scope(true);
    for (nm, ty) in names.iter().zip(sig_params.clone()) {
        ctx.insert_variable(nm.clone(), ty);
    }
    let body_result = infer_expr(ctx, body);
    // 无论成败都弹出作用域（调用方变量环境不得泄漏）
    ctx.pop_scope();

    let (body_hir, body_ty) = match body_result {
        Ok(v) => v,
        Err(TypeError::UndefinedVariable { name, span }) => {
            return Err(TypeError::Unsupported {
                what: format!(
                    "闭包捕获外部变量 `{name}`（捕获闭包 H3 规划；H2 无捕获闭包仅可用参数与字面量）"
                ),
                span,
            })
        }
        Err(e) => return Err(e),
    };
    // 返回类型兼容性：body 尾部表达式须兼容预期返回类型
    if body_ty != Type::Never && !body_ty.compatible_with(&return_type) {
        return Err(TypeError::WrongType {
            expected: return_type.to_string(),
            found: body_ty.to_string(),
            span: body.span,
        });
    }

    // 注册匿名函数签名 + HIR 函数项（注入全局 items，MIR/LIR/codegen 与普通函数同路径）
    ctx.insert_fn_signature(
        name.clone(),
        FnSignature {
            params: sig_params.clone(),
            return_type: return_type.clone(),
        },
    );
    ctx.mono_items.push(HirItem {
        name: name.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params: names
                .iter()
                .map(|n| HirParam { name: n.clone() })
                .collect(),
            body: Some(HirBlock {
                stmts: vec![],
                final_expr: Some(body_hir),
            }),
            is_extern: false,
            extern_sig: None,
        }),
    });
    Ok((
        HirExpr::FnPtr(name),
        Type::Fn(Box::new(FnSignature {
            params: sig_params,
            return_type,
        })),
    ))
}

/// 将调用名解析为完整符号名（use 导入别名 / 模块路径 → 目标符号）。
///
/// 裸名解析优先级（同名遮蔽场景）：
/// 1. 模块内函数引用：优先绑定当前模块内定义（`prefix::name`），
///    避免子模块内部裸名调用被全局同名函数劫持；
/// 2. 顶层用户代码：被遮蔽的用户声明（`read@shadow<N>`）优先——
///    std 预置根函数被用户顶层函数遮蔽时原名保留在 `fn_signatures` 中
///    （std 模块内部裸名调用仍绑定 std 版本），用户顶层裸名经
///    `fn_shadow_of` 绑定自身版本；
/// 3. 其余按裸名 / use 导入别名解析。
fn resolve_callable(ctx: &TypeContext, name: &str) -> String {
    // `r#` 原始标识符：显式引用根命名空间（如 fs 模块内 `r#rename(...)`
    // 引用根 extern `rename`，不被 `fs::rename` 遮蔽）。去前缀后跳过
    // 模块内优先 / 用户遮蔽，直接绑定根命名空间符号（注册名已归一化）。
    if let Some(base) = name.strip_prefix("r#") {
        return base.to_string();
    }
    // 模块内函数引用：优先绑定当前模块内定义（`prefix::name`），
    // 避免子模块内部裸名调用被全局同名函数（含用户顶层覆盖）劫持。
    // 例：std `io.rl` 内部 `read(0, tmp, 256)` 必须绑定 `io::read`，
    // 用户顶层 `fn read(p: &i64)` 只影响用户自己的裸名调用。
    if !name.contains("::") && !ctx.module_prefix.is_empty() {
        let full = format!("{}::{}", ctx.module_prefix, name);
        if ctx.fn_signatures.contains_key(&full) {
            return full;
        }
    }
    // 顶层用户代码：被遮蔽的用户声明优先（std 原名保留供模块内部绑定）
    if !name.contains("::") && ctx.module_prefix.is_empty() {
        if let Some(m) = ctx.fn_shadow_of.get(name) {
            return m.clone();
        }
    }
    if ctx.fn_signatures.contains_key(name) {
        return name.to_string();
    }
    // 兜底：模块前缀 + 裸名（模块内自由函数互相引用）
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
            // `Self::Item`：关联类型引用（U2）——impl 收集时查当前 assoc_types
            // 映射替换为具体类型；trait 声明收集时（无 impl 上下文）退化为
            // 占位 `Type::Generic("Self::Item")`（仅作记录，不参与实例化替换）。
            if let Some(member) = name.strip_prefix("Self::") {
                if let Some(t) = ctx.assoc_types.get(member) {
                    return Ok(t.clone());
                }
                return Ok(Type::Generic(name.clone()));
            }
            // W4 补全：泛型参数关联类型投影 `F::Output`（base 为当前泛型参数时）。
            // 产出 `Type::AssocProjection`；实例化时 base 已替换为具体类型则立即
            // 求值（查该类型的 trait impl 的关联类型），否则保留投影待实例化替换。
            if args.is_empty() {
                if let Some((base, member)) = name.rsplit_once("::") {
                    // base 为泛型参数：模板收集期 type_params 含 base；实例化期
                    // `cloned.generics` 被清空（type_params=[]）但 generic_subst 含 base
                    //（instantiate_generic_fn 克隆后解析签名），两者任一命中即视为投影。
                    if ctx.type_params.iter().any(|p| p == base)
                        || ctx.generic_subst.contains_key(base)
                    {
                        return resolve_assoc_projection(ctx, base, member, span);
                    }
                }
            }
            // `str`：字符串类型关键字（`&str` 引用切片类型的一部分；G2）
            if name == "str" && args.is_empty() {
                return Ok(Type::Str);
            }
            // 带泛型实参的类型也解析完整名（use 别名 / 模块前缀），与 args 为空时
            // 一致（S1 修复：`Poll<i64>` 注解与 `Poll::Ready` 构造的路径一致性——
            // 裸名不做别名展开会产生 `Poll` vs `future::Poll` 的错配）。
            if args.is_empty() {
                ctx.resolve_named_type(name, span)
            } else {
                // 带泛型实参的类型也解析完整名（use 别名 / 模块前缀），与 args 为空
                // 时一致（S1 修复：`Poll<i64>` 注解与 `Poll::Ready` 构造的路径一致性——
                // 裸名不做别名展开会产生 `Poll` vs `future::Poll` 的错配）。
                let full = ctx
                    .resolve_full_name(name)
                    .unwrap_or_else(|| name.to_string());
                let mut resolved = Vec::with_capacity(args.len());
                for a in args {
                    resolved.push(resolve_ast_type(ctx, a, span)?);
                }
                Ok(Type::Named(full, resolved))
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
        AstType::RawPtr(inner, is_mut) => {
            let inner = resolve_ast_type(ctx, inner, span)?;
            Ok(Type::RawPtr(Box::new(inner), *is_mut))
        }
        // trait 对象（H4）：`dyn Trait` → `Type::Dyn(完整 trait 名)`。
        // 布局为 2 槽胖指针（数据指针 + vtable 指针），转换与调用见
        // `coerce_to_dyn` / `check_method_call` 的 Dyn 分支。
        AstType::Dyn(name) => {
            let full = if ctx.trait_defs.contains_key(name) {
                name.clone()
            } else {
                // use 导入别名（`use shape::Shape` 后 `dyn Shape`）
                ctx.use_aliases
                    .get(name)
                    .filter(|f| ctx.trait_defs.contains_key(*f))
                    .cloned()
                    .unwrap_or_else(|| name.clone())
            };
            if !ctx.trait_defs.contains_key(&full) {
                return Err(TypeError::UndefinedType {
                    name: name.clone(),
                    span,
                });
            }
            Ok(Type::Dyn(full))
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
        AstType::Fn(params, ret) => {
            let params = params
                .iter()
                .map(|p| resolve_ast_type(ctx, p, span))
                .collect::<Result<Vec<_>, _>>()?;
            let ret = resolve_ast_type(ctx, ret, span)?;
            Ok(Type::Fn(Box::new(FnSignature {
                params,
                return_type: ret,
            })))
        }
        AstType::Infer => Ok(Type::Infer),
    }
}

/// 解析泛型参数关联类型投影 `F::Output`（W4 补全）。
///
/// `base` 为当前作用域的泛型参数名。若已实例化（`ctx.generic_subst` 有 `base` 的
/// 具体类型），立即求值为该类型实现的 trait 的关联类型；否则保留
/// `Type::AssocProjection { base: Generic(base), assoc }`，待实例化时替换求值。
fn resolve_assoc_projection(
    ctx: &TypeContext,
    base: &str,
    member: &str,
    _span: Span,
) -> Result<Type, TypeError> {
    if let Some(concrete) = ctx.generic_subst.get(base) {
        if let Some(t) = eval_assoc_projection(ctx, concrete, member) {
            return Ok(t);
        }
        // base 已实例化但找不到该类型的关联类型实现：保留投影（防御性，
        // 通常实例化前必能求值）。
        return Ok(Type::AssocProjection {
            base: Box::new(concrete.clone()),
            assoc: member.to_string(),
        });
    }
    Ok(Type::AssocProjection {
        base: Box::new(Type::Generic(base.to_string())),
        assoc: member.to_string(),
    })
}

/// 求值具体类型 `base` 的关联类型 `member`（`base::member`）。
///
/// 遍历 impl 表，找目标类型兼容 `base` 且关联类型含 `member` 的 impl，返回其
/// 关联类型；找不到返回 `None`（调用方保留投影或报错）。MVP 取第一个匹配
/// impl（同名关联类型多 impl 歧义未处理，符合宽松语义）。
fn eval_assoc_projection(ctx: &TypeContext, base: &Type, member: &str) -> Option<Type> {
    // U8 补全：按 self_type 名匹配（忽略泛型实参——泛型 impl `impl<T> Future
    // for GenOut<T>` 的 self_type 实参为 `Generic("T")`，与具体实例实参 `String`
    // 不相等，`compatible_with` 会误判不匹配），再用 base 具体实参替换 assoc_types
    // 里的 `Generic("T")` 占位求值。
    let Type::Named(base_name, base_args) = base else {
        // base 非 Named（引用等）：退回按名 + 宽松匹配
        for imp in &ctx.impl_defs {
            if imp.self_type.compatible_with(base) {
                for (name, ty) in &imp.assoc_types {
                    if name == member {
                        return Some(ty.clone());
                    }
                }
            }
        }
        return None;
    };
    for imp in &ctx.impl_defs {
        let Type::Named(impl_name, self_args) = &imp.self_type else {
            continue;
        };
        if impl_name != base_name {
            continue;
        }
        // 绑泛型参数：self_args[i] = Generic(tp) → base_args[i]
        let mut subst: HashMap<String, Type> = HashMap::new();
        for (i, tp) in imp.type_params.iter().enumerate() {
            if i < self_args.len()
                && i < base_args.len()
                && self_args[i] == Type::Generic(tp.clone())
            {
                subst.insert(tp.clone(), base_args[i].clone());
            }
        }
        for (name, ty) in &imp.assoc_types {
            if name == member {
                let resolved = if subst.is_empty() {
                    ty.clone()
                } else {
                    substitute(ty, &subst)
                };
                return Some(resolved);
            }
        }
    }
    None
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
    type_args: &[AstType],
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

    // U8：泛型结构体构造——解析 `Pair<i64>` 的类型实参，构造实例类型
    // `Named(struct_name, resolved_args)`；字段类型用实参替换泛型参数后校验。
    let mut resolved_args: Vec<Type> = Vec::new();
    for ta in type_args {
        resolved_args.push(resolve_ast_type(ctx, ta, span)?);
    }
    if !resolved_args.is_empty() && resolved_args.len() != def.type_params.len() {
        return Err(TypeError::GenericArityMismatch {
            name: struct_name.clone(),
            expected: def.type_params.len(),
            found: resolved_args.len(),
            span,
        });
    }
    let mut field_subst: HashMap<String, Type> = HashMap::new();
    for (tp, arg) in def.type_params.iter().zip(&resolved_args) {
        field_subst.insert(tp.clone(), arg.clone());
    }

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
    // 标量结构体（≤2 槽）按值分配（栈槽，免 calloc）；槽区均为 8 字节槽，
    // 兼容栈上 `[2 x i64]` 存储。
    //
    // 按值仅允许"字段全为标量槽"的结构体：按值对象以 `[2 x i64]` 栈槽存储，
    // 当它作为另一聚合（enum/struct）的字段/payload 时以"对象地址"语义写入
    // 外层 Ptr 槽——若其含聚合字段，写入的即内部对象地址（栈地址），函数返回/
    // 跨调用后悬垂（如 `Result<SocketAddr, _>::Ok(sa)`，`SocketAddr` 含
    // String 字段 → tcp_addr SIGSEGV）。含聚合/引用/裸指针/泛型字段者按值
    // 构造一律不安全，退化回堆分配。
    let struct_by_value = def.fields.len() <= 2
        && def.fields.iter().all(|(_, fty)| field_is_scalar_slot(fty));
    let base = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: base.clone(),
        init: HirExpr::Alloc {
            slots: def.fields.len(),
            by_value: struct_by_value,
        },
        mutable: false,
    }];
    for (i, (fname, fty)) in def.fields.iter().enumerate() {
        // U8：字段类型用泛型实参替换（`Pair<i64>` 的字段 `T` → `i64`）后校验
        let field_fty = substitute(fty, &field_subst);
        let init = fields.iter().find(|(n, _)| n == fname).map(|(_, e)| e);
        let Some(init) = init else {
            // 递归 struct（desugar 生成的 `__Fut_f` 递归环）构造时允许省略
            // `Box<__Fut_>` 递归槽：缺失 = 空指针占位，首次 poll 求值前不 deref。
            // 仅放行堆指针字段，普通字段缺失仍报 MissingField。
            if matches!(&field_fty, Type::Named(n, _) if n == "Box") {
                stmts.push(HirStmt::Semi(HirExpr::FieldSet {
                    base: Box::new(HirExpr::Variable(base.clone())),
                    index: i,
                    // 空指针占位（槽值 0）；首次 poll 被真实子 future Box 覆盖
                    value: Box::new(HirExpr::IntLiteral(0)),
                    ty: field_scalar_of(&field_fty),
                }));
                continue;
            }
            return Err(TypeError::MissingField {
                struct_name: struct_name.clone(),
                field: fname.clone(),
                span,
            });
        };
        let (hir, arg_ty) = infer_expr(ctx, init)?;
        if !arg_ty.compatible_with(&field_fty) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: format!("struct `{struct_name}` field `{fname}`"),
                index: i,
                expected: field_fty.to_string(),
                found: arg_ty.to_string(),
                span: init.span,
            });
        }
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: i,
            value: Box::new(hir),
            ty: field_scalar_of(&field_fty),
        }));
    }

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named(struct_name, resolved_args),
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
            init: HirExpr::Alloc {
            slots: 3,
            by_value: false,
        },
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
            init: HirExpr::Alloc {
            slots: 3,
            by_value: false,
        },
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
/// 与 `HashMap` 结构体字段顺序一致（7 槽）：槽 0 = keys 指针（`[K; 0]`）、
/// 槽 1 = vals 指针（`[V; 0]`）、槽 2 = states 指针（`[i64; 0]`）、
/// 槽 3 = len = 0、槽 4 = used = 0、槽 5 = cap、槽 6 = dist 指针（`[i64; 0]`，
/// Robin Hood 键探测距离）。四个动态数组均经 `alloc_array` 分配（8 字节步长），
/// 默认容量 8。返回 `HashMap<Infer, Infer>`，类型参数由上下文
/// （如 `let m: HashMap<i64, i64> = ...` 注解）统一。
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
    // with_capacity 的 cap 经标准库 `next_pow2` 规整为 2 的幂：
    // HashMap 的位掩码定位（hash & (cap-1)）与翻倍扩容依赖 cap 恒为 2 的幂，
    // 用户传入任意正整数（如 10）会破坏该不变量。
    let cap_hir = if method == "with_capacity" {
        HirExpr::Call {
            callee: "next_pow2".to_string(),
            args: vec![cap_hir],
        }
    } else {
        cap_hir
    };

    let keys_tmp = ctx.fresh_temp();
    let vals_tmp = ctx.fresh_temp();
    let states_tmp = ctx.fresh_temp();
    let dist_tmp = ctx.fresh_temp();
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
            name: dist_tmp.clone(),
            init: HirExpr::Call {
                callee: "alloc_array".to_string(),
                args: vec![cap_hir.clone()],
            },
            mutable: false,
        },
        HirStmt::Let {
            name: base.clone(),
            init: HirExpr::Alloc {
            slots: 7,
            by_value: false,
        },
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
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 6,
            value: Box::new(HirExpr::Variable(dist_tmp)),
            ty: FieldScalar::Ptr,
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

/// `Iter::new(data, len)` / `IterMut::new(data, cur, len)`：V1 瘦指针迭代器构造
/// （2026-08，编译器直接展开，与 Vec/HashMap 构造同一模式）。
///
/// - `Iter`（2 槽）：槽 0 = data（`*const T` 首元素裸指针，Ptr 标量）、槽 1 =
///   len（剩余元素数，Int）——按值栈分配（`by_value`，与 2 字段 struct 一致）。
/// - `IterMut`（3 槽）：槽 0 = data、槽 1 = cur（写回目标 `*mut T`）、槽 2 =
///   len——堆分配（`by_value: false`，与 3 字段 struct 的 StructCtor 规则一致）。
///
/// 元素类型 T 从 data 参数的裸指针内项反推；返回 `Named("Iter"/"IterMut", [T])`，
/// 函数返回类型（`-> Iter<T>`）经 `compatible_with` 统一。
fn check_iter_construct(
    ctx: &mut TypeContext,
    name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let expected = if name == "Iter" { 2 } else { 3 };
    if args.len() != expected {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{name}::new"),
            expected,
            found: args.len(),
            span,
        });
    }
    // data：裸指针（*const T / *mut T），从内项反推元素类型 T
    let (data_hir, data_ty) = infer_expr(ctx, &args[0])?;
    let Type::RawPtr(elem, _) = data_ty else {
        return Err(TypeError::WrongType {
            expected: "裸指针（*const T / *mut T）".to_string(),
            found: data_ty.to_string(),
            span: args[0].span,
        });
    };
    // cur（仅 IterMut）：裸指针
    let cur_hir = if name == "IterMut" {
        let (cur_hir, cur_ty) = infer_expr(ctx, &args[1])?;
        if !matches!(cur_ty, Type::RawPtr(..)) {
            return Err(TypeError::WrongType {
                expected: "裸指针（*mut T）".to_string(),
                found: cur_ty.to_string(),
                span: args[1].span,
            });
        }
        cur_hir
    } else {
        HirExpr::IntLiteral(0)
    };
    // len：整数（剩余元素数）
    let (len_hir, len_ty) = infer_expr(ctx, &args[expected - 1])?;
    if !len_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: len_ty.to_string(),
            span: args[expected - 1].span,
        });
    }

    let base = ctx.fresh_temp();
    let mut stmts = vec![
        HirStmt::Let {
            name: base.clone(),
            init: HirExpr::Alloc {
                slots: expected,
                by_value: name == "Iter",
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 0,
            value: Box::new(data_hir),
            ty: FieldScalar::Ptr,
        }),
    ];
    // IterMut 额外写 cur 槽（槽 1）；Iter 直接由 len 写槽 1
    if name == "IterMut" {
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(base.clone())),
            index: 1,
            value: Box::new(cur_hir),
            ty: FieldScalar::Ptr,
        }));
    }
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(base.clone())),
        index: expected - 1,
        value: Box::new(len_hir),
        ty: FieldScalar::Int,
    }));

    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(base)),
        })),
        Type::Named(name.to_string(), vec![*elem]),
    ))
}

/// `String::from(...)`：把字符串内容拷贝到动态字节缓冲。
///
/// 支持两类参数（G2，消除 §13 约束 4 的"长度表达未实现"）：
/// - 字符串字面量（或绑定字面量的变量）：编译期已知字节长度，展开为
///   `data = alloc_bytes(len)` + `copy_bytes(data, s, len)` + 三槽构造（cap = len）；
/// - 运行期 `String`：desugar 为 `s.clone()`（深拷贝，标准库逐字节拷贝，
///   运行期从 len 槽读取长度——不再要求编译期已知内容）。
pub(crate) fn check_string_from(
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
    // 运行期 String → 深拷贝：`String::from(s)` ≡ `s.clone()`。
    // clone 是 `&self` 方法（G1 方法调用自动剥引用层），经方法实例化注册
    // 函数体后复用标准库实现；返回独立缓冲，原串不受影响。
    if comparison::is_string_type(ctx, &s_ty) {
        let impl_def = ctx
            .find_impl_for_method(&s_ty, "clone")
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
        let fn_name = instantiate_impl_method(ctx, &impl_def, &method_def, &HashMap::new(), span)?;
        return Ok((
            HirExpr::Call {
                callee: fn_name,
                args: vec![s_hir],
            },
            s_ty,
        ));
    }
    // `&str`（String 对象的只读借用视图）→ 深拷贝：
    // 运行期读 data/len 槽 → alloc_bytes(len+1) → copy_bytes → 三槽构造（cap = len）。
    if let Type::Ref(inner, _) = &s_ty {
        if matches!(**inner, Type::Str) {
        let len_tmp = ctx.fresh_temp();
        let data_tmp = ctx.fresh_temp();
        let base = ctx.fresh_temp();
        let len_plus1 = |var: String| {
            HirExpr::Binary(
                HirBinaryOp::Add,
                Box::new(HirExpr::Variable(var)),
                Box::new(HirExpr::IntLiteral(1)),
            )
        };
        let stmts = vec![
            HirStmt::Let {
                name: len_tmp.clone(),
                init: HirExpr::FieldGet {
                    base: Box::new(s_hir.clone()),
                    index: 1,
                    ty: FieldScalar::Int,
                },
                mutable: false,
            },
            HirStmt::Let {
                name: data_tmp.clone(),
                init: HirExpr::FieldGet {
                    base: Box::new(s_hir),
                    index: 0,
                    ty: FieldScalar::Ptr,
                },
                mutable: false,
            },
            HirStmt::Let {
                name: base.clone(),
                init: HirExpr::Call {
                    callee: "alloc_bytes".to_string(),
                    args: vec![len_plus1(len_tmp.clone())],
                },
                mutable: false,
            },
            HirStmt::Semi(HirExpr::Call {
                callee: "copy_bytes".to_string(),
                args: vec![
                    HirExpr::Variable(base.clone()),
                    HirExpr::Variable(data_tmp.clone()),
                    len_plus1(len_tmp.clone()),
                ],
            }),
            HirStmt::Semi(HirExpr::FieldSet {
                base: Box::new(HirExpr::Variable(base.clone())),
                index: 0,
                value: Box::new(HirExpr::Variable(data_tmp)),
                ty: FieldScalar::Ptr,
            }),
            HirStmt::Semi(HirExpr::FieldSet {
                base: Box::new(HirExpr::Variable(base.clone())),
                index: 1,
                value: Box::new(HirExpr::Variable(len_tmp.clone())),
                ty: FieldScalar::Int,
            }),
            HirStmt::Semi(HirExpr::FieldSet {
                base: Box::new(HirExpr::Variable(base.clone())),
                index: 2,
                value: Box::new(HirExpr::Variable(len_tmp)),
                ty: FieldScalar::Int,
            }),
        ];
        return Ok((
            HirExpr::Block(Box::new(HirBlock {
                stmts,
                final_expr: Some(HirExpr::Variable(base)),
            })),
            Type::Named("String".to_string(), vec![]),
        ));
        }
    }
    // 字面量直用；`let s = "..."` 绑定的变量经 local_inits 表追踪回字面量，
    // 其余非字面量 Str（裸字面量类型）不支持（须先经 String::from/String 变量）
    let s = match &s_hir {
        HirExpr::StringLiteral(s) => Some(s.clone()),
        HirExpr::Variable(name) => match ctx.lookup_local_init(name) {
            Some(HirExpr::StringLiteral(s)) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
    .ok_or_else(|| TypeError::Unsupported {
        what: "String::from 支持字符串字面量（或绑定字面量的变量）与 String 变量；裸 Str 类型不支持"
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
            init: HirExpr::Alloc {
            slots: 3,
            by_value: false,
        },
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

/// Str 值实参自动升级：`String` 形参位置传入字符串字面量（或绑定字面量的
/// 变量）时自动构造 String 对象（与 IIFE / 闭包值实参的升级语义一致），
/// 消除 `f(String::from(".."))` 的书写负担（`f("..")` 直接可用）。
///
/// 升级条件与 IIFE（H3）一致：实参为 `Type::Str` 且形参不是 `Type::Str`
/// （`&str` 形参经 String ↔ `&str` 兼容规则接收升级后的 String 值；
/// `str` 形参保持裸字面量指针直传）。运行期 `str` 值（如函数参数）
/// 无编译期长度信息，`check_string_from` 报 Unsupported。
fn upgrade_str_arg(
    ctx: &mut TypeContext,
    hir: HirExpr,
    ty: Type,
    param_ty: &Type,
    arg: &AstExpr,
) -> Result<(HirExpr, Type), TypeError> {
    if matches!(&ty, Type::Str) && !matches!(param_ty, Type::Str) {
        check_string_from(ctx, std::slice::from_ref(arg), arg.span)
    } else {
        Ok((hir, ty))
    }
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
    // `Box<T>` 接收者：自动剥层后按 `T` 的字段访问（K2）。base 须为堆对象
    // 指针：经槽 0 解出（`box_ptr_hir`），`&Box<T>` 引用求值即对象指针同构
    let base_hir = heap_ptr_hir(base_hir, &base_ty);
    let Type::Named(name, _) = peel_refs_and_heap(&base_ty) else {
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
    // 字段类型经泛型替换（V1 2026-08）：
    // 1) 接收者实例的类型参数（用户代码 `Vec<Infer>.data`：T → Infer；
    //    `Vec<i64>.data`：T → i64）——struct 定义期类型变量（Generic T）
    //    无法直接参与字段访问推断，必须按实例参数替换；
    // 2) 全局 generic_subst（泛型方法实例化 body 检查时 `T` → 具体类型）优先覆盖。
    // 此前仅用全局 generic_subst，用户代码字段访问返回 `[Generic T; 0]`，
    // 赋值/DerefSet 严格兼容检查报 `expected T, found i64` 错配。
    let Type::Named(_, ty_args) = peel_refs_and_heap(&base_ty) else {
        return Err(TypeError::ExpectedStruct {
            found: base_ty.to_string(),
            span,
        });
    };
    let mut subst = ctx.generic_subst.clone();
    for (tp, arg) in def.type_params.iter().zip(ty_args.iter()) {
        subst.insert(tp.clone(), arg.clone());
    }
    let fty_sub = substitute(&fty, &subst);
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
/// `instantiate_impl_method` 注册函数体，复用纯 Rlyeh `substring`）：
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
    // 支持 String / &str / Vec<T> / 数组 [T; N] 四类切片对象：
    // - String → 方法实例化 substring（现有路径）
    // - &str  → 方法实例化 substring（接收者 &self，&str 即 String 对象的借用视图）
    // - Vec<T> → 方法实例化 slice（std 泛型方法，越界 clamp）
    // - 数组 [T; N] → 展开为 Vec 拷贝循环（动态切片，边界 clamp 到 [0, N]）
    let is_vec = matches!(&b_ty, Type::Named(n, _) if n == "Vec");
    let is_arr = matches!(&b_ty, Type::Array(_, _));
    let is_str_view =
        matches!(&b_ty, Type::Ref(inner, _) if matches!(**inner, Type::Str));
    if !comparison::is_string_type(ctx, &b_ty) && !is_str_view && !is_vec && !is_arr {
        return Err(TypeError::Unsupported {
            what: "范围切片（`s[lo..<hi]`）暂仅支持 String / &str / Vec / 数组对象".to_string(),
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
    if comparison::is_string_type(ctx, &b_ty) || is_str_view {
        // String / &str → substring（非泛型，subst 为空；&str 接收者 &self，
        // 方法体对 self.data/self.len 的 FieldGet 经对象指针生效）
        let lookup_ty = if is_str_view {
            Type::Named("String".to_string(), vec![])
        } else {
            b_ty.clone()
        };
        let impl_def = ctx
            .find_impl_for_method(&lookup_ty, "substring")
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
                init: HirExpr::Alloc {
            slots: 3,
            by_value: false,
        },
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

/// 计算类型的对象槽数（用于 `Box::new` 的堆分配大小，单位：8 字节槽）。
///
/// 布局规则与对象分配一致：标量为 1 槽；struct = 字段数（聚合字段各 1
/// 指针槽）；enum = `slot_count`（1 tag + max 字段）；数组 = 元素数（动态
/// 数组长度为 0，按 1 指针槽计）；元组 = 元素数；引用 / 函数指针 / 内嵌
/// Box = 1 指针槽。
fn type_slot_count(ctx: &TypeContext, ty: &Type, span: Span) -> Result<usize, TypeError> {
    match ty {
        Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::I128 | Type::ISize
        | Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::U128 | Type::USize
        | Type::F32 | Type::F64 | Type::Bool | Type::Char => Ok(1),
        Type::Array(_, n) => Ok(if *n > 0 { *n } else { 1 }),
        Type::Tuple(ts) => Ok(ts.len()),
        Type::Named(n, args) => {
            let full = ctx.resolve_full_name(n).unwrap_or_else(|| n.clone());
            if matches!(full.as_str(), "Box" | "Rc" | "Arc" | "Weak" | "Gc") && args.len() == 1 {
                return Ok(1);
            }
            if let Some(def) = ctx.lookup_struct(&full) {
                Ok(def.fields.len())
            } else if let Some(def) = ctx.lookup_enum(&full) {
                Ok(def.slot_count)
            } else {
                Err(TypeError::Unsupported {
                    what: format!("无法确定对象布局（未知类型 `{full}`）"),
                    span,
                })
            }
        }
        Type::Ref(..) | Type::Fn(..) | Type::Closure { .. } => Ok(1),
        _ => Err(TypeError::Unsupported {
            what: format!("该类型不支持堆装箱（`{ty}`）"),
            span,
        }),
    }
}

/// `Box::new(v)`（K2 堆分配装箱）→ 返回 `Box<T>` 对象。
///
/// `Box<T>` 布局：栈上 1 槽（指针），指向堆上 `slot_count(T)` 个 8 字节槽
/// 的连续对象区：
/// - 标量 `T`（i64 / f64 / bool / char 等）：`alloc_bytes(8)` 分配后经
///   `DerefSet` 将值直接写入堆首槽（复用 G1 解引用写链路）；
/// - 聚合 `T`（struct / enum / 数组 / String / Vec 等）：`alloc_bytes(8*n)`
///   分配后经 `array_copy` 整槽区 memcpy（浅拷贝，与 MVP 结构体赋值一致）。
///
/// 返回值 = 1 槽 `Alloc` 对象（槽 0 存堆指针，`FieldScalar::Ptr`），与
/// `&T` 聚合解引用同构：`*b` 标量 load / 聚合指针拷贝，字段访问与方法
/// 调用自动剥 `Box` 层后按 `T` 解析。
fn check_box_new(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "Box::new".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let (v_hir, v_ty) = infer_expr(ctx, &args[0])?;
    let n_slots = type_slot_count(ctx, &v_ty, span)?;
    if n_slots == 0 {
        return Err(TypeError::Unsupported {
            what: "`Box::new(())`：单元类型无法装箱".to_string(),
            span,
        });
    }
    let data = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: data.clone(),
        init: HirExpr::Call {
            callee: "alloc_bytes".to_string(),
            args: vec![HirExpr::IntLiteral(n_slots as i128 * 8)],
        },
        mutable: false,
    }];
    if v_ty.is_numeric() || matches!(v_ty, Type::Bool | Type::Char) {
        // 标量 T：直接写入堆首槽
        stmts.push(HirStmt::Semi(HirExpr::DerefSet {
            base: Box::new(HirExpr::Variable(data.clone())),
            value: Box::new(v_hir),
            ty: field_scalar_of(&v_ty),
        }));
    } else {
        // 聚合 T：整槽区 memcpy（浅拷贝）
        stmts.push(HirStmt::Semi(HirExpr::Call {
            callee: "array_copy".to_string(),
            args: vec![
                HirExpr::Variable(data.clone()),
                v_hir,
                HirExpr::IntLiteral(n_slots as i128),
            ],
        }));
    }
    let box_base = ctx.fresh_temp();
    stmts.push(HirStmt::Let {
        name: box_base.clone(),
        init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
        },
        mutable: false,
    });
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(box_base.clone())),
        index: 0,
        value: Box::new(HirExpr::Variable(data)),
        ty: FieldScalar::Ptr,
    }));
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(box_base)),
        })),
        Type::Named("Box".to_string(), vec![v_ty]),
    ))
}

/// `Rc::new(v)` / `Arc::new(v)`（K3 引用计数装箱）→ 返回 `Rc<T>` / `Arc<T>`。
///
/// 布局：堆 `RcInner` 的 `T` 值区自堆首槽起（槽 0..`n`，`n = slot_count(T)`，
/// 与 `Box<T>` 同构，解引用 / 剥层零差异），尾部两个计数槽：槽 `n` =
/// strong_count、槽 `n + 1` = weak_count；`Rc<T>` 对象栈上 1 槽（Ptr）指向
/// RcInner。分配 `(n + 2)` 个 8 字节槽：先写值区（标量 `DerefSet` / 聚合
/// `array_copy`），再以 `FieldSet`（GEP + store，base 即地址）初始化计数
/// （1 / 0），返回 1 槽 `Alloc`。`Arc` 与 `Rc` 同构（计数槽原子性规划中）。
/// MVP 无自动 drop（计数只增不减，显式释放语义规划）。
fn check_rc_new(
    ctx: &mut TypeContext,
    ty_name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{ty_name}::new"),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let (v_hir, v_ty) = infer_expr(ctx, &args[0])?;
    let n_slots = type_slot_count(ctx, &v_ty, span)?;
    if n_slots == 0 {
        return Err(TypeError::Unsupported {
            what: format!("`{ty_name}::new(())`：单元类型无法装箱"),
            span,
        });
    }
    let inner = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: inner.clone(),
        init: HirExpr::Call {
            callee: "alloc_bytes".to_string(),
            args: vec![HirExpr::IntLiteral((n_slots + 2) as i128 * 8)],
        },
        mutable: false,
    }];
    // `T` 值区自堆首槽起（与 `Box<T>` 同构）
    if v_ty.is_numeric() || matches!(v_ty, Type::Bool | Type::Char) {
        stmts.push(HirStmt::Semi(HirExpr::DerefSet {
            base: Box::new(HirExpr::Variable(inner.clone())),
            value: Box::new(v_hir),
            ty: field_scalar_of(&v_ty),
        }));
    } else {
        stmts.push(HirStmt::Semi(HirExpr::Call {
            callee: "array_copy".to_string(),
            args: vec![
                HirExpr::Variable(inner.clone()),
                v_hir,
                HirExpr::IntLiteral(n_slots as i128),
            ],
        }));
    }
    // 计数槽（FieldSet base 即堆地址，GEP + store）：strong = n、weak = n + 1
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(inner.clone())),
        index: n_slots,
        value: Box::new(HirExpr::IntLiteral(1)),
        ty: FieldScalar::Int,
    }));
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(inner.clone())),
        index: n_slots + 1,
        value: Box::new(HirExpr::IntLiteral(0)),
        ty: FieldScalar::Int,
    }));
    let rc_base = ctx.fresh_temp();
    stmts.push(HirStmt::Let {
        name: rc_base.clone(),
        init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
        },
        mutable: false,
    });
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(rc_base.clone())),
        index: 0,
        value: Box::new(HirExpr::Variable(inner)),
        ty: FieldScalar::Ptr,
    }));
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(rc_base)),
        })),
        Type::Named(ty_name.to_string(), vec![v_ty]),
    ))
}

/// `Gc::new(v)`（K4 追踪 GC 装箱）→ 返回 `Gc<T>`。
///
/// 布局：堆 `GcInner` 的 `T` 值区自堆首槽起（槽 `0..n`，`n = slot_count(T)`，
/// 与 `Box<T>` 完全同构，解引用 / 剥层零差异）；`Gc<T>` 对象栈上 1 槽（Ptr）
/// 指向 GcInner。分配经运行时 `rlyeh_gc_alloc`（`n` 个 8 字节槽，清零 +
/// 注册块表 + 记录当前 epoch），先写值区（标量 `DerefSet` / 聚合 `array_copy`），
/// 返回 1 槽 `Alloc`。标记 / 存活状态存运行时块表（不占对象内存）。GC 周期由
/// `gc_region` 块结束时的 `rlyeh_gc_collect` 触发（epoch 标记-清除，本块存活
/// 对象提升为逃逸 root）。
fn check_gc_new(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "Gc::new".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let (v_hir, v_ty) = infer_expr(ctx, &args[0])?;
    let n_slots = type_slot_count(ctx, &v_ty, span)?;
    if n_slots == 0 {
        return Err(TypeError::Unsupported {
            what: "`Gc::new(())`：单元类型无法装箱".to_string(),
            span,
        });
    }
    let inner = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: inner.clone(),
        init: HirExpr::Call {
            callee: "rlyeh_gc_alloc".to_string(),
            args: vec![HirExpr::IntLiteral(n_slots as i128)],
        },
        mutable: false,
    }];
    // `T` 值区自堆首槽起（与 `Box<T>` 同构）
    if v_ty.is_numeric() || matches!(v_ty, Type::Bool | Type::Char) {
        stmts.push(HirStmt::Semi(HirExpr::DerefSet {
            base: Box::new(HirExpr::Variable(inner.clone())),
            value: Box::new(v_hir),
            ty: field_scalar_of(&v_ty),
        }));
    } else {
        stmts.push(HirStmt::Semi(HirExpr::Call {
            callee: "array_copy".to_string(),
            args: vec![
                HirExpr::Variable(inner.clone()),
                v_hir,
                HirExpr::IntLiteral(n_slots as i128),
            ],
        }));
    }
    let gc_base = ctx.fresh_temp();
    stmts.push(HirStmt::Let {
        name: gc_base.clone(),
        init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
        },
        mutable: false,
    });
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(gc_base.clone())),
        index: 0,
        value: Box::new(HirExpr::Variable(inner)),
        ty: FieldScalar::Ptr,
    }));
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(gc_base)),
        })),
        Type::Named("Gc".to_string(), vec![v_ty]),
    ))
}

/// `Rc<T>` / `Arc<T>` / `Weak<T>` 引用计数内建方法分派（K3）：`clone` /
/// `strong_count` / `weak_count` / `downgrade` / `try_unwrap`（Rc/Arc 接收者）
/// 与 `upgrade`（Weak 接收者）。返回 `None` 表示非引用计数内建，走普通方法解析。
///
/// 须在 receiver 堆指针改写（`heap_ptr_hir`）之前调用：内建需要原始 `Rc<T>`
/// 对象（槽 0 取 RcInner 指针），而非 `T` 值区指针。
fn check_rc_method(
    ctx: &mut TypeContext,
    method: &str,
    recv_ty: &Type,
    recv_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Option<Result<(HirExpr, Type), TypeError>> {
    let base = peel_ref(recv_ty);
    let (wrapper, inner) = match &base {
        Type::Named(n, ps) if matches!(n.as_str(), "Rc" | "Arc") && ps.len() == 1 => {
            (n.clone(), ps[0].clone())
        }
        Type::Named(n, ps) if n == "Weak" && ps.len() == 1 => {
            if method == "upgrade" {
                return Some(weak_upgrade(ctx, ps[0].clone(), recv_hir, args, span));
            }
            return None;
        }
        _ => return None,
    };
    let r = match method {
        "clone" => rc_clone(ctx, &wrapper, inner, recv_hir, args, span),
        "strong_count" => rc_count(ctx, &wrapper, &inner, 0, recv_hir, args, span),
        "weak_count" => rc_count(ctx, &wrapper, &inner, 1, recv_hir, args, span),
        "downgrade" => rc_downgrade(ctx, &wrapper, inner, recv_hir, args, span),
        "try_unwrap" => rc_try_unwrap(ctx, &wrapper, inner, recv_hir, args, span),
        _ => return None,
    };
    Some(r)
}

/// `rc.clone()`：强计数 +1，返回指向同一 RcInner 的新 `Rc<T>`（指针共享）。
fn rc_clone(
    ctx: &mut TypeContext,
    wrapper: &str,
    inner: Type,
    recv_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if !args.is_empty() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{wrapper}::clone"),
            expected: 0,
            found: args.len(),
            span,
        });
    }
    let n = type_slot_count(ctx, &inner, span)?;
    let inner_t = ctx.fresh_temp();
    let cnt = ctx.fresh_temp();
    let rc_t = ctx.fresh_temp();
    let stmts = vec![
        HirStmt::Let {
            name: inner_t.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(recv_hir),
                index: 0,
                ty: FieldScalar::Ptr,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: cnt.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(inner_t.clone())),
                index: n,
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(inner_t.clone())),
            index: n,
            value: Box::new(HirExpr::Binary(
                HirBinaryOp::Add,
                Box::new(HirExpr::Variable(cnt)),
                Box::new(HirExpr::IntLiteral(1)),
            )),
            ty: FieldScalar::Int,
        }),
        HirStmt::Let {
            name: rc_t.clone(),
            init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
        },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(rc_t.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(inner_t)),
            ty: FieldScalar::Ptr,
        }),
    ];
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(rc_t)),
        })),
        Type::Named(wrapper.to_string(), vec![inner]),
    ))
}

/// `rc.strong_count()` / `rc.weak_count()`：读取 RcInner 计数槽
/// （strong = 值区槽数 `n`、weak = `n + 1`）。
fn rc_count(
    ctx: &mut TypeContext,
    wrapper: &str,
    inner: &Type,
    off: usize,
    recv_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let name = if off == 0 { "strong_count" } else { "weak_count" };
    if !args.is_empty() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{wrapper}::{name}"),
            expected: 0,
            found: args.len(),
            span,
        });
    }
    let n = type_slot_count(ctx, inner, span)?;
    let v = HirExpr::FieldGet {
        base: Box::new(HirExpr::FieldGet {
            base: Box::new(recv_hir),
            index: 0,
            ty: FieldScalar::Ptr,
        }),
        index: n + off,
        ty: FieldScalar::Int,
    };
    Ok((v, Type::USize))
}

/// `rc.downgrade()`：弱计数 +1，返回指向同一 RcInner 的 `Weak<T>`。
fn rc_downgrade(
    ctx: &mut TypeContext,
    wrapper: &str,
    inner: Type,
    recv_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if !args.is_empty() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{wrapper}::downgrade"),
            expected: 0,
            found: args.len(),
            span,
        });
    }
    let n = type_slot_count(ctx, &inner, span)?;
    let inner_t = ctx.fresh_temp();
    let cnt = ctx.fresh_temp();
    let w_t = ctx.fresh_temp();
    let stmts = vec![
        HirStmt::Let {
            name: inner_t.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(recv_hir),
                index: 0,
                ty: FieldScalar::Ptr,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: cnt.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(inner_t.clone())),
                index: n + 1,
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(inner_t.clone())),
            index: n + 1,
            value: Box::new(HirExpr::Binary(
                HirBinaryOp::Add,
                Box::new(HirExpr::Variable(cnt)),
                Box::new(HirExpr::IntLiteral(1)),
            )),
            ty: FieldScalar::Int,
        }),
        HirStmt::Let {
            name: w_t.clone(),
            init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
        },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(w_t.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(inner_t)),
            ty: FieldScalar::Ptr,
        }),
    ];
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(w_t)),
        })),
        Type::Named("Weak".to_string(), vec![inner]),
    ))
}

/// `rc.try_unwrap()`：强计数 == 1 时返回 `Ok(T)`（解出值区），否则 `Err(Rc<T>)`。
fn rc_try_unwrap(
    ctx: &mut TypeContext,
    wrapper: &str,
    inner: Type,
    recv_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if !args.is_empty() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{wrapper}::try_unwrap"),
            expected: 0,
            found: args.len(),
            span,
        });
    }
    // 值区首槽 = RcInner 指针（`T` 值区自堆首槽起，与 `*rc` 解引用同构）
    let value_base = HirExpr::FieldGet {
        base: Box::new(recv_hir.clone()),
        index: 0,
        ty: FieldScalar::Ptr,
    };
    let inner_init = HirExpr::FieldGet {
        base: Box::new(recv_hir.clone()),
        index: 0,
        ty: FieldScalar::Ptr,
    };
    // `Ok(v)` 负载：标量 load 值区 / 聚合取值区指针（与 `*rc` 解引用一致）
    let ok_val = if inner.is_numeric() || matches!(inner, Type::Bool | Type::Char) {
        HirExpr::Deref {
            expr: Box::new(value_base),
            ty: field_scalar_of(&inner),
        }
    } else {
        value_base
    };
    let (ok_tag, res_slots) = enum_variant_info(ctx, "Result", "Ok", span)?;
    let (err_tag, _) = enum_variant_info(ctx, "Result", "Err", span)?;
    let inner_t = ctx.fresh_temp();
    let cnt = ctx.fresh_temp();
    let res_t = ctx.fresh_temp();
    let then_stmts = vec![
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(res_t.clone())),
            index: 0,
            value: Box::new(HirExpr::IntLiteral(ok_tag as i128)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(res_t.clone())),
            index: 1,
            value: Box::new(ok_val),
            ty: field_scalar_of(&inner),
        }),
    ];
    let else_stmts = vec![
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(res_t.clone())),
            index: 0,
            value: Box::new(HirExpr::IntLiteral(err_tag as i128)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(res_t.clone())),
            index: 1,
            value: Box::new(recv_hir),
            ty: FieldScalar::Ptr,
        }),
    ];
    let stmts = vec![
        HirStmt::Let {
            name: inner_t.clone(),
            init: inner_init,
            mutable: false,
        },
        HirStmt::Let {
            name: cnt.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(inner_t)),
                index: 0,
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: res_t.clone(),
            init: HirExpr::Alloc {
            slots: res_slots,
            by_value: false,
        },
            mutable: false,
        },
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Eq,
                Box::new(HirExpr::Variable(cnt)),
                Box::new(HirExpr::IntLiteral(1)),
            )),
            then_block: Box::new(HirBlock {
                stmts: then_stmts,
                final_expr: None,
            }),
            else_block: Some(Box::new(HirBlock {
                stmts: else_stmts,
                final_expr: None,
            })),
        }),
    ];
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(res_t)),
        })),
        Type::Named(
            "Result".to_string(),
            vec![inner.clone(), Type::Named(wrapper.to_string(), vec![inner])],
        ),
    ))
}

/// `w.upgrade()`：强计数 > 0 时强计数 +1 并返回 `Some(Rc<T>)`，否则 `None`。
fn weak_upgrade(
    ctx: &mut TypeContext,
    inner: Type,
    w_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if !args.is_empty() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "Weak::upgrade".to_string(),
            expected: 0,
            found: args.len(),
            span,
        });
    }
    let n = type_slot_count(ctx, &inner, span)?;
    let (some_tag, opt_slots) = enum_variant_info(ctx, "Option", "Some", span)?;
    let (none_tag, _) = enum_variant_info(ctx, "Option", "None", span)?;
    let inner_t = ctx.fresh_temp();
    let cnt = ctx.fresh_temp();
    let opt_t = ctx.fresh_temp();
    let c2 = ctx.fresh_temp();
    let rc_t = ctx.fresh_temp();
    let then_stmts = vec![
        HirStmt::Let {
            name: c2.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(inner_t.clone())),
                index: n,
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(inner_t.clone())),
            index: n,
            value: Box::new(HirExpr::Binary(
                HirBinaryOp::Add,
                Box::new(HirExpr::Variable(c2)),
                Box::new(HirExpr::IntLiteral(1)),
            )),
            ty: FieldScalar::Int,
        }),
        HirStmt::Let {
            name: rc_t.clone(),
            init: HirExpr::Alloc {
            slots: 1,
            by_value: false,
        },
            mutable: false,
        },
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(rc_t.clone())),
            index: 0,
            value: Box::new(HirExpr::Variable(inner_t.clone())),
            ty: FieldScalar::Ptr,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(opt_t.clone())),
            index: 0,
            value: Box::new(HirExpr::IntLiteral(some_tag as i128)),
            ty: FieldScalar::Int,
        }),
        HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(opt_t.clone())),
            index: 1,
            value: Box::new(HirExpr::Variable(rc_t)),
            ty: FieldScalar::Ptr,
        }),
    ];
    let else_stmts = vec![HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(opt_t.clone())),
        index: 0,
        value: Box::new(HirExpr::IntLiteral(none_tag as i128)),
        ty: FieldScalar::Int,
    })];
    let stmts = vec![
        HirStmt::Let {
            name: inner_t.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(w_hir),
                index: 0,
                ty: FieldScalar::Ptr,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: cnt.clone(),
            init: HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(inner_t)),
                index: 0,
                ty: FieldScalar::Int,
            },
            mutable: false,
        },
        HirStmt::Let {
            name: opt_t.clone(),
            init: HirExpr::Alloc {
            slots: opt_slots,
            by_value: false,
        },
            mutable: false,
        },
        HirStmt::Expr(HirExpr::If {
            cond: Box::new(HirExpr::Binary(
                HirBinaryOp::Gt,
                Box::new(HirExpr::Variable(cnt)),
                Box::new(HirExpr::IntLiteral(0)),
            )),
            then_block: Box::new(HirBlock {
                stmts: then_stmts,
                final_expr: None,
            }),
            else_block: Some(Box::new(HirBlock {
                stmts: else_stmts,
                final_expr: None,
            })),
        }),
    ];
    Ok((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(HirExpr::Variable(opt_t)),
        })),
        Type::Named("Option".to_string(), vec![Type::Named("Rc".to_string(), vec![inner])]),
    ))
}

/// 枚举变体 tag 与枚举 slot_count 查询（`Rc` 内建构造 Option/Result 用）。
fn enum_variant_info(
    ctx: &mut TypeContext,
    enum_name: &str,
    variant: &str,
    span: Span,
) -> Result<(usize, usize), TypeError> {
    let def = ctx.lookup_enum(enum_name).cloned().ok_or_else(|| TypeError::UndefinedType {
        name: enum_name.to_string(),
        span,
    })?;
    let tag = def
        .variants
        .iter()
        .find(|v| v.name == variant)
        .map(|v| v.tag)
        .ok_or_else(|| TypeError::FunctionNotFound {
            name: format!("{enum_name}::{variant}"),
            span,
        })?;
    Ok((tag, def.slot_count))
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
    // `Box<T>` 索引对象：base 改写为堆对象指针（K2），后续按剥层后的
    // `T`（数组 / String / Vec）走既有索引分支
    let b_hir = heap_ptr_hir(b_hir, &b_ty);
    let (i_hir, i_ty) = infer_expr(ctx, index)?;
    if !i_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: i_ty.to_string(),
            span: index.span,
        });
    }
    // `&str`（String 对象的只读借用）：索引前先取槽 0 的 data 指针（同 String 对象）
    if let Type::Ref(inner, _) = &b_ty {
        if matches!(**inner, Type::Str) {
            return Ok((
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
            ));
        }
    }
    match peel_refs_and_heap(&b_ty) {
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
            by_value: false,
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

/// 字段类型是否可安全作为按值（栈内联）对象的组成部分：
/// 须为具体（非泛型/未推断）标量槽——聚合（struct/enum/String/引用/裸指针/
/// 数组/元组/trait 对象/闭包值/函数指针）以"对象地址"语义参与存储，按值
/// 对象地址逃逸后悬垂（如 `Result<SocketAddr, _>::Ok(sa)` 中 `SocketAddr`
/// 的 String 字段），故含此类字段的对象不得按值构造。
fn field_is_scalar_slot(fty: &Type) -> bool {
    match fty {
        Type::Generic(_) | Type::Infer => false,
        t => field_scalar_of(t) != FieldScalar::Ptr,
    }
}

/// 枚举是否可安全按值（栈内联）构造。
///
/// 按值对象以 `[2 x i64]` 栈槽存储；当其作为另一聚合（enum/struct）的
/// 字段/payload 时以"对象地址"语义写入外层 Ptr 槽——若该对象本身是
/// 按值栈对象，写入的即栈地址，函数返回/跨调用后悬垂。
///
/// 判定规则（与构造点所在变体无关，保证同一枚举所有构造路径一致）：
/// - 槽数 > 2：无法容纳，禁用；
/// - 泛型枚举（`Option`/`Result` 等）：定义时无法预知类型实参是否聚合，
///   且同一枚举不同变体构造点（如 `None`/`Some`）需判定一致，保守禁用；
/// - 非泛型枚举：所有变体的所有字段均为具体标量槽方可启用。
fn enum_instance_by_value(ctx: &TypeContext, enum_name: &str, slot_count: usize) -> bool {
    if slot_count > 2 {
        return false;
    }
    let Some(def) = ctx.lookup_enum(enum_name) else {
        return false;
    };
    if !def.type_params.is_empty() {
        return false;
    }
    def.variants
        .iter()
        .all(|v| v.fields.iter().all(|(_, fty)| field_is_scalar_slot(fty)))
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
    // 标量枚举按值分配（栈槽，免 calloc）：≤2 槽的 enum（如 Option<i64> /
    // Option<bool>）槽区（tag + ≤1 payload）均为 8 字节槽，兼容栈上
    // `[2 x i64]` 存储；判定只看槽数——不依赖具体变体实例化，保证
    // 同一 enum 的 None/Some 等各变体构造路径判定一致。
    let enum_by_value = enum_instance_by_value(ctx, &enum_name, enum_def.slot_count);
    let base = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: base.clone(),
        init: HirExpr::Alloc {
            slots: enum_def.slot_count,
            by_value: enum_by_value,
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
/// K1 `?` 错误传播：`expr?` desugar 为
///
/// - `Option<T>`：`match expr { Some(__v) => __v, None => return None }`
/// - `Result<T, E>`：`match expr { Ok(__v) => __v, Err(__e) => return Err(__e) }`
///
/// 复用 `check_match_with_scrutinee`（模式匹配 → if-else 链 + tag 比较），
/// 失败臂走 `HirExpr::Return` 早返回（类型 `Never`，match 结果类型为 T）。
/// 零新增 HIR 节点，MIR/LIR/codegen 无改动。MVP 约束：返回类型兼容性检查
/// 与现有 `return` 语义一致（宽松，不显式校验函数返回类型）。
fn check_question(
    ctx: &mut TypeContext,
    inner: &AstExpr,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let (inner_hir, inner_ty) = infer_expr(ctx, inner)?;
    // 确定 Option / Result 分支
    let (ok_variant, none_variant, has_err_field) = match &inner_ty {
        Type::Named(n, _) if n == "Option" => ("Some", "None", false),
        Type::Named(n, _) if n == "Result" => ("Ok", "Err", true),
        _ => {
            return Err(TypeError::Unsupported {
                what: format!("`?` 运算符仅支持 Option/Result 类型，得到 `{inner_ty}`"),
                span,
            })
        }
    };
    let val = ctx.fresh_temp();
    let err = ctx.fresh_temp();
    let mk_ident = |name: String| AstExpr::new(ExprKind::Ident(name), span);
    // arm1：成功臂 `Some(__v) => __v` / `Ok(__v) => __v`
    let ok_arm = rlyeh_ast::MatchArm {
        pattern: AstPattern::Enum(ok_variant.to_string(), vec![AstPattern::Ident(val.clone())]),
        guard: None,
        body: mk_ident(val),
        span,
    };
    // arm2：失败臂 `None => return None` / `Err(__e) => return Err(__e)`
    let (none_pat, ret_args) = if has_err_field {
        (
            AstPattern::Enum(none_variant.to_string(), vec![AstPattern::Ident(err.clone())]),
            vec![mk_ident(err)],
        )
    } else {
        (AstPattern::Enum(none_variant.to_string(), vec![]), vec![])
    };
    // 失败变体经完整路径构造（`Option::None` / `Result::Err(e)`）：裸 `None`
    // 是 Ident（infer_expr 无变体兜底），带参变体是 Call（走 check_call 的
    // split_variant_path 兜底）。枚举名取自 `Type::Named`（可能含模块路径）。
    let Type::Named(en_name, _) = &inner_ty else {
        unreachable!("Option/Result 分支已保证 Named")
    };
    let mut callee_segs: Vec<String> = en_name.split("::").map(|s| s.to_string()).collect();
    callee_segs.push(none_variant.to_string());
    let callee = AstExpr::new(ExprKind::Path(callee_segs), span);
    let ret_inner = if has_err_field {
        AstExpr::new(ExprKind::Call { callee, args: ret_args, type_args: Vec::new() }, span)
    } else {
        callee
    };
    let ret_body = AstExpr::new(ExprKind::Return(Some(ret_inner)), span);
    let err_arm = rlyeh_ast::MatchArm {
        pattern: none_pat,
        guard: None,
        body: ret_body,
        span,
    };
    check_match_with_scrutinee(ctx, inner_hir, inner_ty, &[ok_arm, err_arm], span)
}

fn check_match(
    ctx: &mut TypeContext,
    expr: &AstExpr,
    arms: &[rlyeh_ast::MatchArm],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let (s_hir, s_ty) = infer_expr(ctx, expr)?;
    check_match_with_scrutinee(ctx, s_hir, s_ty, arms, span)
}

/// 与 [`check_match`] 相同，但使用调用方已推断的 scrutinee（HIR + 类型）。
///
/// 供 `?` 运算符（K1）desugar 使用：inner 表达式可能内嵌闭包，若经
/// [`check_match`] 重新 infer 会再次触发闭包 desugar（匿名函数重复注入）。
fn check_match_with_scrutinee(
    ctx: &mut TypeContext,
    s_hir: HirExpr,
    s_ty: Type,
    arms: &[rlyeh_ast::MatchArm],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
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
        // 臂作用域：模式绑定变量在 check_pattern 中注册（遮蔽槽名已计算），
        // arm body / guard 内引用经 resolve 解析为槽名；求值后弹出作用域——
        // 臂绑定不再污染外层（U1：与后续同名 let 互不覆盖）。
        // 块作用域穿透：arm body 仍可见外层变量（函数参数、外层 let）。
        ctx.push_scope(false);
        let (cond, binds, is_binding, _bound_tys) =
            check_pattern(ctx, &arm.pattern, &pat_ty, tmp_var.clone(), span)?;
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
        ctx.pop_scope();
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
    pat: &rlyeh_ast::AstPattern,
    pat_ty: &Type,
    scrutinee: HirExpr,
    span: Span,
) -> Result<PatternResult, TypeError> {
    use rlyeh_ast::AstPattern;
    match pat {
        AstPattern::Ident(name) => {
            // U1：绑定变量在臂作用域内注册（遮蔽时 mangle 存储槽名）。
            // Let 绑定、引用解析、类型表全部使用槽名。
            let slot = ctx.insert_variable(name.clone(), pat_ty.clone());
            Ok((
                None,
                vec![HirStmt::Let {
                    name: slot.clone(),
                    init: scrutinee,
                    mutable: false,
                }],
                true,
                vec![(slot, pat_ty.clone())],
            ))
        }
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
            // 枚举名可能为 use 导入的本地名（`use protocol::Msg` 后裸名 `Msg`），
            // 裸名未注册时回退经 resolve_full_name 解析完整符号名（模块前缀 / use 别名）。
            let enum_def = ctx
                .lookup_enum(en)
                .cloned()
                .or_else(|| {
                    ctx.resolve_full_name(en)
                        .and_then(|full| ctx.lookup_enum(&full).cloned())
                })
                .ok_or_else(|| {
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
        AstPattern::Ref(inner, is_mut) => {
            // `ref [mut] pat`：绑定变量为对匹配值的引用（`&T` / `&mut T`），
            // 而非值拷贝。递归检查内层模式后，将绑定语句的初始化改为取匹配
            // 值的引用（聚合 = 对象指针拷贝，标量 = 存储槽地址），并将绑定
            // 变量类型引用化；可变性原样传递给引用与绑定变量。
            let mutability = if *is_mut {
                Mutability::Mutable
            } else {
                Mutability::Immutable
            };
            // U1 修复：内层为简单标识符时直接以引用类型 `&T` 注册变量——
            // 若经递归（Ident 分支内部 insert 值类型）后仅靠 bound_tys 引用化，
            // 类型表仍为 `i64`，臂内 `*r` 解引用报「发现 i64」。
            if let AstPattern::Ident(name) = &**inner {
                let bind_ty = Type::Ref(Box::new(pat_ty.clone()), mutability);
                let slot = ctx.insert_variable(name.clone(), bind_ty.clone());
                return Ok((
                    None,
                    vec![HirStmt::Let {
                        name: slot.clone(),
                        init: HirExpr::Ref {
                            expr: Box::new(scrutinee),
                            is_mut: *is_mut,
                            pointee: field_scalar_of(pat_ty),
                        },
                        mutable: *is_mut,
                    }],
                    true,
                    vec![(slot, bind_ty)],
                ));
            }
            let (cond, binds, is_binding, bound_tys) =
                check_pattern(ctx, inner, pat_ty, scrutinee, span)?;
            let bound_tys = bound_tys
                .into_iter()
                .map(|(n, t)| (n, Type::Ref(Box::new(t), mutability)))
                .collect();
            let binds = binds
                .into_iter()
                .map(|b| match b {
                    HirStmt::Let { name, init, mutable: _ } => HirStmt::Let {
                        name,
                        init: HirExpr::Ref {
                            expr: Box::new(init),
                            is_mut: *is_mut,
                            pointee: field_scalar_of(pat_ty),
                        },
                        mutable: *is_mut,
                    },
                    other => other,
                })
                .collect();
            Ok((cond, binds, is_binding, bound_tys))
        }
    }
}

/// 将字面量值转为 HIR 字面量。
fn literal_to_hir(lit: &rlyeh_ast::LiteralValue, span: Span) -> Result<HirExpr, TypeError> {
    use rlyeh_ast::LiteralValue;
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
/// W6：`Thread::start(f, arg)` 闭包值跨线程捕获。
///
/// 当 `f` 为带参闭包值对象（`Type::Closure`）且提供线程输入参数 `arg` 时：
/// - 构造线程输入聚合对象 `__t_in`（槽 = `[f 各捕获槽值..., arg]`）；
/// - 生成线程入口 thunk `__thread_entry_N(input: i64)`（从输入对象读捕获槽 + arg，
///   调用闭包匿名函数 `__closure_N(cap0..., arg)`）；
/// - 展开为 `thread::__start_with_input(thunk, __t_in)`（Result 构造复用 std）。
///
/// 返回 `None` 表示实参非闭包值形态（走常规 `Thread::start(f: fn() -> i64)`）。
/// MVP：线程闭包限单参数（`|x: i64| ..`），多参数报 Unsupported。
fn check_thread_start_closure(
    ctx: &mut TypeContext,
    _ty_name: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<Option<(HirExpr, Type)>, TypeError> {
    // 需恰好两个实参：闭包值 + 线程输入参数
    if args.len() != 2 {
        return Ok(None);
    }
    // 实参 0 须为闭包值变量（`let f = |x: i64| ..; Thread::start(f, arg)`）
    let ExprKind::Ident(f_name) = &*args[0].kind else {
        return Ok(None);
    };
    let Some(ty) = ctx.lookup_variable(f_name).cloned() else {
        return Ok(None);
    };
    let Type::Closure { captures, params, ret, fn_name } = &ty else {
        return Ok(None);
    };
    // 未固化延迟闭包（绑定处参数类型未知）不支持跨线程；MVP 限单参数
    if fn_name.is_empty() {
        return Ok(None);
    }
    if params.len() != 1 {
        return Err(TypeError::Unsupported {
            what: "跨线程闭包限单参数（`|x: i64| ..`）".to_string(),
            span,
        });
    }
    let (arg_hir, arg_ty) = infer_expr(ctx, &args[1])?;
    if !arg_ty.compatible_with(&params[0]) {
        return Err(TypeError::ArgumentTypeMismatch {
            name: "Thread::start 输入参数".to_string(),
            index: 1,
            expected: params[0].to_string(),
            found: arg_ty.to_string(),
            span: args[1].span,
        });
    }

    // 1. 线程输入对象 __t_in：槽 = [捕获槽值..., arg]
    let n = captures.len();
    let t_in = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::Let {
        name: t_in.clone(),
        init: HirExpr::Alloc {
            slots: n + 1,
            by_value: false,
        },
        mutable: false,
    }];
    for (i, cap_ty) in captures.iter().enumerate() {
        stmts.push(HirStmt::Semi(HirExpr::FieldSet {
            base: Box::new(HirExpr::Variable(t_in.clone())),
            index: i,
            value: Box::new(HirExpr::FieldGet {
                base: Box::new(HirExpr::Variable(f_name.clone())),
                index: i,
                ty: field_scalar_of(cap_ty),
            }),
            ty: field_scalar_of(cap_ty),
        }));
    }
    stmts.push(HirStmt::Semi(HirExpr::FieldSet {
        base: Box::new(HirExpr::Variable(t_in.clone())),
        index: n,
        value: Box::new(arg_hir),
        ty: field_scalar_of(&params[0]),
    }));

    // 2. 生成线程入口 thunk `__thread_entry_N(input: i64)`：读输入对象槽调 __closure_N
    let thunk = emit_thread_entry(ctx, captures, &params[0], ret, fn_name);

    // 3. thunk 函数指针绑定到局部变量（`let __entry = thunk`），经 fn 形参传 std
    let entry_var = ctx.fresh_temp();
    stmts.push(HirStmt::Let {
        name: entry_var.clone(),
        init: HirExpr::FnPtr(thunk),
        mutable: false,
    });

    // 4. 调用 `thread::__start_with_input(__entry, __t_in)`，返回类型取自 std 签名
    let helper = "thread::__start_with_input".to_string();
    let ret_ty = ctx
        .fn_signatures
        .get(&helper)
        .map(|s| s.return_type.clone())
        .unwrap_or(Type::I64);
    let call = HirExpr::Call {
        callee: helper,
        args: vec![
            HirExpr::Variable(entry_var),
            HirExpr::Variable(t_in),
        ],
    };
    Ok(Some((
        HirExpr::Block(Box::new(HirBlock {
            stmts,
            final_expr: Some(call),
        })),
        ret_ty,
    )))
}

/// 生成线程入口 thunk `__thread_entry_N(input: i64) -> Ret`：
/// 从输入对象（槽 = `[cap0..capN-1, arg]`）读捕获槽 + arg，调用闭包匿名函数
/// `__closure_N(cap0..capN-1, arg)`。
fn emit_thread_entry(
    ctx: &mut TypeContext,
    capture_tys: &[Type],
    arg_ty: &Type,
    ret: &Type,
    closure_fn: &str,
) -> String {
    let name = format!("__thread_entry_{}", ctx.closure_seq);
    ctx.closure_seq += 1;
    let mut call_args: Vec<HirExpr> = capture_tys
        .iter()
        .enumerate()
        .map(|(i, cap_ty)| HirExpr::FieldGet {
            base: Box::new(HirExpr::Variable("__input".to_string())),
            index: i,
            ty: field_scalar_of(cap_ty),
        })
        .collect();
    call_args.push(HirExpr::FieldGet {
        base: Box::new(HirExpr::Variable("__input".to_string())),
        index: capture_tys.len(),
        ty: field_scalar_of(arg_ty),
    });
    let body = HirExpr::Call {
        callee: closure_fn.to_string(),
        args: call_args,
    };
    ctx.insert_fn_signature(
        name.clone(),
        FnSignature {
            params: vec![Type::I64],
            return_type: ret.clone(),
        },
    );
    ctx.mono_items.push(HirItem {
        name: name.clone(),
        kind: HirItemKind::Fn(HirFnDecl {
            params: vec![HirParam {
                name: "__input".to_string(),
            }],
            body: Some(HirBlock {
                stmts: vec![],
                final_expr: Some(body),
            }),
            is_extern: false,
            extern_sig: None,
        }),
    });
    name
}

fn check_static_method_call(
    ctx: &mut TypeContext,
    ty_name: &str,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // W6：闭包值跨线程捕获——`Thread::start(f, arg)`（f 为带参闭包值对象）。
    // 展开为：生成线程入口 thunk + 线程输入对象（捕获槽值 + arg），调用
    // std `thread::__start_with_input(thunk, input)`（Result 构造复用 std 语言层）。
    if ty_name == "thread::Thread" && method == "start" {
        if let Some(ret) = check_thread_start_closure(ctx, ty_name, args, span)? {
            return Ok(ret);
        }
    }
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
        // Str 值实参 → 非 Str 形参自动升级（静态方法 `T::m("hi")`）
        let (hir, ty) = upgrade_str_arg(ctx, hir, ty, pty, arg)?;
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
    // `r#` 前缀（关键字转义，如 `fn r#send`）在方法调用处去前缀，
    // 与定义侧 parse_fn 归一化（`r#send` → `send`）保持一致。
    let method = method.strip_prefix("r#").unwrap_or(method);
    let (recv_hir, recv_ty) = infer_expr(ctx, receiver)?;
    // `push_str(字面量实参)` 整体特判：改调 `String::push_bytes(src, n)` 快速路径，
    // 免去字面量实参每次 alloc_bytes + copy_bytes 深拷贝（strcat 类拼接基准收益
    // ~3 个数量级；直接字面量实参的字节数与内容编译期已知）。
    // `src` 实参传 `&__lit`（Str 标量槽地址）：`&str` 的标准表示是 String 3 槽
    // 对象指针（`as_str()` 返回对象指针，`check_index` 对 `&str` 索引先取槽 0 的
    // data 指针），而字面量值本身是裸 data 指针——若直传字面量，`push_bytes` 内
    // `src[i]` 会把常量前 8 字节当对象指针解引用（段错误）。`&__lit` 经 AddrOf
    // 标量分支生成「指向 data 指针槽的指针」= 单槽伪对象头，FieldGet 槽 0 即 data。
    // 条件：String 接收者 + 单实参且为字符串字面量。
    if method == "push_str"
        && args.len() == 1
        && matches!(&recv_ty, Type::Named(n, _) if n == "String")
        && matches!(&*args[0].kind, ExprKind::StringLiteral(_))
    {
        let s = match &*args[0].kind {
            ExprKind::StringLiteral(s) => s.clone(),
            _ => unreachable!(),
        };
        let n = s.len() as i128;
        let lit_tmp = ctx.fresh_temp();
        let impl_def = ctx
            .find_impl_for_method(&recv_ty, "push_bytes")
            .cloned()
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: "String::push_bytes".to_string(),
                span,
            })?;
        let method_def = impl_def
            .methods
            .iter()
            .find(|m| m.sig.name == "push_bytes")
            .cloned()
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: "String::push_bytes".to_string(),
                span,
            })?;
        let fn_name = instantiate_impl_method(ctx, &impl_def, &method_def, &HashMap::new(), span)?;
        return Ok((
            HirExpr::Block(Box::new(HirBlock {
                stmts: vec![HirStmt::Let {
                    name: lit_tmp.clone(),
                    init: HirExpr::StringLiteral(s),
                    mutable: false,
                }],
                final_expr: Some(HirExpr::Call {
                    callee: fn_name,
                    args: vec![
                        recv_hir,
                        HirExpr::Ref {
                            expr: Box::new(HirExpr::Variable(lit_tmp)),
                            is_mut: false,
                            pointee: FieldScalar::Str,
                        },
                        HirExpr::IntLiteral(n),
                    ],
                }),
            })),
            Type::Unit,
        ));
    }
    // `Rc<T>` / `Arc<T>` / `Weak<T>` 引用计数内建方法（K3）：clone /
    // strong_count / weak_count / downgrade / try_unwrap / upgrade。
    // 须在堆指针改写前分派（内建需要原始 Rc 对象取 RcInner 指针）
    if let Some(r) = check_rc_method(ctx, method, &recv_ty, recv_hir.clone(), args, span) {
        return r;
    }
    // H4 `dyn Trait` 接收者：方法经 vtable 间接调用（类型擦除后的多态分派）。
    // 布局：2 槽胖指针（槽 0 = 数据指针，槽 1 = vtable 指针）。
    // desugar 为：
    //   let __obj = <recv>;                       // 胖指针对象（1 指针槽）
    //   let __data = FieldGet(__obj, 0, Ptr);     // 数据指针
    //   let __vtp  = FieldGet(__obj, 1, Ptr);     // vtable 指针
    //   let __m    = Index(__vtp, 3+idx, Ptr);    // vtable[3+idx] 方法函数指针
    //   final: CallIndirect { callee: __m, args: [__data, ...实参], param_names, ret_name }
    if let Type::Dyn(trait_name) = &recv_ty {
        // H4 去虚拟化：接收者为 dyn 局部变量且绑定源具体类型已知时，静态分派到
        // 具体类型方法（vtable 调用在循环中受间接调用屏障阻止优化，静态调用
        // 可被 LLVM 内联 / 常量折叠；dyn 变量被重新赋值时映射已失效回退 vtable）
        if let HirExpr::Variable(var) = &recv_hir {
            if let Some(devirt) =
                devirtualize_dyn_call(ctx, var, trait_name, method, recv_hir.clone(), args, span)?
            {
                return Ok(devirt);
            }
        }
        let trait_def = ctx
            .trait_defs
            .get(trait_name)
            .cloned()
            .ok_or_else(|| TypeError::UndefinedType {
                name: trait_name.clone(),
                span,
            })?;
        let idx = trait_def
            .methods
            .iter()
            .position(|m| m.name == method)
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: format!("dyn {trait_name}::{method}"),
                span,
            })?;
        let sig = &trait_def.methods[idx];
        // MVP 限制：trait 方法签名含 `Self`（关联返回类型 / 参数）时无法确定
        // 具体类型，不支持经 dyn 调用
        if sig.params.iter().skip(1).any(type_mentions_self) || type_mentions_self(&sig.return_type) {
            return Err(TypeError::Unsupported {
                what: format!(
                    "`dyn {trait_name}::{method}`：签名含 `Self` 的方法（关联类型 MVP 不支持 trait 对象调用）"
                ),
                span,
            });
        }
        if args.len() + 1 != sig.params.len() {
            return Err(TypeError::UnexpectedArgumentCount {
                name: format!("dyn {trait_name}::{method}"),
                expected: sig.params.len() - 1,
                found: args.len(),
                span,
            });
        }
        // 实参类型检查 + 组装（self → 数据指针）
        let mut hir_args = Vec::with_capacity(args.len() + 1);
        let mut param_names = vec![type_to_extern_name(&sig.params[0])];
        for (i, a) in args.iter().enumerate() {
            let (h, t) = infer_expr(ctx, a)?;
            let pty = substitute(&sig.params[i + 1], &HashMap::new());
            // Str 值实参 → 非 Str 形参自动升级（`dyn Trait` 方法 String 形参）
            let (h, t) = upgrade_str_arg(ctx, h, t, &pty, a)?;
            if !t.compatible_with(&pty) {
                return Err(TypeError::ArgumentTypeMismatch {
                    name: format!("dyn {trait_name}::{method}"),
                    index: i + 1,
                    expected: pty.to_string(),
                    found: t.to_string(),
                    span: a.span,
                });
            }
            hir_args.push(h);
            param_names.push(type_to_extern_name(&pty));
        }
        // 构造调用块
        let obj = ctx.fresh_temp();
        let data = ctx.fresh_temp();
        let vtp = ctx.fresh_temp();
        let m = ctx.fresh_temp();
        let stmts = vec![
            HirStmt::Let {
                name: obj.clone(),
                init: recv_hir,
                mutable: false,
            },
            HirStmt::Let {
                name: data.clone(),
                init: HirExpr::FieldGet {
                    base: Box::new(HirExpr::Variable(obj.clone())),
                    index: 0,
                    ty: FieldScalar::Ptr,
                },
                mutable: false,
            },
            HirStmt::Let {
                name: vtp.clone(),
                init: HirExpr::FieldGet {
                    base: Box::new(HirExpr::Variable(obj)),
                    index: 1,
                    ty: FieldScalar::Ptr,
                },
                mutable: false,
            },
            HirStmt::Let {
                name: m.clone(),
                init: HirExpr::Index {
                    base: Box::new(HirExpr::Variable(vtp)),
                    index: Box::new(HirExpr::IntLiteral((3 + idx) as i128)),
                    elem: FieldScalar::Ptr,
                    is_str: false,
                },
                mutable: false,
            },
        ];
        let mut call_args = vec![HirExpr::Variable(data)];
        call_args.extend(hir_args);
        let ret_ty = sig.return_type.clone();
        let call = HirExpr::CallIndirect {
            callee: Box::new(HirExpr::Variable(m)),
            args: call_args,
            param_names,
            ret_name: type_to_extern_name(&ret_ty),
        };
        return Ok((
            HirExpr::Block(Box::new(HirBlock {
                stmts,
                final_expr: Some(call),
            })),
            ret_ty,
        ));
    }
    // `Box<T>` 接收者：receiver 改写为堆对象指针（槽 0），使方法按 `T` 解析
    // 且 `&self` 参数收到 `T` 对象指针（K2；`Rc<T>`/`Arc<T>` 取值区槽 2）
    let recv_hir = heap_ptr_hir(recv_hir, &recv_ty);
    // `str` 值接收者（字符串字面量 / 绑定字面量的变量，非 `&str` 引用）：
    // 升级为 String 对象（编译期长度展开），使 len / substring / contains 等
    // 按 String impl 解析；运行期 `str` 值（如函数参数）无长度信息，
    // `check_string_from` 报 Unsupported
    let (recv_hir, recv_ty) = if comparison::is_str_value(&recv_ty) {
        let (h, t) = check_string_from(ctx, std::slice::from_ref(receiver), receiver.span)?;
        (h, t)
    } else {
        (recv_hir, recv_ty)
    };
    // `String::as_str()` → `&str`：零拷贝只读借用视图（对齐 Rust `&self[..]`）。
    // V2 胖指针：构造 StrFat 双槽值 `{ data 指针, len }`（by_value 栈上 [2 x i64]），
    // data = String 槽 0 的 data 指针，len = String 槽 1 的长度。
    // 返回 `&str` 值（内联双字，非堆对象，不悬垂）。
    let string_base = heap_wrapper_inner(&recv_ty).unwrap_or_else(|| recv_ty.clone());
    if method == "as_str" && comparison::is_string_type(ctx, &string_base) {
        let data_tmp = ctx.fresh_temp();
        let len_tmp = ctx.fresh_temp();
        let sf = ctx.fresh_temp();
        let stmts = vec![
            HirStmt::Let {
                name: data_tmp.clone(),
                init: HirExpr::FieldGet {
                    base: Box::new(recv_hir.clone()),
                    index: 0,
                    ty: FieldScalar::Ptr,
                },
                mutable: false,
            },
            HirStmt::Let {
                name: len_tmp.clone(),
                init: HirExpr::FieldGet {
                    base: Box::new(recv_hir),
                    index: 1,
                    ty: FieldScalar::Int,
                },
                mutable: false,
            },
            HirStmt::Let {
                name: sf.clone(),
                init: HirExpr::Alloc {
                    slots: 2,
                    by_value: true,
                },
                mutable: false,
            },
            HirStmt::Semi(HirExpr::FieldSet {
                base: Box::new(HirExpr::Variable(sf.clone())),
                index: 0,
                value: Box::new(HirExpr::Variable(data_tmp)),
                ty: FieldScalar::Ptr,
            }),
            HirStmt::Semi(HirExpr::FieldSet {
                base: Box::new(HirExpr::Variable(sf.clone())),
                index: 1,
                value: Box::new(HirExpr::Variable(len_tmp)),
                ty: FieldScalar::Int,
            }),
        ];
        return Ok((
            HirExpr::Block(Box::new(HirBlock {
                stmts,
                final_expr: Some(HirExpr::Variable(sf)),
            })),
            Type::Ref(Box::new(Type::Str), Mutability::Immutable),
        ));
    }
    // `String::as_str_range(start, end)` → `&str` 子区间视图（V2 零拷贝）：
    // 构造 StrFat `{ data 指针 + start 偏移, end - start }`（对齐 Rust `&self[a..b]`）。
    // receiver 可为 `&String`（std 方法 `as_str_range(&self)` 内 `self` 是 `&String`）。
    if method == "as_str_range"
        && args.len() == 2
        && comparison::is_string_type(ctx, &peel_refs_and_heap(&recv_ty))
    {
        let (start_hir, start_ty) = infer_expr(ctx, &args[0])?;
        let (end_hir, _) = infer_expr(ctx, &args[1])?;
        if !start_ty.compatible_with(&Type::I64) {
            return Err(TypeError::ArgumentTypeMismatch {
                name: "as_str_range start".to_string(),
                index: 0,
                expected: "i64".to_string(),
                found: start_ty.to_string(),
                span: args[0].span,
            });
        }
        let data_tmp = ctx.fresh_temp();
        let start_ptr = ctx.fresh_temp();
        let len_tmp = ctx.fresh_temp();
        let sf = ctx.fresh_temp();
        let stmts = vec![
            HirStmt::Let {
                name: data_tmp.clone(),
                init: HirExpr::FieldGet {
                    base: Box::new(recv_hir),
                    index: 0,
                    ty: FieldScalar::Ptr,
                },
                mutable: false,
            },
            HirStmt::Let {
                name: start_ptr.clone(),
                init: HirExpr::PtrAdd {
                    base: Box::new(HirExpr::Variable(data_tmp)),
                    offset: Box::new(start_hir.clone()),
                    elem: FieldScalar::Str,
                },
                mutable: false,
            },
            HirStmt::Let {
                name: len_tmp.clone(),
                init: HirExpr::Binary(
                    HirBinaryOp::Sub,
                    Box::new(end_hir),
                    Box::new(start_hir),
                ),
                mutable: false,
            },
            HirStmt::Let {
                name: sf.clone(),
                init: HirExpr::Alloc {
                    slots: 2,
                    by_value: true,
                },
                mutable: false,
            },
            HirStmt::Semi(HirExpr::FieldSet {
                base: Box::new(HirExpr::Variable(sf.clone())),
                index: 0,
                value: Box::new(HirExpr::Variable(start_ptr)),
                ty: FieldScalar::Ptr,
            }),
            HirStmt::Semi(HirExpr::FieldSet {
                base: Box::new(HirExpr::Variable(sf.clone())),
                index: 1,
                value: Box::new(HirExpr::Variable(len_tmp)),
                ty: FieldScalar::Int,
            }),
        ];
        return Ok((
            HirExpr::Block(Box::new(HirBlock {
                stmts,
                final_expr: Some(HirExpr::Variable(sf)),
            })),
            Type::Ref(Box::new(Type::Str), Mutability::Immutable),
        ));
    }
    let mut self_ty = peel_refs_and_heap(&recv_ty);
    // `&str` 接收者：方法按 String impl 解析（MVP 中 `&str` 是 String 对象的
    // 只读借用视图，String 的方法视图（len / substring / push_str 等）均可用）
    if matches!(self_ty, Type::Str) && matches!(&recv_ty, Type::Ref(_, _)) {
        self_ty = Type::Named("String".to_string(), vec![]);
    }
    // J3 迭代器适配器：map / filter / fold / collect / take / skip
    // （数组或自定义迭代器 receiver → 内建 desugar，优先于通用方法解析）
    if let Some(res) = try_check_adapter(ctx, receiver, &self_ty, method, args, span)? {
        return Ok(res);
    }
    if matches!(self_ty, Type::Unit) {
        return Err(TypeError::Unsupported {
            what: format!("对单元类型调用方法 `{method}`"),
            span,
        });
    }

    // actor 方法调用：`counter.method(a, b)` → `rlyeh_actor_ask(recv, kind, a, b, 0)`
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
                    callee: "rlyeh_actor_ask".to_string(),
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
    }

    // 参数类型推断 + Infer 回填：期望类型含未定型 `_`（如裸 `Result::Err(7)`
    // 的 `unwrap_or(default: T)`，T 经接收者 unified 后仍为 Infer）时，用实参
    // 类型定型，使返回类型不再泄漏 `_`。
    // 注意：定型须用「原始参数类型」unify（Generic 分支按名精准绑定）。若用
    // substitute 后的 pty（Infer），unify 的 Infer 分支会「替换 subst 中所有
    // Infer」——多类型参数场景（如 `insert("a", 1)` 的 HashMap<K, V>）会把
    // K、V 都定型为第一个实参类型，导致后续参数误报类型不匹配。
    let mut hir_args = vec![recv_hir];
    let mut arg_tys = Vec::with_capacity(args.len());
    let raw_params: Vec<&Type> = method_def.sig.params.iter().skip(1).collect();
    for (raw_p, a) in raw_params.iter().zip(args.iter()) {
        let pty = substitute(raw_p, &subst);
        // H2 无捕获闭包实参：形参为 fn 类型且实参为闭包 → 按预期签名检查
        // （闭包参数无类型注解，无法脱离 fn 上下文推断参数类型）。
        // T1a：泛型方法 fn 形参（如 `Vec::sort_by(cmp: fn(T, T) -> i64)`）经
        // substitute Fn 递归替换后为具体签名（fn(i64, i64) -> i64），闭包按
        // 具体参数类型检查。
        let (hir, ty) =
            if matches!(pty, Type::Fn(_)) && matches!(&*a.kind, ExprKind::Closure { .. }) {
                check_closure_expected(ctx, a, &pty, a.span)?
            } else {
                infer_expr(ctx, a)?
            };
        // Str 值实参 → 非 Str 形参自动升级：`m.push_str("!")` / `m.contains("z")`
        // 等 String 形参位置传入字面量 / 绑定字面量的变量时自动构造 String 对象。
        // 升级后 contains_infer 定型分支的 `Type::Str` 特判不再命中（已是 String），
        // 泛型 K 定型结果不变（Str → String），行为与 `String::from(lit)` 一致。
        // （push_str 字面量实参的整体特判在 check_method_call 方法级进行）
        let (hir, ty) = upgrade_str_arg(ctx, hir, ty, &pty, a)?;
        arg_tys.push(ty.clone());
        if contains_infer(&pty) {
            // 字符串字面量实参定型为 String（与 `String::from(lit)` 语义一致）：
            // 字面量是 &str 视图，直接定型为 Str 会让后续把 String 槽当视图读（崩溃）；
            // 定型为 String 后，实参检查会给出清晰的「expects String」提示（需 String::from）。
            let bind_ty = match &ty {
                Type::Str => Type::Named("String".to_string(), Vec::new()),
                _ => ty.clone(),
            };
            unify(raw_p, &bind_ty, &mut subst)?;
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

/// H4 去虚拟化：`dyn` 绑定变量（`let d: dyn Trait = &obj;`）的方法调用
/// 静态分派到具体类型实现，替代 vtable 间接调用。
///
/// 静态调用可被 LLVM 内联 / 常量折叠（vtable 调用在循环中受间接调用屏障
/// 阻止优化）；dyn 变量被重新赋值时映射已失效，自动回退 vtable。
/// 任何检查失败一律返回 `None`，由调用方走 vtable 分支（错误信息保持一致）。
fn devirtualize_dyn_call(
    ctx: &mut TypeContext,
    var: &str,
    trait_name: &str,
    method: &str,
    recv_hir: HirExpr,
    args: &[AstExpr],
    span: Span,
) -> Result<Option<(HirExpr, Type)>, TypeError> {
    let Some(concrete_ty) = ctx.get_dyn_concrete(var).cloned() else {
        return Ok(None);
    };
    // 找 `impl Trait for 具体类型`（与 coerce_to_dyn 相同的匹配规则）
    let Some(impl_def) = ctx
        .impl_defs
        .iter()
        .find(|d| d.trait_name.as_deref() == Some(trait_name) && d.self_type == concrete_ty)
        .cloned()
    else {
        return Ok(None);
    };
    // 具体实现方法 → mono 符号（含模块前缀 / 泛型实例化）
    let Some(impl_method) = impl_def.methods.iter().find(|im| im.sig.name == method) else {
        return Ok(None);
    };
    let subst = HashMap::new();
    let fn_name = match instantiate_impl_method(ctx, &impl_def, impl_method, &subst, span) {
        Ok(n) => n,
        Err(_) => return Ok(None), // 实例化失败 → 回退 vtable（vtable 分支报错）
    };
    let sig = &impl_method.sig;
    // 含 `Self` 的签名无法静态确定（vtable 分支报 Unsupported，此处回退）
    if sig.params.iter().skip(1).any(type_mentions_self) || type_mentions_self(&sig.return_type) {
        return Ok(None);
    }
    // 参数数量（vtable 分支报错）
    if args.len() + 1 != sig.params.len() {
        return Ok(None);
    }
    // 实参类型检查（与 vtable 分支一致；`&str` 实参升级）
    let mut hir_args = Vec::with_capacity(args.len());
    for (i, a) in args.iter().enumerate() {
        let (h, t) = infer_expr(ctx, a)?;
        let pty = substitute(&sig.params[i + 1], &HashMap::new());
        let (h, t) = upgrade_str_arg(ctx, h, t, &pty, a)?;
        if !t.compatible_with(&pty) {
            return Ok(None);
        }
        hir_args.push(h);
    }
    // 数据指针 = 胖指针槽 0（与 vtable 分支一致）
    let data = HirExpr::FieldGet {
        base: Box::new(recv_hir),
        index: 0,
        ty: FieldScalar::Ptr,
    };
    let mut call_args = Vec::with_capacity(1 + hir_args.len());
    call_args.push(data);
    call_args.extend(hir_args);
    Ok(Some((
        HirExpr::Call {
            callee: fn_name,
            args: call_args,
        },
        sig.return_type.clone(),
    )))
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
        // 保守升级：仅形参为具体 String 时升级（`fn f<T>(s: String, x: T)` 的
        // `f("hi", 1)`）；泛型 T 位置保持 Str 值推断（`f("hi")` → T = str 字面量值），
        // 不改变既有泛型推断结果。
        let (hir, ty) = if matches!(&ty, Type::Str)
            && matches!(pty, Type::Named(n, _) if n == "String")
        {
            check_string_from(ctx, std::slice::from_ref(arg), arg.span)?
        } else {
            (hir, ty)
        };
        unify(pty, &ty, &mut subst)?;
        hir_args.push(hir);
    }

    // U3：泛型约束调用点校验（宽松：不推导，仅检查已由实参确定的类型参数）
    check_generic_bounds(ctx, &template.bounds, &subst, span)?;

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
        // Str 值实参 → 非 Str 形参自动升级（与第一个循环一致，仅取类型做兼容检查）
        let ty = if matches!(&ty, Type::Str) && !matches!(pty, Type::Str) {
            let (_, t2) = check_string_from(ctx, std::slice::from_ref(arg), arg.span)?;
            t2
        } else {
            ty
        };
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

/// U3：校验泛型实参满足声明约束（宽松校验：不推导，仅检查已由实参确定的类型参数；
/// 未确定的泛型参数跳过；bound 必须是已声明 trait）。
fn check_generic_bounds(
    ctx: &TypeContext,
    bounds: &HashMap<String, Vec<String>>,
    subst: &HashMap<String, Type>,
    span: Span,
) -> Result<(), TypeError> {
    for (param, bound_list) in bounds {
        let concrete = match subst.get(param) {
            Some(t) => t,
            None => continue, // 未从实参确定（不推导，跳过）
        };
        for bound in bound_list {
            // 把 bound 解析为 trait_defs 的完整键：裸名可能经 import 提升到
            // 根命名空间，但 trait_defs 以模块路径（`future::Future`）注册，
            // 需按 Display 名末段匹配（与 `impl Trait for T` / `dyn Trait` 的
            // 裸名解析一致）。
            let full = resolve_trait_def_name(ctx, bound);
            if !ctx.trait_defs.contains_key(&full) {
                return Err(TypeError::UndefinedType {
                    name: bound.clone(),
                    span,
                });
            }
            if !type_implements_trait(ctx, &full, concrete) {
                return Err(TypeError::GenericBoundMismatch {
                    param: param.clone(),
                    bound: bound.clone(),
                    ty: concrete.to_string(),
                    span,
                });
            }
        }
    }
    Ok(())
}

/// 把 bound / trait 裸名解析为 `trait_defs` 中的完整键。
///
/// 优先级：① 本身就是键（裸名或完整路径）；② use 导入别名（`use shape::Shape` 后
/// `T: Shape`）；③ 按 Display 名末段匹配（std `import future::Future` 提升的裸名，
/// 键为 `future::Future`）。找不到时原样返回，由调用方报 UndefinedType。
fn resolve_trait_def_name(ctx: &TypeContext, name: &str) -> String {
    if ctx.trait_defs.contains_key(name) {
        return name.to_string();
    }
    if let Some(full) = ctx
        .use_aliases
        .get(name)
        .filter(|f| ctx.trait_defs.contains_key(*f))
    {
        return full.clone();
    }
    ctx.trait_defs
        .keys()
        .find(|k| k.rsplit("::").next() == Some(name))
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

/// 具体类型是否实现指定 trait（U3：impl 的 `trait_name` 匹配 + 目标类型匹配；
/// 基本类型按 Display 名与 impl 的 `Named` 目标比较，如 `impl PartialEq for i64`）。
fn type_implements_trait(ctx: &TypeContext, bound: &str, concrete: &Type) -> bool {
    ctx.impl_defs.iter().any(|imp| {
        if imp.trait_name.as_deref() != Some(bound) {
            return false;
        }
        let Type::Named(iname, _) = &imp.self_type else {
            return false;
        };
        match concrete {
            Type::Named(cname, _) => iname == cname,
            t => iname == &t.to_string(),
        }
    })
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
    let body_ast = method_def.body.clone().ok_or_else(|| TypeError::Unsupported {
        what: "无函数体的抽象方法被调用".to_string(),
        span,
    })?;
    // U7：impl 泛型（T）+ 方法泛型（U）的确定值并入后缀——方法泛型 U 已在
    // `check_method_call`（7244-7250）并入 subst，但原先 mono 键/后缀只含
    // impl 泛型，不同 U 调用（`bar<i64>` vs `bar<str>`）产生相同键 → 缓存命中
    // 复用首次实例，U 被错误绑定。此处把方法泛型参数并入后缀/键，消除碰撞。
    let mut mono_parts: Vec<String> = impl_def
        .type_params
        .iter()
        .map(|tp| {
            subst
                .get(tp)
                .map(type_mono_key)
                .unwrap_or_else(|| tp.clone())
        })
        .collect();
    for mp in &body_ast.generics {
        mono_parts.push(
            subst
                .get(&mp.name)
                .map(type_mono_key)
                .unwrap_or_else(|| mp.name.clone()),
        );
    }
    let suffix = mono_parts.join("_");
    let mono_name = if suffix.is_empty() {
        base_fn.clone()
    } else {
        format!("{base_fn}__{suffix}")
    };
    let key = format!("{base_fn}#{suffix}");
    if ctx.mono_instances.contains_key(&key) {
        return Ok(mono_name);
    }

    let mut cloned = body_ast.clone();
    cloned.name = mono_name.clone();
    cloned.generics.clear();

    ctx.mono_instances.insert(key, mono_name.clone());

    let saved_params = std::mem::take(&mut ctx.type_params);
    let saved_subst = std::mem::take(&mut ctx.generic_subst);
    let saved_assoc = std::mem::take(&mut ctx.assoc_types);
    ctx.type_params = impl_def.type_params.clone();
    ctx.generic_subst = subst.clone();
    // 关联类型映射（U2）：`Self::Item` 签名重解析 / 方法体检查时替换为
    // impl 定义的具体类型（经泛型替换）。
    ctx.assoc_types = impl_def
        .assoc_types
        .iter()
        .map(|(n, t)| (n.clone(), substitute(t, subst)))
        .collect();

    // self 参数类型：impl 方法签名的首个参数（`&self` 层级已含）经替换。
    // 静态方法（无 self 参数）传 None。
    let self_param = method_def.sig.params.first().map(|p| substitute(p, subst));
    // U4：方法体检查时 `Self` 类型解析为 impl 目标类型（经泛型替换）
    let saved_self = ctx.self_type.clone();
    ctx.self_type = Some(substitute(&impl_def.self_type, subst));
    let sig = crate::check_item::fn_signature_with_self(ctx, &cloned, self_param.as_ref(), span)?;
    let body = crate::check_item::check_fn_body_with_self(ctx, &cloned, self_param.as_ref())?;
    ctx.self_type = saved_self;

    ctx.type_params = saved_params;
    ctx.generic_subst = saved_subst;
    ctx.assoc_types = saved_assoc;

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
        // T1a：函数类型递归统一（fn 形参含泛型类型参数时按实参 fn 签名反推，
        // 与 substitute 的 Fn 递归替换配套——此前静默接受不反推，导致
        // `Vec::sort_by(cmp: fn(T, T) -> i64)` 传 `fn(i64, i64) -> i64` 报错）。
        Type::Fn(sig) => {
            if let Type::Fn(asig) = arg {
                for (p, a) in sig.params.iter().zip(asig.params.iter()) {
                    unify(p, a, subst)?;
                }
                unify(&sig.return_type, &asig.return_type, subst)?;
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

// ===== 内置格式化宏（I2：`{}` 占位符格式化引擎）=====

/// 检查内置格式化宏调用（`println!` / `print!` / `format!` / `dbg!`）。
fn check_macro_call(
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

/// 格式串片段：字面量段 或 值占位段（`{}` → Display，`{:?}` → Debug）。
struct FormatSeg {
    text: String,
    is_value: bool,
    /// `{:?}` → true（Q3b：Debug 路径；`{}` → false，Display 路径）
    is_debug: bool,
}

/// 解析格式串占位符：`{}`（Display）、`{:?}`（Debug）、`{{`/`}}` 转义。
fn parse_format_string(s: &str, span: Span) -> Result<Vec<FormatSeg>, TypeError> {
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

/// `String::from(字面量)` 调用 AST。
fn string_from_lit_ast(s: String, span: Span) -> AstExpr {
    let callee = AstExpr::new(
        ExprKind::Path(vec!["String".to_string(), "from".to_string()]),
        span,
    );
    let arg = AstExpr::new(ExprKind::StringLiteral(s), span);
    AstExpr::new(ExprKind::Call { callee, args: vec![arg], type_args: Vec::new() }, span)
}

/// 简单标识符调用 AST（`int_to_string(x)` / `json_escape(s)` 等）。
fn mk_ident_call(name: String, args: Vec<AstExpr>, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(ExprKind::Ident(name), span),
            args,
            type_args: Vec::new(),
        },
        span,
    )
}

/// 路径调用 AST（`String::from(x)` / `json.stringify(x)`）。
fn mk_path_call(segments: Vec<String>, args: Vec<AstExpr>, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(ExprKind::Path(segments), span),
            args,
            type_args: Vec::new(),
        },
        span,
    )
}

/// `a + b` 拼接 AST。
fn bin_add(left: AstExpr, right: AstExpr, span: Span) -> AstExpr {
    AstExpr::new(ExprKind::Binary { op: BinaryOp::Add, left, right }, span)
}

/// 折叠拼接：`p0 + p1 + ...`。
fn fold_add(parts: Vec<AstExpr>, span: Span) -> AstExpr {
    parts
        .into_iter()
        .reduce(|a, b| bin_add(a, b, span))
        .expect("parts 非空")
}

/// L2 `json.stringify(v)` → JSON 文本（编译器内建，AST 层 desugar，零新增 IR 节点）。
///
/// 支持类型：`i64` / `bool` / `String` / `&str` / 数组 `[T; N]` / `Vec<T>` / 结构体（嵌套递归）；
/// `HashMap<K, V>`（键限 `i64` / `String`，值递归；输出 `{"k":v,...}`，遍历顺序 = 哈希槽序）；
/// `f64` 报 Unsupported（规划）。
fn check_json_stringify(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "json.stringify".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    // 预推断参数类型（供递归 desugar 使用）
    let (_, ty) = infer_expr(ctx, &args[0])?;
    let text_ast = json_serialize_ast(ctx, &ty, &args[0], span)?;
    let (hir, _) = infer_expr(ctx, &text_ast)?;
    Ok((hir, Type::Named("String".to_string(), Vec::new())))
}

/// 递归 JSON 序列化 AST 构建。
fn json_serialize_ast(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    span: Span,
) -> Result<AstExpr, TypeError> {
    match ty {
        // i64 → `int_to_string(x)`（std）
        Type::I64 => Ok(mk_ident_call(
            "int_to_string".to_string(),
            vec![arg.clone()],
            span,
        )),
        // bool → `if b { "true" } else { "false" }`
        Type::Bool => {
            let mk = |s: &str| string_from_lit_ast(s.to_string(), span);
            Ok(AstExpr::new(
                ExprKind::If {
                    cond: arg.clone(),
                    then_block: AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(mk("true")),
                        span,
                    },
                    else_block: Some(AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(mk("false")),
                        span,
                    }),
                },
                span,
            ))
        }
        // String → `"` + json_escape(s) + `"`（json_escape 在 core.rl）
        Type::Named(n, _) if n == "String" => {
            let quote = |s: &str| string_from_lit_ast(s.to_string(), span);
            let esc = mk_ident_call("json_escape".to_string(), vec![arg.clone()], span);
            Ok(fold_add(vec![quote("\""), esc, quote("\"")], span))
        }
        // &str / 字符串字面量 → `"` + json_escape(String::from(arg)) + `"`
        Type::Str => {
            let quote = |s: &str| string_from_lit_ast(s.to_string(), span);
            let sf = mk_path_call(
                vec!["String".to_string(), "from".to_string()],
                vec![arg.clone()],
                span,
            );
            let esc = mk_ident_call("json_escape".to_string(), vec![sf], span);
            Ok(fold_add(vec![quote("\""), esc, quote("\"")], span))
        }
        // 数组 `[T; N]`：静态展开 `[e0,e1,...]`（长度编译期已知）
        Type::Array(elem, len) => {
            let mut parts = vec![string_from_lit_ast("[".to_string(), span)];
            for i in 0..*len {
                if i > 0 {
                    parts.push(string_from_lit_ast(",".to_string(), span));
                }
                let idx = AstExpr::new(
                    ExprKind::Index {
                        expr: arg.clone(),
                        index: AstExpr::new(ExprKind::IntLiteral(i as i128), span),
                    },
                    span,
                );
                parts.push(json_serialize_ast(ctx, elem, &idx, span)?);
            }
            parts.push(string_from_lit_ast("]".to_string(), span));
            Ok(fold_add(parts, span))
        }
        // Vec<T> → while 循环构建 `[e0,e1,...]`：
        // 注意：必须置于 struct 分支之前——`Vec` 本身是 std struct（data/len/cap 字段），
        // 若先命中 lookup_struct 分支会被误序列化为 `{"data":...,"len":...,"cap":...}`。
        // `{ let mut __o = String::new(); __o.push_str("["); let mut __i = 0;
        //    while __i < v.len() { if __i > 0 { __o.push_str(","); }
        //                           __o.push_str(json.stringify(v[__i])); __i = __i + 1; }
        //    __o.push_str("]"); __o }`
        Type::Named(n, args) if n == "Vec" && args.len() == 1 => {
            // 元素类型定型：`vec![...]` 字面量绑定后为 `Vec<Infer>`（push 不反向精化接收者
            // 类型），Infer 无法确定元素序列化路径——与 check_for_vec 一致，要求上下文 /
            // 显式注解定型（如 `let v: Vec<i64> = vec![1, 2, 3]`）。
            let elem_ty = substitute(&args[0], &ctx.generic_subst);
            if matches!(elem_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "json.stringify：Vec 元素类型未确定（如 `let v: Vec<i64> = vec![...]` 注解）"
                        .to_string(),
                    span,
                });
            }
            let out_name = ctx.fresh_temp();
            let i_name = ctx.fresh_temp();
            let out_id = AstExpr::new(ExprKind::Ident(out_name.clone()), span);
            let i_id = AstExpr::new(ExprKind::Ident(i_name.clone()), span);
            let push = |recv: AstExpr, val: AstExpr| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: "push_str".to_string(),
                        args: vec![val],
                    },
                    span,
                )
            };
            let mut body_stmts = vec![
                AstStmt::Let {
                    pattern: AstPattern::Ident(out_name),
                    type_anno: None,
                    init: mk_path_call(
                        vec!["String".to_string(), "new".to_string()],
                        Vec::new(),
                        span,
                    ),
                    mutable: true,
                },
                AstStmt::Let {
                    pattern: AstPattern::Ident(i_name),
                    type_anno: None,
                    init: AstExpr::new(ExprKind::IntLiteral(0), span),
                    mutable: true,
                },
                AstStmt::Semi(push(out_id.clone(), string_from_lit_ast("[".to_string(), span))),
            ];
            // while __i < v.len()
            let len_call = AstExpr::new(
                ExprKind::MethodCall {
                    receiver: arg.clone(),
                    method: "len".to_string(),
                    args: Vec::new(),
                },
                span,
            );
            let cond = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![i_id.clone(), len_call],
                    operators: vec![CompareOp::Lt],
                },
                span,
            );
            let mut loop_stmts = Vec::new();
            // if __i > 0 { __o.push_str(",") }
            let gt_zero = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![
                        i_id.clone(),
                        AstExpr::new(ExprKind::IntLiteral(0), span),
                    ],
                    operators: vec![CompareOp::Gt],
                },
                span,
            );
            loop_stmts.push(AstStmt::Semi(AstExpr::new(
                ExprKind::If {
                    cond: gt_zero,
                    then_block: AstBlock {
                        stmts: vec![AstStmt::Semi(push(
                            out_id.clone(),
                            string_from_lit_ast(",".to_string(), span),
                        ))],
                        final_expr: None,
                        span,
                    },
                    else_block: None,
                },
                span,
            )));
            // __o.push_str(json.stringify(v[__i]))
            let idx = AstExpr::new(
                ExprKind::Index {
                    expr: arg.clone(),
                    index: i_id.clone(),
                },
                span,
            );
            let json_elem = mk_path_call(
                vec!["json".to_string(), "stringify".to_string()],
                vec![idx],
                span,
            );
            loop_stmts.push(AstStmt::Semi(push(out_id.clone(), json_elem)));
            // __i = __i + 1
            loop_stmts.push(AstStmt::Semi(AstExpr::new(
                ExprKind::Assign {
                    target: i_id.clone(),
                    op: AssignOp::Assign,
                    value: bin_add(
                        i_id.clone(),
                        AstExpr::new(ExprKind::IntLiteral(1), span),
                        span,
                    ),
                },
                span,
            )));
            body_stmts.push(AstStmt::Semi(AstExpr::new(
                ExprKind::While {
                    cond,
                    body: AstBlock {
                        stmts: loop_stmts,
                        final_expr: None,
                        span,
                    },
                },
                span,
            )));
            body_stmts.push(AstStmt::Semi(push(
                out_id.clone(),
                string_from_lit_ast("]".to_string(), span),
            )));
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: body_stmts,
                    final_expr: Some(out_id),
                    span,
                }),
                span,
            ))
        }
        // HashMap<K, V> → `{"k":v,...}`（键转 JSON 字符串键；值递归）。
        // desugar 为块表达式 + `for (k, v) in m`（check_for_hashmap 槽位遍历）：
        // `{ let mut __o = String::new(); let mut __first = 1; __o.push_str("{");
        //    for (k, v) in m {
        //      if __first > 0 { __first = 0; } else { __o.push_str(","); }
        //      __o.push_str(<键>); __o.push_str(":"); __o.push_str(<值>);
        //    }
        //    __o.push_str("}"); __o }`
        // 键：i64 → `"` + int_to_string(k) + `"`；String → `json.stringify(k)`（自带引号 + 转义）。
        // 值：递归 `json_serialize_ast`（支持嵌套 HashMap / Vec / struct / 数组）。
        // 注意：须置于 struct 分支之前——HashMap 本身是 std struct（keys/vals/states 槽），
        // 若先命中 lookup_struct 分支会被误序列化为 `{"keys":...,"vals":...}`。
        Type::Named(n, args) if n == "HashMap" && args.len() == 2 => {
            let k_ty = substitute(&args[0], &ctx.generic_subst);
            let v_ty = substitute(&args[1], &ctx.generic_subst);
            if matches!(k_ty, Type::Infer) || matches!(v_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "json.stringify：HashMap 键/值类型未确定（如 `let m: HashMap<i64, i64> = map![...]` 注解定型）".to_string(),
                    span,
                });
            }
            let key_is_i64 = matches!(k_ty, Type::I64);
            let key_is_string = matches!(&k_ty, Type::Named(kn, _) if kn == "String");
            if !key_is_i64 && !key_is_string {
                return Err(TypeError::Unsupported {
                    what: format!("json.stringify：HashMap 键类型 `{k_ty}`（MVP 支持 i64 / String）"),
                    span,
                });
            }
            let out_name = ctx.fresh_temp();
            let first_name = ctx.fresh_temp();
            let k_name = ctx.fresh_temp();
            let v_name = ctx.fresh_temp();
            let out_id = AstExpr::new(ExprKind::Ident(out_name.clone()), span);
            let first_id = AstExpr::new(ExprKind::Ident(first_name.clone()), span);
            let k_id = AstExpr::new(ExprKind::Ident(k_name.clone()), span);
            let v_id = AstExpr::new(ExprKind::Ident(v_name.clone()), span);
            let push = |recv: AstExpr, val: AstExpr| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: "push_str".to_string(),
                        args: vec![val],
                    },
                    span,
                )
            };
            // 键序列化：i64 → `"` + int_to_string(k) + `"`；String → `json.stringify(k)`
            let key_ser = if key_is_i64 {
                fold_add(
                    vec![
                        string_from_lit_ast("\"".to_string(), span),
                        mk_ident_call("int_to_string".to_string(), vec![k_id.clone()], span),
                        string_from_lit_ast("\"".to_string(), span),
                    ],
                    span,
                )
            } else {
                mk_path_call(
                    vec!["json".to_string(), "stringify".to_string()],
                    vec![k_id.clone()],
                    span,
                )
            };
            // 值序列化（递归）
            let val_ser = json_serialize_ast(ctx, &v_ty, &v_id, span)?;
            let mut loop_stmts = Vec::new();
            // if __first > 0 { __first = 0 } else { __o.push_str(",") }
            let first_gt_zero = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![
                        first_id.clone(),
                        AstExpr::new(ExprKind::IntLiteral(0), span),
                    ],
                    operators: vec![CompareOp::Gt],
                },
                span,
            );
            loop_stmts.push(AstStmt::Semi(AstExpr::new(
                ExprKind::If {
                    cond: first_gt_zero,
                    then_block: AstBlock {
                        stmts: vec![AstStmt::Semi(AstExpr::new(
                            ExprKind::Assign {
                                target: first_id.clone(),
                                op: AssignOp::Assign,
                                value: AstExpr::new(ExprKind::IntLiteral(0), span),
                            },
                            span,
                        ))],
                        final_expr: None,
                        span,
                    },
                    else_block: Some(AstBlock {
                        stmts: vec![AstStmt::Semi(push(
                            out_id.clone(),
                            string_from_lit_ast(",".to_string(), span),
                        ))],
                        final_expr: None,
                        span,
                    }),
                },
                span,
            )));
            // __o.push_str(<键>); __o.push_str(":"); __o.push_str(<值>)
            loop_stmts.push(AstStmt::Semi(push(out_id.clone(), key_ser)));
            loop_stmts.push(AstStmt::Semi(push(
                out_id.clone(),
                string_from_lit_ast(":".to_string(), span),
            )));
            loop_stmts.push(AstStmt::Semi(push(out_id.clone(), val_ser)));
            // for (k, v) in m { ... }
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Tuple(vec![
                        AstPattern::Ident(k_name.clone()),
                        AstPattern::Ident(v_name.clone()),
                    ]),
                    iterator: arg.clone(),
                    body: AstBlock {
                        stmts: loop_stmts,
                        final_expr: None,
                        span,
                    },
                },
                span,
            );
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(out_name),
                            type_anno: None,
                            init: mk_path_call(
                                vec!["String".to_string(), "new".to_string()],
                                Vec::new(),
                                span,
                            ),
                            mutable: true,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(first_name),
                            type_anno: None,
                            init: AstExpr::new(ExprKind::IntLiteral(1), span),
                            mutable: true,
                        },
                        AstStmt::Semi(push(
                            out_id.clone(),
                            string_from_lit_ast("{".to_string(), span),
                        )),
                        AstStmt::Semi(for_expr),
                        AstStmt::Semi(push(
                            out_id.clone(),
                            string_from_lit_ast("}".to_string(), span),
                        )),
                    ],
                    final_expr: Some(out_id),
                    span,
                }),
                span,
            ))
        }
        // 结构体 → `{"f1":v1,"f2":v2}`（字段序 = 定义序）。
        // guard 排除 Vec / HashMap（两者是 std struct 但各有专用分支，须先于本分支命中）。
        Type::Named(name, _)
            if ctx.lookup_struct(name).is_some()
                && ctx
                    .resolve_full_name(name)
                    .unwrap_or_else(|| name.clone())
                    != "Vec"
                && ctx
                    .resolve_full_name(name)
                    .unwrap_or_else(|| name.clone())
                    != "HashMap" =>
        {
            let def = ctx.lookup_struct(name).cloned().ok_or_else(|| {
                TypeError::UndefinedType {
                    name: name.clone(),
                    span,
                }
            })?;
            let mut parts = Vec::new();
            for (i, (fname, fty_ast)) in def.fields.iter().enumerate() {
                let prefix = if i == 0 {
                    format!("{{\"{}\":", fname)
                } else {
                    format!(",\"{}\":", fname)
                };
                parts.push(string_from_lit_ast(prefix, span));
                let farg = AstExpr::new(
                    ExprKind::FieldAccess {
                        expr: arg.clone(),
                        field: fname.clone(),
                    },
                    span,
                );
                parts.push(json_serialize_ast(ctx, fty_ast, &farg, span)?);
            }
            parts.push(string_from_lit_ast("}".to_string(), span));
            Ok(fold_add(parts, span))
        }
        other => Err(TypeError::Unsupported {
            what: format!("json.stringify：类型 `{other}` 序列化"),
            span,
        }),
    }
}

/// L2 `json.parse::<T>(s)` → T（编译器内建，AST 层 desugar）。
///
/// MVP 支持：`i64` / `bool` / `String`（引号剥离 + 转义还原）；数组 / Vec / 结构体反序列化规划。
fn check_json_parse(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "json.parse".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    if type_args.len() != 1 {
        return Err(TypeError::Unsupported {
            what: "json.parse 需要 1 个类型实参（turbofish `json.parse::<T>(s)`）".to_string(),
            span,
        });
    }
    let target = resolve_ast_type(ctx, &type_args[0], span)?;
    let parse_ast = json_parse_ast(ctx, &target, &args[0], span)?;
    let (hir, _) = infer_expr(ctx, &parse_ast)?;
    Ok((hir, target))
}

/// 递归 JSON 反序列化 AST 构建（MVP：标量 + String）。
fn json_parse_ast(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    span: Span,
) -> Result<AstExpr, TypeError> {
    // 统一实参转 String（JSON 文本实参可为字符串字面量 / String / &str）
    let s = mk_path_call(
        vec!["String".to_string(), "from".to_string()],
        vec![arg.clone()],
        span,
    );
    match ty {
        // i64 → `string_to_int(s)`（std；JSON 数字文本无引号，MVP 直接解析）
        Type::I64 => Ok(mk_ident_call(
            "string_to_int".to_string(),
            vec![s],
            span,
        )),
        // bool → `if s == "true" { true } else { false }`（String 内容相等 → bytes_eq）
        Type::Bool => {
            let eq = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![s, string_from_lit_ast("true".to_string(), span)],
                    operators: vec![CompareOp::Eq],
                },
                span,
            );
            Ok(AstExpr::new(
                ExprKind::If {
                    cond: eq,
                    then_block: AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(AstExpr::new(ExprKind::BoolLiteral(true), span)),
                        span,
                    },
                    else_block: Some(AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(AstExpr::new(ExprKind::BoolLiteral(false), span)),
                        span,
                    }),
                },
                span,
            ))
        }
        // String → `json_unescape(s)`（引号剥离 + 转义还原，core.rl）
        Type::Named(n, _) if n == "String" => Ok(mk_ident_call(
            "json_unescape".to_string(),
            vec![s],
            span,
        )),
        // HashMap<K, V> → JSON 对象 `{"k":v,...}` 反序列化。
        // desugar 为块表达式 + `for x in vec`（check_for_vec 遍历 split 结果）：
        // `{ let __s = String::from(<arg>);                 // JSON 文本
        //    let __body = __s.substring(1, __s.len() - 1);  // 剥离首尾 { }
        //    let __parts = __body.split(",");               // 逗号分段（键/值含逗号 MVP 限制）
        //    let mut __m: HashMap<K, V> = HashMap::new();   // 注解定型（空对象 {} 亦定型）
        //    for __part in __parts {
        //      let __c = __part.find(":");
        //      if __c >= 0 {
        //        let __kpart = __part.substring(0, __c);
        //        let __vpart = __part.substring(__c + 1, __part.len());
        //        let __k = <键解析>;                          // i64: string_to_int(json_unescape(..))
        //                                                      // String: json_unescape(..)
        //        let __v = <值解析，递归 json_parse_ast>;
        //        __m.insert(__k, __v);
        //      }
        //    }
        //    __m }`
        // 键限 i64 / String（JSON 键恒为带引号字符串，如 `"1"`——先 json_unescape 剥引号，
        // i64 再经 string_to_int 转整数）；值限标量（i64 / bool / String）。嵌套 HashMap 值
        // MVP 显式 Unsupported（split(",") 分段无法正确处理内层逗号）；数组 / Vec / 结构体值
        // 经 json_parse_ast 递归自然落 Unsupported。须置于 struct 分支之前（见 stringify）。
        Type::Named(n, args) if n == "HashMap" && args.len() == 2 => {
            let k_ty = substitute(&args[0], &ctx.generic_subst);
            let v_ty = substitute(&args[1], &ctx.generic_subst);
            if matches!(k_ty, Type::Infer) || matches!(v_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "json.parse：HashMap 键/值类型未确定（turbofish 显式定型，如 `json.parse::<HashMap<i64, i64>>(s)`）".to_string(),
                    span,
                });
            }
            let key_is_i64 = matches!(k_ty, Type::I64);
            let key_is_string = matches!(&k_ty, Type::Named(kn, _) if kn == "String");
            if !key_is_i64 && !key_is_string {
                return Err(TypeError::Unsupported {
                    what: format!("json.parse：HashMap 键类型 `{k_ty}`（MVP 支持 i64 / String）"),
                    span,
                });
            }
            if matches!(&v_ty, Type::Named(vn, _) if vn == "HashMap") {
                return Err(TypeError::Unsupported {
                    what: "json.parse：嵌套 HashMap 值反序列化（MVP 支持标量值 i64 / bool / String）"
                        .to_string(),
                    span,
                });
            }
            // 临时变量：JSON 文本 / 剥离后主体 / 逗号分段 / 结果 map / 段 / 冒号下标 /
            // 键段 / 值段 / 键 / 值
            let s_name = ctx.fresh_temp();
            let body_name = ctx.fresh_temp();
            let parts_name = ctx.fresh_temp();
            let m_name = ctx.fresh_temp();
            let part_name = ctx.fresh_temp();
            let c_name = ctx.fresh_temp();
            let kpart_name = ctx.fresh_temp();
            let vpart_name = ctx.fresh_temp();
            let k_name = ctx.fresh_temp();
            let v_name = ctx.fresh_temp();
            let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
            let body_id = AstExpr::new(ExprKind::Ident(body_name.clone()), span);
            let parts_id = AstExpr::new(ExprKind::Ident(parts_name.clone()), span);
            let m_id = AstExpr::new(ExprKind::Ident(m_name.clone()), span);
            let part_id = AstExpr::new(ExprKind::Ident(part_name.clone()), span);
            let c_id = AstExpr::new(ExprKind::Ident(c_name.clone()), span);
            let kpart_id = AstExpr::new(ExprKind::Ident(kpart_name.clone()), span);
            let vpart_id = AstExpr::new(ExprKind::Ident(vpart_name.clone()), span);
            let k_id = AstExpr::new(ExprKind::Ident(k_name.clone()), span);
            let v_id = AstExpr::new(ExprKind::Ident(v_name.clone()), span);
            // `recv.method(args)` 方法调用 AST
            let mcall = |recv: AstExpr, method: &str, args: Vec<AstExpr>| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: method.to_string(),
                        args,
                    },
                    span,
                )
            };
            // 键解析：JSON 键恒带引号 → `json_unescape(__kpart)` 剥引号 + 还原转义；
            // i64 键再经 `string_to_int` 转整数
            let unescaped_key =
                mk_ident_call("json_unescape".to_string(), vec![kpart_id.clone()], span);
            let key_parse = if key_is_i64 {
                mk_ident_call("string_to_int".to_string(), vec![unescaped_key], span)
            } else {
                unescaped_key
            };
            // 值解析（递归；标量 i64 / bool / String，其余落 Unsupported）
            let val_parse = json_parse_ast(ctx, &v_ty, &vpart_id, span)?;
            // if __c >= 0 { ... __m.insert(__k, __v) }
            let c_ge_zero = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![c_id.clone(), AstExpr::new(ExprKind::IntLiteral(0), span)],
                    operators: vec![CompareOp::Ge],
                },
                span,
            );
            let if_parse = AstExpr::new(
                ExprKind::If {
                    cond: c_ge_zero,
                    then_block: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(kpart_name),
                                type_anno: None,
                                init: mcall(
                                    part_id.clone(),
                                    "substring",
                                    vec![
                                        AstExpr::new(ExprKind::IntLiteral(0), span),
                                        c_id.clone(),
                                    ],
                                ),
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(vpart_name),
                                type_anno: None,
                                init: mcall(
                                    part_id.clone(),
                                    "substring",
                                    vec![
                                        AstExpr::new(
                                            ExprKind::Binary {
                                                op: BinaryOp::Add,
                                                left: c_id.clone(),
                                                right: AstExpr::new(
                                                    ExprKind::IntLiteral(1),
                                                    span,
                                                ),
                                            },
                                            span,
                                        ),
                                        mcall(part_id.clone(), "len", Vec::new()),
                                    ],
                                ),
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(k_name),
                                type_anno: None,
                                init: key_parse,
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(v_name),
                                type_anno: None,
                                init: val_parse,
                                mutable: false,
                            },
                            AstStmt::Semi(mcall(
                                m_id.clone(),
                                "insert",
                                vec![k_id, v_id],
                            )),
                        ],
                        final_expr: None,
                        span,
                    },
                    else_block: None,
                },
                span,
            );
            // for __part in __parts { let __c = __part.find(":"); <if 解析+insert> }
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Ident(part_name),
                    iterator: parts_id.clone(),
                    body: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(c_name),
                                type_anno: None,
                                init: mcall(
                                    part_id,
                                    "find",
                                    vec![string_from_lit_ast(":".to_string(), span)],
                                ),
                                mutable: false,
                            },
                            AstStmt::Semi(if_parse),
                        ],
                        final_expr: None,
                        span,
                    },
                },
                span,
            );
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(s_name),
                            type_anno: None,
                            init: mk_path_call(
                                vec!["String".to_string(), "from".to_string()],
                                vec![arg.clone()],
                                span,
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(body_name),
                            type_anno: None,
                            init: mcall(
                                s_id.clone(),
                                "substring",
                                vec![
                                    AstExpr::new(ExprKind::IntLiteral(1), span),
                                    AstExpr::new(
                                        ExprKind::Binary {
                                            op: BinaryOp::Sub,
                                            left: mcall(s_id, "len", Vec::new()),
                                            right: AstExpr::new(ExprKind::IntLiteral(1), span),
                                        },
                                        span,
                                    ),
                                ],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(parts_name),
                            type_anno: None,
                            init: mcall(
                                body_id,
                                "split",
                                vec![string_from_lit_ast(",".to_string(), span)],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(m_name),
                            type_anno: Some(AstType::Path(
                                "HashMap".to_string(),
                                vec![ty_to_ast(&k_ty), ty_to_ast(&v_ty)],
                            )),
                            init: mk_path_call(
                                vec!["HashMap".to_string(), "new".to_string()],
                                Vec::new(),
                                span,
                            ),
                            mutable: true,
                        },
                        AstStmt::Semi(for_expr),
                    ],
                    final_expr: Some(m_id),
                    span,
                }),
                span,
            ))
        }
        // 用户 struct → JSON 对象 `{"f0":v0,"f1":v1}` 反序列化（字段名匹配，顺序无关，
        // 缺失字段保持零值，未知字段忽略）。desugar 为块表达式 + `for x in vec`
        // （复用 check_for_vec 遍历 split 结果）：
        // `{ let __s = String::from(<arg>);                 // JSON 文本
        //    let __body = __s.substring(1, __s.len() - 1);  // 剥离首尾 { }
        //    let __parts = __body.split(",");               // 逗号分段
        //    let mut __p: Point = Point { x: 0, y: 0 };     // 零值构造
        //    for __part in __parts {
        //      let __c = __part.find(":");
        //      if __c >= 0 {
        //        let __name = json_unescape(__part.substring(0, __c));  // 字段名（剥引号）
        //        let __val = __part.substring(__c + 1, __part.len());
        //        if __name == "x" { __p.x = <字段 x 值解析>; }   // 递归 json_parse_ast
        //        else if __name == "y" { __p.y = <字段 y 值解析>; }
        //        else { }                                        // 未知字段忽略
        //      }
        //    }
        //    __p }`
        // 字段值限 json_parse_ast 支持类型（i64 / bool / String / 嵌套 struct）；
        // 嵌套 struct / Vec / HashMap 值含逗号经 split(",") 分段错误的 MVP 限制
        // （与 HashMap 分支一致）；泛型 struct（带类型实参）MVP 不支持（零值与字段
        // 解析需按实参定型）。
        Type::Named(n, args) if ctx.lookup_struct(&n).is_some() && args.is_empty() => {
            let def = ctx.lookup_struct(&n).cloned().unwrap();
            if def.fields.is_empty() {
                return Err(TypeError::Unsupported {
                    what: format!("json.parse：空结构体 `{n}` 反序列化"),
                    span,
                });
            }
            // 临时变量：JSON 文本 / 剥离后主体 / 逗号分段 / 结果 struct / 段 /
            // 冒号下标 / 字段名 / 值段
            let s_name = ctx.fresh_temp();
            let body_name = ctx.fresh_temp();
            let parts_name = ctx.fresh_temp();
            let p_name = ctx.fresh_temp();
            let part_name = ctx.fresh_temp();
            let c_name = ctx.fresh_temp();
            let name_name = ctx.fresh_temp();
            let val_name = ctx.fresh_temp();
            let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
            let body_id = AstExpr::new(ExprKind::Ident(body_name.clone()), span);
            let parts_id = AstExpr::new(ExprKind::Ident(parts_name.clone()), span);
            let p_id = AstExpr::new(ExprKind::Ident(p_name.clone()), span);
            let part_id = AstExpr::new(ExprKind::Ident(part_name.clone()), span);
            let c_id = AstExpr::new(ExprKind::Ident(c_name.clone()), span);
            let name_id = AstExpr::new(ExprKind::Ident(name_name.clone()), span);
            let val_id = AstExpr::new(ExprKind::Ident(val_name.clone()), span);
            // `recv.method(args)` 方法调用 AST
            let mcall = |recv: AstExpr, method: &str, args: Vec<AstExpr>| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: method.to_string(),
                        args,
                    },
                    span,
                )
            };
            // 字段值解析（递归 json_parse_ast）
            let field_parse = |ctx: &mut TypeContext, fty: &Type| {
                json_parse_ast(ctx, fty, &val_id.clone(), span)
            };
            // 零值构造（缺失字段保持零值）
            let mut zero_fields = Vec::new();
            for (fname, fty) in &def.fields {
                zero_fields.push((fname.clone(), ty_to_zero_ast(ctx, fty, span)?));
            }
            let zero_ctor = AstExpr::new(
                ExprKind::StructCtor {
                    type_name: vec![n.clone()],
                    type_args: Vec::new(),
                    fields: zero_fields,
                },
                span,
            );
            // if-else 链：`if __name == "x" { __p.x = <解析>; } else if ... else { }`
            // 自后向前构建；最内层 else 为空块（未知字段忽略）
            let mut chain: Option<AstExpr> = None;
            for (fname, fty) in def.fields.iter().rev() {
                let name_eq = AstExpr::new(
                    ExprKind::ComparisonChain {
                        elements: vec![
                            name_id.clone(),
                            string_from_lit_ast(fname.clone(), span),
                        ],
                        operators: vec![CompareOp::Eq],
                    },
                    span,
                );
                let inner = chain.take();
                let then_block = AstBlock {
                    stmts: vec![AstStmt::Semi(AstExpr::new(
                        ExprKind::Assign {
                            target: AstExpr::new(
                                ExprKind::FieldAccess {
                                    expr: p_id.clone(),
                                    field: fname.clone(),
                                },
                                span,
                            ),
                            op: AssignOp::Assign,
                            value: field_parse(ctx, fty)?,
                        },
                        span,
                    ))],
                    final_expr: None,
                    span,
                };
                chain = Some(AstExpr::new(
                    ExprKind::If {
                        cond: name_eq,
                        then_block,
                        else_block: Some(AstBlock {
                            stmts: Vec::new(),
                            final_expr: inner,
                            span,
                        }),
                    },
                    span,
                ));
            }
            let if_chain = chain.expect("struct 至少一个字段");
            // `if __c >= 0 { let __name = ...; let __val = ...; <if 链> }`
            let c_ge_zero = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![c_id.clone(), AstExpr::new(ExprKind::IntLiteral(0), span)],
                    operators: vec![CompareOp::Ge],
                },
                span,
            );
            let if_parse = AstExpr::new(
                ExprKind::If {
                    cond: c_ge_zero,
                    then_block: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(name_name),
                                type_anno: None,
                                init: mk_ident_call(
                                    "json_unescape".to_string(),
                                    vec![mcall(
                                        part_id.clone(),
                                        "substring",
                                        vec![
                                            AstExpr::new(ExprKind::IntLiteral(0), span),
                                            c_id.clone(),
                                        ],
                                    )],
                                    span,
                                ),
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(val_name),
                                type_anno: None,
                                init: mcall(
                                    part_id.clone(),
                                    "substring",
                                    vec![
                                        AstExpr::new(
                                            ExprKind::Binary {
                                                op: BinaryOp::Add,
                                                left: c_id.clone(),
                                                right: AstExpr::new(
                                                    ExprKind::IntLiteral(1),
                                                    span,
                                                ),
                                            },
                                            span,
                                        ),
                                        mcall(part_id.clone(), "len", Vec::new()),
                                    ],
                                ),
                                mutable: false,
                            },
                            AstStmt::Semi(if_chain),
                        ],
                        final_expr: None,
                        span,
                    },
                    else_block: None,
                },
                span,
            );
            // `for __part in __parts { let __c = __part.find(":"); <if 解析> }`
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Ident(part_name),
                    iterator: parts_id.clone(),
                    body: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(c_name),
                                type_anno: None,
                                init: mcall(
                                    part_id,
                                    "find",
                                    vec![string_from_lit_ast(":".to_string(), span)],
                                ),
                                mutable: false,
                            },
                            AstStmt::Semi(if_parse),
                        ],
                        final_expr: None,
                        span,
                    },
                },
                span,
            );
            // 顶层块：`{ let __s; let __body; let __parts; let mut __p; <for>; __p }`
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(s_name),
                            type_anno: None,
                            init: mk_path_call(
                                vec!["String".to_string(), "from".to_string()],
                                vec![arg.clone()],
                                span,
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(body_name),
                            type_anno: None,
                            init: mcall(
                                s_id.clone(),
                                "substring",
                                vec![
                                    AstExpr::new(ExprKind::IntLiteral(1), span),
                                    AstExpr::new(
                                        ExprKind::Binary {
                                            op: BinaryOp::Sub,
                                            left: mcall(s_id, "len", Vec::new()),
                                            right: AstExpr::new(
                                                ExprKind::IntLiteral(1),
                                                span,
                                            ),
                                        },
                                        span,
                                    ),
                                ],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(parts_name),
                            type_anno: None,
                            init: mcall(
                                body_id,
                                "split",
                                vec![string_from_lit_ast(",".to_string(), span)],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(p_name),
                            type_anno: Some(AstType::Path(n.clone(), Vec::new())),
                            init: zero_ctor,
                            mutable: true,
                        },
                        AstStmt::Semi(for_expr),
                    ],
                    final_expr: Some(p_id),
                    span,
                }),
                span,
            ))
        }
        other => Err(TypeError::Unsupported {
            what: format!("json.parse：类型 `{other}` 反序列化"),
            span,
        }),
    }
}

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
fn check_toml_stringify(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "toml.stringify".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let (_, ty) = infer_expr(ctx, &args[0])?;
    let text_ast = toml_serialize_ast(ctx, &ty, &args[0], span, true)?;
    let (hir, _) = infer_expr(ctx, &text_ast)?;
    Ok((hir, Type::Named("String".to_string(), Vec::new())))
}

/// 递归 TOML 序列化 AST 构建。`top_level`：顶层结构体输出多行 `key = value`（标准 TOML
/// 顶层键值对），嵌套字段输出内联表 `{ ... }`。
fn toml_serialize_ast(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    span: Span,
    top_level: bool,
) -> Result<AstExpr, TypeError> {
    match ty {
        // i64 → `int_to_string(x)`（std）
        Type::I64 => Ok(mk_ident_call(
            "int_to_string".to_string(),
            vec![arg.clone()],
            span,
        )),
        // bool → `if b { "true" } else { "false" }`
        Type::Bool => {
            let mk = |s: &str| string_from_lit_ast(s.to_string(), span);
            Ok(AstExpr::new(
                ExprKind::If {
                    cond: arg.clone(),
                    then_block: AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(mk("true")),
                        span,
                    },
                    else_block: Some(AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(mk("false")),
                        span,
                    }),
                },
                span,
            ))
        }
        // String → `"` + json_escape(s) + `"`（TOML 基本转义与 JSON 一致，复用）
        Type::Named(n, _) if n == "String" => {
            let quote = |s: &str| string_from_lit_ast(s.to_string(), span);
            let esc = mk_ident_call("json_escape".to_string(), vec![arg.clone()], span);
            Ok(fold_add(vec![quote("\""), esc, quote("\"")], span))
        }
        // &str / 字符串字面量 → `"` + json_escape(String::from(arg)) + `"`
        Type::Str => {
            let quote = |s: &str| string_from_lit_ast(s.to_string(), span);
            let sf = mk_path_call(
                vec!["String".to_string(), "from".to_string()],
                vec![arg.clone()],
                span,
            );
            let esc = mk_ident_call("json_escape".to_string(), vec![sf], span);
            Ok(fold_add(vec![quote("\""), esc, quote("\"")], span))
        }
        // 数组 `[T; N]`：静态展开 `[e0,e1,...]`（长度编译期已知）
        Type::Array(elem, len) => {
            let mut parts = vec![string_from_lit_ast("[".to_string(), span)];
            for i in 0..*len {
                if i > 0 {
                    parts.push(string_from_lit_ast(",".to_string(), span));
                }
                let idx = AstExpr::new(
                    ExprKind::Index {
                        expr: arg.clone(),
                        index: AstExpr::new(ExprKind::IntLiteral(i as i128), span),
                    },
                    span,
                );
                parts.push(toml_serialize_ast(ctx, elem, &idx, span, false)?);
            }
            parts.push(string_from_lit_ast("]".to_string(), span));
            Ok(fold_add(parts, span))
        }
        // Vec<T> → while 循环构建 `[e0,e1,...]`（须置于 struct 分支之前——Vec 是 std struct）
        Type::Named(n, args) if n == "Vec" && args.len() == 1 => {
            let elem_ty = substitute(&args[0], &ctx.generic_subst);
            if matches!(elem_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "toml.stringify：Vec 元素类型未确定（如 `let v: Vec<i64> = vec![...]` 注解）"
                        .to_string(),
                    span,
                });
            }
            let out_name = ctx.fresh_temp();
            let i_name = ctx.fresh_temp();
            let out_id = AstExpr::new(ExprKind::Ident(out_name.clone()), span);
            let i_id = AstExpr::new(ExprKind::Ident(i_name.clone()), span);
            let push = |recv: AstExpr, val: AstExpr| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: "push_str".to_string(),
                        args: vec![val],
                    },
                    span,
                )
            };
            let mut body_stmts = vec![
                AstStmt::Let {
                    pattern: AstPattern::Ident(out_name),
                    type_anno: None,
                    init: mk_path_call(
                        vec!["String".to_string(), "new".to_string()],
                        Vec::new(),
                        span,
                    ),
                    mutable: true,
                },
                AstStmt::Let {
                    pattern: AstPattern::Ident(i_name),
                    type_anno: None,
                    init: AstExpr::new(ExprKind::IntLiteral(0), span),
                    mutable: true,
                },
                AstStmt::Semi(push(out_id.clone(), string_from_lit_ast("[".to_string(), span))),
            ];
            // while __i < v.len()
            let len_call = AstExpr::new(
                ExprKind::MethodCall {
                    receiver: arg.clone(),
                    method: "len".to_string(),
                    args: Vec::new(),
                },
                span,
            );
            let cond = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![i_id.clone(), len_call],
                    operators: vec![CompareOp::Lt],
                },
                span,
            );
            let mut loop_stmts = Vec::new();
            // if __i > 0 { __o.push_str(",") }
            let gt_zero = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![
                        i_id.clone(),
                        AstExpr::new(ExprKind::IntLiteral(0), span),
                    ],
                    operators: vec![CompareOp::Gt],
                },
                span,
            );
            loop_stmts.push(AstStmt::Semi(AstExpr::new(
                ExprKind::If {
                    cond: gt_zero,
                    then_block: AstBlock {
                        stmts: vec![AstStmt::Semi(push(
                            out_id.clone(),
                            string_from_lit_ast(",".to_string(), span),
                        ))],
                        final_expr: None,
                        span,
                    },
                    else_block: None,
                },
                span,
            )));
            // __o.push_str(<元素递归>)
            let idx = AstExpr::new(
                ExprKind::Index {
                    expr: arg.clone(),
                    index: i_id.clone(),
                },
                span,
            );
            let elem_ast = toml_serialize_ast(ctx, &elem_ty, &idx, span, false)?;
            loop_stmts.push(AstStmt::Semi(push(out_id.clone(), elem_ast)));
            // __i = __i + 1
            loop_stmts.push(AstStmt::Semi(AstExpr::new(
                ExprKind::Assign {
                    target: i_id.clone(),
                    op: AssignOp::Assign,
                    value: bin_add(
                        i_id.clone(),
                        AstExpr::new(ExprKind::IntLiteral(1), span),
                        span,
                    ),
                },
                span,
            )));
            body_stmts.push(AstStmt::Semi(AstExpr::new(
                ExprKind::While {
                    cond,
                    body: AstBlock {
                        stmts: loop_stmts,
                        final_expr: None,
                        span,
                    },
                },
                span,
            )));
            body_stmts.push(AstStmt::Semi(push(
                out_id.clone(),
                string_from_lit_ast("]".to_string(), span),
            )));
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: body_stmts,
                    final_expr: Some(out_id),
                    span,
                }),
                span,
            ))
        }
        // HashMap<K, V> → 内联表 `{"k" = v, ...}`（键带引号，TOML 合法；值递归）。
        // desugar 为块表达式 + `for (k, v) in m`（check_for_hashmap 槽位遍历）：
        // `{ let mut __o = String::new(); let mut __first = 1; __o.push_str("{");
        //    for (k, v) in m {
        //      if __first > 0 { __first = 0; } else { __o.push_str(","); }
        //      __o.push_str(<键>); __o.push_str(" = "); __o.push_str(<值>);
        //    }
        //    __o.push_str("}"); __o }`
        // 键：i64 → `"` + int_to_string(k) + `"`；String → json_escape(k)（自带引号）。
        // 值：递归 `toml_serialize_ast`（top_level=false）。须置于 struct 分支之前（同 json）。
        Type::Named(n, args) if n == "HashMap" && args.len() == 2 => {
            let k_ty = substitute(&args[0], &ctx.generic_subst);
            let v_ty = substitute(&args[1], &ctx.generic_subst);
            if matches!(k_ty, Type::Infer) || matches!(v_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "toml.stringify：HashMap 键/值类型未确定（如 `let m: HashMap<i64, i64> = map![...]` 注解定型）".to_string(),
                    span,
                });
            }
            let key_is_i64 = matches!(k_ty, Type::I64);
            let key_is_string = matches!(&k_ty, Type::Named(kn, _) if kn == "String");
            if !key_is_i64 && !key_is_string {
                return Err(TypeError::Unsupported {
                    what: format!("toml.stringify：HashMap 键类型 `{k_ty}`（MVP 支持 i64 / String）"),
                    span,
                });
            }
            let out_name = ctx.fresh_temp();
            let first_name = ctx.fresh_temp();
            let k_name = ctx.fresh_temp();
            let v_name = ctx.fresh_temp();
            let out_id = AstExpr::new(ExprKind::Ident(out_name.clone()), span);
            let first_id = AstExpr::new(ExprKind::Ident(first_name.clone()), span);
            let k_id = AstExpr::new(ExprKind::Ident(k_name.clone()), span);
            let v_id = AstExpr::new(ExprKind::Ident(v_name.clone()), span);
            let push = |recv: AstExpr, val: AstExpr| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: "push_str".to_string(),
                        args: vec![val],
                    },
                    span,
                )
            };
            // 键序列化：i64 → `"` + int_to_string(k) + `"`；String → json_escape(k)
            let key_ser = if key_is_i64 {
                fold_add(
                    vec![
                        string_from_lit_ast("\"".to_string(), span),
                        mk_ident_call("int_to_string".to_string(), vec![k_id.clone()], span),
                        string_from_lit_ast("\"".to_string(), span),
                    ],
                    span,
                )
            } else {
                mk_ident_call("json_escape".to_string(), vec![k_id.clone()], span)
            };
            let val_ser = toml_serialize_ast(ctx, &v_ty, &v_id, span, false)?;
            // `if __first > 0 { __first = 0 } else { __o.push_str(",") }`
            let first_gt_zero = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![
                        first_id.clone(),
                        AstExpr::new(ExprKind::IntLiteral(0), span),
                    ],
                    operators: vec![CompareOp::Gt],
                },
                span,
            );
            let mut loop_stmts = vec![AstStmt::Semi(AstExpr::new(
                ExprKind::If {
                    cond: first_gt_zero,
                    then_block: AstBlock {
                        stmts: vec![AstStmt::Semi(AstExpr::new(
                            ExprKind::Assign {
                                target: first_id.clone(),
                                op: AssignOp::Assign,
                                value: AstExpr::new(ExprKind::IntLiteral(0), span),
                            },
                            span,
                        ))],
                        final_expr: None,
                        span,
                    },
                    else_block: Some(AstBlock {
                        stmts: vec![AstStmt::Semi(push(
                            out_id.clone(),
                            string_from_lit_ast(",".to_string(), span),
                        ))],
                        final_expr: None,
                        span,
                    }),
                },
                span,
            ))];
            // __o.push_str(<键>); __o.push_str(" = "); __o.push_str(<值>)
            loop_stmts.push(AstStmt::Semi(push(
                out_id.clone(),
                key_ser,
            )));
            loop_stmts.push(AstStmt::Semi(push(
                out_id.clone(),
                string_from_lit_ast("=".to_string(), span),
            )));
            loop_stmts.push(AstStmt::Semi(push(out_id.clone(), val_ser)));
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Tuple(vec![
                        AstPattern::Ident(k_name),
                        AstPattern::Ident(v_name),
                    ]),
                    iterator: arg.clone(),
                    body: AstBlock {
                        stmts: loop_stmts,
                        final_expr: None,
                        span,
                    },
                },
                span,
            );
            let body_stmts = vec![
                AstStmt::Let {
                    pattern: AstPattern::Ident(out_name),
                    type_anno: None,
                    init: mk_path_call(
                        vec!["String".to_string(), "new".to_string()],
                        Vec::new(),
                        span,
                    ),
                    mutable: true,
                },
                AstStmt::Let {
                    pattern: AstPattern::Ident(first_name),
                    type_anno: None,
                    init: AstExpr::new(ExprKind::IntLiteral(1), span),
                    mutable: true,
                },
                AstStmt::Semi(push(
                    out_id.clone(),
                    string_from_lit_ast("{".to_string(), span),
                )),
                AstStmt::Semi(for_expr),
                AstStmt::Semi(push(
                    out_id.clone(),
                    string_from_lit_ast("}".to_string(), span),
                )),
            ];
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: body_stmts,
                    final_expr: Some(out_id),
                    span,
                }),
                span,
            ))
        }
        // 结构体（嵌套递归）。
        // guard 排除 Vec / HashMap（两者是 std struct 但各有专用分支，须先于本分支命中）。
        Type::Named(name, _)
            if ctx.lookup_struct(name).is_some()
                && ctx
                    .resolve_full_name(name)
                    .unwrap_or_else(|| name.clone())
                    != "Vec"
                && ctx
                    .resolve_full_name(name)
                    .unwrap_or_else(|| name.clone())
                    != "HashMap" =>
        {
            let def = ctx.lookup_struct(name).cloned().ok_or_else(|| {
                TypeError::UndefinedType {
                    name: name.clone(),
                    span,
                }
            })?;
            if def.fields.is_empty() {
                // 顶层空结构体 → 空文本；嵌套空结构体 → `{}` 内联表
                if top_level {
                    return Ok(mk_path_call(
                        vec!["String".to_string(), "new".to_string()],
                        Vec::new(),
                        span,
                    ));
                }
                return Ok(string_from_lit_ast("{}".to_string(), span));
            }
            // 字段访问 AST：`arg.field`
            let field_arg = |fname: &str| {
                AstExpr::new(
                    ExprKind::FieldAccess {
                        expr: arg.clone(),
                        field: fname.to_string(),
                    },
                    span,
                )
            };
            // 顶层：多行 `f1 = v1\nf2 = v2`（字段序 = 定义序）；嵌套：内联表 `{f1 = v1,f2 = v2}`
            let mut parts = if top_level {
                Vec::new()
            } else {
                vec![string_from_lit_ast("{".to_string(), span)]
            };
            for (i, (fname, fty_ast)) in def.fields.iter().enumerate() {
                if i > 0 {
                    let sep = if top_level { "\n" } else { "," };
                    parts.push(string_from_lit_ast(sep.to_string(), span));
                }
                parts.push(string_from_lit_ast(format!("{fname}="), span));
                let farg = field_arg(fname);
                parts.push(toml_serialize_ast(ctx, fty_ast, &farg, span, false)?);
            }
            if !top_level {
                parts.push(string_from_lit_ast("}".to_string(), span));
            }
            Ok(fold_add(parts, span))
        }
        other => Err(TypeError::Unsupported {
            what: format!("toml.stringify：类型 `{other}` 序列化"),
            span,
        }),
    }
}

/// Q4 `toml.from_str::<T>(s)` / `toml.parse::<T>(s)` → T（编译器内建，AST 层 desugar）。
///
/// MVP 支持（round-trip 对齐 `toml::to_string` 的紧凑输出，无多余空白）：
/// `i64` / `bool` / `String`（引号剥离 + 转义还原）；`Vec<T>`（数组 `[e1,e2]`，元素限
/// 标量）；`HashMap<K, V>`（内联表 `{"k" = v, ...}`，键/值限标量）；结构体（顶层多行
/// `key = value` / 嵌套内联表 `{ ... }`）。数组类型 `[T; N]` 与 f64 报 Unsupported。
fn check_toml_parse(
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

/// 递归 TOML 反序列化 AST 构建。`inline`：结构体目标的文本形态——`false` 顶层多行
/// （`key = value` 行，`\n` 分隔，不剥括号）；`true` 内联表（`{ ... }`，剥首尾 `{ }`，
/// `,` 分隔）。标量 / 数组 / HashMap 分支忽略 `inline`（数组自身剥 `[ ]`，HashMap 恒为
/// 内联表）。
fn toml_parse_ast(
    ctx: &mut TypeContext,
    ty: &Type,
    arg: &AstExpr,
    span: Span,
    inline: bool,
) -> Result<AstExpr, TypeError> {
    // 统一实参转 String（TOML 文本实参可为字符串字面量 / String / &str）
    let s = mk_path_call(
        vec!["String".to_string(), "from".to_string()],
        vec![arg.clone()],
        span,
    );
    match ty {
        // i64 → `string_to_int(s)`（std；数字文本无引号，MVP 直接解析）
        Type::I64 => Ok(mk_ident_call(
            "string_to_int".to_string(),
            vec![s],
            span,
        )),
        // bool → `if s == "true" { true } else { false }`
        Type::Bool => {
            let eq = AstExpr::new(
                ExprKind::ComparisonChain {
                    elements: vec![s, string_from_lit_ast("true".to_string(), span)],
                    operators: vec![CompareOp::Eq],
                },
                span,
            );
            Ok(AstExpr::new(
                ExprKind::If {
                    cond: eq,
                    then_block: AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(AstExpr::new(ExprKind::BoolLiteral(true), span)),
                        span,
                    },
                    else_block: Some(AstBlock {
                        stmts: Vec::new(),
                        final_expr: Some(AstExpr::new(ExprKind::BoolLiteral(false), span)),
                        span,
                    }),
                },
                span,
            ))
        }
        // String → `json_unescape(s)`（引号剥离 + 转义还原，core.rl）
        Type::Named(n, _) if n == "String" => Ok(mk_ident_call(
            "json_unescape".to_string(),
            vec![s],
            span,
        )),
        // Vec<T> → 数组 `[e1,e2]` 反序列化（元素限标量 i64 / bool / String）。
        // desugar 为块表达式 + `for x in vec`：
        // `{ let __s = String::from(<arg>);                 // TOML 文本
        //    let __body = __s.substring(1, __s.len() - 1);  // 剥离首尾 [ ]
        //    let __parts = __body.split(",");               // 逗号分段
        //    let mut __v: Vec<T> = Vec::new();              // 注解定型（空数组 [] 亦定型）
        //    for __part in __parts {
        //      let __e = <元素递归 toml_parse_ast>;
        //      __v.push(__e);
        //    }
        //    __v }`
        Type::Named(n, args) if n == "Vec" && args.len() == 1 => {
            let elem_ty = substitute(&args[0], &ctx.generic_subst);
            if matches!(elem_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "toml.parse：Vec 元素类型未确定（turbofish 显式定型，如 `toml.parse::<Vec<i64>>(s)`）".to_string(),
                    span,
                });
            }
            let s_name = ctx.fresh_temp();
            let body_name = ctx.fresh_temp();
            let parts_name = ctx.fresh_temp();
            let v_name = ctx.fresh_temp();
            let part_name = ctx.fresh_temp();
            let e_name = ctx.fresh_temp();
            let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
            let body_id = AstExpr::new(ExprKind::Ident(body_name.clone()), span);
            let parts_id = AstExpr::new(ExprKind::Ident(parts_name.clone()), span);
            let v_id = AstExpr::new(ExprKind::Ident(v_name.clone()), span);
            let part_id = AstExpr::new(ExprKind::Ident(part_name.clone()), span);
            let e_id = AstExpr::new(ExprKind::Ident(e_name.clone()), span);
            let mcall = |recv: AstExpr, method: &str, args: Vec<AstExpr>| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: method.to_string(),
                        args,
                    },
                    span,
                )
            };
            let elem_parse = toml_parse_ast(ctx, &elem_ty, &part_id.clone(), span, false)?;
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Ident(part_name),
                    iterator: parts_id.clone(),
                    body: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(e_name),
                                type_anno: None,
                                init: elem_parse,
                                mutable: false,
                            },
                            AstStmt::Semi(AstExpr::new(
                                ExprKind::MethodCall {
                                    receiver: v_id.clone(),
                                    method: "push".to_string(),
                                    args: vec![e_id],
                                },
                                span,
                            )),
                        ],
                        final_expr: None,
                        span,
                    },
                },
                span,
            );
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(s_name),
                            type_anno: None,
                            init: s,
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(body_name),
                            type_anno: None,
                            init: mcall(
                                s_id.clone(),
                                "substring",
                                vec![
                                    AstExpr::new(ExprKind::IntLiteral(1), span),
                                    AstExpr::new(
                                        ExprKind::Binary {
                                            op: BinaryOp::Sub,
                                            left: mcall(s_id, "len", Vec::new()),
                                            right: AstExpr::new(ExprKind::IntLiteral(1), span),
                                        },
                                        span,
                                    ),
                                ],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(parts_name),
                            type_anno: None,
                            init: mcall(
                                body_id,
                                "split",
                                vec![string_from_lit_ast(",".to_string(), span)],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(v_name),
                            type_anno: Some(AstType::Path(
                                "Vec".to_string(),
                                vec![ty_to_ast(&elem_ty)],
                            )),
                            init: mk_path_call(
                                vec!["Vec".to_string(), "new".to_string()],
                                Vec::new(),
                                span,
                            ),
                            mutable: true,
                        },
                        AstStmt::Semi(for_expr),
                    ],
                    final_expr: Some(v_id),
                    span,
                }),
                span,
            ))
        }
        // HashMap<K, V> → 内联表 `{"k" = v, ...}` 反序列化（键/值限标量）。
        // 与 json.parse 的 HashMap 分支同构，仅分隔符 `:` → ` = ` 与键剥引号路径一致：
        // `{ let __s = String::from(<arg>);
        //    let __body = __s.substring(1, __s.len() - 1);  // 剥离首尾 { }
        //    let __parts = __body.split(",");
        //    let mut __m: HashMap<K, V> = HashMap::new();
        //    for __part in __parts {
        //      let __c = __part.find(" = ");               // 等号分隔（键/值含等号 MVP 限制）
        //      if __c >= 0 {
        //        let __kpart = __part.substring(0, __c);
        //        let __vpart = __part.substring(__c + 1, __part.len());
        //        let __k = <键解析>;                        // i64: string_to_int(json_unescape(kpart))
        //                                                    // String: json_unescape(kpart)
        //        let __v = <值解析，递归标量>;
        //        __m.insert(__k, __v);
        //      }
        //    }
        //    __m }`
        // 注意：stringify 键为带引号（`"1"` / `"a"`）——键段先 json_unescape 剥引号，
        // i64 再经 string_to_int 转整数。须置于 struct 分支之前（同 stringify）。
        Type::Named(n, args) if n == "HashMap" && args.len() == 2 => {
            let k_ty = substitute(&args[0], &ctx.generic_subst);
            let v_ty = substitute(&args[1], &ctx.generic_subst);
            if matches!(k_ty, Type::Infer) || matches!(v_ty, Type::Infer) {
                return Err(TypeError::Unsupported {
                    what: "toml.parse：HashMap 键/值类型未确定（turbofish 显式定型，如 `toml.parse::<HashMap<i64, i64>>(s)`）".to_string(),
                    span,
                });
            }
            let key_is_i64 = matches!(k_ty, Type::I64);
            let key_is_string = matches!(&k_ty, Type::Named(kn, _) if kn == "String");
            if !key_is_i64 && !key_is_string {
                return Err(TypeError::Unsupported {
                    what: format!("toml.parse：HashMap 键类型 `{k_ty}`（MVP 支持 i64 / String）"),
                    span,
                });
            }
            let s_name = ctx.fresh_temp();
            let body_name = ctx.fresh_temp();
            let parts_name = ctx.fresh_temp();
            let m_name = ctx.fresh_temp();
            let part_name = ctx.fresh_temp();
            let c_name = ctx.fresh_temp();
            let kpart_name = ctx.fresh_temp();
            let vpart_name = ctx.fresh_temp();
            let k_name = ctx.fresh_temp();
            let v_name = ctx.fresh_temp();
            let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
            let body_id = AstExpr::new(ExprKind::Ident(body_name.clone()), span);
            let parts_id = AstExpr::new(ExprKind::Ident(parts_name.clone()), span);
            let m_id = AstExpr::new(ExprKind::Ident(m_name.clone()), span);
            let part_id = AstExpr::new(ExprKind::Ident(part_name.clone()), span);
            let c_id = AstExpr::new(ExprKind::Ident(c_name.clone()), span);
            let kpart_id = AstExpr::new(ExprKind::Ident(kpart_name.clone()), span);
            let vpart_id = AstExpr::new(ExprKind::Ident(vpart_name.clone()), span);
            let k_id = AstExpr::new(ExprKind::Ident(k_name.clone()), span);
            let v_id = AstExpr::new(ExprKind::Ident(v_name.clone()), span);
            let mcall = |recv: AstExpr, method: &str, args: Vec<AstExpr>| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: method.to_string(),
                        args,
                    },
                    span,
                )
            };
            // 键解析：i64 → `string_to_int(json_unescape(kpart))`；String → `json_unescape(kpart)`
            let k_unescaped = mk_ident_call("json_unescape".to_string(), vec![kpart_id.clone()], span);
            let key_parse = if key_is_i64 {
                mk_ident_call("string_to_int".to_string(), vec![k_unescaped], span)
            } else {
                k_unescaped
            };
            let val_parse = toml_parse_ast(ctx, &v_ty, &vpart_id.clone(), span, false)?;
            let if_parse = AstExpr::new(
                ExprKind::If {
                    cond: AstExpr::new(
                        ExprKind::ComparisonChain {
                            elements: vec![c_id.clone(), AstExpr::new(ExprKind::IntLiteral(0), span)],
                            operators: vec![CompareOp::Ge],
                        },
                        span,
                    ),
                    then_block: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(kpart_name),
                                type_anno: None,
                                init: mcall(
                                    part_id.clone(),
                                    "substring",
                                    vec![
                                        AstExpr::new(ExprKind::IntLiteral(0), span),
                                        c_id.clone(),
                                    ],
                                ),
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(vpart_name),
                                type_anno: None,
                                init: mcall(
                                    part_id.clone(),
                                    "substring",
                                    vec![
                                        AstExpr::new(
                                            ExprKind::Binary {
                                                op: BinaryOp::Add,
                                                left: c_id.clone(),
                                                right: AstExpr::new(ExprKind::IntLiteral(1), span),
                                            },
                                            span,
                                        ),
                                        mcall(part_id.clone(), "len", Vec::new()),
                                    ],
                                ),
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(k_name),
                                type_anno: None,
                                init: key_parse,
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(v_name),
                                type_anno: None,
                                init: val_parse,
                                mutable: false,
                            },
                            AstStmt::Semi(AstExpr::new(
                                ExprKind::MethodCall {
                                    receiver: m_id.clone(),
                                    method: "insert".to_string(),
                                    args: vec![k_id, v_id],
                                },
                                span,
                            )),
                        ],
                        final_expr: None,
                        span,
                    },
                    else_block: None,
                },
                span,
            );
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Ident(part_name),
                    iterator: parts_id.clone(),
                    body: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(c_name),
                                type_anno: None,
                                init: mcall(
                                    part_id,
                                    "find",
                                    vec![string_from_lit_ast("=".to_string(), span)],
                                ),
                                mutable: false,
                            },
                            AstStmt::Semi(if_parse),
                        ],
                        final_expr: None,
                        span,
                    },
                },
                span,
            );
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(s_name),
                            type_anno: None,
                            init: s,
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(body_name),
                            type_anno: None,
                            init: mcall(
                                s_id.clone(),
                                "substring",
                                vec![
                                    AstExpr::new(ExprKind::IntLiteral(1), span),
                                    AstExpr::new(
                                        ExprKind::Binary {
                                            op: BinaryOp::Sub,
                                            left: mcall(s_id, "len", Vec::new()),
                                            right: AstExpr::new(ExprKind::IntLiteral(1), span),
                                        },
                                        span,
                                    ),
                                ],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(parts_name),
                            type_anno: None,
                            init: mcall(
                                body_id,
                                "split",
                                vec![string_from_lit_ast(",".to_string(), span)],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(m_name),
                            type_anno: Some(AstType::Path(
                                "HashMap".to_string(),
                                vec![ty_to_ast(&k_ty), ty_to_ast(&v_ty)],
                            )),
                            init: mk_path_call(
                                vec!["HashMap".to_string(), "new".to_string()],
                                Vec::new(),
                                span,
                            ),
                            mutable: true,
                        },
                        AstStmt::Semi(for_expr),
                    ],
                    final_expr: Some(m_id),
                    span,
                }),
                span,
            ))
        }
        // 结构体 → 顶层多行 `key = value` / 嵌套内联表 `{ ... }` 反序列化。
        // 字段序 = 定义序；缺失字段保持零值，未知字段忽略。desugar 为块表达式 +
        // `for x in vec`（复用 check_for_vec 遍历 split 结果）：
        // `{ let __s = String::from(<arg>);
        //    let __body = <inline ? __s.substring(1, __s.len() - 1) : __s>;  // 内联表剥 { }
        //    let __parts = __body.split(<inline ? "," : "\n">);
        //    let mut __p: Point = Point { x: 0, y: 0 };     // 零值构造
        //    for __part in __parts {
        //      let __c = __part.find("=");
        //      if __c >= 0 {
        //        let __name = __part.substring(0, __c);      // 裸键，直接比较
        //        let __val = __part.substring(__c + 1, __part.len());
        //        if __name == "x" { __p.x = <解析>; } else if ... else { }
        //      }
        //    }
        //    __p }`
        // 字段值限 toml_parse_ast 支持类型（i64 / bool / String / Vec / 嵌套 struct /
        // HashMap）；嵌套 struct / HashMap 值为内联表 `{...}`（含逗号经 split 分段错误
        // 的 MVP 限制与 json 一致）；泛型 struct MVP 不支持（与 json 一致）。
        Type::Named(n, args) if ctx.lookup_struct(&n).is_some() && args.is_empty() => {
            let def = ctx.lookup_struct(&n).cloned().unwrap();
            if def.fields.is_empty() {
                return Err(TypeError::Unsupported {
                    what: format!("toml.parse：空结构体 `{n}` 反序列化"),
                    span,
                });
            }
            let s_name = ctx.fresh_temp();
            let body_name = ctx.fresh_temp();
            let parts_name = ctx.fresh_temp();
            let p_name = ctx.fresh_temp();
            let part_name = ctx.fresh_temp();
            let c_name = ctx.fresh_temp();
            let name_name = ctx.fresh_temp();
            let val_name = ctx.fresh_temp();
            let s_id = AstExpr::new(ExprKind::Ident(s_name.clone()), span);
            let body_id = AstExpr::new(ExprKind::Ident(body_name.clone()), span);
            let parts_id = AstExpr::new(ExprKind::Ident(parts_name.clone()), span);
            let p_id = AstExpr::new(ExprKind::Ident(p_name.clone()), span);
            let part_id = AstExpr::new(ExprKind::Ident(part_name.clone()), span);
            let c_id = AstExpr::new(ExprKind::Ident(c_name.clone()), span);
            let name_id = AstExpr::new(ExprKind::Ident(name_name.clone()), span);
            let val_id = AstExpr::new(ExprKind::Ident(val_name.clone()), span);
            let mcall = |recv: AstExpr, method: &str, args: Vec<AstExpr>| {
                AstExpr::new(
                    ExprKind::MethodCall {
                        receiver: recv,
                        method: method.to_string(),
                        args,
                    },
                    span,
                )
            };
            // 字段值解析（递归 toml_parse_ast，inline=true——嵌套值可能为内联表）
            let field_parse = |ctx: &mut TypeContext, fty: &Type| {
                toml_parse_ast(ctx, fty, &val_id.clone(), span, true)
            };
            // 零值构造（缺失字段保持零值）
            let mut zero_fields = Vec::new();
            for (fname, fty) in &def.fields {
                zero_fields.push((fname.clone(), ty_to_zero_ast(ctx, fty, span)?));
            }
            let zero_ctor = AstExpr::new(
                ExprKind::StructCtor {
                    type_name: vec![n.clone()],
                    type_args: Vec::new(),
                    fields: zero_fields,
                },
                span,
            );
            // if-else 链：`if __name == "x" { __p.x = <解析>; } else if ... else { }`
            // 自后向前构建；最内层 else 为空块（未知字段忽略）
            let mut chain: Option<AstExpr> = None;
            for (fname, fty) in def.fields.iter().rev() {
                let name_eq = AstExpr::new(
                    ExprKind::ComparisonChain {
                        elements: vec![
                            name_id.clone(),
                            string_from_lit_ast(fname.clone(), span),
                        ],
                        operators: vec![CompareOp::Eq],
                    },
                    span,
                );
                let inner = chain.take();
                let then_block = AstBlock {
                    stmts: vec![AstStmt::Semi(AstExpr::new(
                        ExprKind::Assign {
                            target: AstExpr::new(
                                ExprKind::FieldAccess {
                                    expr: p_id.clone(),
                                    field: fname.clone(),
                                },
                                span,
                            ),
                            op: AssignOp::Assign,
                            value: field_parse(ctx, fty)?,
                        },
                        span,
                    ))],
                    final_expr: None,
                    span,
                };
                chain = Some(AstExpr::new(
                    ExprKind::If {
                        cond: name_eq,
                        then_block,
                        else_block: Some(AstBlock {
                            stmts: Vec::new(),
                            final_expr: inner,
                            span,
                        }),
                    },
                    span,
                ));
            }
            let if_chain = chain.expect("struct 至少一个字段");
            // `if __c >= 0 { let __name = ...; let __val = ...; <if 链> }`
            let if_parse = AstExpr::new(
                ExprKind::If {
                    cond: AstExpr::new(
                        ExprKind::ComparisonChain {
                            elements: vec![
                                c_id.clone(),
                                AstExpr::new(ExprKind::IntLiteral(0), span),
                            ],
                            operators: vec![CompareOp::Ge],
                        },
                        span,
                    ),
                    then_block: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(name_name),
                                type_anno: None,
                                init: mcall(
                                    part_id.clone(),
                                    "substring",
                                    vec![
                                        AstExpr::new(ExprKind::IntLiteral(0), span),
                                        c_id.clone(),
                                    ],
                                ),
                                mutable: false,
                            },
                            AstStmt::Let {
                                pattern: AstPattern::Ident(val_name),
                                type_anno: None,
                                init: mcall(
                                    part_id.clone(),
                                    "substring",
                                    vec![
                                        AstExpr::new(
                                            ExprKind::Binary {
                                                op: BinaryOp::Add,
                                                left: c_id.clone(),
                                                right: AstExpr::new(ExprKind::IntLiteral(1), span),
                                            },
                                            span,
                                        ),
                                        mcall(part_id.clone(), "len", Vec::new()),
                                    ],
                                ),
                                mutable: false,
                            },
                            AstStmt::Semi(if_chain),
                        ],
                        final_expr: None,
                        span,
                    },
                    else_block: None,
                },
                span,
            );
            // `for __part in __parts { let __c = __part.find("="); <if 解析> }`
            let for_expr = AstExpr::new(
                ExprKind::For {
                    pattern: AstPattern::Ident(part_name),
                    iterator: parts_id.clone(),
                    body: AstBlock {
                        stmts: vec![
                            AstStmt::Let {
                                pattern: AstPattern::Ident(c_name),
                                type_anno: None,
                                init: mcall(
                                    part_id,
                                    "find",
                                    vec![string_from_lit_ast("=".to_string(), span)],
                                ),
                                mutable: false,
                            },
                            AstStmt::Semi(if_parse),
                        ],
                        final_expr: None,
                        span,
                    },
                },
                span,
            );
            // 主体：inline → `substring(1, len-1)`（剥 { }）；否则原文本
            let body_init = if inline {
                mcall(
                    s_id.clone(),
                    "substring",
                    vec![
                        AstExpr::new(ExprKind::IntLiteral(1), span),
                        AstExpr::new(
                            ExprKind::Binary {
                                op: BinaryOp::Sub,
                                left: mcall(s_id.clone(), "len", Vec::new()),
                                right: AstExpr::new(ExprKind::IntLiteral(1), span),
                            },
                            span,
                        ),
                    ],
                )
            } else {
                s_id.clone()
            };
            let sep = if inline { "," } else { "\n" };
            Ok(AstExpr::new(
                ExprKind::Block(AstBlock {
                    stmts: vec![
                        AstStmt::Let {
                            pattern: AstPattern::Ident(s_name),
                            type_anno: None,
                            init: s,
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(body_name),
                            type_anno: None,
                            init: body_init,
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(parts_name),
                            type_anno: None,
                            init: mcall(
                                body_id,
                                "split",
                                vec![string_from_lit_ast(sep.to_string(), span)],
                            ),
                            mutable: false,
                        },
                        AstStmt::Let {
                            pattern: AstPattern::Ident(p_name),
                            type_anno: Some(AstType::Path(n.clone(), Vec::new())),
                            init: zero_ctor,
                            mutable: true,
                        },
                        AstStmt::Semi(for_expr),
                    ],
                    final_expr: Some(p_id),
                    span,
                }),
                span,
            ))
        }
        other => Err(TypeError::Unsupported {
            what: format!("toml.parse：类型 `{other}` 反序列化"),
            span,
        }),
    }
}

/// Q2b `json.to_writer(w, v)` → `w.write_all(json.stringify(v))`（编译器内建）。
///
/// 参数：`w` 为 `File` / `&File` / `&mut File`（写句柄，方法调用自动剥引用层）；
/// `v` 递归 `check_json_stringify` 序列化。返回 `Result<i64, io::error::IoError>`
/// （`File::write_all` 的返回类型，调用方可 match / `?` 处理）。
fn check_json_to_writer(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 2 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "json.to_writer".to_string(),
            expected: 2,
            found: args.len(),
            span,
        });
    }
    // 首个参数须为 File 写句柄（File / &File / &mut File；名称按短名或路径后缀匹配，
    // 解析后完整名可能为 `io::file::File`）
    let (_w_hir, w_ty) = infer_expr(ctx, &args[0])?;
    let inner = match &w_ty {
        Type::Named(n, _) => Some(n),
        Type::Ref(t, _) => match &**t {
            Type::Named(n, _) => Some(n),
            _ => None,
        },
        _ => None,
    };
    let is_file = inner
        .map(|n| n == "File" || n.ends_with("::File"))
        .unwrap_or(false);
    if !is_file {
        return Err(TypeError::Unsupported {
            what: format!(
                "json.to_writer：首个参数须为 `File`（MVP；TcpStream 留待流式接线），实为 `{w_ty}`"
            ),
            span,
        });
    }
    // 构造 `w.write_all(json.stringify(v))` 调用 AST
    let ser = mk_path_call(
        vec!["json".to_string(), "stringify".to_string()],
        vec![args[1].clone()],
        span,
    );
    let call_ast = AstExpr::new(
        ExprKind::MethodCall {
            receiver: args[0].clone(),
            method: "write_all".to_string(),
            args: vec![ser],
        },
        span,
    );
    infer_expr(ctx, &call_ast)
}

/// Q2b `json.from_reader::<T>(r)` → `json.parse::<T>(r.read_to_string().unwrap())`
/// （编译器内建）。
///
/// 参数：`r` 为 `File` / `&mut File`（读句柄）。读取失败经 `Result::unwrap` 死循环
/// （MVP 语义，与 std `Result::unwrap` 一致）；返回 `T`（须经 turbofish 指定）。
fn check_json_from_reader(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "json.from_reader".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    if type_args.len() != 1 {
        return Err(TypeError::Unsupported {
            what: "json.from_reader：须显式泛型实参 `json.from_reader::<T>(r)`".to_string(),
            span,
        });
    }
    // 首个参数须为 File 读句柄（名称按短名或路径后缀匹配）
    let (_r_hir, r_ty) = infer_expr(ctx, &args[0])?;
    let inner = match &r_ty {
        Type::Named(n, _) => Some(n),
        Type::Ref(t, _) => match &**t {
            Type::Named(n, _) => Some(n),
            _ => None,
        },
        _ => None,
    };
    let is_file = inner
        .map(|n| n == "File" || n.ends_with("::File"))
        .unwrap_or(false);
    if !is_file {
        return Err(TypeError::Unsupported {
            what: format!(
                "json.from_reader：首个参数须为 `File`（MVP；TcpStream 留待流式接线），实为 `{r_ty}`"
            ),
            span,
        });
    }
    // 构造 `r.read_to_string().unwrap()` → `json.parse::<T>(...)` 调用 AST
    let read_ast = AstExpr::new(
        ExprKind::MethodCall {
            receiver: args[0].clone(),
            method: "read_to_string".to_string(),
            args: Vec::new(),
        },
        span,
    );
    let unwrap_ast = AstExpr::new(
        ExprKind::MethodCall {
            receiver: read_ast,
            method: "unwrap".to_string(),
            args: Vec::new(),
        },
        span,
    );
    let parse_ast = AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(
                ExprKind::Path(vec!["json".to_string(), "parse".to_string()]),
                span,
            ),
            args: vec![unwrap_ast],
            type_args: type_args.to_vec(),
        },
        span,
    );
    infer_expr(ctx, &parse_ast)
}

/// 字段类型 → 零值 AST（struct 反序列化零值构造用；缺失字段保持零值）。
fn ty_to_zero_ast(ctx: &TypeContext, ty: &Type, span: Span) -> Result<AstExpr, TypeError> {
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

/// 值 → String 的 AST（`{}` 显示 / `{:?}` Debug；MVP 内建类型走内建转换，
/// 自定义类型经 `Display::fmt` / `Debug::fmt_debug` 方法调用）。
fn value_to_string_ast(
    ctx: &mut TypeContext,
    arg: &AstExpr,
    debug: bool,
    span: Span,
) -> Result<AstExpr, TypeError> {
    let (_, ty) = infer_expr(ctx, arg)?;
    value_to_string_for_ty(ctx, &ty, arg, debug, span)
}

/// 按预推断类型构造值 → String 的 AST（`dbg!` 等场景参数尚未绑定为变量时复用）。
fn value_to_string_for_ty(
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
    // （`{}` → `fmt`，`{:?}` → `fmt_debug`；方法名查找 impl，与手写 impl 调用
    // 路径一致，MVP 无 trait bound 检查）。`fmt` 直接返回 String（拼接模式）。
    let method = if debug { "fmt_debug" } else { "fmt" };
    if ctx.find_impl_for_method(&base, method).is_some() {
        // 块表达式：`let mut __fmt_q3 = Formatter::new(); x.fmt(&mut __fmt_q3)`
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
                method: method.to_string(),
                args: vec![f_ref],
            },
            span,
        );
        return Ok(AstExpr::new(
            ExprKind::Block(AstBlock {
                stmts: vec![bind],
                final_expr: Some(call),
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

/// `println!` / `print!` / `format!`：`{}` 占位符格式化 → String 拼接 + 打印内建。
fn check_format_macro(
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
            value_to_string_ast(ctx, arg, seg.is_debug, span)?
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

/// `dbg!(expr)`：打印 `dbg: <值>` 并返回原值（MVP 无源码文本标签）。
fn check_dbg_macro(
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
    Ok((HirExpr::Block(Box::new(hir)), ty))
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
        Type::RawPtr(inner, m) => Type::RawPtr(Box::new(substitute(inner, subst)), *m),
        Type::Tuple(ts) => Type::Tuple(ts.iter().map(|t| substitute(t, subst)).collect()),
        Type::Array(inner, n) => Type::Array(Box::new(substitute(inner, subst)), *n),
        // T1a：函数类型递归替换（fn(T, T) -> i64 中 T 经 impl 泛型统一后须替换为
        // 具体类型——此前 `_` 兜底不深入 Fn 内部，泛型方法 fn 参数（如
        // `Vec::sort_by(cmp: fn(T, T) -> i64)`）调用时形参保持 `fn(T, T)`，
        // 函数名 / 闭包实参均无法 unify（expects fn(T, T) -> i64 错误）。
        Type::Fn(sig) => Type::Fn(Box::new(FnSignature {
            params: sig.params.iter().map(|p| substitute(p, subst)).collect(),
            return_type: substitute(&sig.return_type, subst),
        })),
        Type::AssocProjection { base, assoc } => Type::AssocProjection {
            base: Box::new(substitute(base, subst)),
            assoc: assoc.clone(),
        },
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

/// 堆指针包装 `Box<T>` / `Rc<T>` / `Arc<T>` / `Gc<T>` → 内层类型 `T`。
fn heap_wrapper_inner(ty: &Type) -> Option<Type> {
    match ty {
        Type::Named(n, args)
            if matches!(n.as_str(), "Box" | "Rc" | "Arc" | "Gc") && args.len() == 1 =>
        {
            Some(args[0].clone())
        }
        _ => None,
    }
}

/// 连续剥离引用与堆指针包装（`&Box<T>` / `&Rc<T>` → `T`）。
fn peel_refs_and_heap(ty: &Type) -> Type {
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

/// 堆包装对象表达式 → 堆内 `T` 对象区首槽地址。
///
/// - `Box<T>`（K2）/ `Rc<T>` / `Arc<T>`（K3）/ `Gc<T>`（K4）：栈上 1 槽存堆指针
///   （Box 指向值区首槽；Rc / Gc 指向内层对象，值区自堆首槽起、元数据在尾部），
///   解引用槽 0 即堆首槽地址；
/// - `&T` / 普通聚合对象：求值即对象指针，原样返回。
///
/// 字段访问 / 方法调用 / 索引 / 解引用的 base 统一经此改写为堆对象指针。
fn heap_ptr_hir(hir: HirExpr, ty: &Type) -> HirExpr {
    match peel_ref(ty) {
        Type::Named(n, args)
            if matches!(n.as_str(), "Box" | "Rc" | "Arc" | "Gc") && args.len() == 1 =>
        {
            HirExpr::FieldGet {
                base: Box::new(hir),
                index: 0,
                ty: FieldScalar::Ptr,
            }
        }
        _ => hir,
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
