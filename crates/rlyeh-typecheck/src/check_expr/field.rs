//! 表达式检查子模块：字段访问与切片。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use super::*;

pub(super) fn check_field_access(
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

pub(super) fn check_slice(
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
            is_strfat: false,
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
