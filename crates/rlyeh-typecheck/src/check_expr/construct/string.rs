//! construct/string：`String` 构造（`String::from`）与 `str` 实参升级。
//! （由 construct.rs 拆分而来，保持语义等价）

use super::*;

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
    // 运行期读 data/len 槽 → 独立 alloc_bytes(len+1) 数据缓冲 + 独立 3 槽 String 对象。
    // 注意：数据缓冲与 String 对象必须**两个独立分配**——若共用 base，copy_bytes 写入的
    // 字符串字节会被随后的 FieldSet 对象头（前 24 字节）覆盖，破坏数据（V2-D 修复）。
    if let Type::Ref(inner, _) = &s_ty {
        if matches!(**inner, Type::Str) {
            let len_tmp = ctx.fresh_temp();
            let data_src = ctx.fresh_temp();
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
                    name: data_src.clone(),
                    init: HirExpr::FieldGet {
                        base: Box::new(s_hir),
                        index: 0,
                        ty: FieldScalar::Ptr,
                    },
                    mutable: false,
                },
                // 独立数据缓冲（len+1 字节，含 NUL）
                HirStmt::Let {
                    name: data_tmp.clone(),
                    init: HirExpr::Call {
                        callee: "alloc_bytes".to_string(),
                        args: vec![len_plus1(len_tmp.clone())],
                    },
                    mutable: false,
                },
                HirStmt::Semi(HirExpr::Call {
                    callee: "copy_bytes".to_string(),
                    args: vec![
                        HirExpr::Variable(data_tmp.clone()),
                        HirExpr::Variable(data_src),
                        len_plus1(len_tmp.clone()),
                    ],
                }),
                // 独立 String 对象（3 槽：data/len/cap）
                HirStmt::Let {
                    name: base.clone(),
                    init: HirExpr::Alloc {
                        slots: 3,
                        by_value: false,
                        is_strfat: false,
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
        HirExpr::Variable(name) => {
            // P9b（2026-08-29）：多层直链追踪——`let a = "x"; let b = a; String::from(b)`
            // （此前仅查一层 `lookup_local_init`，`b` 的 init 是变量 `a` 时失败）。
            // 深度上限 8 防自引用/长链开销；fn 边界由 lookup_local_init 天然不穿透。
            let mut cur = ctx.lookup_local_init(name).cloned();
            let mut depth = 0;
            loop {
                match cur {
                    Some(HirExpr::StringLiteral(s)) => break Some(s),
                    Some(HirExpr::Variable(n)) if depth < 8 => {
                        cur = ctx.lookup_local_init(&n).cloned();
                        depth += 1;
                    }
                    _ => break None,
                }
            }
        }
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
            is_strfat: false,
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

pub(crate) fn upgrade_str_arg(
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
