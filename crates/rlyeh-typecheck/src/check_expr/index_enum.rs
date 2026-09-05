//! 表达式检查子模块：索引/数组/枚举变体/匹配。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use super::*;
use crate::check_expr::util::{pattern_bind_names, type_to_ast};
use crate::comparison::{check_comparison, compare_hir};
use rlyeh_ast::CompareOp;
use rlyeh_hir::FieldScalar;

pub(super) fn check_index(
    ctx: &mut TypeContext,
    expr: &AstExpr,
    index: &AstExpr,
    span: Span,
    depth: usize,
) -> Result<(HirExpr, Type), TypeError> {
    let (_, b_ty) = infer_expr(ctx, expr)?;
    match check_index_inner(ctx, expr, index, span) {
        Ok(r) => Ok(r),
        // M2（SH-P1-4）：索引对象未命中且接收者类型实现了 `deref` 时，对接收者
        // 插入 `*(recv.deref())` 递归重试（限深度，避免无限）。零新增 IR 节点。
        Err(_)
            if depth < MAX_DEREF_DEPTH
                && ctx
                    .find_impl_for_method(&peel_refs_and_heap(&b_ty), "deref")
                    .is_some() =>
        {
            let deref_ast = make_deref_receiver(expr, span);
            check_index(ctx, &deref_ast, index, span, depth + 1)
        }
        Err(e) => Err(e),
    }
}

pub(super) fn check_index_inner(
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
        // P8：边界可为 None（切片省略边界 `v[..]`/`v[0..]`/`v[..<3]`）
        return check_slice(
            ctx,
            expr,
            lower.as_ref(),
            upper.as_ref(),
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
    // U3 核心项（2026-08-30）：标量枚举值即 tag（整数），可作数组索引。
    if !i_ty.is_integer() && !matches!(&i_ty, Type::ScalarEnum(_)) {
        return Err(TypeError::ExpectedInt {
            found: i_ty.to_string(),
            span: index.span,
        });
    }
    // `&str`（String 对象的只读借用）：索引前先取槽 0 的 data 指针（同 String 对象）
    if let Type::Ref(inner, _) = &b_ty {
        if matches!(**inner, Type::Str) {
            return Ok((
                HirExpr::new(HirExprKind::Index{
                    base: Box::new(HirExpr::new(HirExprKind::FieldGet{
                        base: Box::new(b_hir),
                        index: 0,
                        ty: FieldScalar::Ptr,
                    }, Span::dummy())),
                    index: Box::new(i_hir),
                    elem: FieldScalar::Int,
                    is_str: true,
                }, Span::dummy()),
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
                HirExpr::new(HirExprKind::Index{
                    base: Box::new(b_hir),
                    index: Box::new(i_hir),
                    elem: field_scalar_of(&elem_sub),
                    is_str: is_byte,
                }, Span::dummy()),
                elem_sub,
            ))
        }
        Type::Str => Ok((
            HirExpr::new(HirExprKind::Index{
                base: Box::new(b_hir),
                index: Box::new(i_hir),
                elem: FieldScalar::Char,
                is_str: true,
            }, Span::dummy()),
            Type::Char,
        )),
        // S2 切片索引 `s[i]`：切片胖指针 `{data, len}` 的槽 0 即 data 指针，
        // 取槽 0 后按元素步长 GEP（同 Vec / &str 索引）；`&[u8]` 按字节步长 1。
        Type::Slice(elem_ty) => {
            let elem_sub = substitute(&elem_ty, &ctx.generic_subst);
            let is_byte = matches!(elem_sub, Type::U8);
            Ok((
                HirExpr::new(HirExprKind::Index{
                    base: Box::new(HirExpr::new(HirExprKind::FieldGet{
                        base: Box::new(b_hir),
                        index: 0,
                        ty: FieldScalar::Ptr,
                    }, Span::dummy())),
                    index: Box::new(i_hir),
                    elem: field_scalar_of(&elem_sub),
                    is_str: is_byte,
                }, Span::dummy()),
                elem_sub,
            ))
        }
        Type::RawPtr(elem_ty, _) => {
            // 裸指针索引 `p[i]`：对 base 指针做 GEP 到元素 i（base 即元素 0 地址），
            // 返回元素类型。与数组/Vec 索引同构（codegen 对 base 做 GEP），支持
            // 引用迭代器 `&p[0]` 取元素引用（V1 IterRef 零拷贝视图）。
            // SH-P0-1（裸指针字节步长）：元素为 `u8` 时按 1 字节步长（is_str），
            // 与 `Vec<u8>`/`String` 紧凑字节存储一致（如 `vec.as_ptr()[i]` 取第 i 字节）；
            // 其余元素沿用 8 字节槽步长（维持既有裸指针语义）。
            let elem_sub = substitute(&elem_ty, &ctx.generic_subst);
            let is_byte = matches!(elem_sub, Type::U8);
            Ok((
                HirExpr::new(HirExprKind::Index{
                    base: Box::new(b_hir),
                    index: Box::new(i_hir),
                    elem: field_scalar_of(&elem_sub),
                    is_str: is_byte,
                }, Span::dummy()),
                elem_sub,
            ))
        }
        Type::Named(n, args) => {
            let full = ctx.resolve_full_name(&n).unwrap_or_else(|| n.clone());
            // `s[i]`：String 对象按字节索引（步长 1），base 取槽 0 的 data 指针
            if full == "String" && ctx.lookup_struct(&full).is_some() {
                Ok((
                    HirExpr::new(HirExprKind::Index{
                        base: Box::new(HirExpr::new(HirExprKind::FieldGet{
                            base: Box::new(b_hir),
                            index: 0,
                            ty: FieldScalar::Ptr,
                        }, Span::dummy())),
                        index: Box::new(i_hir),
                        elem: FieldScalar::Int,
                        is_str: true,
                    }, Span::dummy()),
                    Type::U8,
                ))
            } else if full == "Vec" && ctx.lookup_struct(&full).is_some() {
                // `v[i]`：Vec 动态数组按元素索引，base 取槽 0 的 data 指针；
                // 元素类型取 `Vec<T>` 的类型参数并经泛型替换。
                // 步长按元素类型：`Vec<u8>` 为紧凑字节存储（步长 1），与数组
                // `[u8; N]` / 切片 `&[u8]` 一致（原固定步长 8，与紧凑存储不符）。
                let elem_ty = args.first().cloned().unwrap_or(Type::Infer);
                let elem_sub = substitute(&elem_ty, &ctx.generic_subst);
                let is_byte = matches!(elem_sub, Type::U8);
                Ok((
                    HirExpr::new(HirExprKind::Index{
                        base: Box::new(HirExpr::new(HirExprKind::FieldGet{
                            base: Box::new(b_hir),
                            index: 0,
                            ty: FieldScalar::Ptr,
                        }, Span::dummy())),
                        index: Box::new(i_hir),
                        elem: field_scalar_of(&elem_sub),
                        is_str: is_byte,
                    }, Span::dummy()),
                    elem_sub,
                ))
            } else {
                Err(TypeError::WrongType {
                    expected: "array or string".to_string(),
                    found: b_ty.to_string(),
                    span,
                    related: vec![],
                })
            }
        }
        other => Err(TypeError::WrongType {
            expected: "array or string".to_string(),
            found: other.to_string(),
            span,
            related: vec![],
        }),
    }
}

pub(super) fn check_array_lit(
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
                    related: vec![],
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
    let mut stmts = vec![HirStmt::new(HirStmtKind::Let{
        name: base.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: elems.len(),
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy())];
    for (i, h) in hir_elems.into_iter().enumerate() {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: i,
            value: Box::new(h),
            ty: elem_scalar,
        }, Span::dummy())), Span::dummy()));
    }
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(base), Span::dummy())),
        })), Span::dummy()),
        Type::Array(Box::new(elem_ty), elems.len()),
    ))
}

pub(super) fn field_is_scalar_slot(fty: &Type) -> bool {
    match fty {
        Type::Generic(_) | Type::Infer => false,
        t => field_scalar_of(t) != FieldScalar::Ptr,
    }
}

pub(super) fn enum_instance_by_value(ctx: &TypeContext, enum_name: &str, slot_count: usize) -> bool {
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

pub(super) fn check_variant_construct(
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

    // U3 核心项（2026-08-30）：受限标量枚举紧凑为单标量存储——构造直接产出
    // `IntLiteral(tag)`，值即 tag 本身（不再 Alloc 对象指针）；类型记为
    // `ScalarEnum` 以便 `field_scalar_of` 等按 Int 布局处理。
    if ctx.is_scalar_enum(&enum_name) {
        return Ok((
            HirExpr::new(HirExprKind::IntLiteral(variant.tag as i128), Span::dummy()),
            Type::ScalarEnum(enum_name.to_string()),
        ));
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
    let mut stmts = vec![HirStmt::new(HirStmtKind::Let{
        name: base.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: enum_def.slot_count,
            by_value: enum_by_value,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy())];
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
        index: 0,
        value: Box::new(HirExpr::new(HirExprKind::IntLiteral(variant.tag as i128), Span::dummy())),
        ty: FieldScalar::Int,
    }, Span::dummy())), Span::dummy()));
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
                related: vec![],
            });
        }
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 1 + i,
            value: Box::new(hir),
            ty: field_scalar_of(&fty),
        }, Span::dummy())), Span::dummy()));
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
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(HirExpr::new(HirExprKind::Variable(base), Span::dummy())),
        })), Span::dummy()),
        ty,
    ))
}

pub(super) fn check_question(
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
    let (none_pat, _ret_args) = if has_err_field {
        (
            AstPattern::Enum(none_variant.to_string(), vec![AstPattern::Ident(err.clone())]),
            vec![mk_ident(err.clone())],
        )
    } else {
        (AstPattern::Enum(none_variant.to_string(), vec![]), vec![])
    };
    // 失败变体经完整路径构造（`Option::None` / `Result::Err(e)`）：裸 `None`
    // 是 Ident（infer_expr 无变体兜底），带参变体是 Call（走 check_call 的
    // split_variant_path 兜底）。枚举名取自 `Type::Named`（可能含模块路径）。
    // P6c（2026-08-29）：若错误类型不同（`E1` ≠ `E2`），经 trait 关联函数
    // `From::<E1>::from(__e)` 自动转换（需 `impl From<E1> for E2`），语义对齐 Rust `?`。
    let err_arg: AstExpr = if has_err_field {
        let target_err = match &ctx.current_return_type {
            Some(Type::Named(rn, rargs)) if rn.ends_with("Result") => rargs.get( 1).cloned(),
            _ => None,
        };
        let e1 = match &inner_ty {
            Type::Named(_, iargs) => iargs.get(1).cloned(),
            _ => None,
        };
        match (e1, target_err) {
            (Some(e1), Some(e2)) if !e1.compatible_with(&e2) => AstExpr::new(
                ExprKind::Call {
                    callee: AstExpr::new(
                        ExprKind::Path(vec!["From".to_string(), "from".to_string()]),
                        span,
                    ),
                    args: vec![mk_ident(err.clone())],
                    type_args: vec![type_to_ast(&e1)],
                },
                span,
            ),
            _ => mk_ident(err),
        }
    } else {
        mk_ident(err)
    };

    let Type::Named(en_name, _) = &inner_ty else {
        unreachable!("Option/Result 分支已保证 Named")
    };
    let mut callee_segs: Vec<String> = en_name.split("::").map(|s| s.to_string()).collect();
    callee_segs.push(none_variant.to_string());
    let callee = AstExpr::new(ExprKind::Path(callee_segs), span);
    let ret_inner = if has_err_field {
        AstExpr::new(
            ExprKind::Call {
                callee,
                args: vec![err_arg],
                type_args: Vec::new(),
            },
            span,
        )
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

pub(super) fn check_match(
    ctx: &mut TypeContext,
    expr: &AstExpr,
    arms: &[rlyeh_ast::MatchArm],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let (s_hir, s_ty) = infer_expr(ctx, expr)?;
    check_match_with_scrutinee(ctx, s_hir, s_ty, arms, span)
}

pub(super) fn check_match_with_scrutinee(
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
    let tmp_var = HirExpr::new(HirExprKind::Variable(tmp.clone()), Span::dummy());
    let stmts = vec![HirStmt::new(HirStmtKind::Let{
        name: tmp.clone(),
        init: s_hir,
        mutable: true,
    }, Span::dummy())];

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
            // 守卫条件（`pattern if guard => body`）：与模式条件合并。
            //
            // **守卫必须在绑定之后求值**——`Option::Some(v) if v > 10` 的 `v`
            // 由模式绑定产生，而绑定语句位于臂体块内、晚于条件求值。
            // 不可直接 `模式条件 && 守卫`：MIR 的 `&&` 是**非短路**的
            // （`lower_expr` 对 `Binary` 两侧无条件求值），绑定会被无条件
            // 执行，指针类负载上还会解引用到未初始化值。
            // 改为以 `if <模式条件> { <绑定>; <守卫> } else { false }` 作条件，
            // 借用 HIR `If` 的真实 CFG 分叉获得短路语义——绑定与守卫仅在
            // 模式命中后求值。代价：绑定被求值两次（条件内一次、臂体内
            // 一次），均为纯字段读取 / 取址，无副作用。
            let cond = match (&arm.guard, cond) {
                (Some(guard), Some(c)) => {
                    let (g_hir, _) = infer_expr(ctx, guard)?;
                    Some(HirExpr::new(HirExprKind::If{
                        cond: Box::new(c),
                        then_block: Box::new(HirBlock { span: Span::dummy(),
                            stmts: binds.clone(),
                            final_expr: Some(g_hir),
                        }),
                        // 兜底分支用 `BoolLiteral(false)`：bool 的 LLVM 表示为
                        // i1（`llvm_type(LirType::Bool) == "i1"`），与守卫值同源。
                        else_block: Some(Box::new(HirBlock { span: Span::dummy(),
                            stmts: Vec::new(),
                            final_expr: Some(HirExpr::new(HirExprKind::BoolLiteral(false), Span::dummy())),
                        })),
                    }, Span::dummy()))
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
                related: vec![],
            });
        }
        result_ty = body_ty;
        let then_block = HirBlock { span: Span::dummy(),
            stmts: binds,
            final_expr: Some(body_hir),
        };
        else_hir = Some(if is_binding {
            // 兜底模式（标识符 / 通配符）：直接作为 else 分支
            HirExpr::new(HirExprKind::Block(Box::new(then_block)), Span::dummy())
        } else {
            HirExpr::new(HirExprKind::If{
                cond: Box::new(cond.ok_or_else(|| TypeError::Unsupported {
                    what: "无条件的非兜底 match 模式".to_string(),
                    span,
                })?),
                then_block: Box::new(then_block),
                else_block: else_hir.map(|e| {
                    Box::new(HirBlock { span: Span::dummy(),
                        stmts: Vec::new(),
                        final_expr: Some(e),
                    })
                }),
            }, Span::dummy())
        });
    }

    let final_expr = else_hir.unwrap_or(HirExpr::new(HirExprKind::Unit, Span::dummy()));
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts,
            final_expr: Some(final_expr),
        })), Span::dummy()),
        result_ty,
    ))
}

pub(super) fn check_pattern(
    ctx: &mut TypeContext,
    pat: &rlyeh_ast::AstPattern,
    pat_ty: &Type,
    scrutinee: HirExpr,
    span: Span,
) -> Result<PatternResult, TypeError> {
    use rlyeh_ast::AstPattern;
    match pat {
        AstPattern::Ident(name) => {
            // U2：**类型臂收窄**——`pat_ty` 为联合且 `name` 恰为某成员的类型名时，
            // 按该成员收窄：`cond` = `tag == 成员下标`，并把 payload（槽 1）按成员
            // 类型绑定到**同名变量**（MVP 约定：`match x { i64 => println(i64) }`；
            // 后续可引入 `x @ T` 绑定语法以免类型名 / 变量名混用）。
            // 联合的运行时表示即匿名 enum（槽 0 tag + 槽 1 payload），故与
            // `AstPattern::Enum` 分支同构，复用现有 tag 比较与 FieldGet 通道。
            if let Type::Union(us) = pat_ty {
                if let Some(idx) = us.iter().position(|u| u.to_string() == *name) {
                    let member_ty = us[idx].clone();
                    let slot = ctx.insert_variable(name.clone(), member_ty.clone());
                    let tag_cond = HirExpr::new(HirExprKind::Binary(
                        HirBinaryOp::Eq,
                        Box::new(HirExpr::new(HirExprKind::FieldGet{
                            base: Box::new(scrutinee.clone()),
                            index: 0,
                            ty: FieldScalar::Int,
                        }, Span::dummy())),
                        Box::new(HirExpr::new(HirExprKind::IntLiteral(idx as i128), Span::dummy())),
                    ), Span::dummy());
                    let payload = HirExpr::new(HirExprKind::FieldGet{
                        base: Box::new(scrutinee),
                        index: 1,
                        ty: field_scalar_of(&member_ty),
                    }, Span::dummy());
                    return Ok((
                        Some(tag_cond),
                        vec![HirStmt::new(HirStmtKind::Let{
                            name: slot,
                            init: payload,
                            mutable: false,
                        }, Span::dummy())],
                        false,
                        vec![(name.clone(), member_ty)],
                    ));
                }
            }
            // U1：绑定变量在臂作用域内注册（遮蔽时 mangle 存储槽名）。
            // Let 绑定、引用解析、类型表全部使用槽名。
            let slot = ctx.insert_variable(name.clone(), pat_ty.clone());
            Ok((
                None,
                vec![HirStmt::new(HirStmtKind::Let{
                    name: slot.clone(),
                    init: scrutinee,
                    mutable: false,
                }, Span::dummy())],
                true,
                vec![(slot, pat_ty.clone())],
            ))
        }
        AstPattern::Wildcard => Ok((None, Vec::new(), true, Vec::new())),
        AstPattern::Literal(lit) => {
            let lit_hir = literal_to_hir(lit, span)?;
            let cond = HirExpr::new(HirExprKind::Binary(
                HirBinaryOp::Eq,
                Box::new(scrutinee),
                Box::new(lit_hir),
            ), Span::dummy());
            Ok((Some(cond), Vec::new(), false, Vec::new()))
        }
        AstPattern::Enum(variant, sub_pats) => {
            let en = match pat_ty {
                Type::Named(en, _) => en.clone(),
                Type::ScalarEnum(en) => en.clone(),
                _ => {
                    return Err(TypeError::Unsupported {
                        what: format!("对非枚举类型 `{pat_ty}` 使用枚举模式 `{variant}`"),
                        span,
                    });
                }
            };
            // 枚举名可能为 use 导入的本地名（`use protocol::Msg` 后裸名 `Msg`），
            // 裸名未注册时回退经 resolve_full_name 解析完整符号名（模块前缀 / use 别名）。
            let enum_def = ctx
                .lookup_enum(&en)
                .cloned()
                .or_else(|| {
                    ctx.resolve_full_name(&en)
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
            // 主条件：标量枚举直接整数比较（`scrutinee == tag`，值即 tag 本身），
            // 非标量枚举取对象槽 0 的 tag 比较。
            let tag_cond = if matches!(pat_ty, Type::ScalarEnum(_)) {
                HirExpr::new(HirExprKind::Binary(
                    HirBinaryOp::Eq,
                    Box::new(scrutinee.clone()),
                    Box::new(HirExpr::new(HirExprKind::IntLiteral(variant_def.tag as i128), Span::dummy())),
                ), Span::dummy())
            } else {
                HirExpr::new(HirExprKind::Binary(
                    HirBinaryOp::Eq,
                    Box::new(HirExpr::new(HirExprKind::FieldGet{
                        base: Box::new(scrutinee.clone()),
                        index: 0,
                        ty: FieldScalar::Int,
                    }, Span::dummy())),
                    Box::new(HirExpr::new(HirExprKind::IntLiteral(variant_def.tag as i128), Span::dummy())),
                ), Span::dummy())
            };
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
                    HirExpr::new(HirExprKind::FieldGet{
                        base: Box::new(scrutinee.clone()),
                        index: 1 + i,
                        ty: field_scalar_of(&fty_sub),
                    }, Span::dummy()),
                    span,
                )?;
                binds.extend(sub_binds);
                bound_tys.extend(sub_tys);
                if let Some(sc) = sub_cond {
                    cond = HirExpr::new(HirExprKind::Binary(
                        HirBinaryOp::And,
                        Box::new(cond),
                        Box::new(sc),
                    ), Span::dummy());
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
        AstPattern::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => {
            // P-M2（SH-P0-7）：范围模式 `lo..<hi` / `lo...hi` / `lo<..hi`。
            // 边界由 parser 从字面量模式转写为表达式，此处经 `infer_expr` 求值；
            // 类型校验与 HIR 构造分别复用比较链的 `check_comparison` /
            // `compare_hir`，故模式侧语义与 `x in lo..<hi` 完全一致
            // （排序仅放行数值与字符，其余报 `MissingPartialOrd`）。
            let (lower_hir, lower_ty) = infer_expr(ctx, lower)?;
            let (upper_hir, upper_ty) = infer_expr(ctx, upper)?;
            let lo_op = if *lower_inclusive {
                CompareOp::Ge
            } else {
                CompareOp::Gt
            };
            let hi_op = if *upper_inclusive {
                CompareOp::Le
            } else {
                CompareOp::Lt
            };
            check_comparison(pat_ty, &lower_ty, lo_op, span)?;
            check_comparison(pat_ty, &upper_ty, hi_op, span)?;
            let lo = compare_hir(&scrutinee, lo_op, &lower_hir);
            let hi = compare_hir(&scrutinee, hi_op, &upper_hir);
            Ok((
                Some(HirExpr::new(HirExprKind::Binary(
                    HirBinaryOp::And,
                    Box::new(lo),
                    Box::new(hi),
                ), Span::dummy())),
                Vec::new(),
                false,
                Vec::new(),
            ))
        }
        AstPattern::Or(alts) => {
            // P-M3（SH-P0-7）：或模式 `A | B`——任一备选命中即进入 arm。
            // 绑定**只取首个备选**（其变量注册在当前 arm 作用域内）；其余备选在
            // 临时作用域内检查，仅取条件，注册随即丢弃（条件只依赖 scrutinee，
            // 不引用被丢弃的槽名）。
            // 一致性：各备选须绑定同名同序的变量集（与 Rust 一致，
            // `Some(x) | None` / `Some(x) | Some(y)` 均报 Unsupported）。
            let first = alts.first().ok_or_else(|| TypeError::Unsupported {
                what: "空或模式".to_string(),
                span,
            })?;
            let (first_cond, binds, first_binding, bound_tys) =
                check_pattern(ctx, first, pat_ty, scrutinee.clone(), span)?;
            if first_binding {
                return Err(TypeError::Unsupported {
                    what: "或模式含不可反驳备选（标识符 / `_`）".to_string(),
                    span,
                });
            }
            let expect_names = pattern_bind_names(first);
            let mut cond = first_cond.ok_or_else(|| TypeError::Unsupported {
                what: "或模式备选缺少匹配条件".to_string(),
                span,
            })?;
            for alt in &alts[1..] {
                ctx.push_scope(false);
                let checked = check_pattern(ctx, alt, pat_ty, scrutinee.clone(), span);
                ctx.pop_scope();
                let (alt_cond, _, alt_binding, _) = checked?;
                if alt_binding {
                    return Err(TypeError::Unsupported {
                        what: "或模式含不可反驳备选（标识符 / `_`）".to_string(),
                        span,
                    });
                }
                let got_names = pattern_bind_names(alt);
                if got_names != expect_names {
                    return Err(TypeError::Unsupported {
                        what: format!(
                            "或模式各备选绑定不一致（首个备选绑定 [{}]，此备选绑定 [{}]）",
                            expect_names.join(", "),
                            got_names.join(", ")
                        ),
                        span,
                    });
                }
                let alt_cond = alt_cond.ok_or_else(|| TypeError::Unsupported {
                    what: "或模式备选缺少匹配条件".to_string(),
                    span,
                })?;
                cond = HirExpr::new(HirExprKind::Binary(HirBinaryOp::Or, Box::new(cond), Box::new(alt_cond)), Span::dummy());
            }
            Ok((Some(cond), binds, false, bound_tys))
        }
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
                    vec![HirStmt::new(HirStmtKind::Let{
                        name: slot.clone(),
                        init: HirExpr::new(HirExprKind::Ref{
                            expr: Box::new(scrutinee),
                            is_mut: *is_mut,
                            pointee: field_scalar_of(pat_ty),
                        }, Span::dummy()),
                        mutable: *is_mut,
                    }, Span::dummy())],
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
                .map(|b| match b.kind {
                    HirStmtKind::Let{ name, init, mutable: _ } => HirStmt::new(HirStmtKind::Let{
                        name,
                        init: HirExpr::new(HirExprKind::Ref{
                            expr: Box::new(init),
                            is_mut: *is_mut,
                            pointee: field_scalar_of(pat_ty),
                        }, Span::dummy()),
                        mutable: *is_mut,
                    }, Span::dummy()),
                    other => HirStmt::new(other, Span::dummy()),
                })
                .collect();
            Ok((cond, binds, is_binding, bound_tys))
        }
    }
}

pub(super) fn literal_to_hir(lit: &rlyeh_ast::LiteralValue, span: Span) -> Result<HirExpr, TypeError> {
    use rlyeh_ast::LiteralValue;
    Ok(match lit {
        LiteralValue::Int(v) => HirExpr::new(HirExprKind::IntLiteral(*v), Span::dummy()),
        LiteralValue::Float(v) => HirExpr::new(HirExprKind::FloatLiteral(*v), Span::dummy()),
        LiteralValue::Str(s) => HirExpr::new(HirExprKind::StringLiteral(s.clone()), Span::dummy()),
        LiteralValue::Char(c) => HirExpr::new(HirExprKind::CharLiteral(*c), Span::dummy()),
        LiteralValue::Bool(b) => HirExpr::new(HirExprKind::BoolLiteral(*b), Span::dummy()),
        LiteralValue::Time { .. } => {
            return Err(TypeError::Unsupported {
                what: "时间字面量模式".to_string(),
                span,
            })
        }
    })
}
