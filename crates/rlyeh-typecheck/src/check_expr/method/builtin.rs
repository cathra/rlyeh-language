//! method/builtin：接收者内建方法特判。
//! （由 method.rs 的 `check_method_call` 拆分而来，保持语义等价）
//!
//! 覆盖：切片胖指针 `len`/`first`/`last`、`Vec` 切片视图、`&str` → String 升级、
//! `push_str(字面量)` 快速路径、`Rc`/`Arc`/`Weak` 引用计数、`dyn Trait` 虚调用。
//! 这些分支必须早于常规 impl 分派——切片在 core.rl 无对应 impl（无法为 `[T]`
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
    // 切片在 core.rl 无对应 impl（无法为 `[T]` 写 impl），故在 impl 分派前特判，
    // 避免落入 impl 查找报「无此方法」。布局与 StrFat 同为 `{data, len}`：
    // 槽 0 = data 指针、槽 1 = 长度。
    if args.is_empty()
        && matches!(&recv_ty, Type::Ref(inner, _) if matches!(&**inner, Type::Slice(_)))
    {
        let elem_ty = match &recv_ty {
            Type::Ref(inner, _) => match &**inner {
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
                return Ok(BuiltinOutcome::Handled(
                    HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                        stmts,
                        final_expr: Some(HirExpr::new(HirExprKind::Variable(base), Span::dummy())),
                    })), Span::dummy()),
                    Type::Named("IterRef".to_string(), vec![elem_ty]),
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
            Type::Ref(inner, _) => &**inner,
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
                    Type::Ref(Box::new(Type::Slice(Box::new(elem_sub))), m),
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
        return Ok(BuiltinOutcome::Handled(
            HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                stmts: vec![HirStmt::new(HirStmtKind::Let{
                    name: lit_tmp.clone(),
                    init: HirExpr::new(HirExprKind::StringLiteral(s), Span::dummy()),
                    mutable: false,
                }, Span::dummy())],
                final_expr: Some(HirExpr::new(HirExprKind::Call{
                    callee: fn_name,
                    args: vec![
                        recv_hir,
                        HirExpr::new(HirExprKind::Ref{
                            expr: Box::new(HirExpr::new(HirExprKind::Variable(lit_tmp), Span::dummy())),
                            is_mut: false,
                            pointee: FieldScalar::Str,
                        }, Span::dummy()),
                        HirExpr::new(HirExprKind::IntLiteral(n), Span::dummy()),
                    ],
                }, Span::dummy())),
            })), Span::dummy()),
            Type::Unit,
        ));
    }
    // `Rc<T>` / `Arc<T>` / `Weak<T>` 引用计数内建方法（K3）：clone /
    // strong_count / weak_count / downgrade / try_unwrap / upgrade。
    // 须在堆指针改写前分派（内建需要原始 Rc 对象取 RcInner 指针）
    if let Some(r) = check_rc_method(ctx, method, &recv_ty, recv_hir.clone(), args, span) {
        let (h, t) = r?;
        return Ok(BuiltinOutcome::Handled(h, t));
    }
    // H4 `dyn Trait` 接收者：方法经 vtable 间接调用（类型擦除后的多态分派）。
    // 布局：2 槽胖指针（槽 0 = 数据指针，槽 1 = vtable 指针）。
    // desugar 为：
    //   let __obj = <recv>;                       // 胖指针对象（1 指针槽）
    //   let __data = FieldGet(__obj, 0, Ptr);     // 数据指针
    //   let __vtp  = FieldGet(__obj, 1, Ptr);     // vtable 指针
    //   let __m    = Index(__vtp, 3+idx, Ptr);    // vtable[3+idx] 方法函数指针
    //   final: CallIndirect { callee: __m, args: [__data, ...实参], param_names, ret_name }
    // P4（2026-08-28）：`&dyn Trait` 接收者同样走 vtable 虚调用（receiver 为
    // `Ref(Dyn)`，胖指针布局与 `dyn Trait` 相同：槽 0=data 指针、槽 1=vtable）。
    let dyn_trait_name = match &recv_ty {
        Type::Dyn(t) => Some(t.clone()),
        Type::Ref(inner, _) => match &**inner {
            Type::Dyn(t) => Some(t.clone()),
            _ => None,
        },
        _ => None,
    };
    if let Some(trait_name) = dyn_trait_name.as_ref() {
        let trait_name = trait_name.clone();
        // H4 去虚拟化：接收者为 dyn 局部变量且绑定源具体类型已知时，静态分派到
        // 具体类型方法（vtable 调用在循环中受间接调用屏障阻止优化，静态调用
        // 可被 LLVM 内联 / 常量折叠；dyn 变量被重新赋值时映射已失效回退 vtable）
        if let HirExprKind::Variable(var) = &recv_hir.kind {
            if let Some(devirt) = devirtualize_dyn_call(
                ctx,
                var,
                trait_name.as_str(),
                method,
                recv_hir.clone(),
                args,
                span,
            )? {
                let (h, t) = devirt;
                return Ok(BuiltinOutcome::Handled(h, t));
            }
        }
        let trait_def = ctx
            .trait_defs
            .get(&trait_name)
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
