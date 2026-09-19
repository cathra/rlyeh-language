//! 表达式类型推断与 HIR 生成。

use std::collections::HashMap;

use rlyeh_ast::{AssignOp, AstBlock, AstExpr, AstPattern, AstStmt, AstType, BinaryOp, CaptureMode, CompareOp, ExprKind, RegionStrategy, UnaryOp};
use rlyeh_hir::{
    FieldScalar, HirAssignOp, HirBinaryOp, HirBlock, HirExpr, HirFnDecl, HirItem, HirItemKind,
    HirParam, HirRegionOptions, HirRegionStrategy, HirStmt, HirUnaryOp, ReprConv, HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;

use crate::check_item::type_to_extern_name;
use crate::comparison;
use crate::context::{DeferredClosure, FnTemplate, TypeContext};
use crate::error::TypeError;
use crate::in_expr;
use crate::types::{field_scalar_of, type_mono_key, FnSignature, ImplDef, Mutability, Type};

/// S2：切片胖指针构造（`&[T; N]` → `&[T]` unsize coercion 的 HIR 结果），
/// 供 `check_stmt`（let 绑定标注位置）与 `check_expr::call`（实参位置）共用。
pub(crate) use call::make_slice_fat;

/// PC-4：父协议一致性校验（`protocol A: B` → 实现 A 必须实现 B），供 `check_item` 调用。
pub(crate) use generic::validate_supertraits;
/// PC-10：trait 方法线性化（`dyn T` vtable 槽顺序）与 `dyn A → dyn B` 上转判定——
/// 供 `check_stmt`（上转）与 `check_expr::method::builtin`（虚调用槽索引）复用。
pub(crate) use generic::{dyn_supertrait_upshift, linearize_trait_methods};

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

// SH-P1-4（M2，2026-09-04）：自动解引用强制辅助。
/// 自动解引用最大递归深度（对齐 Rust 强制链的有界展开）。
pub(super) const MAX_DEREF_DEPTH: usize = 16;

/// 构造自动解引用回退的接收者表达式 `recv.deref()`，用于字段 / 方法 / 索引
/// 解析失败时递归重试（限深度，避免无限）。`deref` 既可是 `Deref` trait 方法
/// （`-> &Target`，聚合 Target 在 HIR 即对象指针，无需再剥 `*`），也可是内建
/// 智能指针 `deref`（`-> T` 值，见 sync/module.rl）。返回 `&Target` 引用后，
/// 下游方法 / 字段解析经 `peel_refs_and_heap` 透明按 `Target` 处理（零新增 IR 节点）。
pub(super) fn make_deref_receiver(receiver: &AstExpr, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::MethodCall {
            receiver: receiver.clone(),
            method: "deref".to_string(),
            args: vec![],
            trait_hint: None,
        },
        span,
    )
}

/// B-2（P0'）：coerce 点自动解引用强制。
///
/// 期望类型 `expected`、实参类型 `actual`、实参 AST `expr`。当 `actual == &T`
/// （`&mut T`）且 `expected == T` 且 `T: Copy` 时，将 `expr` 重写为 `*expr` 并重新
/// 推断，得到类型 `T` 的 `(HIR, Type)`；否则返回 `None`，调用方维持原错误路径。
///
/// 复用既有 `*` 取值路径（`infer_expr_inner` 的 `UnaryOp::Deref` 分支对 `&T` /
/// `&mut T` 直接返回内层 `T`），零新增 IR 节点语义。
pub(super) fn try_auto_deref_coerce(
    ctx: &mut TypeContext,
    expected: &Type,
    actual: &Type,
    expr: &AstExpr,
    span: Span,
) -> Option<Result<(HirExpr, Type), TypeError>> {
    let inner = match actual {
        Type::Ref(inner, _, _) => inner.as_ref(),
        _ => return None,
    };
    if expected != inner {
        return None;
    }
    if !inner.is_copy() {
        return None;
    }
    let deref_expr = AstExpr::new(
        ExprKind::Unary {
            op: UnaryOp::Deref,
            operand: expr.clone(),
        },
        span,
    );
    Some(infer_expr(ctx, &deref_expr))
}

/// 推断表达式的类型并生成对应 HIR（内部实现）。
///
/// 对外入口见 [`infer_expr`]：本函数在返回前由包装层将根 `HirExpr` 的 `span`
/// 设为源 `AstExpr.span`，实现 Span 全量传播（表达式级精确错误坐标）。
pub(crate) fn infer_expr_inner(
    ctx: &mut TypeContext,
    expr: &AstExpr,
) -> Result<(HirExpr, Type), TypeError> {
    let span = expr.span;
    match &*expr.kind {
        ExprKind::IntLiteral(n) => Ok((HirExpr::new(HirExprKind::IntLiteral(*n), Span::dummy()), Type::I64)),
        ExprKind::FloatLiteral(f) => Ok((HirExpr::new(HirExprKind::FloatLiteral(*f), Span::dummy()), Type::F64)),
        ExprKind::StringLiteral(s) => Ok((HirExpr::new(HirExprKind::StringLiteral(s.clone()), Span::dummy()), Type::Str)),
        ExprKind::CharLiteral(c) => Ok((HirExpr::new(HirExprKind::CharLiteral(*c), Span::dummy()), Type::Char)),
        ExprKind::BoolLiteral(b) => Ok((HirExpr::new(HirExprKind::BoolLiteral(*b), Span::dummy()), Type::Bool)),
        // X4：单元类型字面量 `()`（`Result::Ok(())` 的值；空 tuple）
        ExprKind::Unit => Ok((HirExpr::new(HirExprKind::Unit, Span::dummy()), Type::Unit)),
        ExprKind::TimeLiteral { hour, minute, .. } => {
            // 时间字面量归一化为分钟值，按整数处理（可与整数集合/范围统一比较）
            let minutes = i128::from(*hour) * 60 + i128::from(*minute);
            Ok((HirExpr::new(HirExprKind::IntLiteral(minutes), Span::dummy()), Type::I64))
        }

        ExprKind::Ident(name) => {
            // 1. 局部变量（U1：HIR 引用用存储槽名——遮蔽变量经 resolve 返回
            //    mangle 槽名，下游按槽名区分变量存储）
            if let Some((slot, ty)) = ctx.resolve_variable(name) {
                return Ok((HirExpr::new(HirExprKind::Variable(slot.to_string()), Span::dummy()), ty.clone()));
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
                        HirExpr::new(HirExprKind::FnPtr(resolved), Span::dummy()),
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
                        HirExpr::new(HirExprKind::FnPtr(resolved), Span::dummy()),
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
            Ok((HirExpr::new(HirExprKind::Unit, Span::dummy()), lo_ty))
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
                // SH-P0-1（裸指针字节步长）：元素为 `u8` 时 `elem: Str` → MIR 降级
                // 为 1 字节步长（与 `Vec<u8>::as_mut_ptr` 字节寻址一致）；其余沿用
                // 8 字节槽步长。
                let elem = if matches!(*inner, Type::U8) {
                    FieldScalar::Str
                } else {
                    field_scalar_of(&inner)
                };
                let ptr_hir = HirExpr::new(HirExprKind::PtrAdd{
                    base: Box::new(l_hir),
                    offset: Box::new(r_hir),
                    elem,
                }, Span::dummy());
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
                    HirStmt::new(HirStmtKind::Let{
                        name: s_name.clone(),
                        init: HirExpr::new(HirExprKind::Call{
                            callee: clone_fn,
                            args: vec![l_hir],
                        }, Span::dummy()),
                        mutable: true,
                    }, Span::dummy()),
                    HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Call{
                        callee: fn_name,
                        args: vec![HirExpr::new(HirExprKind::Variable(s_name.clone()), Span::dummy()), r_hir],
                    }, Span::dummy())), Span::dummy()),
                ];
                let hir = HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                    stmts,
                    final_expr: Some(HirExpr::new(HirExprKind::Variable(s_name), Span::dummy())),
                })), Span::dummy());
                return Ok((hir, l_ty));
            }
            match check_binary(*op, &l_ty, &r_ty, span) {
                Ok((hir_op, result_ty)) => Ok((
                    HirExpr::new(HirExprKind::Binary(hir_op, Box::new(l_hir), Box::new(r_hir)), Span::dummy()),
                    result_ty,
                )),
                Err(builtin_err) => {
                    // V5d（2026-09-02）：运算符重载回退——内建路径失败且为可重载
                    // 二元运算符时，降级为 `left.<method>(right)` 方法调用
                    //（复用既有 method-call 全链路，codegen 无需改动）。重载成功则
                    // 采用；重载失败（无对应 trait impl 等）则维持内建错误，保留既有
                    // 诊断、不破坏存量行为。
                    if let Some(method) = overload_method(*op) {
                        let args = [right.clone()];
                        if let Ok((hir, ty)) = check_method_call(ctx, left, method, &args, None, span, 0) {
                            return Ok((hir, ty));
                        }
                    }
                    Err(builtin_err)
                }
            }
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
                    Ok((HirExpr::new(HirExprKind::Unary(HirUnaryOp::Neg, Box::new(o_hir)), Span::dummy()), o_ty))
                }
                UnaryOp::Not => {
                    if !o_ty.is_bool() {
                        return Err(TypeError::ExpectedBool {
                            found: o_ty.to_string(),
                            span,
                        });
                    }
                    Ok((HirExpr::new(HirExprKind::Unary(HirUnaryOp::Not, Box::new(o_hir)), Span::dummy()), Type::Bool))
                }
                UnaryOp::Deref => {
                    let inner = match &o_ty {
                        Type::Ref(inner, _, _) | Type::RawPtr(inner, _) => (**inner).clone(),
                        _ => match heap_wrapper_inner(&o_ty) {
                            Some(t) => t,
                            None => {
                                // Y4b-1（2026-08-28）：自定义 `Deref` trait 解引用——
                                // o_ty 实现了 `deref` 方法时，`*x` 生成 `x.deref()` 调用
                                // （返回 `deref()` 的目标类型）；否则维持内建类型限制报错。
                                if ctx.find_impl_for_method(&o_ty, "deref").is_some() {
                                    let (deref_hir, t) = check_method_call(
                                        ctx, operand, "deref", &[], None, span, 0,
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
                    // SH-P0-1（裸指针 u8 解引用）：`*p` 对 `*const u8`/`*mut u8` 按 1 字节
                    // 读取（load i8 后 zext 到宽值），与字节步长裸指针算术 / 索引一致，
                    // 支持 FFI 字节缓冲逐字节访问（如结构体字节级布局校验）。其余
                    // 元素沿用 8 字节槽 load（维持既有裸指针 / 引用解引用语义）。
                    let ty = if matches!(o_ty, Type::RawPtr(_, _)) && matches!(inner, Type::U8) {
                        FieldScalar::ReprCField {
                            offset: 0,
                            field_ty: "i8",
                            conv: ReprConv::Zext,
                        }
                    } else {
                        field_scalar_of(&inner)
                    };
                    Ok((
                        HirExpr::new(HirExprKind::Deref{
                            expr: Box::new(heap_ptr_hir(o_hir, &o_ty)),
                            ty,
                        }, Span::dummy()),
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
                    if matches!(o_ty, Type::Ref(_, _, _)) {
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
                        HirExpr::new(HirExprKind::Ref{
                            expr: Box::new(o_hir),
                            is_mut,
                            pointee,
                        }, Span::dummy()),
                        Type::Ref(Box::new(o_ty), m, None),
                    ))
                }
            }
        }

        // 控制流 / 调用 / 宏等变体下沉至 `ctrl`（文件大小约束：单个文件 ≤1000 行）
        _ => crate::check_expr::ctrl::infer_expr_tail(ctx, expr, span),
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
///     （TOML 基本转义与 JSON 一致，复用 标准库 `json_escape` / `json_unescape`）

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
mod ctrl;
mod call;
mod resolve;
pub(crate) mod construct;
mod field;
mod heap;
mod index_enum;
mod method;
mod generic;
pub(crate) mod util;
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
pub(crate) use util::substitute;
// block/misc 对外 API
pub(crate) use block::{check_block, check_block_inner};
pub(crate) use misc::{builtin_signature, coerce_to_dyn, type_mentions_self,
    is_any_trait, check_any_type_id, check_any_downcast_ref};
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

/// 推断表达式类型并生成 HIR（对外入口）。
///
/// 在 [`infer_expr_inner`] 基础上，将生成的根 `HirExpr` 的 `span` 设为源
/// `AstExpr.span`，实现 Span 全量传播——下游 borrowck / regionck 可据此给出
/// 表达式级精确错误坐标（取代此前函数级 `item.span` 的粗粒度坐标）。
pub(crate) fn infer_expr(
    ctx: &mut TypeContext,
    expr: &AstExpr,
) -> Result<(HirExpr, Type), TypeError> {
    let mut hir = infer_expr_inner(ctx, expr)?;
    hir.0.span = expr.span;
    Ok(hir)
}