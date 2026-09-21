//! method/builtin：接收者内建方法特判。
//! （由 method.rs 的 `check_method_call` 拆分而来，保持语义等价）
//!
//! 覆盖：切片胖指针 `len`/`first`/`last`、`Vec` 切片视图、`&str` → String 升级、
//! `Rc`/`Arc`/`Weak` 引用计数、`dyn Protocol` 虚调用。
//! 这些分支必须早于常规 impl 分派——切片在 标准库无对应 impl（无法为 `[T]`
//! 写 impl），引用计数需要原始对象，虚调用走 vtable 而非静态分派。

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use super::*;

/// 内建分派结果。
pub(super) enum BuiltinOutcome {
    /// 命中内建分支，调用方直接以该结果返回。
    Handled(HirExpr, Type),
    /// 未命中——回传接收者（可能已被规范化，如 `&str` 深拷贝为 String 对象），
    /// 交由常规 impl 分派继续处理。
    NotHandled(HirExpr, Type),
}

/// 接收者内建方法特判。
///
/// **接收者按值传入并回传**：区域内的分支既会移动（`Box::new(recv_hir)`）
/// 也会改写（`&str` 升级为 String），按值传递可让原有代码逐字保留；未命中时
/// 经 `NotHandled` 把（可能已被规范化的）接收者交还调用方。
pub(super) fn try_builtin_method_call(
    ctx: &mut TypeContext,
    mut recv_hir: HirExpr,
    mut recv_ty: Type,
    method: &str,
    args: &[AstExpr],
    span: Span,
) -> Result<BuiltinOutcome, TypeError> {
    // S3：切片胖指针 `&[T]` / `&mut [T]` 的内建方法（`len` / `first` / `last`）。
    // 切片在 标准库无对应 impl（无法为 `[T]` 写 impl），故在 impl 分派前特判，
    // 避免落入 impl 查找报「无此方法」。布局与 StrFat 同为 `{data, len}`：
    // 槽 0 = data 指针、槽 1 = 长度。
    if args.is_empty()
        && matches!(&recv_ty, Type::Ref(inner, _, _) if matches!(&**inner, Type::Slice(_)))
    {
        let elem_ty = match &recv_ty {
            Type::Ref(inner, _, _) => match &**inner {
                Type::Slice(e) => (**e).clone(),
                _ => unreachable!("已由守卫确认接收者为切片引用"),
            },
            _ => unreachable!("已由守卫确认接收者为切片引用"),
        };
        match method {
            "len" => {
                return Ok(BuiltinOutcome::Handled(
                    HirExpr::new(HirExprKind::FieldGet{
                        base: Box::new(recv_hir),
                        index: 1,
                        ty: FieldScalar::Int,
                    }, Span::dummy()),
                    Type::I64,
                ));
            }
            // `first()` / `last()` ≡ `s[0]` / `s[len - 1]`（复用索引语义与元素
            // 步长；空切片取元素属越界读，MVP 不额外检查，与数组索引行为一致）
            "first" | "last" => {
                let idx: HirExpr = if method == "first" {
                    HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())
                } else {
                    HirExpr::new(HirExprKind::Binary(
                        HirBinaryOp::Sub,
                        Box::new(HirExpr::new(HirExprKind::FieldGet{
                            base: Box::new(recv_hir.clone()),
                            index: 1,
                            ty: FieldScalar::Int,
                        }, Span::dummy())),
                        Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
                    ), Span::dummy())
                };
                let is_byte = matches!(elem_ty, Type::U8);
                return Ok(BuiltinOutcome::Handled(
                    HirExpr::new(HirExprKind::Index{
                        base: Box::new(HirExpr::new(HirExprKind::FieldGet{
                            base: Box::new(recv_hir),
                            index: 0,
                            ty: FieldScalar::Ptr,
                        }, Span::dummy())),
                        index: Box::new(idx),
                        elem: field_scalar_of(&elem_ty),
                        is_str: is_byte,
                        len: None,
                    }, Span::dummy()),
                    elem_ty,
                ));
            }
            // `iter()` → `IterRef<T>`：复用 V3 引用迭代器（2 槽 `{data, len}`）。
            // 切片胖指针与 IterRef 布局同构，可零拷贝构造；此处绕过
            // `IterRef::new` 的实参类型检查（其要求 data 为 `RawPtr`，
            // 而切片槽 0 是 `Ptr`），直接按同一布局落槽。
            "iter" => {
                let base = ctx.fresh_temp();
                let stmts = vec![
                    HirStmt::new(HirStmtKind::Let{
                        name: base.clone(),
                        init: HirExpr::new(HirExprKind::Alloc{
                            slots: 2,
                            by_value: false,
                            is_strfat: false,
                        }, Span::dummy()),
                        mutable: false,
                    }, Span::dummy()),
                    HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                        base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
                        index: 0,
                        value: Box::new(HirExpr::new(HirExprKind::FieldGet{
                            base: Box::new(recv_hir.clone()),
                            index: 0,
                            ty: FieldScalar::Ptr,
                        }, Span::dummy())),
                        ty: FieldScalar::Ptr,
                    }, Span::dummy())), Span::dummy()),
                    HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                        base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
                        index: 1,
                        value: Box::new(HirExpr::new(HirExprKind::FieldGet{
                            base: Box::new(recv_hir),
                            index: 1,
                            ty: FieldScalar::Int,
                        }, Span::dummy())),
                        ty: FieldScalar::Int,
                    }, Span::dummy())), Span::dummy()),
                ];
                // 指针元素切片（`String` / 联合等 Ptr 槽宽类型）：`IterRef` 的
                // `next() -> Option<&T>` 返回元素槽地址 E，而方法接收者需要元素槽中
                // 直接存放的对象指针 O，二者不一致（指针元素切片 for 迭代 pre-existing
                // bug）。改用值迭代器 `Iter<T>`（`next() -> Option<T>` 产出 O），使
                // `for x in cs.iter()` 中 `x` 即为可用作方法接收者的对象指针；非指针
                // 元素（i64 等）仍走 `IterRef` 以保留原地写回语义。
                let iter_ty = if field_scalar_of(&elem_ty) == FieldScalar::Ptr {
                    "Iter"
                } else {
                    "IterRef"
                };
                return Ok(BuiltinOutcome::Handled(
                    HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                        stmts,
                        final_expr: Some(HirExpr::new(HirExprKind::Variable(base), Span::dummy())),
                    })), Span::dummy()),
                    Type::Named(iter_ty.to_string(), vec![elem_ty]),
                ));
            }
            // `as_ptr()` / `as_mut_ptr()` → `*const T` / `*mut T`：取胖指针槽 0 的
            // data 指针（供 extern / FFI 场景传递缓冲区首地址）
            "as_ptr" | "as_mut_ptr" => {
                return Ok(BuiltinOutcome::Handled(
                    HirExpr::new(HirExprKind::FieldGet{
                        base: Box::new(recv_hir),
                        index: 0,
                        ty: FieldScalar::Ptr,
                    }, Span::dummy()),
                    Type::RawPtr(Box::new(elem_ty), method == "as_mut_ptr"),
                ));
            }
            _ => {}
        }
    }
    // S3：`Vec<T>` 的 `as_slice()` / `as_mut_slice()` → `&[T]` / `&mut [T]` 零拷贝
    // 切片视图（Vec 槽 0 = data、槽 1 = len，与切片胖指针 `{data, len}` 同构）。
    // 这是 `File::read(&mut [u8])` 等二进制 API 的前提——`Vec<u8>` 是紧凑字节缓冲，
    // 而 `[u8; N]` 数组非紧凑（每元素 8 字节），不宜作为字节切片来源。
    if (method == "as_slice" || method == "as_mut_slice") && args.is_empty() {
        let vec_ty = match &recv_ty {
            Type::Ref(inner, _, _) => &**inner,
            other => other,
        };
        if let Type::Named(n, targs) = vec_ty {
            if n == "Vec" && targs.len() == 1 && ctx.lookup_struct("Vec").is_some() {
                let elem_sub = substitute(&targs[0], &ctx.generic_subst);
                let base = ctx.fresh_temp();
                let stmts = vec![
                    HirStmt::new(HirStmtKind::Let{
                        name: base.clone(),
                        init: HirExpr::new(HirExprKind::Alloc{
                            slots: 2,
                            by_value: true,
                            is_strfat: true,
                        }, Span::dummy()),
                        mutable: false,
                    }, Span::dummy()),
                    HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                        base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
                        index: 0,
                        value: Box::new(HirExpr::new(HirExprKind::FieldGet{
                            base: Box::new(recv_hir.clone()),
                            index: 0,
                            ty: FieldScalar::Ptr,
                        }, Span::dummy())),
                        ty: FieldScalar::Ptr,
                    }, Span::dummy())), Span::dummy()),
                    HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                        base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
                        index: 1,
                        value: Box::new(HirExpr::new(HirExprKind::FieldGet{
                            base: Box::new(recv_hir),
                            index: 1,
                            ty: FieldScalar::Int,
                        }, Span::dummy())),
                        ty: FieldScalar::Int,
                    }, Span::dummy())), Span::dummy()),
                ];
                let m = if method == "as_mut_slice" {
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                return Ok(BuiltinOutcome::Handled(
                    HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                        stmts,
                        final_expr: Some(HirExpr::new(HirExprKind::Variable(base), Span::dummy())),
                    })), Span::dummy()),
                    Type::Ref(Box::new(Type::Slice(Box::new(elem_sub))), m, None),
                ));
            }
        }
    }
    // V2-B：`&str`（StrFat）接收者调 String 方法时，先深拷贝为临时 String 对象。
    // StrFat 是 by_value `{data,len}`，而 String 方法 self 期望 String 对象指针（Ptr）；
    // 直接传 StrFat 会污染 LIR param_types 传播（同一方法被 String receiver 调用时
    // 也误标 StrFat，破坏 `x.trim()`）。深拷贝后 self 统一为 String，无类型冲突。
    // （V2-B 链式 `x.trim().trim()` / `s.trim().to_upper()`）
    if comparison::is_str_view(&recv_ty) {
        let tmp = ctx.fresh_temp();
        let data_h = HirExpr::new(HirExprKind::FieldGet{
            base: Box::new(recv_hir.clone()),
            index: 0,
            ty: FieldScalar::Ptr,
        }, Span::dummy());
        let len_h = HirExpr::new(HirExprKind::FieldGet{
            base: Box::new(recv_hir),
            index: 1,
            ty: FieldScalar::Int,
        }, Span::dummy());
        let conv = make_strfat_to_string(ctx, data_h, len_h);
        recv_hir = HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts: vec![HirStmt::new(HirStmtKind::Let{
                name: tmp.clone(),
                init: conv,
                mutable: false,
            }, Span::dummy())],
            final_expr: Some(HirExpr::new(HirExprKind::Variable(tmp), Span::dummy())),
        })), Span::dummy());
        recv_ty = Type::Named("String".to_string(), vec![]);
    }
    // `push_str(字面量实参)` 快速路径（原改调 `String::push_bytes(src, n)`）已移除：
    // 该特判把字面量经 `&__lit`（单槽伪对象头，仅含 data 指针）传给 `push_bytes`，
    // 而 `push_bytes` 内部的 `src[i]` 索引按 `&str` 胖指针（StrFat `{data, len}`）
    // 读取 `len` 字段做边界检查——单槽对象在偏移 +8 处是栈上垃圾值，导致边界检查
    // `i >= len` 立即成立、`abort()`（典型崩溃：`m.push_str(" world")`、`float_to_string`
    // 内的 `push_str(数字串)`）。回退到常规 `push_str(String)` 路径（字面量经
    // `upgrade_str_arg` 升级为 `String`，`push_str` 直接索引 `other.data[i]` 且边界
    // 用正确的 `other.len`），行为正确、零新增回归；性能优化（避免字面量深拷贝）应改为
    // 在 codegen 为字面量构造完整 StrFat `{data, len}` 后再接 `push_bytes`，而非当前
    // 单槽伪对象头。
    // `Rc<T>` / `Arc<T>` / `Weak<T>` 引用计数内建方法（K3）：clone /
    // strong_count / weak_count / downgrade / try_unwrap / upgrade。
    // 须在堆指针改写前分派（内建需要原始 Rc 对象取 RcInner 指针）
    if let Some(r) = check_rc_method(ctx, method, &recv_ty, recv_hir.clone(), args, span) {
        let (h, t) = r?;
        return Ok(BuiltinOutcome::Handled(h, t));
    }
    // H4 `dyn Protocol` 接收者：方法经 vtable 间接调用（类型擦除后的多态分派）。
    // 布局：2 槽胖指针（槽 0 = 数据指针，槽 1 = vtable 指针）。
    // desugar 为：
    //   let __obj = <recv>;                       // 胖指针对象（1 指针槽）
    //   let __data = FieldGet(__obj, 0, Ptr);     // 数据指针
    //   let __vtp  = FieldGet(__obj, 1, Ptr);     // vtable 指针
    //   let __m    = Index(__vtp, 3+idx, Ptr);    // vtable[3+idx] 方法函数指针
    //   final: CallIndirect { callee: __m, args: [__data, ...实参], param_names, ret_name }
    // P4（2026-08-28）：`&dyn Protocol` 接收者同样走 vtable 虚调用（receiver 为
    // `Ref(Dyn)`，胖指针布局与 `dyn Protocol` 相同：槽 0=data 指针、槽 1=vtable）。
    let dyn_protocol_name = match &recv_ty {
        Type::Dyn(t) => Some(t.clone()),
        Type::Ref(inner, _, _) => match &**inner {
            Type::Dyn(t) => Some(t.clone()),
            _ => None,
        },
        // EH-4（2026-09-21）：堆包裹的 protocol 对象（`Box<dyn Protocol>` /
        // `Rc` / `Arc` / `Gc`）。`Box<dyn P>` 是指向 2 槽区域（槽 0 = 数据指针、
        // 槽 1 = vtable 指针）的对象指针，与 `Box<Point>` 字段访问同为
        // 「指针指向对象」布局，故下方 FieldGet(obj, 0/1) 直接读到胖指针两槽。
        _ => heap_wrapper_inner(&recv_ty).and_then(|inner| match inner {
            Type::Dyn(t) => Some(t),
            _ => None,
        }),
    };
    if let Some(protocol_name) = dyn_protocol_name.as_ref() {
        let protocol_name = protocol_name.clone();
        // EH-4（2026-09-21）：堆包裹接收者（`Box<dyn P>` 等）先取槽 0 得堆对象指针
        // ——`Box<T>` 局部为 1 槽聚合（槽 0 = 指向堆对象的指针），须解出后再按
        // 「指针指向 2 槽胖指针」读 槽 0/1。对非堆包裹类型 `heap_ptr_hir` 为恒等
        // （裸 `dyn` / `&dyn` 接收者行为不变）。
        let recv_hir = heap_ptr_hir(recv_hir, &recv_ty);
        // H4 去虚拟化：接收者为 dyn 局部变量且绑定源具体类型已知时，静态分派到
        // 具体类型方法（vtable 调用在循环中受间接调用屏障阻止优化，静态调用
        // 可被 LLVM 内联 / 常量折叠；dyn 变量被重新赋值时映射已失效回退 vtable）
        if let HirExprKind::Variable(var) = &recv_hir.kind {
            if let Some(devirt) = devirtualize_dyn_call(
                ctx,
                var,
                protocol_name.as_str(),
                method,
                recv_hir.clone(),
                args,
                span,
            )? {
                let (h, t) = devirt;
                return Ok(BuiltinOutcome::Handled(h, t));
            }
        }
        // 校验协议存在（未定义时报 UndefinedType，保持既有行为）。
        let _protocol_def = ctx
            .protocol_defs
            .get(&protocol_name)
            .cloned()
            .ok_or_else(|| TypeError::UndefinedType {
                name: protocol_name.clone(),
                span,
            })?;
        // PC-10：方法槽索引按**线性化顺序**（superprotocol 方法在前）查找——与
        // `coerce_to_dyn` 的 vtable 填充顺序一致；因此 `dyn 子协议` 接收者也可调用
        // 父协议方法（其槽位在 vtable 前部）。
        let lin = crate::check_expr::linearize_protocol_methods(ctx, &protocol_name);
        let idx = lin
            .iter()
            .position(|(_, m)| m.name == method)
            .ok_or_else(|| TypeError::FunctionNotFound {
                name: format!("dyn {protocol_name}::{method}"),
                span,
            })?;
        let sig = lin[idx].1.clone();
        // MVP 限制：protocol 方法签名含 `Self`（关联返回类型 / 参数）时无法确定
        // 具体类型，不支持经 dyn 调用
        if sig.params.iter().skip(1).any(type_mentions_self) || type_mentions_self(&sig.return_type) {
            return Err(TypeError::Unsupported {
                what: format!(
                    "`dyn {protocol_name}::{method}`：签名含 `Self` 的方法（关联类型 MVP 不支持 protocol 对象调用）"
                ),
                span,
            });
        }
        if args.len() + 1 != sig.params.len() {
            return Err(TypeError::UnexpectedArgumentCount {
                name: format!("dyn {protocol_name}::{method}"),
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
            // Str 值实参 → 非 Str 形参自动升级（`dyn Protocol` 方法 String 形参）
            let (h, t) = upgrade_str_arg(ctx, h, t, &pty, a)?;
            if !t.compatible_with(&pty) {
                return Err(TypeError::ArgumentTypeMismatch {
                    name: format!("dyn {protocol_name}::{method}"),
                    index: i + 1,
                    expected: pty.to_string(),
                    found: t.to_string(),
                    span: a.span,
                    related: vec![],
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
            HirStmt::new(HirStmtKind::Let{
                name: obj.clone(),
                init: recv_hir,
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: data.clone(),
                init: HirExpr::new(HirExprKind::FieldGet{
                    base: Box::new(HirExpr::new(HirExprKind::Variable(obj.clone()), Span::dummy())),
                    index: 0,
                    ty: FieldScalar::Ptr,
                }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: vtp.clone(),
                init: HirExpr::new(HirExprKind::FieldGet{
                    base: Box::new(HirExpr::new(HirExprKind::Variable(obj), Span::dummy())),
                    index: 1,
                    ty: FieldScalar::Ptr,
                }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: m.clone(),
                init: HirExpr::new(HirExprKind::Index{
                    base: Box::new(HirExpr::new(HirExprKind::Variable(vtp), Span::dummy())),
                    index: Box::new(HirExpr::new(HirExprKind::IntLiteral((3 + idx) as i128), Span::dummy())),
                    elem: FieldScalar::Ptr,
                    is_str: false,
                    len: None,
                }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
        ];
        let mut call_args = vec![HirExpr::new(HirExprKind::Variable(data), Span::dummy())];
        call_args.extend(hir_args);
        let ret_ty = sig.return_type.clone();
        let call = HirExpr::new(HirExprKind::CallIndirect{
            callee: Box::new(HirExpr::new(HirExprKind::Variable(m), Span::dummy())),
            args: call_args,
            param_names,
            ret_name: type_to_extern_name(&ret_ty),
        }, Span::dummy());
        return Ok(BuiltinOutcome::Handled(
            HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                stmts,
                final_expr: Some(call),
            })), Span::dummy()),
            ret_ty,
        ));
    }
    Ok(BuiltinOutcome::NotHandled(recv_hir, recv_ty))
}
