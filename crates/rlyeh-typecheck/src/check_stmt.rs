//! 语句类型检查。

use rlyeh_lexer::Span;
use rlyeh_ast::{AstPattern, AstStmt, ExprKind};
use rlyeh_hir::{FieldScalar, HirBlock, HirExpr, HirStmt, HirExprKind, HirStmtKind};

use crate::check_expr::{
    check_closure_expected, check_closure_value_binding, check_deferred_closure_binding, coerce_to_dyn,
    infer_expr, make_slice_fat, resolve_ast_type, substitute,
};
use crate::context::TypeContext;
use crate::error::TypeError;
use crate::types::{field_scalar_of, Mutability, Type};

/// U2：构造联合值——匿名 enum 布局（槽 0 = tag、槽 1 = payload）。
///
/// 联合的运行时表示即**匿名 enum**：不注册 `EnumDef`（避免污染全局变体名空间），
/// 直接内联生成与 `check_variant_construct` 同构的 HIR——`Alloc(2)` +
/// `FieldSet(0, tag)` + `FieldSet(1, value)`。布局与具名 enum 一致，故完全
/// 复用现有 enum codegen 通道，无新增 codegen 逻辑。
///
/// 分配方式统一走 **calloc 堆对象**（`by_value: false`）：联合的不同成员可能
/// 分别是标量（`i64`）与聚合（`String`），若按成员分别决定栈槽 / 堆分配，
/// 同一联合的各构造路径判定会不一致（`check_variant_construct` 明确要求一致），
/// 且栈槽地址存入联合值后传出函数会悬垂。统一堆分配规避这两类问题。
pub(crate) fn make_union_ctor(
    ctx: &mut TypeContext,
    value: HirExpr,
    tag: usize,
    member_ty: &Type,
) -> HirExpr {
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
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(tag as i128), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 1,
            value: Box::new(value),
            ty: field_scalar_of(member_ty),
        }, Span::dummy())), Span::dummy()),
    ];
    HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
        stmts,
        final_expr: Some(HirExpr::new(HirExprKind::Variable(base), Span::dummy())),
    })), Span::dummy())
}

/// 检查语句并生成 HIR 语句。
///
/// 返回**语句序列**而非单条：M2（SH-P0-5）元组解构 `let (a, b) = e;` 需展开为
/// 多条 `Let`（临时变量承载元组值 + 各元素按位置绑定），其余语句恒为单条。

/// 递归展开元组解构（含嵌套 `let ((a, b), c) = e;`）。
///
/// `base` 为待读取的元组值表达式（或指向元组的引用，见 `Type::Ref` 分支），
/// `base_ty` 为其类型，`pats` 为当前层级的模式，`pat_span` 用于类型 / 元数
/// 不匹配时回指模式声明处。每个元素按位置经 `FieldGet` 取出：标识符直接绑定，
/// 通配符 `_` 跳过，嵌套 `Tuple` 则以该字段值（元组）为新的 `base` 递归展开。
fn lower_tuple_destructure(
    ctx: &mut TypeContext,
    base: HirExpr,
    base_ty: &Type,
    pats: &[AstPattern],
    pat_span: Span,
    mutable: bool,
    span: Span,
) -> Result<Vec<HirStmt>, TypeError> {
    let ts = match base_ty {
        Type::Tuple(ts) => Some(ts.clone()),
        // 引用到元组：剥一层引用后按元组解构（`let (a, b) = &t;`）
        Type::Ref(inner, _) => match &**inner {
            Type::Tuple(ts) => Some(ts.clone()),
            _ => None,
        },
        _ => None,
    };
    let ts = match ts {
        Some(ts) => ts,
        None => {
            return Err(TypeError::WrongType {
                expected: format!("元组（{} 元）", pats.len()),
                found: base_ty.to_string(),
                span,
                related: vec![(pat_span, "元组解构模式声明于此".to_string())],
            })
        }
    };
    if ts.len() != pats.len() {
        return Err(TypeError::WrongType {
            expected: format!("{} 元元组", ts.len()),
            found: format!("{} 元解构模式", pats.len()),
            span,
            related: vec![(pat_span, "元组解构模式声明于此".to_string())],
        });
    }
    let mut out = Vec::new();
    for (i, p) in pats.iter().enumerate() {
        let fty = ts[i].clone();
        let val = HirExpr::new(
            HirExprKind::FieldGet {
                base: Box::new(base.clone()),
                index: i,
                ty: field_scalar_of(&fty),
            },
            Span::dummy(),
        );
        match p {
            AstPattern::Wildcard => {}
            AstPattern::Ident(name) => {
                let stored = ctx.insert_variable(name.clone(), fty);
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: stored,
                        init: val,
                        mutable,
                    },
                    Span::dummy(),
                ));
            }
            AstPattern::Tuple(nested, nested_span) => {
                // 嵌套元组：以当前字段值（元组）为新的 base 递归展开；
                // 内层临时变量不可变（仅内部中转），最终标识符绑定沿用 `mutable`。
                let inner_tmp = ctx.fresh_temp();
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: inner_tmp.clone(),
                        init: val,
                        mutable: false,
                    },
                    Span::dummy(),
                ));
                let inner = lower_tuple_destructure(
                    ctx,
                    HirExpr::new(HirExprKind::Variable(inner_tmp), Span::dummy()),
                    &fty,
                    nested,
                    *nested_span,
                    mutable,
                    span,
                )?;
                out.extend(inner);
            }
            AstPattern::Enum(nv, ns) => {
                let inner_tmp = ctx.fresh_temp();
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: inner_tmp.clone(),
                        init: val,
                        mutable: false,
                    },
                    Span::dummy(),
                ));
                let inner = lower_enum_destructure(
                    ctx,
                    HirExpr::new(HirExprKind::Variable(inner_tmp), Span::dummy()),
                    &fty,
                    nv,
                    ns,
                    mutable,
                    span,
                )?;
                out.extend(inner);
            }
            AstPattern::EnumPath(segments, ns) => {
                let nv = segments.last().cloned().ok_or_else(|| {
                    TypeError::Unsupported {
                        what: "空路径枚举模式".to_string(),
                        span,
                    }
                })?;
                let inner_tmp = ctx.fresh_temp();
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: inner_tmp.clone(),
                        init: val,
                        mutable: false,
                    },
                    Span::dummy(),
                ));
                let inner = lower_enum_destructure(
                    ctx,
                    HirExpr::new(HirExprKind::Variable(inner_tmp), Span::dummy()),
                    &fty,
                    &nv,
                    ns,
                    mutable,
                    span,
                )?;
                out.extend(inner);
            }
            _ => {
                return Err(TypeError::Unsupported {
                    what: "嵌套解构模式（元组内仅支持标识符 / `_` / 嵌套元组 / 嵌套枚举）".to_string(),
                    span,
                })
            }
        }
    }
    Ok(out)
}

/// 递归展开结构体解构（含嵌套 `let Point { x, y: (a, b) } = e;`）。
///
/// `base` 为待读取的结构体值表达式（或 `&` 引用），`base_ty` 须为具名结构体
/// `Type::Named`（或其引用）。`fields` 为模式中的「字段名 → 子模式」列表；每个字段
/// 按名查 `StructDef.fields` 得下标，经 `FieldGet` 取出后按子模式绑定（标识符 / `_` /
/// 嵌套 `Tuple` / 嵌套 `Struct`），与 `p.x` 字段访问同构。
fn lower_struct_destructure(
    ctx: &mut TypeContext,
    base: HirExpr,
    base_ty: &Type,
    fields: &[(String, AstPattern)],
    mutable: bool,
    span: Span,
) -> Result<Vec<HirStmt>, TypeError> {
    let struct_name = match base_ty {
        Type::Named(n, _) => n.clone(),
        // 引用到结构体：剥一层引用后按结构体解构（`let Point { x } = &p;`）
        Type::Ref(inner, _) => match &**inner {
            Type::Named(n, _) => n.clone(),
            _ => {
                return Err(TypeError::ExpectedStruct {
                    found: base_ty.to_string(),
                    span,
                })
            }
        },
        _ => {
            return Err(TypeError::ExpectedStruct {
                found: base_ty.to_string(),
                span,
            })
        }
    };
    let def = ctx
        .lookup_struct(&struct_name)
        .cloned()
        .ok_or_else(|| TypeError::UndefinedType {
            name: struct_name.clone(),
            span,
        })?;
    let mut out = Vec::new();
    for (fname, p) in fields {
        let idx = def
            .fields
            .iter()
            .position(|(n, _)| n == fname)
            .ok_or_else(|| TypeError::UnknownField {
                struct_name: struct_name.clone(),
                field: fname.clone(),
                span,
            })?;
        let fty = def.fields[idx].1.clone();
        let val = HirExpr::new(
            HirExprKind::FieldGet {
                base: Box::new(base.clone()),
                index: idx,
                ty: field_scalar_of(&fty),
            },
            Span::dummy(),
        );
        match p {
            AstPattern::Wildcard => {}
            AstPattern::Ident(name) => {
                let stored = ctx.insert_variable(name.clone(), fty);
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: stored,
                        init: val,
                        mutable,
                    },
                    Span::dummy(),
                ));
            }
            AstPattern::Tuple(nested, nested_span) => {
                // 嵌套元组字段：`field: (a, b)` —— 取出字段值（元组）后递归展开
                let inner_tmp = ctx.fresh_temp();
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: inner_tmp.clone(),
                        init: val,
                        mutable: false,
                    },
                    Span::dummy(),
                ));
                let inner = lower_tuple_destructure(
                    ctx,
                    HirExpr::new(HirExprKind::Variable(inner_tmp), Span::dummy()),
                    &fty,
                    nested,
                    *nested_span,
                    mutable,
                    span,
                )?;
                out.extend(inner);
            }
            AstPattern::Struct(_, nested_fields) => {
                // 嵌套结构体字段：`inner: Inner { a, b }` —— 取出字段值（结构体）
                // 后递归展开（内层临时变量不可变，最终标识符绑定沿用 `mutable`）
                let inner_tmp = ctx.fresh_temp();
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: inner_tmp.clone(),
                        init: val,
                        mutable: false,
                    },
                    Span::dummy(),
                ));
                let inner = lower_struct_destructure(
                    ctx,
                    HirExpr::new(HirExprKind::Variable(inner_tmp), Span::dummy()),
                    &fty,
                    nested_fields,
                    mutable,
                    span,
                )?;
                out.extend(inner);
            }
            AstPattern::Enum(nv, ns) => {
                let inner_tmp = ctx.fresh_temp();
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: inner_tmp.clone(),
                        init: val,
                        mutable: false,
                    },
                    Span::dummy(),
                ));
                let inner = lower_enum_destructure(
                    ctx,
                    HirExpr::new(HirExprKind::Variable(inner_tmp), Span::dummy()),
                    &fty,
                    nv,
                    ns,
                    mutable,
                    span,
                )?;
                out.extend(inner);
            }
            AstPattern::EnumPath(segments, ns) => {
                let nv = segments.last().cloned().ok_or_else(|| {
                    TypeError::Unsupported {
                        what: "空路径枚举模式".to_string(),
                        span,
                    }
                })?;
                let inner_tmp = ctx.fresh_temp();
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: inner_tmp.clone(),
                        init: val,
                        mutable: false,
                    },
                    Span::dummy(),
                ));
                let inner = lower_enum_destructure(
                    ctx,
                    HirExpr::new(HirExprKind::Variable(inner_tmp), Span::dummy()),
                    &fty,
                    &nv,
                    ns,
                    mutable,
                    span,
                )?;
                out.extend(inner);
            }
            _ => {
                return Err(TypeError::Unsupported {
                    what: "嵌套解构模式（结构体字段仅支持标识符 / `_` / 嵌套元组 / 嵌套结构体 / 嵌套枚举）"
                        .to_string(),
                    span,
                })
            }
        }
    }
    Ok(out)
}

/// 枚举解构绑定 `let Some(x) = e;` / `let Some((a, b)) = e;` / `let Point { v: Some(a) } = e;`。
///
/// desugar 为「临时变量承载枚举值 + 按变体字段槽经 `FieldGet` 取出后按子模式绑定」，
/// 与 match 位置 `AstPattern::Enum` 收窄（check_expr/index_enum.rs）同构；此处只绑定、
/// 不生成运行时 tag 校验（与 `let (a,b)` / `let Point{..}` 同哲学：信任类型推断）。
///
/// 非标量枚举运行时为对象 {槽0=tag, 槽1..=payload}，字段按 `1+i` 槽经 `FieldGet` 读取
/// （与构造时对齐）；标量枚举值即 tag 本身、变体均为单元（无字段），故子模式必为空。
fn lower_enum_destructure(
    ctx: &mut TypeContext,
    base: HirExpr,
    base_ty: &Type,
    variant: &str,
    sub_pats: &[AstPattern],
    mutable: bool,
    span: Span,
) -> Result<Vec<HirStmt>, TypeError> {
    let en = match base_ty {
        Type::Named(en, _) => en.clone(),
        Type::ScalarEnum(en) => en.clone(),
        _ => {
            return Err(TypeError::Unsupported {
                what: format!("枚举解构绑定需要枚举类型，得到 `{base_ty}`"),
                span,
            })
        }
    };
    // 枚举名可能为 use 导入的本地名，回退经 resolve_full_name 解析完整符号名。
    let enum_def = ctx
        .lookup_enum(&en)
        .cloned()
        .or_else(|| {
            ctx.resolve_full_name(&en)
                .and_then(|full| ctx.lookup_enum(&full).cloned())
        })
        .ok_or_else(|| TypeError::UndefinedType {
            name: en.clone(),
            span,
        })?;
    let variant_def = enum_def
        .variants
        .iter()
        .find(|v| v.name == variant)
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
    // 字段类型经泛型替换（优先 generic_subst，再从 `base_ty` 类型实参推导枚举泛型映射）。
    let mut subst = ctx.generic_subst.clone();
    if let Type::Named(_, pat_args) = base_ty {
        if !pat_args.is_empty() && pat_args.len() == enum_def.type_params.len() {
            for (tp, arg) in enum_def.type_params.iter().zip(pat_args) {
                subst.insert(tp.clone(), arg.clone());
            }
        }
    }
    let mut out = Vec::new();
    for (i, (sub, (_, fty))) in sub_pats.iter().zip(&variant_def.fields).enumerate() {
        let fty_sub = substitute(fty, &subst);
        let val = HirExpr::new(
            HirExprKind::FieldGet {
                base: Box::new(base.clone()),
                index: 1 + i,
                ty: field_scalar_of(&fty_sub),
            },
            Span::dummy(),
        );
        match sub {
            AstPattern::Wildcard => {}
            AstPattern::Ident(name) => {
                let stored = ctx.insert_variable(name.clone(), fty_sub);
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: stored,
                        init: val,
                        mutable,
                    },
                    Span::dummy(),
                ));
            }
            AstPattern::Tuple(nested, nested_span) => {
                let inner_tmp = ctx.fresh_temp();
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: inner_tmp.clone(),
                        init: val,
                        mutable: false,
                    },
                    Span::dummy(),
                ));
                let inner = lower_tuple_destructure(
                    ctx,
                    HirExpr::new(HirExprKind::Variable(inner_tmp), Span::dummy()),
                    &fty_sub,
                    nested,
                    *nested_span,
                    mutable,
                    span,
                )?;
                out.extend(inner);
            }
            AstPattern::Struct(_, nested_fields) => {
                let inner_tmp = ctx.fresh_temp();
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: inner_tmp.clone(),
                        init: val,
                        mutable: false,
                    },
                    Span::dummy(),
                ));
                let inner = lower_struct_destructure(
                    ctx,
                    HirExpr::new(HirExprKind::Variable(inner_tmp), Span::dummy()),
                    &fty_sub,
                    nested_fields,
                    mutable,
                    span,
                )?;
                out.extend(inner);
            }
            AstPattern::Enum(nested_variant, nested_sub) => {
                let inner_tmp = ctx.fresh_temp();
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: inner_tmp.clone(),
                        init: val,
                        mutable: false,
                    },
                    Span::dummy(),
                ));
                let inner = lower_enum_destructure(
                    ctx,
                    HirExpr::new(HirExprKind::Variable(inner_tmp), Span::dummy()),
                    &fty_sub,
                    nested_variant,
                    nested_sub,
                    mutable,
                    span,
                )?;
                out.extend(inner);
            }
            AstPattern::EnumPath(segments, nested_sub) => {
                let nv = segments.last().cloned().ok_or_else(|| TypeError::Unsupported {
                    what: "空路径枚举模式".to_string(),
                    span,
                })?;
                let inner_tmp = ctx.fresh_temp();
                out.push(HirStmt::new(
                    HirStmtKind::Let {
                        name: inner_tmp.clone(),
                        init: val,
                        mutable: false,
                    },
                    Span::dummy(),
                ));
                let inner = lower_enum_destructure(
                    ctx,
                    HirExpr::new(HirExprKind::Variable(inner_tmp), Span::dummy()),
                    &fty_sub,
                    &nv,
                    nested_sub,
                    mutable,
                    span,
                )?;
                out.extend(inner);
            }
            _ => {
                return Err(TypeError::Unsupported {
                    what: "枚举解构子模式仅支持标识符 / `_` / 嵌套元组 / 嵌套结构体 / 嵌套枚举"
                        .to_string(),
                    span,
                })
            }
        }
    }
    Ok(out)
}

pub(crate) fn check_stmt_inner(
    ctx: &mut TypeContext,
    stmt: &AstStmt,
) -> Result<(Vec<HirStmt>, Type), TypeError> {
    match stmt {
        AstStmt::Let {
            pattern,
            type_anno,
            init,
            mutable,
        } => {
            let span = init.span;
            // SH-P2-6 L2：类型标注处位置，供类型不匹配时把 `= note:` 次级标注指向该
            // 标注（如 `let x: i64 = "s";` 的 `i64` 处）；无标注时为 `None`。
            let anno_span = type_anno.as_ref().map(|s| s.span);
            // H4 去虚拟化：`let d: dyn T = &obj;` 时待记录的具体类型
            // （在 Ident 分支按绑定名写入 ctx.dyn_concrete）
            let mut pending_dyn_concrete: Option<Type> = None;
            // 闭包 let 绑定分四路：
            // - fn 类型注解 → H2 无捕获闭包按预期签名检查（`let f: fn(A) -> B = |x| ..;`）；
            // - 无注解且参数全带类型注解 → 闭包值对象（`let f = |x: i64| ..; f(..)`）；
            // - 无注解且存在无注解参数 → 延迟闭包值绑定（参数类型由首次调用点
            //   实参推断，见 check_deferred_closure_binding；`let f = |x| ..; f(..)`）；
            // - 其余 → 常规推断 + 一致性检查。
            let all_annotated = if let ExprKind::Closure { param_types, .. } = &*init.kind {
                param_types.iter().all(|t| t.is_some())
            } else {
                false
            };
            let (mut h_init, mut ty) =
                if type_anno.is_some() && matches!(&*init.kind, ExprKind::Closure { .. }) {
                    let at = resolve_ast_type(ctx, &type_anno.as_ref().unwrap().ty, span)?;
                    if !matches!(&at, Type::Fn(_)) {
                        return Err(TypeError::WrongType {
                            expected: at.to_string(),
                            found: "闭包".to_string(),
                            span,
                            related: anno_span
                                .into_iter()
                                .filter(|s| s.line != 0)
                                .map(|s| (s, format!("类型标注 `{at}`")))
                                .collect(),
                        });
                    }
                    check_closure_expected(ctx, init, &at, span)?
                } else if type_anno.is_none() && matches!(&*init.kind, ExprKind::Closure { .. }) {
                    if all_annotated {
                        check_closure_value_binding(ctx, init, span)?
                    } else {
                        check_deferred_closure_binding(ctx, init, span)?
                    }
                } else {
                    infer_expr(ctx, init)?
                };

            // 类型标注一致性检查；标注存在时以标注类型作为绑定类型，
            // 以便统一 init 中残留的 `_`（Infer）占位（如 `Vec::with_capacity` 返回 `Vec<_>`）
            let anno_ty = match type_anno {
                Some(anno) => {
                    let at = resolve_ast_type(ctx, &anno.ty, span)?;
                    // H4 `dyn Trait` 转换：注解为 `dyn Trait`、init 为 `&T`
                    // （T 实现了该 trait）时，把 init 转成 trait 对象胖指针，
                    // 并同步绑定类型，使后续 `at.compatible_with(&ty)` 一致。
                    if let (Type::Dyn(trait_name), Type::Ref(inner, _)) = (&at, &ty) {
                        let inner_ty = (**inner).clone();
                        h_init = coerce_to_dyn(ctx, h_init, inner, trait_name, span)?;
                        ty = at.clone();
                        // H4 去虚拟化：记录绑定变量 → 具体类型
                        // （后续 `dyn_var.method()` 可静态分派到具体类型实现）
                        if let Type::Named(_, _) = &inner_ty {
                            pending_dyn_concrete = Some(inner_ty);
                        }
                    }
                    // P4（2026-08-28）：`&dyn Trait` 上转型——注解为 `&dyn Trait`、
                    // init 为 `&T`（T 实现该 trait）时，把 `&T` 引用上转为胖指针引用
                    // `&dyn Trait`（data 指向引用目标、vtable 指向 T 的实现）。
                    // 与 H4 值上转型同构，仅注解为 `Ref(Dyn)`；胖指针布局相同（2 槽）。
                    else if let (Type::Ref(inner_at, _), Type::Ref(inner_init, _)) = (&at, &ty) {
                        if let Type::Dyn(trait_name) = &**inner_at {
                            if let Type::Named(_, _) = &**inner_init {
                                let concrete = (**inner_init).clone();
                                h_init =
                                    coerce_to_dyn(ctx, h_init, &concrete, trait_name, span)?;
                                ty = at.clone();
                                pending_dyn_concrete = Some(concrete);
                            }
                        }
                    }
                    // U2：联合构造——注解为 `A | B`、init 为其中某成员类型的值时，
                    // desugar 为匿名 enum 构造（槽 0 = tag、槽 1 = payload），
                    // 运行时表示与具名 enum 一致，复用现有 enum codegen 通道。
                    // （成员下标即 tag，由成员在联合中的声明顺序决定）
                    if let Type::Union(us) = &at {
                        if let Some(idx) = us.iter().position(|u| ty.compatible_with(u)) {
                            let member_ty = us[idx].clone();
                            h_init = make_union_ctor(ctx, h_init, idx, &member_ty);
                            ty = at.clone();
                        }
                    }
                    // S2 unsize coercion：let s: &[T] = &arr; —— &[T; N] 经 unsize
                    // 降级为切片胖指针 {data, len}（len = 编译期数组长度），与
                    // check_expr::call 实参位置同构；仅当元素类型兼容时转换。
                    if let (Type::Ref(ia, _), Type::Ref(ib, _)) = (&ty, &at) {
                        if let (Type::Array(_, n), Type::Slice(_)) = (&**ia, &**ib) {
                            if ty.compatible_with(&at) {
                                h_init = make_slice_fat(ctx, h_init, *n as i128);
                                ty = at.clone();
                            }
                        }
                    }
                    if !at.compatible_with(&ty) {
                        return Err(TypeError::WrongType {
                            expected: at.to_string(),
                            found: ty.to_string(),
                            span,
                            related: anno_span
                                .into_iter()
                                .filter(|s| s.line != 0)
                                .map(|s| (s, format!("类型标注 `{at}`")))
                                .collect(),
                        });
                    }
                    Some(at)
                }
                None => None,
            };

            match pattern {
                AstPattern::Ident(name) => {
                    let bind_ty = anno_ty.clone().unwrap_or_else(|| ty.clone());
                    // 非注解延迟闭包绑定：绑定处 push 记录时变量名未知，
                    // 此处填充（首次调用点按变量名查延迟绑定记录）
                    if matches!(&ty, Type::Closure { fn_name, .. } if fn_name.is_empty()) {
                        if let Some(d) = ctx.deferred_closures.last_mut() {
                            d.var_name = name.clone();
                        }
                    }
                    // H4 去虚拟化：dyn 绑定记录具体类型名
                    if let Some(concrete) = pending_dyn_concrete.take() {
                        ctx.insert_dyn_concrete(name.clone(), concrete);
                    }
                    // U1：insert 返回存储槽名（块级遮蔽时 mangle），
                    // Let 绑定名与后续引用解析的槽名保持一致。
                    let stored = ctx.insert_variable(name.clone(), bind_ty);
                    // 记录初始化表达式，供 `String::from(s)` 追踪字面量绑定
                    ctx.insert_local_init(stored.clone(), h_init.clone());
                    Ok((
                        vec![HirStmt::new(HirStmtKind::Let{
                            name: stored.clone(),
                            init: h_init,
                            mutable: *mutable,
                        }, Span::dummy())],
                        ty,
                    ))
                }
                AstPattern::Wildcard => {
                    // `let _ = expr;`：丢弃绑定
                    let _ = &h_init;
                    Ok((
                        vec![HirStmt::new(HirStmtKind::Let{
                            name: "_".to_string(),
                            init: h_init,
                            mutable: *mutable,
                        }, Span::dummy())],
                        ty,
                    ))
                }
                AstPattern::Ref(inner, is_mut) => {
                    // `let ref [mut] x = expr;`：绑定为对 init 的引用（`&T` / `&mut T`）。
                    // 仅支持 `ref` 包裹标识符；复杂内层模式暂不支持。
                    if let AstPattern::Ident(name) = &**inner {
                        let mutability = if *is_mut {
                            Mutability::Mutable
                        } else {
                            Mutability::Immutable
                        };
                        let bind_ty = Type::Ref(Box::new(ty.clone()), mutability);
                        let stored = ctx.insert_variable(name.clone(), bind_ty);
                        // 记录初始化表达式，供 `String::from(s)` 追踪字面量绑定
                        ctx.insert_local_init(stored.clone(), h_init.clone());
                        Ok((
                            vec![HirStmt::new(HirStmtKind::Let{
                                name: stored.clone(),
                                init: HirExpr::new(HirExprKind::Ref{
                                    expr: Box::new(h_init),
                                    is_mut: *is_mut,
                                    pointee: field_scalar_of(&ty),
                                }, Span::dummy()),
                                mutable: *is_mut,
                            }, Span::dummy())],
                            ty,
                        ))
                    } else {
                        Err(TypeError::Unsupported {
                            what: "复杂 ref 绑定模式（仅支持 `let ref x = expr;`）".to_string(),
                            span,
                        })
                    }
                }
                // M2（SH-P0-5，2026-09-02）：元组解构绑定，递归支持嵌套
                // `let (a, b) = e;` / `let (a, _, c) = e;` / `let ((a, b), c) = e;`。
                // desugar 为「临时变量承载元组值 + 各元素按位置取字段后绑定」：
                //   __tup = e;            // init 只求值一次
                //   a = __tup.f0;         // 与 `t.f0` 字段访问同构（FieldGet index）
                //   __tup1 = __tup.f1;    // 嵌套元组：内层再按位置取字段
                //   b = __tup1.f0;
                //   c = __tup.f2;
                // 元素模式支持标识符 / `_` / 嵌套 `Tuple`（其余模式仍报错）。
                AstPattern::Tuple(pats, pat_span) => {
                    let ts = match &ty {
                        Type::Tuple(ts) => Some(ts.clone()),
                        // 引用到元组：剥一层引用后按元组解构（`let (a, b) = &t;`）
                        Type::Ref(inner, _) => match &**inner {
                            Type::Tuple(ts) => Some(ts.clone()),
                            _ => None,
                        },
                        _ => None,
                    };
                    let ts = match ts {
                        Some(ts) => ts,
                        None => {
                            return Err(TypeError::WrongType {
                                expected: format!("元组（{} 元）", pats.len()),
                                found: ty.to_string(),
                                span,
                                related: vec![(
                                    *pat_span,
                                    "元组解构模式声明于此".to_string(),
                                )],
                            })
                        }
                    };
                    if ts.len() != pats.len() {
                        return Err(TypeError::WrongType {
                            expected: format!("{} 元元组", ts.len()),
                            found: format!("{} 元解构模式", pats.len()),
                            span,
                            related: vec![(
                                *pat_span,
                                "元组解构模式声明于此".to_string(),
                            )],
                        });
                    }
                    let tmp = ctx.fresh_temp();
                    let mut out = vec![HirStmt::new(HirStmtKind::Let{
                        name: tmp.clone(),
                        init: h_init,
                        mutable: false,
                    }, Span::dummy())];
                    let base = HirExpr::new(HirExprKind::Variable(tmp), Span::dummy());
                    let nested = lower_tuple_destructure(
                        ctx,
                        base,
                        &ty,
                        pats,
                        *pat_span,
                        *mutable,
                        span,
                    )?;
                    out.extend(nested);
                    Ok((out, ty))
                }
                // 结构体解构绑定 `let Point { x, y } = e;` / `let Point { x, .. } = e;`
                // （含嵌套字段模式 `let Point { p: (a, b) } = e;`）。desugar 为
                // 「临时变量承载结构体值 + 各命名字段按名取字段后绑定」，与 `p.x`
                // 字段访问同构（FieldGet index = 字段在 `def.fields` 中的序）。
                // 实际结构体类型以推断出的 `ty` 为准（模式名不强制校验，兼容模块路径）。
                AstPattern::Struct(_, fields) => {
                    let tmp = ctx.fresh_temp();
                    let mut out = vec![HirStmt::new(HirStmtKind::Let{
                        name: tmp.clone(),
                        init: h_init,
                        mutable: false,
                    }, Span::dummy())];
                    let base = HirExpr::new(HirExprKind::Variable(tmp), Span::dummy());
                    let nested = lower_struct_destructure(
                        ctx,
                        base,
                        &ty,
                        fields,
                        *mutable,
                        span,
                    )?;
                    out.extend(nested);
                    Ok((out, ty))
                }
                // 枚举解构绑定 `let Some(x) = e;`（含 `lib::Opt::Some(x)` 路径形式）。
                // 临时变量承载枚举值后按变体字段槽展开（与 lower_enum_destructure 同构）。
                AstPattern::Enum(variant, sub_pats) => {
                    let tmp = ctx.fresh_temp();
                    let mut out = vec![HirStmt::new(
                        HirStmtKind::Let {
                            name: tmp.clone(),
                            init: h_init,
                            mutable: false,
                        },
                        Span::dummy(),
                    )];
                    let base = HirExpr::new(HirExprKind::Variable(tmp), Span::dummy());
                    let nested = lower_enum_destructure(
                        ctx,
                        base,
                        &ty,
                        variant,
                        sub_pats,
                        *mutable,
                        span,
                    )?;
                    out.extend(nested);
                    Ok((out, ty))
                }
                AstPattern::EnumPath(segments, sub_pats) => {
                    let variant = segments.last().cloned().ok_or_else(|| {
                        TypeError::Unsupported {
                            what: "空路径枚举模式".to_string(),
                            span,
                        }
                    })?;
                    let tmp = ctx.fresh_temp();
                    let mut out = vec![HirStmt::new(
                        HirStmtKind::Let {
                            name: tmp.clone(),
                            init: h_init,
                            mutable: false,
                        },
                        Span::dummy(),
                    )];
                    let base = HirExpr::new(HirExprKind::Variable(tmp), Span::dummy());
                    let nested = lower_enum_destructure(
                        ctx,
                        base,
                        &ty,
                        &variant,
                        sub_pats,
                        *mutable,
                        span,
                    )?;
                    out.extend(nested);
                    Ok((out, ty))
                }
                _ => Err(TypeError::Unsupported {
                    what: "复杂 let 绑定模式（元组 / 结构体等）".to_string(),
                    span,
                }),
            }
        }
        AstStmt::Expr(e) => {
            let (hir, ty) = infer_expr(ctx, e)?;
            Ok((vec![HirStmt::new(HirStmtKind::Expr(hir), Span::dummy())], ty))
        }
        AstStmt::Semi(e) => {
            let (hir, _) = infer_expr(ctx, e)?;
            Ok((vec![HirStmt::new(HirStmtKind::Semi(hir), Span::dummy())], Type::Unit))
        }
        AstStmt::Item(item) => {
            // 语句级嵌套项（如函数体内的局部 fn）：检查但不在顶层生成 HIR
            let mut scratch = Vec::new();
            crate::check_item::check_item(ctx, item, "", &mut scratch)?;
            Ok((vec![HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::Unit, Span::dummy())), Span::dummy())], Type::Unit))
        }
    }
}

/// 取语句的源坐标：`AstStmt` 本身不携带 `span`，由其内含的 `AstExpr` 推断。
fn ast_stmt_span(stmt: &AstStmt) -> Span {
    match stmt {
        AstStmt::Let { init, .. } => init.span,
        AstStmt::Expr(e) => e.span,
        AstStmt::Semi(e) => e.span,
        AstStmt::Item(_) => Span::dummy(),
    }
}

/// 检查语句（对外入口）。
///
/// 在 [`check_stmt_inner`] 基础上，将生成的全部 `HirStmt` 的 `span` 设为源语句
/// 坐标（由内含 `AstExpr` 推断），实现 Span 全量传播，供下游 borrowck /
/// regionck 给出语句级精确错误坐标。
pub(crate) fn check_stmt(
    ctx: &mut TypeContext,
    stmt: &AstStmt,
) -> Result<(Vec<HirStmt>, Type), TypeError> {
    let s = ast_stmt_span(stmt);
    let (mut hirs, ty) = check_stmt_inner(ctx, stmt)?;
    for h in &mut hirs {
        h.span = s;
    }
    Ok((hirs, ty))
}
