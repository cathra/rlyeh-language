//! 表达式检查子模块：方法调用与动态分派。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use super::*;

// 跨线程闭包检查、`dyn Trait` 去虚拟化、接收者内建方法特判已按簇下沉到子模块
// （文件大小约束：单个文件 ≤1000 行）。
mod builtin;
mod dyn_call;
mod thread;

use dyn_call::*;
use thread::*;

pub(super) fn check_static_method_call(
    ctx: &mut TypeContext,
    ty_name: &str,
    method: &str,
    args: &[AstExpr],
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    // W6：闭包值跨线程捕获——`Thread::start(f, arg)`（f 为带参闭包值对象）。
    // 展开为：生成线程入口 thunk + 线程输入对象（捕获槽值 + arg），调用
    // std `thread::__start_with_input(thunk, input)`（Result 构造复用 std 语言层）。
    // 名等价（精确或短名相同）：std 拆分后 `Thread` 注册为 `thread::handle::Thread`，
    // 此处仍以 `thread::Thread` 查询。
    if crate::context::names_match(ty_name, "thread::Thread") && method == "start" {
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
    // P7b-2（2026-08-29）：turbofish 类型实参优先——`Bag::<i64>::new()` 给出 `T = i64`
    // （静态方法无参可推断时，turbofish 是唯一类型实参来源；`Vec::<i64>::new()` 等）。
    if !type_args.is_empty() && !impl_def.type_params.is_empty() {
        for (tp, ta) in impl_def.type_params.iter().zip(type_args) {
            let ta_ty = resolve_ast_type(ctx, ta, span)?;
            subst.insert(tp.clone(), ta_ty);
        }
    }
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
                related: vec![],
            });
        }
        hir_args.push(hir);
    }
    Ok((
        HirExpr::new(HirExprKind::Call{
            callee: fn_name,
            args: hir_args,
        }, Span::dummy()),
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
    depth: usize,
) -> Result<(HirExpr, Type), TypeError> {
    // `r#` 前缀（关键字转义，如 `fn r#send`）在方法调用处去前缀，
    // 与定义侧 parse_fn 归一化（`r#send` → `send`）保持一致。
    let method = method.strip_prefix("r#").unwrap_or(method);
    let (mut recv_hir, mut recv_ty) = infer_expr(ctx, receiver)?;
    // 接收者内建方法特判（切片 / Vec 切片视图 / `&str` 升级 / `push_str` 字面量 /
    // 引用计数 / `dyn Trait` 虚调用）——**必须早于常规 impl 分派**。
    // 实现见 `method/builtin`（文件大小约束：单个文件 ≤1000 行）。
    match builtin::try_builtin_method_call(ctx, recv_hir, recv_ty, method, args, span)? {
        builtin::BuiltinOutcome::Handled(hir, ty) => return Ok((hir, ty)),
        builtin::BuiltinOutcome::NotHandled(hir, ty) => {
            recv_hir = hir;
            recv_ty = ty;
        }
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
            HirStmt::new(HirStmtKind::Let{
                name: data_tmp.clone(),
                init: HirExpr::new(HirExprKind::FieldGet{
                    base: Box::new(recv_hir.clone()),
                    index: 0,
                    ty: FieldScalar::Ptr,
                }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: len_tmp.clone(),
                init: HirExpr::new(HirExprKind::FieldGet{
                    base: Box::new(recv_hir),
                    index: 1,
                    ty: FieldScalar::Int,
                }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: sf.clone(),
                init: HirExpr::new(HirExprKind::Alloc{
                    slots: 2,
                    by_value: true,
                    is_strfat: true,
                }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(sf.clone()), Span::dummy())),
                index: 0,
                value: Box::new(HirExpr::new(HirExprKind::Variable(data_tmp), Span::dummy())),
                ty: FieldScalar::Ptr,
            }, Span::dummy())), Span::dummy()),
            HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(sf.clone()), Span::dummy())),
                index: 1,
                value: Box::new(HirExpr::new(HirExprKind::Variable(len_tmp), Span::dummy())),
                ty: FieldScalar::Int,
            }, Span::dummy())), Span::dummy()),
        ];
        return Ok((
            HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                stmts,
                final_expr: Some(HirExpr::new(HirExprKind::Variable(sf), Span::dummy())),
            })), Span::dummy()),
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
                related: vec![],
            });
        }
        let data_tmp = ctx.fresh_temp();
        let start_ptr = ctx.fresh_temp();
        let len_tmp = ctx.fresh_temp();
        let sf = ctx.fresh_temp();
        let stmts = vec![
            HirStmt::new(HirStmtKind::Let{
                name: data_tmp.clone(),
                init: HirExpr::new(HirExprKind::FieldGet{
                    base: Box::new(recv_hir),
                    index: 0,
                    ty: FieldScalar::Ptr,
                }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: start_ptr.clone(),
                init: HirExpr::new(HirExprKind::PtrAdd{
                    base: Box::new(HirExpr::new(HirExprKind::Variable(data_tmp), Span::dummy())),
                    offset: Box::new(start_hir.clone()),
                    elem: FieldScalar::Str,
                }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: len_tmp.clone(),
                init: HirExpr::new(HirExprKind::Binary(
                    HirBinaryOp::Sub,
                    Box::new(end_hir),
                    Box::new(start_hir),
                ), Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Let{
                name: sf.clone(),
                init: HirExpr::new(HirExprKind::Alloc{
                    slots: 2,
                    by_value: true,
                    is_strfat: true,
                }, Span::dummy()),
                mutable: false,
            }, Span::dummy()),
            HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(sf.clone()), Span::dummy())),
                index: 0,
                value: Box::new(HirExpr::new(HirExprKind::Variable(start_ptr), Span::dummy())),
                ty: FieldScalar::Ptr,
            }, Span::dummy())), Span::dummy()),
            HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
                base: Box::new(HirExpr::new(HirExprKind::Variable(sf.clone()), Span::dummy())),
                index: 1,
                value: Box::new(HirExpr::new(HirExprKind::Variable(len_tmp), Span::dummy())),
                ty: FieldScalar::Int,
            }, Span::dummy())), Span::dummy()),
        ];
        return Ok((
            HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
                stmts,
                final_expr: Some(HirExpr::new(HirExprKind::Variable(sf), Span::dummy())),
            })), Span::dummy()),
            Type::Ref(Box::new(Type::Str), Mutability::Immutable),
        ));
    }
    let mut self_ty = peel_refs_and_heap(&recv_ty);
    // `&str` 接收者：方法按 String impl 解析（MVP 中 `&str` 是 String 对象的
    // 只读借用视图，String 的方法视图（len / substring / push_str 等）均可用）
    if matches!(self_ty, Type::Str) && matches!(&recv_ty, Type::Ref(_, _)) {
        self_ty = Type::Named("String".to_string(), vec![]);
    }
    // 规范化接收者类型名（别名 → 规范符号名）：标准库子模块拆分后，接收者类型
    // 常经 `pub import` 以别名出现（如 `sync::RwLockWriteGuard`），而 impl 的
    // `self_type` 注册为规范名（`sync::rwlock::RwLockWriteGuard`）。不规范化则
    // 下方 `unify(cand.self_type, self_ty)` 无法绑定泛型 `T`，方法体 / 返回类型
    // 中的 `T` 会泄漏为未定义类型。
    self_ty = ctx.canonical_type(&self_ty);
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
            let mut call_args = vec![recv_hir, HirExpr::new(HirExprKind::IntLiteral(kind as i128), Span::dummy())];
            for arg in args {
                let (h, t) = infer_expr(ctx, arg)?;
                if !t.compatible_with(&Type::I64) {
                    return Err(TypeError::ArgumentTypeMismatch {
                        name: format!("{self_ty}::{method}"),
                        index: call_args.len() - 2,
                        expected: "i64".to_string(),
                        found: t.to_string(),
                        span: arg.span,
                        related: vec![],
                    });
                }
                call_args.push(h);
            }
            while call_args.len() < 5 {
                call_args.push(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy()));
            }
            return Ok((
                HirExpr::new(HirExprKind::Call{
                    callee: "rlyeh_actor_ask".to_string(),
                    args: call_args,
                }, Span::dummy()),
                Type::I64,
            ));
        }
    }

    // 查找含该方法的 impl 块候选（inherent 优先，trait 次之）。
    // V3（2026-08-26）：类型匹配的 trait impl 且 trait 声明了该方法默认实现时，
    // 回退到 `find_trait_default_impl`——类型匹配的 trait impl 且 trait 声明了
    // 该方法的默认实现（`impl Trait for X {}` 未显式实现该方法）。
    // X4：`trait_hint`（如 `fmt::Display` / `fmt::Debug`）时按 trait 名区分——
    // 同名 trait 方法（Display::fmt 与 Debug::fmt）经此精确分派。
    // A2（SH-P1-1，2026-09-02）：同一 `self_type` 上同一泛型 trait 的**多 impl**
    // （`impl Wrap<i64> for W` 与 `impl Wrap<bool> for W`）此前按首匹配选取，
    // 无法按 trait 类型实参 / 实参类型区分。现收集全部候选，按「代入 trait 类型
    // 实参后的方法签名与实参类型兼容」选取首个匹配者。
    let candidates: Vec<ImplDef> = if let Some(tn) = trait_hint {
        ctx.find_trait_method_candidates(&self_ty, tn, method)
    } else {
        ctx.find_impl_candidates(&self_ty, method)
    };

    // 选取首个「签名与实参兼容」的候选。
    // 每候选：先代入 trait 类型实参、再 unify 接收者类型、再由实参反推 impl /
    // 方法级未定泛型，最后校验各实参类型与（代入后的）预期参数兼容。
    let mut selected: Option<(ImplDef, crate::types::ImplMethod, HashMap<String, Type>)> = None;
    for cand in &candidates {
        let Some(mdef) = cand
            .methods
            .iter()
            .find(|m| m.sig.name == method)
            .cloned()
            // V3 回退：impl 未实现该方法但 trait 有默认实现 → 构造 ImplMethod
            // （签名取 trait 方法签名，body 取 trait 默认实现 AST）。
            .or_else(|| trait_default_method(ctx, cand, method))
        else {
            continue;
        };
        // 实参类型（仅取类型用于兼容性校验；最终实参 HIR 仍由下方实参循环重建）。
        // 推论失败（罕见：实参是脱离上下文无法定型的裸闭包）则跳过该候选，
        // 不阻塞调用——回退路径会沿用既有逐实参检查。
        let Ok(arg_types) = args
            .iter()
            .map(|a| infer_expr(ctx, a).map(|(_, t)| t))
            .collect::<Result<Vec<Type>, _>>()
        else {
            continue;
        };
        let mut subst = HashMap::new();
        // A2：trait 类型实参代入方法签名（先于 self_type unify，避免同名泛型被
        // 覆盖）。`impl Wrap<bool> for W` 的 `wrap(&self, v: T)` 经此变为
        // `wrap(&self, v: bool)`，使多 impl 按实参区分；`impl<T> Wrap<T> for W`
        // 的 `T` 替换为 impl 泛型参数（同名），留待下方由实参反推。
        if let Some(tn) = &cand.trait_name {
            if let Some(td) = ctx.trait_defs.get(tn) {
                for (pn, ta) in td.type_params.iter().zip(&cand.trait_type_args) {
                    subst.insert(pn.clone(), ta.clone());
                }
            }
        }
        unify(&cand.self_type, &self_ty, &mut subst)?;
        // impl / 方法级泛型参数由对应实参反推（A2：`impl<T> Wrap<T> for W` 的
        // T 此前仅能从接收者类型推导，`self_type` 非泛型时无来源 → undefined type T）。
        let mut generics = cand.type_params.clone();
        if let Some(b) = &mdef.body {
            for g in &b.generics {
                if !generics.contains(&g.name) {
                    generics.push(g.name.clone());
                }
            }
        }
        let expected: Vec<Type> = mdef
            .sig
            .params
            .iter()
            .skip(1)
            .map(|p| substitute(p, &subst))
            .collect();
        for (pty, aty) in expected.iter().zip(&arg_types) {
            // 形参为 fn 类型（实参可能是无注解闭包）时跳过兼容性判定，
            // 交由下方实参循环按预期签名检查，避免误拒合法闭包实参。
            if !matches!(pty, Type::Fn(_))
                && crate::check_expr::generic::contains_generic_named(pty, &generics)
            {
                let _ = unify(pty, aty, &mut subst);
            }
        }
        let expected: Vec<Type> = mdef
            .sig
            .params
            .iter()
            .skip(1)
            .map(|p| substitute(p, &subst))
            .collect();
        let compatible = arg_types.len() == expected.len()
            && arg_types
                .iter()
                .zip(&expected)
                .all(|(aty, pty)| aty.compatible_with(pty));
        if compatible {
            selected = Some((cand.clone(), mdef, subst));
            break;
        }
    }

    let (impl_def, method_def, mut subst) = match selected {
        Some(s) => s,
        None => {
            // M2（SH-P1-4）：自动解引用强制——无任何方法候选时，若接收者类型
            // 实现了 `deref`（Deref trait 或内建智能指针 deref），对接收者插入
            // `*(recv.deref())` 递归重试解析（限深度，避免无限）。零新增 IR 节点。
            if depth < MAX_DEREF_DEPTH && ctx.find_impl_for_method(&self_ty, "deref").is_some() {
                let deref_ast = make_deref_receiver(receiver, span);
                return check_method_call(ctx, &deref_ast, method, args, trait_hint, span, depth + 1);
            }
            // 无兼容候选：回退到首个候选（V3 默认 impl / 首匹配），由下方兼容
            // 性检查产出清晰的类型不匹配诊断。
            let fallback = candidates
                .into_iter()
                .next()
                .or_else(|| {
                    if trait_hint.is_none() {
                        ctx.find_trait_default_impl(&self_ty, method).cloned()
                    } else {
                        None
                    }
                })
                .ok_or_else(|| TypeError::FunctionNotFound {
                    name: format!("{self_ty}::{method}"),
                    span,
                })?;
            let mdef = fallback
                .methods
                .iter()
                .find(|m| m.sig.name == method)
                .cloned()
                .or_else(|| trait_default_method(ctx, &fallback, method))
                .ok_or_else(|| TypeError::FunctionNotFound {
                    name: format!("{self_ty}::{method}"),
                    span,
                })?;
            let mut subst = HashMap::new();
            if let Some(tn) = &fallback.trait_name {
                if let Some(td) = ctx.trait_defs.get(tn) {
                    for (pn, ta) in td.type_params.iter().zip(&fallback.trait_type_args) {
                        subst.insert(pn.clone(), ta.clone());
                    }
                }
            }
            unify(&fallback.self_type, &self_ty, &mut subst)?;
            (fallback, mdef, subst)
        }
    };

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
    let expected: Vec<Type> = method_def
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
    // A4（SH-P1-1，2026-09-02）：**impl 块级泛型约束强制校验**。
    // 此前 impl 的 `where` / 内联 bound 只被解析与记录（`ImplDef.bounds`），
    // 实例化方法时并不校验——违反约束时不在调用点报错，而是直接实例化方法体、
    // 在体内部报出误导性的「`i64::speak` not found」。函数级 bound 早已有
    // `check_generic_bounds` 校验，此处复用同一套诊断补齐 impl 级。
    crate::check_expr::generic::check_generic_bounds(ctx, &impl_def.bounds, &subst, span)?;
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
                related: vec![],
            });
        }
    }
    Ok((
        HirExpr::new(HirExprKind::Call{
            callee: fn_name,
            args: hir_args,
        }, Span::dummy()),
        ret_ty,
    ))
}

