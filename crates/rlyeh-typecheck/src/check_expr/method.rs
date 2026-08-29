//! 表达式检查子模块：方法调用与动态分派。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use super::*;

pub(super) fn check_thread_start_closure(
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
            is_strfat: false,
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

pub(super) fn emit_thread_entry(
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

pub(super) fn check_static_method_call(
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

    // 泛型 impl 的静态方法：Y4b-2（2026-08-28）从实参推断类型参数（替代 MVP
    // unsupported）。`MyMutex::new(42)` → 参数 `v: T` 与实参 `i64` 填充 `T = i64`；
    // 支持裸 `Generic(tp)` 参数的直接推断；复合参数（`Vec<T>` 等）统一推断待扩展。
    let mut subst: HashMap<String, Type> = HashMap::new();
    if !impl_def.type_params.is_empty() {
        for (pty, arg) in method_def.sig.params.iter().zip(args) {
            if let Type::Generic(tp) = pty {
                if impl_def.type_params.iter().any(|p| p == tp) && !subst.contains_key(tp) {
                    let (_, arg_ty) = infer_expr(ctx, arg)?;
                    subst.insert(tp.clone(), arg_ty);
                }
            }
        }
    }

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

pub(super) fn check_method_call(
    ctx: &mut TypeContext,
    receiver: &AstExpr,
    method: &str,
    args: &[AstExpr],
    trait_hint: Option<&str>,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // `r#` 前缀（关键字转义，如 `fn r#send`）在方法调用处去前缀，
    // 与定义侧 parse_fn 归一化（`r#send` → `send`）保持一致。
    let method = method.strip_prefix("r#").unwrap_or(method);
    let (mut recv_hir, mut recv_ty) = infer_expr(ctx, receiver)?;
    // V2-B：`&str`（StrFat）接收者调 String 方法时，先深拷贝为临时 String 对象。
    // StrFat 是 by_value `{data,len}`，而 String 方法 self 期望 String 对象指针（Ptr）；
    // 直接传 StrFat 会污染 LIR param_types 传播（同一方法被 String receiver 调用时
    // 也误标 StrFat，破坏 `x.trim()`）。深拷贝后 self 统一为 String，无类型冲突。
    // （V2-B 链式 `x.trim().trim()` / `s.trim().to_upper()`）
    if comparison::is_str_view(&recv_ty) {
        let tmp = ctx.fresh_temp();
        let data_h = HirExpr::FieldGet {
            base: Box::new(recv_hir.clone()),
            index: 0,
            ty: FieldScalar::Ptr,
        };
        let len_h = HirExpr::FieldGet {
            base: Box::new(recv_hir),
            index: 1,
            ty: FieldScalar::Int,
        };
        let conv = make_strfat_to_string(ctx, data_h, len_h);
        recv_hir = HirExpr::Block(Box::new(HirBlock {
            stmts: vec![HirStmt::Let {
                name: tmp.clone(),
                init: conv,
                mutable: false,
            }],
            final_expr: Some(HirExpr::Variable(tmp)),
        }));
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
        if let HirExpr::Variable(var) = &recv_hir {
            if let Some(devirt) = devirtualize_dyn_call(
                ctx,
                var,
                trait_name.as_str(),
                method,
                recv_hir.clone(),
                args,
                span,
            )? {
                return Ok(devirt);
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
                    is_strfat: true,
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
    // receiver 可为 `&String`（std 方法 `as_str_range(&self)` 内 `self` 是 `&String`），
    // 也可为 `&str`（V2-B 链式 `x.trim().trim()`：对 StrFat 子区间再裁剪，data 槽即 StrFat
    // 的 data 指针，FieldGet(recv,0) 读之）。
    let recv_core = peel_refs_and_heap(&recv_ty);
    if method == "as_str_range"
        && args.len() == 2
        && (comparison::is_string_type(ctx, &recv_core)
            || comparison::is_str_view(&recv_ty)
            || comparison::is_str_value(&recv_core))
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
                    is_strfat: true,
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

    // 查找含该方法的 impl 块（inherent 优先，trait 次之）。
    // V3（2026-08-26）：`find_impl_for_method` 找不到"实现了该方法的 impl"时，
    // 回退到 `find_trait_default_impl`——类型匹配的 trait impl 且 trait 声明了
    // 该方法的默认实现（`impl Trait for X {}` 未显式实现该方法）。
    // X4：`trait_hint`（如 `fmt::Display` / `fmt::Debug`）时按 trait 名区分——
    // 同名 trait 方法（Display::fmt 与 Debug::fmt）经此精确分派。
    let impl_def = if let Some(tn) = trait_hint {
        ctx.find_impl_for_trait_method(&self_ty, tn, method)
            .cloned()
    } else {
        ctx.find_impl_for_method(&self_ty, method)
            .cloned()
            .or_else(|| ctx.find_trait_default_impl(&self_ty, method).cloned())
    }
    .ok_or_else(|| TypeError::FunctionNotFound {
        name: format!("{self_ty}::{method}"),
        span,
    })?;
    let method_def = impl_def
        .methods
        .iter()
        .find(|m| m.sig.name == method)
        .cloned()
        // V3 回退：impl 未实现该方法但 trait 有默认实现 → 构造 ImplMethod
        // （签名取 trait 方法签名，body 取 trait 默认实现 AST）。
        .or_else(|| trait_default_method(ctx, &impl_def, method))
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
    // V3-B（2026-08-27）：参数类型中的 `Self`（如 `chain(self, other: Self)`）同样
    // 替换为 impl 目标具体类型——否则默认方法参数 `Self` 占位无法与实参匹配。
    let impl_self_ty = substitute(&impl_def.self_type, &subst);
    expected = method_def
        .sig
        .params
        .iter()
        .skip(1)
        .map(|p| replace_type_self(&substitute(p, &subst), &impl_self_ty))
        .collect();
    // V3-D（2026-08-27）：返回类型中的 `Self` 替换为 impl 目标具体类型。
    // 默认方法返回 `Take2<Self>` 时，`Self`（trait 实现类型 = impl_def.self_type）
    // 须替换为具体类型，否则 `Take2<Self>` 的 `next` 内 `Self::next` 无法解析。
    let mut ret_ty = substitute(&method_def.sig.return_type, &subst);
    ret_ty = replace_type_self(&ret_ty, &impl_self_ty);

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

pub(super) fn make_strfat_to_string(ctx: &mut TypeContext, data_h: HirExpr, len_h: HirExpr) -> HirExpr {
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
            init: len_h,
            mutable: false,
        },
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
                data_h,
                len_plus1(len_tmp.clone()),
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
    HirExpr::Block(Box::new(HirBlock {
        stmts,
        final_expr: Some(HirExpr::Variable(base)),
    }))
}

pub(super) fn devirtualize_dyn_call(
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

/// V3-D（2026-08-27）：递归替换类型中的 `Self`（`Type::Generic("Self")`）为
/// 具体类型 `concrete`。用于 trait 默认方法返回 `Take2<Self>` 等含 `Self`
/// 的签名实例化——`Self` 表示 impl 目标类型，须替换后方法体/后续调用才能解析。
pub(super) fn replace_type_self(ty: &Type, concrete: &Type) -> Type {
    use Type::*;
    match ty {
        Generic(n) if n == "Self" => concrete.clone(),
        // 含子类型需递归的变体
        Named(name, args) => Named(
            name.clone(),
            args.iter().map(|a| replace_type_self(a, concrete)).collect(),
        ),
        Ref(inner, m) => Ref(Box::new(replace_type_self(inner, concrete)), *m),
        RawPtr(inner, m) => RawPtr(Box::new(replace_type_self(inner, concrete)), *m),
        Tuple(items) => Tuple(items.iter().map(|i| replace_type_self(i, concrete)).collect()),
        Array(inner, size) => Array(Box::new(replace_type_self(inner, concrete)), *size),
        Fn(sig) => {
            let params = sig
                .params
                .iter()
                .map(|p| replace_type_self(p, concrete))
                .collect();
            let ret = replace_type_self(&sig.return_type, concrete);
            Fn(Box::new(crate::types::FnSignature {
                params,
                return_type: ret,
            }))
        }
        Closure {
            captures,
            params,
            ret,
            fn_name,
        } => Closure {
            captures: captures.iter().map(|c| replace_type_self(c, concrete)).collect(),
            params: params.iter().map(|p| replace_type_self(p, concrete)).collect(),
            ret: Box::new(replace_type_self(ret, concrete)),
            fn_name: fn_name.clone(),
        },
        AssocProjection { base, assoc } => AssocProjection {
            base: Box::new(replace_type_self(base, concrete)),
            assoc: assoc.clone(),
        },
        // 其余变体（标量 / 不可递归 / 非 Self 泛型占位）原样返回
        other => other.clone(),
    }
}
