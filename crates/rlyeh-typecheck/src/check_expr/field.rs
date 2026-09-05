//! 表达式检查子模块：字段访问与切片。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use super::*;


pub(super) fn check_field_access(
    ctx: &mut TypeContext,
    expr: &AstExpr,
    field: &str,
    span: Span,
    depth: usize,
) -> Result<(HirExpr, Type), TypeError> {
    let (base_hir, base_ty) = infer_expr(ctx, expr)?;
    match check_field_access_inner(ctx, base_hir, base_ty.clone(), field, span) {
        Ok(r) => Ok(r),
        // M2（SH-P1-4）：字段未命中且接收者类型实现了 `deref` 时，对接收者插入
        // `*(recv.deref())` 递归重试（限深度，避免无限）。零新增 IR 节点。
        Err(_)
            if depth < MAX_DEREF_DEPTH
                && ctx
                    .find_impl_for_method(&peel_refs_and_heap(&base_ty), "deref")
                    .is_some() =>
        {
            let deref_ast = make_deref_receiver(expr, span);
            check_field_access(ctx, &deref_ast, field, span, depth + 1)
        }
        Err(e) => Err(e),
    }
}

pub(super) fn check_field_access_inner(
    ctx: &mut TypeContext,
    base_hir: HirExpr,
    base_ty: Type,
    field: &str,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // `Box<T>` 接收者：自动剥层后按 `T` 的字段访问（K2）。base 须为堆对象
    // 指针：经槽 0 解出（`box_ptr_hir`），`&Box<T>` 引用求值即对象指针同构
    let base_hir = heap_ptr_hir(base_hir, &base_ty);
    // M1：元组按位置字段访问（`t.f0` / `t.f1` / ...）
    if let Type::Tuple(ts) = peel_refs_and_heap(&base_ty) {
        let idx = field
            .strip_prefix('f')
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&i| i < ts.len())
            .ok_or_else(|| TypeError::UnknownField {
                struct_name: "(tuple)".to_string(),
                field: field.to_string(),
                span,
            })?;
        let fty = ts[idx].clone();
        return Ok((
            HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(base_hir),
                index: idx,
                ty: field_scalar_of(&fty),
            }, Span::dummy()),
            fty,
        ));
    }
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
            HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(base_hir),
                index: idx,
                ty: field_scalar_of(&fty),
            }, Span::dummy()),
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
        // V3-D（2026-08-27）：实例类型参数为 `Generic` 占位（如 trait 默认方法
        // 返回 `Take2<Self>` 时 `Self` 未实例化）时，不覆盖全局 generic_subst 的
        // 已解析映射——否则 `self.inner`（inner: I）替换成占位 `Generic("Self")`，
        // 方法查找 `Self::next` 匹配不到 impl。仅当实例参数为具体类型时才覆盖。
        if matches!(arg, Type::Generic(_)) {
            continue;
        }
        subst.insert(tp.clone(), arg.clone());
    }
    let fty_sub = substitute(&fty, &subst);
    // SH-P0-1 E2（repr(C) 真布局）：repr(C) 结构体字段按 C 规则打包——经
    // `FieldScalar::ReprCField` 把真实字节偏移 / 内存类型 / 提升方式烤入指令，
    // 下传至 codegen（HIR→MIR→LIR 透传 `ty`，无需改三个 Program 结构体）。
    let ty = if def.repr_c {
        let layout = crate::types::compute_repr_c(&def.fields, ctx, span)?;
        match &layout.fields[idx] {
            crate::types::CField::Scalar {
                offset,
                field_ty,
                conv,
            } => rlyeh_hir::FieldScalar::ReprCField {
                offset: *offset,
                field_ty,
                conv: *conv,
            },
            crate::types::CField::Nested { offset, size } => {
                rlyeh_hir::FieldScalar::ReprCSubPtr {
                    offset: *offset,
                    size: *size,
                }
            }
        }
    } else {
        field_scalar_of(&fty_sub)
    };
    Ok((
        HirExpr::new(HirExprKind::FieldGet{
            base: Box::new(base_hir),
            index: idx,
            ty,
        }, Span::dummy()),
        fty_sub,
    ))
}

pub(super) fn check_slice(
    ctx: &mut TypeContext,
    expr: &AstExpr,
    lower: Option<&AstExpr>,
    upper: Option<&AstExpr>,
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
    // S2：切片胖指针 `&[T]` / `&mut [T]` 亦支持再切片（零拷贝子区间视图）
    let is_slice_view =
        matches!(&b_ty, Type::Ref(inner, _) if matches!(**inner, Type::Slice(_)));
    if !comparison::is_string_type(ctx, &b_ty)
        && !is_str_view
        && !is_slice_view
        && !is_vec
        && !is_arr
    {
        return Err(TypeError::Unsupported {
            what: "范围切片（`s[lo..<hi]`）暂仅支持 String / &str / 切片 / Vec / 数组对象".to_string(),
            span,
        });
    }
    // P8（2026-08-29）：省略边界——lower=None → `0`；upper=None → `i64::MAX`
    // （依赖 std `substring`/`slice` 的 clamp：`e > len → len`，故大值落到实际长度）。
    let (lo_hir, lo_ty) = match lower {
        Some(l) => infer_expr(ctx, l)?,
        None => (HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy()), Type::I64),
    };
    let (hi_hir, hi_ty) = match upper {
        Some(u) => infer_expr(ctx, u)?,
        None => (HirExpr::new(HirExprKind::IntLiteral(i64::MAX as i128), Span::dummy()), Type::I64),
    };
    if !lo_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: lo_ty.to_string(),
            span: lower.map(|l| l.span).unwrap_or(span),
        });
    }
    if !hi_ty.is_integer() {
        return Err(TypeError::ExpectedInt {
            found: hi_ty.to_string(),
            span: upper.map(|u| u.span).unwrap_or(span),
        });
    }
    // 区间 → substring 的半开参数 [start, end)：
    // `..<` 含下界不含上界；`...` 双闭（end + 1）；`<..` 不含下界（start + 1）。
    // P8：省略下界 → 固定 0（不再 `+1`）；省略上界 → 固定 i64::MAX（不再 `+1`）。
    let start = if lower.is_none() {
        HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())
    } else if lower_inclusive {
        lo_hir
    } else {
        HirExpr::new(HirExprKind::Binary(
            HirBinaryOp::Add,
            Box::new(lo_hir),
            Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
        ), Span::dummy())
    };
    let end = if upper.is_none() {
        HirExpr::new(HirExprKind::IntLiteral(i64::MAX as i128), Span::dummy())
    } else if upper_inclusive {
        HirExpr::new(HirExprKind::Binary(
            HirBinaryOp::Add,
            Box::new(hi_hir),
            Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
        ), Span::dummy())
    } else {
        hi_hir
    };
    // S2 切片再切片 `s[lo..<hi]`：零拷贝子区间视图——
    //   `{ data + lo * sizeof(T), clamp(hi) - lo }`，边界 clamp 到 `[0, len]`。
    // 展开为 Block：先绑定 base/len/start/end 四个临时（避免 clamp 条件中
    // 重复求值含副作用的边界表达式），再 clamp、算 data 偏移、构造胖指针。
    if let Type::Ref(inner, _) = &b_ty {
        if let Type::Slice(elem_box) = &**inner {
            let elem_sub = substitute(elem_box, &ctx.generic_subst);
            let elem_scalar = field_scalar_of(&elem_sub);
            // `PtrAdd` 仅 `elem: Str` 走 1 字节步长（见 `rlyeh-mir/src/lower.rs`），
            // 故 `&[u8]` 的字节偏移须传 `Str`，其余元素按 8 字节步长。
            let is_byte = matches!(elem_sub, Type::U8);
            let ptr_elem = if is_byte { FieldScalar::Str } else { elem_scalar };

            let len = ctx.fresh_temp();
            let st = ctx.fresh_temp();
            let en = ctx.fresh_temp();
            let st2 = ctx.fresh_temp();
            let en2 = ctx.fresh_temp();
            let en3 = ctx.fresh_temp();
            let data = ctx.fresh_temp();
            let ptr = ctx.fresh_temp();
            let out = ctx.fresh_temp();
            let v = |n: &String| HirExpr::new(HirExprKind::Variable(n.clone()), Span::dummy());
            let lit = HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy());
            let clamp = |cond: HirExpr, t: HirExpr, e: HirExpr| HirExpr::new(HirExprKind::If{
                cond: Box::new(cond),
                then_block: Box::new(HirBlock { span: Span::dummy(),
                    stmts: vec![],
                    final_expr: Some(t),
                }),
                else_block: Some(Box::new(HirBlock { span: Span::dummy(),
                    stmts: vec![],
                    final_expr: Some(e),
                })),
            }, Span::dummy());
            let stmts = vec![
                // 直接以 `b_hir` 为接收者（不经 `let` 绑定）：LIR 的 `Assign`
                // 不传播类型，中间绑定会把胖指针落成默认 `i64` 槽而破坏布局
                HirStmt::new(HirStmtKind::Let{
                    name: len.clone(),
                    init: HirExpr::new(HirExprKind::FieldGet{
                        base: Box::new(b_hir.clone()),
                        index: 1,
                        ty: FieldScalar::Int,
                    }, Span::dummy()),
                    mutable: false,
                }, Span::dummy()),
                HirStmt::new(HirStmtKind::Let{
                    name: st.clone(),
                    init: start,
                    mutable: false,
                }, Span::dummy()),
                HirStmt::new(HirStmtKind::Let{
                    name: en.clone(),
                    init: end,
                    mutable: false,
                }, Span::dummy()),
                HirStmt::new(HirStmtKind::Let{
                    name: st2.clone(),
                    init: clamp(
                        HirExpr::new(HirExprKind::Binary(HirBinaryOp::Lt, Box::new(v(&st)), Box::new(lit.clone())), Span::dummy()),
                        lit.clone(),
                        v(&st),
                    ),
                    mutable: false,
                }, Span::dummy()),
                HirStmt::new(HirStmtKind::Let{
                    name: en2.clone(),
                    init: clamp(
                        HirExpr::new(HirExprKind::Binary(HirBinaryOp::Gt, Box::new(v(&en)), Box::new(v(&len))), Span::dummy()),
                        v(&len),
                        v(&en),
                    ),
                    mutable: false,
                }, Span::dummy()),
                HirStmt::new(HirStmtKind::Let{
                    name: en3.clone(),
                    init: clamp(
                        HirExpr::new(HirExprKind::Binary(HirBinaryOp::Lt, Box::new(v(&en2)), Box::new(v(&st2))), Span::dummy()),
                        v(&st2),
                        v(&en2),
                    ),
                    mutable: false,
                }, Span::dummy()),
                HirStmt::new(HirStmtKind::Let{
                    name: data.clone(),
                    init: HirExpr::new(HirExprKind::FieldGet{
                        base: Box::new(b_hir),
                        index: 0,
                        ty: FieldScalar::Ptr,
                    }, Span::dummy()),
                    mutable: false,
                }, Span::dummy()),
                HirStmt::new(HirStmtKind::Let{
                    name: ptr.clone(),
                    init: HirExpr::new(HirExprKind::PtrAdd{
                        base: Box::new(v(&data)),
                        offset: Box::new(v(&st2)),
                        elem: ptr_elem,
                    }, Span::dummy()),
                    mutable: false,
                }, Span::dummy()),
                HirStmt::new(HirStmtKind::Let{
                    name: out.clone(),
                    init: HirExpr::new(HirExprKind::Alloc{
                        slots: 2,
                        by_value: true,
                        is_strfat: true,
                    }, Span::dummy()),
                    mutable: false,
                }, Span::dummy()),
                HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                    base: Box::new(v(&out)),
                    index: 0,
                    value: Box::new(v(&ptr)),
                    ty: FieldScalar::Ptr,
                }, Span::dummy())), Span::dummy()),
                HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                    base: Box::new(v(&out)),
                    index: 1,
                    value: Box::new(HirExpr::new(HirExprKind::Binary(
                        HirBinaryOp::Sub,
                        Box::new(v(&en3)),
                        Box::new(v(&st2)),
                    ), Span::dummy())),
                    ty: FieldScalar::Int,
                }, Span::dummy())), Span::dummy()),
            ];
            return Ok((
                HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                    stmts,
                    final_expr: Some(v(&out)),
                })), Span::dummy()),
                Type::Ref(
                    Box::new(Type::Slice(Box::new(elem_sub))),
                    Mutability::Immutable,
                ),
            ));
        }
    }
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
            HirExpr::new(HirExprKind::Call{
                callee: fn_name,
                args: vec![b_hir, start, end],
            }, Span::dummy()),
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
            HirExpr::new(HirExprKind::Call{
                callee: fn_name,
                args: vec![b_hir, start, end],
            }, Span::dummy()),
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
            HirStmt::new(HirStmtKind::Let{
                name: base_name.clone(),
                init: b_hir,
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: data_name.clone(),
                init: HirExpr::new(HirExprKind::Call{
                    callee: "alloc_array".to_string(),
                    args: vec![HirExpr::new(HirExprKind::IntLiteral(4), Span::dummy())],
                }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: out_name.clone(),
                init: HirExpr::new(HirExprKind::Alloc{
            slots: 3,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(out_name.clone()), Span::dummy())),
                index: 0,
                value: Box::new(HirExpr::new(HirExprKind::Variable(data_name), Span::dummy())),
                ty: FieldScalar::Ptr,
            }, Span::dummy())), Span::dummy()),
            HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(out_name.clone()), Span::dummy())),
                index: 1,
                value: Box::new(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())),
                ty: FieldScalar::Int,
            }, Span::dummy())), Span::dummy()),
            HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(out_name.clone()), Span::dummy())),
                index: 2,
                value: Box::new(HirExpr::new(HirExprKind::IntLiteral(4), Span::dummy())),
                ty: FieldScalar::Int,
            }, Span::dummy())), Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: i_name.clone(),
                init: start,
                mutable: true,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: hi_name.clone(),
                init: end,
                mutable: true,
            }, Span::dummy()),
            // clamp 下界：if __i < 0 { __i = 0 }
            HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::If{
                cond: Box::new(HirExpr::new(HirExprKind::Binary(
                    HirBinaryOp::Lt,
                    Box::new(HirExpr::new(HirExprKind::Variable(i_name.clone()), Span::dummy())),
                    Box::new(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())),
                ), Span::dummy())),
                then_block: Box::new(HirBlock { span: Span::dummy(),
                    stmts: vec![HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Assign{
                        target: i_name.clone(),
                        op: HirAssignOp::Assign,
                        value: Box::new(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())),
                    }, Span::dummy())), Span::dummy())],
                    final_expr: None,
                }),
                else_block: None,
            }, Span::dummy())), Span::dummy()),
            // clamp 上界：if __hi > N { __hi = N }
            HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::If{
                cond: Box::new(HirExpr::new(HirExprKind::Binary(
                    HirBinaryOp::Gt,
                    Box::new(HirExpr::new(HirExprKind::Variable(hi_name.clone()), Span::dummy())),
                    Box::new(HirExpr::new(HirExprKind::IntLiteral(*n as i128), Span::dummy())),
                ), Span::dummy())),
                then_block: Box::new(HirBlock { span: Span::dummy(),
                    stmts: vec![HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Assign{
                        target: hi_name.clone(),
                        op: HirAssignOp::Assign,
                        value: Box::new(HirExpr::new(HirExprKind::IntLiteral(*n as i128), Span::dummy())),
                    }, Span::dummy())), Span::dummy())],
                    final_expr: None,
                }),
                else_block: None,
            }, Span::dummy())), Span::dummy()),
        ];
        // 循环：while __i < __hi { __out.push(__base[__i]); __i += 1 }
        stmts.push(HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::While{
            cond: Box::new(HirExpr::new(HirExprKind::Binary(
                HirBinaryOp::Lt,
                Box::new(HirExpr::new(HirExprKind::Variable(i_name.clone()), Span::dummy())),
                Box::new(HirExpr::new(HirExprKind::Variable(hi_name.clone()), Span::dummy())),
            ), Span::dummy())),
            body: Box::new(HirBlock { span: Span::dummy(),
                stmts: vec![
                    HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Call{
                        callee: push_name,
                        args: vec![
                            HirExpr::new(HirExprKind::Variable(out_name.clone()), Span::dummy()),
                            HirExpr::new(HirExprKind::Index{
                                base: Box::new(HirExpr::new(HirExprKind::Variable(base_name.clone()), Span::dummy())),
                                index: Box::new(HirExpr::new(HirExprKind::Variable(i_name.clone()), Span::dummy())),
                                elem: elem_scalar,
                                is_str: is_byte,
                            }, Span::dummy()),
                        ],
                    }, Span::dummy())), Span::dummy()),
                    HirStmt::new(HirStmtKind::Expr(HirExpr::new(HirExprKind::Assign{
                        target: i_name.clone(),
                        op: HirAssignOp::AddAssign,
                        value: Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
                    }, Span::dummy())), Span::dummy()),
                ],
                final_expr: None,
            }),
        }, Span::dummy())), Span::dummy()));
        Ok((
            HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                stmts,
                final_expr: Some(HirExpr::new(HirExprKind::Variable(out_name), Span::dummy())),
            })), Span::dummy()),
            vec_ty,
        ))
    } else {
        unreachable!("切片对象类型已被前置判断过滤")
    }
}

pub(super) fn type_slot_count(ctx: &TypeContext, ty: &Type, span: Span) -> Result<usize, TypeError> {
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
