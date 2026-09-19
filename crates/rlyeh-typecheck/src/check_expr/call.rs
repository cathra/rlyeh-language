//! 表达式检查子模块：函数调用与闭包。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use super::*;
use std::collections::HashMap;
use crate::context::type_matches;

/// S2：构造切片胖指针 `{data, len}`（`&[T]` / `&mut [T]` 的 unsize coercion 结果）。
///
/// HIR 层与 `&str` 的 StrFat **同构**（布局同为 `{i8*, i64}` 双槽）：
/// `Alloc{slots:2, by_value:true, is_strfat:true}` 后 `FieldSet` 槽 0 = 数据指针、
/// 槽 1 = 长度。整块包在 `Block` 中返回临时变量。
pub(crate) fn make_slice_fat(ctx: &mut TypeContext, data: HirExpr, len: i128) -> HirExpr {
    let sf = ctx.fresh_temp();
    let stmts = vec![
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
            value: Box::new(data),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()),
        HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(sf.clone()), Span::dummy())),
            index: 1,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(len), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()),
    ];
    HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
        stmts,
        final_expr: Some(HirExpr::new(HirExprKind::Variable(sf), Span::dummy())),
    })), Span::dummy())
}

/// P6c（2026-08-29）：trait 关联函数调用（`From::from` / `Into::into` 等）。
///
/// 查找并实例化 `impl Trait<Args> for Self` 中的关联方法；Self 可由 `self_target`
/// （如 `?` 运算符的目标错误类型）给定，或从 impl 的具体 `self_type` 推断（手动调用）。
pub(super) fn check_trait_static_call(
    ctx: &mut TypeContext,
    trait_key: &str,
    method: &str,
    args: &[AstExpr],
    type_args: &[Type],
    self_target: Option<Type>,
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let trait_def = ctx.lookup_trait(trait_key).cloned();
    let trait_params = trait_def
        .as_ref()
        .map(|t| t.type_params.clone())
        .unwrap_or_default();

    // 推断实参类型（亦用于无 turbofish 时按实参位置推断 trait 参数）
    let arg_infos: Vec<(HirExpr, Type)> = args
        .iter()
        .map(|a| infer_expr(ctx, a))
        .collect::<Result<Vec<_>, _>>()?;

    // P6c-1/2（2026-08-29）：`Into::into` 经 blanket 语义实现——`Into<U>::into(x)`
    // 等价于 `From::from(x)`，当且仅当存在 `impl From<A_source> for U_target`。
    // 不注册 blanket impl（其方法体 `From::from(self)` 无法在泛型层面静态检查），
    // 改由类型检查器在此直接改写，并以其约束求解确认 `From` impl 存在。
    let trait_short = trait_key.rsplit("::").next().unwrap_or(trait_key);
    if trait_short == "Into" && method == "into" {
        if type_args.len() != 1 {
            return Err(TypeError::Unsupported {
                what: "`Into::into` 需经 turbofish 指定目标类型（如 `Into::<Target>::into(x)`）；`x.into()` 方法形式的目标类型推断待专项".to_string(),
                span,
            });
        }
        let u_target = type_args[0].clone();
        let a_source = match arg_infos.first() {
            Some((_, t)) => t.clone(),
            None => {
                return Err(TypeError::UnexpectedArgumentCount {
                    name: "Into::into".to_string(),
                    expected: 1,
                    found: 0,
                    span,
                })
            }
        };
        // 约束求解：确认 `impl From<A_source> for U_target` 存在（即 `From<A_source>`
        // 对 `U_target` 有可用 trait impl；缺失则报错，语义对齐 Rust `U: From<T>`）。
        let from_key = ctx
            .resolve_trait_key("From")
            .unwrap_or_else(|| "From".to_string());
        if ctx
            .find_impl_for_trait_method(&u_target, &from_key, "from")
            .is_none()
        {
            return Err(TypeError::Unsupported {
                what: format!(
                    "`Into::<{u_target}>::into` 不可用：缺少 `impl From<{a_source}> for {u_target}`"
                ),
                span,
            });
        }
        // 改写：`From::from(x)`（Self = U_target，turbofish 绑定 From 泛型参数 = A_source）。
        // 复用 trait 关联函数调用路径，零新增 IR 节点。
        return check_trait_static_call(
            ctx,
            &from_key,
            "from",
            args,
            &[a_source],
            Some(u_target),
            span,
        );
    }

    let mut chosen: Option<&ImplDef> = None;
    for d in &ctx.impl_defs {
        if d.trait_name.as_deref() != Some(trait_key) {
            continue;
        }
        let method_def = match d.methods.iter().find(|m| m.sig.name == method) {
            Some(m) => m,
            None => continue,
        };
        // Self 约束（给定时要求匹配）
        if let Some(st) = &self_target {
            if !type_matches(d, st) {
                continue;
            }
        }
        // 构造 trait 参数绑定：turbofish 优先，否则从实参位置推断
        let mut bind: HashMap<String, Type> = HashMap::new();
        for (i, ta) in type_args.iter().enumerate() {
            if let Some(tp) = trait_params.get(i) {
                bind.insert(tp.clone(), ta.clone());
            }
        }
        if bind.is_empty() {
            for (i, p) in method_def.sig.params.iter().enumerate() {
                if let Type::Generic(tp) = p {
                    if trait_params.iter().any(|x| x == tp) && !bind.contains_key(tp) {
                        if let Some((_, aty)) = arg_infos.get(i) {
                            bind.insert(tp.clone(), aty.clone());
                        }
                    }
                }
            }
        }
        // 与 impl 记录的 trait_type_args 对齐（均为具体类型时一致性校验）
        let mut ok = true;
        if d.trait_type_args.len() == trait_params.len() {
            for (tp, ta) in trait_params.iter().zip(&d.trait_type_args) {
                if let Some(b) = bind.get(tp) {
                    if !b.compatible_with(ta) {
                        ok = false;
                        break;
                    }
                }
            }
        }
        if ok {
            chosen = Some(d);
            break;
        }
    }

    let impl_def: ImplDef = match chosen {
        Some(d) => d.clone(),
        None => {
            return Err(TypeError::Unsupported {
                what: format!(
                    "找不到 `{trait_key}::{method}` 的可用 trait impl（需实现 `impl {trait_key}<..> for <Self>`）"
                ),
                span,
            })
        }
    };

    // 确定 Self：调用上下文给定，否则取自 impl 的具体 self_type（必须非泛型）
    let self_ty = match &self_target {
        Some(st) => st.clone(),
        None => {
            let s = &impl_def.self_type;
            if matches!(s, Type::Generic(_)) {
                return Err(TypeError::Unsupported {
                    what: format!(
                        "`{trait_key}::{method}` 的 Self 无法从上下文确定（需目标类型注解 / 返回值上下文）"
                    ),
                    span,
                });
            }
            s.clone()
        }
    };

    // 组装替换：trait 参数绑定 + Self
    let mut subst: HashMap<String, Type> = HashMap::new();
    for (i, ta) in type_args.iter().enumerate() {
        if let Some(tp) = trait_params.get(i) {
            subst.insert(tp.clone(), ta.clone());
        }
    }
    if subst.is_empty() {
        let method_def = impl_def
            .methods
            .iter()
            .find(|m| m.sig.name == method)
            .unwrap();
        for (i, p) in method_def.sig.params.iter().enumerate() {
            if let Type::Generic(tp) = p {
                if trait_params.iter().any(|x| x == tp) && !subst.contains_key(tp) {
                    if let Some((_, aty)) = arg_infos.get(i) {
                        subst.insert(tp.clone(), aty.clone());
                    }
                }
            }
        }
    }
    subst.insert("Self".to_string(), self_ty.clone());

    let method_def = impl_def
        .methods
        .iter()
        .find(|m| m.sig.name == method)
        .cloned()
        .ok_or_else(|| TypeError::FunctionNotFound {
            name: format!("{trait_key}::{method}"),
            span,
        })?;

    let expected: Vec<Type> = method_def
        .sig
        .params
        .iter()
        .map(|p| substitute(p, &subst))
        .collect();
    let ret_ty = substitute(&method_def.sig.return_type, &subst);

    let fn_name = instantiate_impl_method(ctx, &impl_def, &method_def, &subst, span)?;

    if arg_infos.len() != expected.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: format!("{trait_key}::{method}"),
            expected: expected.len(),
            found: arg_infos.len(),
            span,
        });
    }
    let mut hir_args = Vec::with_capacity(arg_infos.len());
    for (i, (hir, ty)) in arg_infos.into_iter().enumerate() {
        if !ty.compatible_with(&expected[i]) {
            // B-2（P0'）：实参为 `&T`（Copy）而形参为 `T` 时自动解引用取值
            match crate::check_expr::try_auto_deref_coerce(ctx, &expected[i], &ty, &args[i], args[i].span) {
                Some(Ok((c_hir, _))) => {
                    hir_args.push(c_hir);
                    continue;
                }
                _ => {
                    return Err(TypeError::ArgumentTypeMismatch {
                        name: format!("{trait_key}::{method}"),
                        index: i,
                        expected: expected[i].to_string(),
                        found: ty.to_string(),
                        span,
                        related: vec![],
                    });
                }
            }
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

pub(super) fn check_call(
    ctx: &mut TypeContext,
    callee: &AstExpr,
    args: &[AstExpr],
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    let name = match &*callee.kind {
        ExprKind::Ident(n) => n.clone(),
        // 模块路径调用：`math::add(...)`
        ExprKind::Path(segments) => segments.join("::"),
        _ => {
            // H3 捕获闭包 IIFE：`(|x, y| body)(args)` 立即调用——
            // callee 为闭包表达式时走捕获闭包路径（body 中引用的外层变量
            // 按值捕获，desugar 为匿名函数 + 捕获变量前置调用）
            if let ExprKind::Closure { params, param_types: _, body, capture } = &*callee.kind {
                return check_capture_closure_iife(ctx, params, body, capture, args, span);
            }
            // 函数值调用（函数指针）：`fns[i](x)` / `get_fn()(x)`
            let (callee_hir, callee_ty) = infer_expr(ctx, callee)?;
            let signature = match &callee_ty {
                Type::Fn(sig) => (**sig).clone(),
                other => {
                    return Err(TypeError::Unsupported {
                        what: format!("复杂被调用表达式（类型 `{other}` 不可调用）"),
                        span,
                    });
                }
            };
            return check_indirect_call(ctx, callee_hir, signature, args, span);
        }
    };

    // G-M2/G-M3（SH-P0-3）：`Any` 类型擦除——运行时类型标识读取与安全向下转换。
    // `any_type_id(x: dyn Any) -> i64` 读 vtable 槽 0；
    // `any_downcast_ref::<T>(x: dyn Any) -> Option<&T>` 比对 type_id 后产出
    // `Some(&T)` / `None`（无未检查转换）。
    if name == "any_type_id" {
        return check_any_type_id(ctx, args, span);
    }
    if name == "any_downcast_ref" {
        return check_any_downcast_ref(ctx, args, type_args, span);
    }
    // L2 编译器内建 JSON 序列化/反序列化（`json.stringify(v)` / `json.parse::<T>(s)`）：
    // AST 层 desugar 为 String 构建 / 解析表达式，零新增 IR 节点。
    // Q2a 泛型 API 别名：`json.to_string(v)` ≡ `json.stringify(v)`；
    // `json.from_str::<T>(s)` ≡ `json.parse::<T>(s)`。MVP 无泛型 trait 约束
    // （`T: Serialize` / `T: Deserialize` bound 不支持），签名退化为无 bound
    // turbofish 形式：序列化类型由实参推断，反序列化经 turbofish 指定。
    if name == "json::stringify" || name == "json::to_string" {
        return check_json_stringify(ctx, args, span);
    }
    if name == "json::parse" || name == "json::from_str" {
        return check_json_parse(ctx, args, type_args, span);
    }
    // P2：`json::try_parse::<T>(s) -> Result<T, JsonError>` 严格解析
    // （非法输入返回 Err，替代 `json::parse` 的静默零值/宽松解析）。
    if name == "json::try_parse" {
        return check_json_try_parse(ctx, args, type_args, span);
    }
    // Q4 `toml` 模块（轻量 MVP）：`toml.to_string`/`toml.stringify` 序列化（基础标量 /
    // 嵌套表（内联表）/ 数组），`toml.from_str`/`toml.parse` 反序列化（round-trip 对齐
    // stringify 的紧凑输出）。MVP 无泛型 trait 约束（`T: Serialize` / `T: Deserialize`
    // bound 不支持），签名退化为无 bound turbofish 形式（同 Q2 json）。
    if name == "toml::stringify" || name == "toml::to_string" {
        return check_toml_stringify(ctx, args, span);
    }
    if name == "toml::parse" || name == "toml::from_str" {
        return check_toml_parse(ctx, args, type_args, span);
    }
    // P3：`toml::try_parse::<T>(s) -> Result<T, TomlError>` 严格解析
    if name == "toml::try_parse" {
        return check_toml_try_parse(ctx, args, type_args, span);
    }
    // Q2b 流式 writer/reader（目标 `File`；TcpStream 留待流式 read_all 方法化）：
    // `json.to_writer(w, v)` → `w.write_all(json.stringify(v))`，返回
    // `Result<i64, io::error::IoError>`；`json.from_reader::<T>(r)` →
    // `json.parse::<T>(r.read_to_string().unwrap())`，读失败经 `unwrap` 死循环
    // （MVP 语义，与 std `Result::unwrap` 一致）。
    if name == "json::to_writer" {
        return check_json_to_writer(ctx, args, span);
    }
    if name == "json::from_reader" {
        return check_json_from_reader(ctx, args, type_args, span);
    }

    // 内建函数（`print` / `println` / `alloc_array` 等，由代码生成层映射到运行时）：
    // 按签名检查参数、返回签名类型
    if let Some((params, ret)) = builtin_signature(&name) {
        // `print` / `println` / `eprint` / `eprintln` 允许 0..=1 个参数
        // （`println()` 打印空行；`eprint*` 输出到 stderr）
        let is_console = name == "print"
            || name == "println"
            || name == "eprint"
            || name == "eprintln";
        let max_args = if is_console { 1 } else { params.len() };
        if args.len() > max_args {
            return Err(TypeError::UnexpectedArgumentCount {
                name: name.clone(),
                expected: max_args,
                found: args.len(),
                span,
            });
        }
        // `print(s)` / `println(s)` / `eprint(s)` / `eprintln(s)` 参数为 String →
        // 展开为 `print_string` / `println_string` / `eprint_string` / `eprintln_string`
        // 内建（动态缓冲按 `%.*s` 打印；typecheck 无法在内建签名层表达对象槽读取）
        if is_console && args.len() == 1 {
            let (mut hir, mut ty) = infer_expr(ctx, &args[0])?;
            // 引用参数自动剥一层（G1 剥层语义覆盖 print/println 内建：
            // `println(r)` 打印解引用值而非地址，与字段访问 / 方法调用剥层一致）
            // 例外：`&str`（`Ref(Str)`）保留为 StrFat 双槽胖指针 `{data, len}`，
            // **不得剥层成瘦指针 `Str`**——否则子区间视图的 len 信息丢失，
            // codegen 走 `Str` 打印（到 NUL 读到全串）而非 `StrFat` 打印
            // （`%.*s` 长度限定，仅打印子区间）。`&str` 本就是 by-value 双槽值。
            if let Type::Ref(inner, _, _) = &ty {
                if !matches!(&**inner, Type::Str) {
                    hir = HirExpr::new(HirExprKind::Deref{
                        expr: Box::new(hir),
                        ty: field_scalar_of(inner),
                    }, Span::dummy());
                    ty = (**inner).clone();
                }
            }
            if let Type::Named(n, _) = peel_ref(&ty) {
                let full = ctx
                    .resolve_full_name(&n)
                    .unwrap_or_else(|| n.clone());
                if full == "String" && ctx.lookup_struct(&full).is_some() {
                    let callee = match name.as_str() {
                        "println" => "println_string",
                        "print" => "print_string",
                        "eprintln" => "eprintln_string",
                        _ => "eprint_string",
                    };
                    return Ok((
                        HirExpr::new(HirExprKind::Call{
                            callee: callee.to_string(),
                            args: vec![hir],
                        }, Span::dummy()),
                        Type::Unit,
                    ));
                }
            }
            // 非 String 参数：直接生成 print / println / eprint / eprintln 调用
            // （引用已剥层，避免落入下方通用路径时对原始实参重新 infer 而丢失剥层结果）
            return Ok((
                HirExpr::new(HirExprKind::Call{
                    callee: name.clone(),
                    args: vec![hir],
                }, Span::dummy()),
                Type::Unit,
            ));
        }
        // `hash_value(s)` 参数为 String → 展开为 djb2 内容哈希（逐字节散列，
        // 同一内容字符串恒同哈希，保证 HashMap 探测链正确；字节索引 `s[i]`
        // 步长 1，typecheck 无法在内建签名层表达对象槽读取 + 循环）
        if name == "hash_value" && args.len() == 1 {
            let (hir, ty) = infer_expr(ctx, &args[0])?;
            if let Type::Named(n, _) = peel_ref(&ty) {
                let full = ctx
                    .resolve_full_name(&n)
                    .unwrap_or_else(|| n.clone());
                if full == "String" && ctx.lookup_struct(&full).is_some() {
                    let hash = string_hash_hir(ctx, &hir);
                    return Ok((hash, Type::I64));
                }
            }
        }
        let mut hir_args = Vec::with_capacity(args.len());
        for (i, (a, pty)) in args.iter().zip(&params).enumerate() {
            let (mut hir, mut ty) = infer_expr(ctx, a)?;
            // B-2（P0'）：实参为 `&T`（Copy）而形参为 `T` 时自动解引用取值
            if let Some(Ok((c_hir, c_ty))) =
                crate::check_expr::try_auto_deref_coerce(ctx, pty, &ty, a, a.span)
            {
                hir = c_hir;
                ty = c_ty;
            }
            if !ty.compatible_with(pty) {
                return Err(TypeError::ArgumentTypeMismatch {
                    name: name.clone(),
                    index: i,
                    expected: pty.to_string(),
                    found: ty.to_string(),
                    span: a.span,
                    related: vec![],
                });
            }
            hir_args.push(hir);
        }
        return Ok((
            HirExpr::new(HirExprKind::Call{
                callee: name,
                args: hir_args,
            }, Span::dummy()),
            ret,
        ));
    }

    // 宏调用（`println!` 等）：检查参数、返回 `()`
    if name.ends_with('!') {
        for a in args {
            let (_, _) = infer_expr(ctx, a)?;
        }
        return Ok((
            HirExpr::new(HirExprKind::Call{
                callee: name,
                args: Vec::new(),
            }, Span::dummy()),
            Type::Unit,
        ));
    }

    // actor 构造函数：`Counter::new()` → `rlyeh_actor_spawn("<handle>", <state_new>())`；
    // `Counter::new_supervised(strategy)` → `rlyeh_actor_spawn_supervised("<handle>", "<state_new>", strategy)`
    if let Some((actor_part, seg)) = name.rsplit_once("::") {
        if seg == "new" || seg == "new_supervised" {
            if let Some(actor_full) = ctx.lookup_actor(actor_part).map(|_| {
                ctx.resolve_full_name(actor_part)
                    .unwrap_or_else(|| actor_part.to_string())
            }) {
                let supervised = seg == "new_supervised";
                // 受监督构造必须提供策略参数（i64）：0=OneForOne 1=AllForOne 2=RestartForOne
                let strategy_hir = if supervised {
                    if args.len() != 1 {
                        return Err(TypeError::Unsupported {
                            what: format!("`{actor_part}::new_supervised` 需要 1 个策略参数（i64）"),
                            span,
                        });
                    }
                    let (s_hir, s_ty) = infer_expr(ctx, &args[0])?;
                    if !matches!(s_ty, Type::I64) {
                        return Err(TypeError::Unsupported {
                            what: "actor 监督策略参数必须是 i64".to_string(),
                            span: args[0].span,
                        });
                    }
                    s_hir
                } else {
                    if !args.is_empty() {
                        return Err(TypeError::Unsupported {
                            what: format!("`{actor_part}::new` 不接受参数"),
                            span,
                        });
                    }
                    HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())
                };
                // handler / factory 名必须为 3 槽 String 结构体（data/len/cap）——runtime 侧
                // `cstr()` 按 C 字符串读取。不能传裸字符串字面量（瘦 data 指针，
                // codegen 对 extern `String` 参数会按结构体再解引用一层）。
                // 复用 `String::from` 展开（alloc_bytes + copy_bytes + 三槽构造）。
                let handle = format!("{actor_full}::__handle");
                let (handle_hir, _) = check_string_from(
                    ctx,
                    &[AstExpr {
                        kind: Box::new(ExprKind::StringLiteral(handle)),
                        span,
                    }],
                    span,
                )?;
                let state_new_call = HirExpr::new(HirExprKind::Call{
                    callee: format!("{actor_full}::__state_new"),
                    args: vec![],
                }, Span::dummy());
                let (callee, args) = if supervised {
                    let factory = format!("{actor_full}::__state_new");
                    let (factory_hir, _) = check_string_from(
                        ctx,
                        &[AstExpr {
                            kind: Box::new(ExprKind::StringLiteral(factory)),
                            span,
                        }],
                        span,
                    )?;
                    (
                        "rlyeh_actor_spawn_supervised".to_string(),
                        vec![handle_hir, factory_hir, strategy_hir],
                    )
                } else {
                    ("rlyeh_actor_spawn".to_string(), vec![handle_hir, state_new_call])
                };
                return Ok((
                    HirExpr::new(HirExprKind::Call{ callee, args }, Span::dummy()),
                    Type::Named(actor_full, vec![]),
                ));
            }
        }
    }

    // 普通函数调用：先经 use 别名 / 模块路径解析到完整符号名，再查签名
    let resolved = resolve_callable(ctx, &name);

    // 枚举变体构造：`Option::Some(x)`、`shape::Kind::Pair(x, y)` 或裸 `Some(x)`
    // （普通函数同名时优先函数路径）
    if !ctx.fn_signatures.contains_key(&resolved) {
        if let Some((en, vr)) = split_variant_path(ctx, &resolved) {
            return check_variant_construct(ctx, &en, &vr, args, span);
        }
    }

    // `Vec` 构造器特判：`Vec::with_capacity(n)` / `Vec::new()`
    // （泛型 impl 静态方法 MVP 不支持，编译器直接展开为动态数组分配 + 结构体构造）
    if let Some((ty_name, method)) = resolved.split_once("::") {
        let ty_full = ctx
            .resolve_full_name(ty_name)
            .unwrap_or_else(|| ty_name.to_string());
        if ty_full == "Vec" && ctx.lookup_struct(&ty_full).is_some()
            && (method == "with_capacity" || method == "new")
        {
            return check_vec_construct(ctx, method, args, span);
        }
        // `String` 构造器特判：`new()` / `with_capacity(n)` / `from("字面量")`
        if ty_full == "String" && ctx.lookup_struct(&ty_full).is_some() {
            match method {
                "new" | "with_capacity" => {
                    return check_string_construct(ctx, method, args, span);
                }
                "from" => return check_string_from(ctx, args, span),
                _ => {}
            }
        }
        // `HashMap` 构造器特判：`new()` / `with_capacity(n)`（7 槽 Robin Hood 哈希表）
        if ty_full == "HashMap" && ctx.lookup_struct(&ty_full).is_some()
            && (method == "with_capacity" || method == "new")
        {
            return check_hashmap_construct(ctx, method, args, span);
        }
        // `VecDeque` 构造器特判（V5，2026-08-26）：`new()` / `with_capacity(n)`
        // 3 槽环形双端队列（槽 0 = buf: Vec<T> 指针、槽 1 = front、槽 2 = len）
        if ty_full == "VecDeque" && ctx.lookup_struct(&ty_full).is_some()
            && (method == "with_capacity" || method == "new")
        {
            return check_vecdeque_construct(ctx, method, args, span);
        }
        // `HashSet` 构造器特判（V5，2026-08-26）：`new()` / `with_capacity(n)`
        // 5 槽哈希集合（槽 0 = items 指针、1 = states、2 = len、3 = used、4 = cap）
        if ty_full == "HashSet" && ctx.lookup_struct(&ty_full).is_some()
            && (method == "with_capacity" || method == "new")
        {
            return check_hashset_construct(ctx, method, args, span);
        }
        // `BTreeMap` 构造器特判（V5，2026-08-26）：`new()` / `with_capacity(n)`
        // 3 槽有序映射（槽 0 = keys 指针、1 = vals 指针、2 = len）
        if ty_full == "BTreeMap" && ctx.lookup_struct(&ty_full).is_some()
            && (method == "with_capacity" || method == "new")
        {
            return check_btreemap_construct(ctx, method, args, span);
        }
        // `Iter` / `IterMut` / `IterRef` 构造器特判（V1 瘦指针迭代器，2026-08）：
        // `Iter::new(data, len)`（2 槽）/ `IterMut::new(data, cur, len)`（3 槽）/
        // `IterRef::new(data, len)`（2 槽，返回 `Option<&T>` 引用视图）。
        if matches!(ty_full.as_str(), "Iter" | "IterMut" | "IterRef")
            && ctx.lookup_struct(&ty_full).is_some()
            && method == "new"
        {
            return check_iter_construct(ctx, &ty_full, args, span);
        }
        // `Box` 构造器特判：`Box::new(v)`（K2 堆分配装箱）
        // （Box 为编译器内建智能指针，无需 std 结构体定义）
        if ty_full == "Box" && method == "new" {
            return check_box_new(ctx, args, span);
        }
        // `Rc` / `Arc` 构造器特判：`Rc::new(v)` / `Arc::new(v)`（K3 引用计数装箱）
        if matches!(ty_full.as_str(), "Rc" | "Arc") && method == "new" {
            return check_rc_new(ctx, &ty_full, args, span);
        }
        // `Gc` 构造器特判：`Gc::new(v)`（K4 追踪 GC 装箱）
        // （Gc 为编译器内建智能指针，分配经 rlyeh-gc-runtime 注册块表）
        if ty_full == "Gc" && method == "new" {
            return check_gc_new(ctx, args, span);
        }
        // `Box::leak` 特判（T3a / Y5）：泄漏堆对象，返回指向堆 `T` 的 `&'static mut T`
        // 引用（目标签名），不再释放。U5 AddrOf 已就绪——返回 `&mut T`（引用），
        // codegen 中引用与裸指针同为地址值（取 Box 槽 0 指针），`*leaked` 解引用
        // 走 `Type::Ref` 分支得 `T`；`'static` 生命周期标注宽松丢弃（G4）。
        if ty_full == "Box" && method == "leak" {
            if args.len() != 1 {
                return Err(TypeError::UnexpectedArgumentCount {
                    name: "Box::leak".to_string(),
                    expected: 1,
                    found: args.len(),
                    span,
                });
            }
            let (b_hir, b_ty) = infer_expr(ctx, &args[0])?;
            let inner = peel_refs_and_heap(&b_ty);
            let ptr = HirExpr::new(HirExprKind::FieldGet{
                base: Box::new(b_hir),
                index: 0,
                ty: FieldScalar::Ptr,
            }, Span::dummy());
            return Ok((ptr, Type::Ref(Box::new(inner), Mutability::Mutable, None)));
        }
        // `Weak` 升级特判：`Weak::upgrade(w)`（K3 弱引用升级为强引用）
        if ty_full == "Weak" && method == "upgrade" {
            if args.len() != 1 {
                return Err(TypeError::UnexpectedArgumentCount {
                    name: "Weak::upgrade".to_string(),
                    expected: 1,
                    found: args.len(),
                    span,
                });
            }
            let (w_hir, w_ty) = infer_expr(ctx, &args[0])?;
            match peel_ref(&w_ty) {
                Type::Named(n, ps) if n == "Weak" && ps.len() == 1 => {
                    return weak_upgrade(ctx, ps[0].clone(), w_hir, &[], span);
                }
                found => {
                    return Err(TypeError::ArgumentTypeMismatch {
                        name: "Weak::upgrade".to_string(),
                        index: 0,
                        expected: "Weak<T>".to_string(),
                        found: found.to_string(),
                        span,
                        related: vec![],
                    });
                }
            }
        }
    }

    // 静态方法调用：`Point::origin()`（impl 中无 self 的方法）
    // （仅当 `Type::method` 不是普通函数/泛型模板时才走此路径）
    // P7b-2（2026-08-29）：turbofish 调用（type_args 非空）优先走 static method
    // call——impl 泛型方法只注册为普通 fn_signatures（不注册为 fn_templates），
    // 若按原条件跳过则 turbofish 实参被忽略（`Bag::<i64>::new()` 返回无参 `Bag`）。
    let is_turbofish = !type_args.is_empty();
    if is_turbofish
        || (!ctx.fn_signatures.contains_key(&resolved)
            && !ctx.fn_templates.contains_key(&resolved))
    {
        if let Some((ty_name, method)) = resolved.split_once("::") {
            let ty_full = ctx
                .resolve_full_name(ty_name)
                .unwrap_or_else(|| ty_name.to_string());
            if ctx.lookup_struct(&ty_full).is_some() || ctx.lookup_enum(&ty_full).is_some() {
                // P7b-2：传入 turbofish 类型实参（`Bag::<i64>::new()` → T = i64）
                return check_static_method_call(ctx, &ty_full, method, args, type_args, span);
            }
            // P6c（2026-08-29）：trait 关联函数调用（`From::from` / `Into::into` 等）。
            // Self 类型无法从调用点单独确定时（无期望类型上下文），由 impl 的具体
            // self_type 推断（如 `From::<E1>::from(e)` 的 Self = 该 impl 的目标类型）。
            if let Some(trait_key) = ctx.resolve_trait_key(&ty_full).or_else(|| ctx.resolve_trait_key(ty_name)) {
                let resolved_args: Vec<Type> = type_args
                    .iter()
                    .map(|t| resolve_ast_type(ctx, t, span))
                    .collect::<Result<Vec<_>, _>>()?;
                return check_trait_static_call(ctx, &trait_key, method, args, &resolved_args, None, span);
            }
        }
    }

    // 泛型函数模板：调用点按实参类型实例化（P7b-2：turbofish 实参优先预填）
    if ctx.fn_templates.contains_key(&resolved) {
        return check_generic_call(ctx, &resolved, args, type_args, span);
    }

    let signature = match ctx.lookup_fn_signature(&resolved).cloned() {
        Some(s) => s,
        None => {
            // 闭包值对象调用兜底：callee 为闭包值变量（`let f = |x: i64| ..; f(1)`）。
            if let Some(ty) = ctx.lookup_variable(&name).cloned() {
                if let Type::Closure { .. } = &ty {
                    return check_closure_value_call(ctx, &name, &ty, args, span);
                }
            }
            // 函数指针调用兜底：callee 为函数值表达式（如 `let f = add; f(1, 2)`）。
            // 推断失败（未定义变量等）时保留原有 FunctionNotFound 诊断。
            if let Ok((callee_hir, Type::Fn(sig))) = infer_expr(ctx, callee).as_ref() {
                return check_indirect_call(ctx, callee_hir.clone(), (**sig).clone(), args, span);
            }
            return Err(TypeError::FunctionNotFound {
                name: name.clone(),
                span,
            });
        }
    };

    // SH-P0-1 E3：extern 函数调用强制要求在 `unsafe` 块内。
    // 标准库预置（prelude，字节范围 [0, prelude_len)）内的调用视为受信任，豁免。
    if ctx.extern_fns.contains(&resolved)
        && !ctx.in_unsafe
        && span.start >= ctx.prelude_len
    {
        return Err(TypeError::UnsafeExternCall {
            name: resolved.clone(),
            span,
        });
    }

    if args.len() != signature.params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name,
            expected: signature.params.len(),
            found: args.len(),
            span,
        });
    }
    let mut hir_args = Vec::with_capacity(args.len());
    for (i, (arg, param_ty)) in args.iter().zip(&signature.params).enumerate() {
        // H2 无捕获闭包实参：形参为 fn 类型且实参为闭包 → 按预期签名检查
        // （闭包参数无类型注解，无法脱离 fn 上下文推断参数类型）
        let (mut hir, mut ty) =
            if matches!(param_ty, Type::Fn(_)) && matches!(&*arg.kind, ExprKind::Closure { .. }) {
                check_closure_expected(ctx, arg, param_ty, arg.span)?
            } else {
                infer_expr(ctx, arg)?
            };
        // B-2（P0'）：实参为 `&T`（Copy）而形参为 `T` 时自动解引用取值
        if let Some(Ok((c_hir, c_ty))) =
            crate::check_expr::try_auto_deref_coerce(ctx, param_ty, &ty, arg, arg.span)
        {
            hir = c_hir;
            ty = c_ty;
        }
        // Str 值实参 → 非 Str 形参自动升级（`fn f(s: String)` 传 `f("hi")`）
        let (hir, ty) = upgrade_str_arg(ctx, hir, ty, param_ty, arg)?;
        // 闭包值实参 → fn 形参（H5 补全）：
        // - 未固化延迟闭包值（`let f = |x| x + 1; apply(f, 41);`）：按 fn
        //   形参签名固化参数类型，无捕获时降级为函数指针（捕获非空报 Unsupported）；
        // - 已固化无捕获闭包值（`let f = |x: i64| x + 1; apply(f, 41);`）：
        //   直接降级为函数指针；
        // - 有捕获闭包值保持闭包对象类型 → 与 fn 形参不兼容，报 ArgumentTypeMismatch。
        let (hir, ty) = if let Type::Fn(sig) = param_ty {
            if matches!(&ty, Type::Closure { fn_name, .. } if fn_name.is_empty()) {
                if let ExprKind::Ident(n) = &*arg.kind {
                    fix_deferred_closure_with_sig(ctx, n, sig, arg.span)?
                } else {
                    (hir, ty)
                }
            } else {
                try_closure_value_as_fn(&ty).unwrap_or((hir, ty))
            }
        } else {
            (hir, ty)
        };
        // 函数指针实参 → i64 形参（S0 线程入口地址整数化）：
        // `__rlyeh_thread_spawn(f, 0)` 中 f 为 `fn() -> i64` 函数指针值，extern 形参
        // 是 i64——函数指针按地址整数传递，codegen 在 extern 调用点做 ptrtoint。
        let (hir, ty) = if matches!(param_ty, Type::I64) && matches!(&ty, Type::Fn(_)) {
            (hir, Type::I64)
        } else {
            (hir, ty)
        };
        if !ty.compatible_with(param_ty) {
            let related = if signature.param_spans.get(i).map_or(true, |s| *s == Span::dummy()) {
                vec![]
            } else {
                vec![(signature.param_spans[i], format!("形参 #{} 声明于此", i + 1))]
            };
            return Err(TypeError::ArgumentTypeMismatch {
                name: name.clone(),
                index: i,
                expected: param_ty.to_string(),
                found: ty.to_string(),
                span: arg.span,
                related,
            });
        }
        // S2 unsize coercion：`&[T; N]` 实参传给 `&[T]` / `&mut [T]` 形参时构造
        // 切片胖指针 `{data, len}`（len 为编译期数组长度，data 为数组首元素指针）
        let mut hir = hir;
        if let (Type::Ref(ia, _, _), Type::Ref(ib, _, _)) = (&ty, param_ty) {
            if let (Type::Array(_, n), Type::Slice(_)) = (&**ia, &**ib) {
                hir = make_slice_fat(ctx, hir, *n as i128);
            }
        }
        hir_args.push(hir);
    }
    Ok((
        HirExpr::new(HirExprKind::Call{
            callee: resolved,
            args: hir_args,
        }, Span::dummy()),
        signature.return_type,
    ))
}


pub(super) fn check_indirect_call(
    ctx: &mut TypeContext,
    callee_hir: HirExpr,
    signature: FnSignature,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != signature.params.len() {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "<函数指针>".to_string(),
            expected: signature.params.len(),
            found: args.len(),
            span,
        });
    }
    let mut hir_args = Vec::with_capacity(args.len());
    for (i, (arg, param_ty)) in args.iter().zip(&signature.params).enumerate() {
        // H2 无捕获闭包实参：形参为 fn 类型且实参为闭包 → 按预期签名检查
        // （闭包参数无类型注解，无法脱离 fn 上下文推断参数类型）
        let (mut hir, mut ty) =
            if matches!(param_ty, Type::Fn(_)) && matches!(&*arg.kind, ExprKind::Closure { .. }) {
                check_closure_expected(ctx, arg, param_ty, arg.span)?
            } else {
                infer_expr(ctx, arg)?
            };
        // B-2（P0'）：实参为 `&T`（Copy）而形参为 `T` 时自动解引用取值
        if let Some(Ok((c_hir, c_ty))) =
            crate::check_expr::try_auto_deref_coerce(ctx, param_ty, &ty, arg, arg.span)
        {
            hir = c_hir;
            ty = c_ty;
        }
        // Str 值实参 → 非 Str 形参自动升级（函数指针调用 `f("a", "b")`）
        let (hir, ty) = upgrade_str_arg(ctx, hir, ty, param_ty, arg)?;
        // 无捕获闭包值实参 → fn 形参：降级为函数指针（H5 补全，
        // `let f = |x: i64| x + 1; apply(f, 41);`）
        let (hir, ty) = if matches!(param_ty, Type::Fn(_)) {
            try_closure_value_as_fn(&ty).unwrap_or((hir, ty))
        } else {
            (hir, ty)
        };
        if !ty.compatible_with(param_ty) {
            let related = if signature.param_spans.get(i).map_or(true, |s| *s == Span::dummy()) {
                vec![]
            } else {
                vec![(signature.param_spans[i], format!("形参 #{} 声明于此", i + 1))]
            };
            return Err(TypeError::ArgumentTypeMismatch {
                name: "<函数指针>".to_string(),
                index: i,
                expected: param_ty.to_string(),
                found: ty.to_string(),
                span: arg.span,
                related,
            });
        }
        hir_args.push(hir);
    }
    let param_names = signature.params.iter().map(type_to_extern_name).collect();
    Ok((
        HirExpr::new(HirExprKind::CallIndirect{
            callee: Box::new(callee_hir),
            args: hir_args,
            param_names,
            ret_name: type_to_extern_name(&signature.return_type),
        }, Span::dummy()),
        signature.return_type,
    ))
}
