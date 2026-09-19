//! 表达式检查子模块：fn_sig。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;
// P4c：`&T → &dyn Trait` 返回上转型（coerce_to_dyn 在 check_expr）
use crate::check_expr::coerce_to_dyn;
use rlyeh_hir::HirExprKind;

/// 收集全部顶层函数签名（用于跨模块解析与检查前声明）。
pub fn collect_fn_signatures(
    program: &AstProgram,
) -> Result<Vec<(String, FnSignature)>, TypeError> {
    // S1c：async/await 状态机 desugar——接口签名须反映 desugar 后的真实签名
    // （async fn 实际签名返回 `__Fut_X`，且用户类型可能引用生成项）。
    // 与 typecheck_source 挂载点保持一致，clone 后处理，不修改调用方持有的 AST。
    let mut program = program.clone();
    rlyeh_desugar::desugar_program(&mut program).map_err(|e| TypeError::Unsupported {
        what: e.to_string(),
        span: e.span(),
    })?;
    let program = &program;
    let mut ctx = TypeContext::new();
    // 先注册全部 use 导入别名，再收集结构体 / 枚举 / trait / impl / 模块。
    // 关键：若某 `module X;` 声明排在 `pub import X::Y;` 之前，子模块收集时别名
    // 尚未注册，其内 `sync::Mutex` 类全限定引用会退化为未规范化的别名串
    // （sync 模块拆分回归：Mutex::new 返回类型存成别名 `sync::Mutex` 而非
    // `sync::mutex::Mutex`，导致与字段类型 `sync::mutex::Mutex` 不匹配）。
    for item in &program.items {
        if let AstItem::UseDecl(u) = item {
            register_use(&mut ctx, u, "")?;
        }
    }
    for item in &program.items {
        match item {
            AstItem::StructDecl(s) => collect_struct(&mut ctx, s, "")?,
            AstItem::EnumDecl(e) => collect_enum(&mut ctx, e, "")?,
            AstItem::TraitDecl(t) => collect_trait(&mut ctx, t, "")?,
            AstItem::ImplBlock(imp) => collect_impl(&mut ctx, imp, "")?,
            AstItem::ModDecl(m) => collect_mod_types(&mut ctx, m)?,
            _ => {}
        }
    }
    let mut sigs = Vec::new();
    for item in &program.items {
        match item {
            AstItem::FnDecl(f) => {
                let sig = fn_signature(&mut ctx, f, f.span)?;
                sigs.push((f.name.clone(), sig));
            }
            AstItem::ModDecl(m) => collect_mod_fn_sigs(&mut ctx, m, &m.name, &mut sigs)?,
            _ => {}
        }
    }
    sigs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sigs)
}

pub(crate) fn fn_signature(ctx: &mut TypeContext, f: &AstFnDecl, span: Span) -> Result<FnSignature, TypeError> {
    fn_signature_with_self(ctx, f, None, span)
}

pub(crate) fn fn_signature_with_self(
    ctx: &mut TypeContext,
    f: &AstFnDecl,
    self_ty: Option<&Type>,
    span: Span,
) -> Result<FnSignature, TypeError> {
    let saved_params = std::mem::take(&mut ctx.type_params);
    ctx.type_params = f.generics.iter().map(|p| p.name.clone()).collect();

    let mut params = Vec::with_capacity(f.params.len());
    for p in &f.params {
        if p.name == "self" {
            if let Some(st) = self_ty {
                params.push(st.clone());
                continue;
            }
        }
        params.push(resolve_ast_type(ctx, &p.type_, span)?);
    }
    let return_type = match &f.return_type {
        Some(t) => resolve_ast_type(ctx, t, span)?,
        None => Type::Unit,
    };
    // H4 MVP 限制：`dyn Trait` 为 2 槽胖指针，暂不支持作为函数/方法参数与返回值
    // （LIR 参数/返回为标量槽，无法表达胖指针；局部变量 + vtable 调用为主路径）。
    for p in &params {
        if matches!(p, Type::Dyn(_)) {
            return Err(TypeError::Unsupported {
                what: format!(
                    "`{}` 作为函数/方法参数（H4 MVP 仅支持 `let d: dyn Trait = &obj;` 局部变量）",
                    p
                ),
                span,
            });
        }
    }
    if matches!(&return_type, Type::Dyn(_)) {
        return Err(TypeError::Unsupported {
            what: format!(
                "`{return_type}` 作为函数/方法返回类型（H4 MVP 仅支持局部变量绑定）"
            ),
            span,
        });
    }

    ctx.type_params = saved_params;
    Ok(FnSignature {
        params,
        param_spans: f.params.iter().map(|p| p.span).collect(),
        return_type,
    })
}

pub(crate) fn check_fn_body(
    ctx: &mut TypeContext,
    f: &AstFnDecl,
) -> Result<Option<rlyeh_hir::HirBlock>, TypeError> {
    check_fn_body_with_self(ctx, f, None)
}

pub(crate) fn check_fn_body_with_self(
    ctx: &mut TypeContext,
    f: &AstFnDecl,
    self_ty: Option<&Type>,
) -> Result<Option<rlyeh_hir::HirBlock>, TypeError> {
    let body = match &f.body {
        Some(b) => b,
        None => {
            // trait 抽象方法 / extern 声明允许无函数体
            if f.is_pub || f.is_extern {
                return Ok(None);
            }
            return Err(TypeError::MissingFunctionBody {
                name: f.name.clone(),
                span: f.span,
            });
        }
    };

    // 参数进入局部作用域：函数边界（隔离，防止本函数体局部变量名污染
    // 调用方——尤其 std 方法体同名变量会覆盖调用方 `local_inits` 的字面量
    // 绑定追踪，导致 `String::from(s)` / Str 值升级查不到绑定内容）
    ctx.push_scope(true);
    for p in &f.params {
        let ty = if p.name == "self" && self_ty.is_some() {
            self_ty.cloned().unwrap()
        } else {
            resolve_ast_type(ctx, &p.type_, f.span)?
        };
        ctx.insert_variable(p.name.clone(), ty);
    }

    // 返回类型检查（签名在收集阶段已存入，此处重新解析以保持一致性）
    let return_type = fn_signature_with_self(ctx, f, self_ty, f.span)?.return_type;

    // P6c：`?` 运算符 From 自动转换需知函数返回类型（目标错误类型），
    // 在函数体检查期间写入，退出时恢复（避免嵌套 fn/闭包互相污染）。
    let saved_return_type = std::mem::replace(
        &mut ctx.current_return_type,
        Some(return_type.clone()),
    );

    // `fn make() -> fn(i64) -> i64 { |x| x + 1 }`：返回类型为 fn 且函数体
    // 尾表达式为闭包时，按 H2 无捕获闭包签名检查（H5 补全，返回闭包的函数）。
    // U1：函数体不额外开块作用域（`check_block_inner`），体内部 let 直接留在
    // 函数 fn 层——尾表达式处理（延迟闭包固化、返回闭包签名检查等）仍需
    // 访问体内变量（如 `let base = 10; let f = |x| x + base; f` 的 base）。
    // 函数结束时 fn 层随 `pop_scope` 整体弹出（隔离调用方环境）。
    let (mut hir_body, body_ty) = if matches!(&return_type, Type::Fn(_))
        && body
            .final_expr
            .as_ref()
            .is_some_and(|e| matches!(&*e.kind, rlyeh_ast::ExprKind::Closure { .. }))
    {
        check_block_inner(ctx, body, Some(&return_type))?
    } else {
        check_block_inner(ctx, body, None)?
    };

    // 返回类型一致性：函数体类型应兼容声明的返回类型
    if body_ty != Type::Never && !body_ty.compatible_with(&return_type) {
        // 无捕获闭包值 → fn 指针降级（H5 补全）：
        // `fn make() -> fn(i64) -> i64 { let f = |x: i64| x + 1; f }`——
        // 函数体尾表达式为闭包值变量，闭包对象与 fn 返回类型不兼容，但
        // 零捕获闭包值等价于 fn 指针（调用展开为空字段读取），签名匹配时
        // 将尾表达式替换为 `FnPtr(__closure_N)`。
        let mut downgraded = false;
        // P4c（2026-08-28）：`&dyn Trait` 作函数返回值——尾表达式为 `&T`
        // （T 实现该 trait）时上转为胖指针引用 `&dyn Trait`（Y6 `source() -> &dyn Error` 前提）。
        let mut upshifted = false;
        if let (Type::Ref(inner_ret, _), Type::Ref(inner_body, _)) = (&return_type, &body_ty) {
            if let Type::Dyn(trait_name) = &**inner_ret {
                if let Type::Named(..) = &**inner_body {
                    if let Some(fe) = hir_body.final_expr.take() {
                        let concrete = (**inner_body).clone();
                        let up = coerce_to_dyn(ctx, fe, &concrete, trait_name, f.span)?;
                        hir_body.final_expr = Some(up);
                        upshifted = true;
                    }
                }
            }
        }
        if let Type::Fn(sig) = &return_type {
            // 已固化无捕获闭包值 → fn 指针降级
            if let Some((fp_hir, fp_ty)) = try_closure_value_as_fn(&body_ty) {
                if fp_ty.compatible_with(&return_type) && hir_body.final_expr.is_some() {
                    hir_body.final_expr = Some(fp_hir);
                    downgraded = true;
                }
            }
            // 未固化延迟闭包值 → 按返回签名固化
            // （`fn make() -> fn(..) { let f = |x| ..; f }`：参数类型由返回
            // 签名给出，无捕获时降级为 fn 指针；捕获非空报 Unsupported）
            if !downgraded
                && matches!(&body_ty, Type::Closure { fn_name, .. } if fn_name.is_empty())
            {
                let var_name = hir_body.final_expr.as_ref().and_then(|fe| match &fe.kind {
                    HirExprKind::Variable(n) => Some(n.to_string()),
                    _ => None,
                });
                if let Some(n) = var_name {
                    let (fp_hir, fp_ty) = fix_deferred_closure_with_sig(ctx, &n, sig, f.span)?;
                    if fp_ty.compatible_with(&return_type) {
                        hir_body.final_expr = Some(fp_hir);
                        downgraded = true;
                    }
                }
            }
        }
        if !downgraded && !upshifted {
            ctx.current_return_type = saved_return_type;
            ctx.pop_scope();
            return Err(TypeError::WrongType {
                expected: return_type.to_string(),
                found: body_ty.to_string(),
                span: f.span,
                related: vec![(
                    f.span,
                    format!("期望返回类型 `{}` 声明于此", return_type),
                )],
            });
        }
    }

    ctx.current_return_type = saved_return_type;
    ctx.pop_scope();
    Ok(Some(hir_body))
}
