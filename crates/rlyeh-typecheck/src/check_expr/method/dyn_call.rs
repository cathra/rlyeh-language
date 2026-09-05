//! method/dyn_call：`dyn Trait` 虚调用去虚拟化、`Self` 类型替换与 StrFat → String 转换。
//! （由 method.rs 拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use super::*;

pub(super) fn make_strfat_to_string(ctx: &mut TypeContext, data_h: HirExpr, len_h: HirExpr) -> HirExpr {
    let len_tmp = ctx.fresh_temp();
    let data_tmp = ctx.fresh_temp();
    let base = ctx.fresh_temp();
    let len_plus1 = |var: String| {
        HirExpr::new(HirExprKind::Binary(
            HirBinaryOp::Add,
            Box::new(HirExpr::new(HirExprKind::Variable(var), Span::dummy())),
            Box::new(HirExpr::new(HirExprKind::IntLiteral(1), Span::dummy())),
        ), Span::dummy())
    };
    let stmts = vec![
        HirStmt::new(HirStmtKind::Let{
            name: len_tmp.clone(),
            init: len_h,
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: data_tmp.clone(),
            init: HirExpr::new(HirExprKind::Call{
                callee: "alloc_bytes".to_string(),
                args: vec![len_plus1(len_tmp.clone())],
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::Call{
            callee: "copy_bytes".to_string(),
            args: vec![
                HirExpr::new(HirExprKind::Variable(data_tmp.clone()), Span::dummy()),
                data_h,
                len_plus1(len_tmp.clone()),
            ],
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Let{
            name: base.clone(),
            init: HirExpr::new(HirExprKind::Alloc{
                slots: 3,
                by_value: false,
                is_strfat: false,
            }, Span::dummy()),
            mutable: false,
        }, Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 0,
            value: Box::new(HirExpr::new(HirExprKind::Variable(data_tmp), Span::dummy())),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 1,
            value: Box::new(HirExpr::new(HirExprKind::Variable(len_tmp.clone()), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 2,
            value: Box::new(HirExpr::new(HirExprKind::Variable(len_tmp), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
    ];
    HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
        stmts,
        final_expr: Some(HirExpr::new(HirExprKind::Variable(base), Span::dummy())),
    })), Span::dummy())
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
    // G-M1（SH-P0-3）：dyn 变量绑定源具体类型已知时，含 `Self` 签名的方法可
    // 静态分派——将签名中的 `Self` 替换为具体类型后检查实参、推导返回类型
    // （`Self` 返回的方法经 `dyn Trait` 调用时返回具体类型，合法）。
    // 真正擦除具体类型的 `dyn Trait`（如作函数参数传递）仍由 vtable 分支
    // 以 object-unsafe 拒绝（与 Rust 一致）。
    let concrete_params: Vec<Type> = sig
        .params
        .iter()
        .map(|p| replace_type_self(p, &concrete_ty))
        .collect();
    let concrete_ret = replace_type_self(&sig.return_type, &concrete_ty);
    // 参数数量（vtable 分支报错）
    if args.len() + 1 != sig.params.len() {
        return Ok(None);
    }
    // 实参类型检查（与 vtable 分支一致；`&str` 实参升级）
    let mut hir_args = Vec::with_capacity(args.len());
    for (i, a) in args.iter().enumerate() {
        let (h, t) = infer_expr(ctx, a)?;
        let pty = &concrete_params[i + 1];
        let (h, t) = upgrade_str_arg(ctx, h, t, pty, a)?;
        if !t.compatible_with(pty) {
            return Ok(None);
        }
        hir_args.push(h);
    }
    // 数据指针 = 胖指针槽 0（与 vtable 分支一致）
    let data = HirExpr::new(HirExprKind::FieldGet{
        base: Box::new(recv_hir),
        index: 0,
        ty: FieldScalar::Ptr,
    }, Span::dummy());
    let mut call_args = Vec::with_capacity(1 + hir_args.len());
    call_args.push(data);
    call_args.extend(hir_args);
    Ok(Some((
        HirExpr::new(HirExprKind::Call{
            callee: fn_name,
            args: call_args,
        }, Span::dummy()),
        concrete_ret,
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
            is_move,
        } => Closure {
            captures: captures.iter().map(|c| replace_type_self(c, concrete)).collect(),
            params: params.iter().map(|p| replace_type_self(p, concrete)).collect(),
            ret: Box::new(replace_type_self(ret, concrete)),
            fn_name: fn_name.clone(),
            is_move: *is_move,
        },
        AssocProjection { base, assoc } => AssocProjection {
            base: Box::new(replace_type_self(base, concrete)),
            assoc: assoc.clone(),
        },
        // 其余变体（标量 / 不可递归 / 非 Self 泛型占位）原样返回
        other => other.clone(),
    }
}
