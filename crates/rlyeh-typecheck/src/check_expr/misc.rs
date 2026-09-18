//! 表达式检查子模块：misc。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use rlyeh_hir::{HirExprKind, HirStmtKind};
use rlyeh_lexer::Span;
use super::*;

pub fn builtin_signature(name: &str) -> Option<(Vec<Type>, Type)> {
    let dyn_arr = || Type::Array(Box::new(Type::Infer), 0);
    match name {
        "print" | "println" | "eprint" | "eprintln" => Some((vec![Type::Infer], Type::Unit)),
        "alloc_array" => Some((vec![Type::I64], dyn_arr())),
        "array_copy" => Some((vec![dyn_arr(), dyn_arr(), Type::I64], Type::Unit)),
        "array_free" => Some((vec![dyn_arr()], Type::Unit)),
        // String 动态缓冲（按字节）：
        "alloc_bytes" => Some((vec![Type::I64], Type::Array(Box::new(Type::U8), 0))),
        "copy_bytes" => Some((vec![Type::Infer, Type::Infer, Type::I64], Type::Unit)),
        // String 内容相等：`bytes_eq(a, b, n)` → `memcmp(a, b, n) == 0`
        "bytes_eq" => Some((vec![Type::Infer, Type::Infer, Type::I64], Type::Bool)),
        // String 字典序：`bytes_cmp(a, b, n)` → `memcmp(a, b, n)` 有符号扩展为 i64
        // （负/零/正 → 小于/等于/大于；前缀相等时长度兜底由 desugar 层处理）
        "bytes_cmp" => Some((vec![Type::Infer, Type::Infer, Type::I64], Type::I64)),
        // String 打印：`print_string` / `println_string` 接收 String 对象指针
        "print_string" | "println_string" => Some((vec![Type::Infer], Type::Unit)),
        // HashMap 键散列：Knuth 乘法混合散列（MVP 仅支持整数键），返回非负散列值
        "hash_value" => Some((vec![Type::Infer], Type::I64)),
        _ => None,
    }
}

/// G-M2（SH-P0-3）：运行时类型标识（TypeId 式）。对具体类型的规范字符串做
/// FNV-1a 64 位散列——编译期确定、全程序稳定：`type_id_of(P)` 在任何模块、
/// 任何 `dyn Any` 强制转换点都得到相同值，故 `downcast` 可用标量相等比较判定。
pub(crate) fn type_id_of(ty: &Type) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in ty.to_string().as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h as i64
}

/// `dyn Any` 的类型擦除标签（编译器内置，不依赖 trait 声明）。
pub(crate) fn is_any_trait(name: &str) -> bool {
    name == "Any" || name.ends_with("::Any")
}

/// Q-M1（SH-P0-8）：析构 trait 标签（编译器内置，不依赖 trait 声明）。
///
/// 与 `Any` 同构——`impl Drop for T` 无论 `trait Drop` 是否显式声明都生效，
/// 避免因「trait 未声明」阻断析构注册。`Drop` 的解析名可能带模块前缀
/// （`collect.rs` 的路径解析），故两种形式都要匹配。
pub(crate) fn is_drop_trait(name: &str) -> bool {
    name == "Drop" || name.ends_with("::Drop")
}

/// Q-M1（SH-P0-8）：类型 `ty` 是否实现了 `Drop`（存在 `impl Drop for ty`）。
///
/// 仅按 trait 名 + 目标类型匹配，不要求 `drop` 方法已单态化——具体方法解析
/// 交由 `check_method_call` 在块尾插入 `x.drop()` 时完成。
pub(crate) fn has_drop_impl(ctx: &TypeContext, ty: &Type) -> bool {
    ctx.impl_defs.iter().any(|d| {
        d.trait_name.as_deref().is_some_and(is_drop_trait)
            && crate::context::type_matches(d, ty)
    })
}

/// Q-M2（SH-P0-8）：为当前作用域本层声明的变量生成块尾析构语句。
///
/// 规则：
/// - **逆声明序**（与 Rust 一致：后声明者先析构）；
/// - 仅**拥有所有权**的绑定——引用（`Ref`）不持有所有权，跳过；
/// - 仅类型存在 `impl Drop for T` 的变量；**无 Drop 实现的类型完全不受影响**
///   （当前 std 无任何 Drop 实现，故本机制对存量代码零影响）；
/// - 析构调用经 `check_stmt` 走常规方法解析（`x.drop()`），故 `&mut self`
///   借用、泛型单态化、trait 分派等全部复用既有路径，零新增 IR。
///
/// 已知限制（MVP）：不跟踪 move——被 `return` 移出或转移给其他值的变量仍会被
/// 析构；函数**形参**在外层 fn 作用域，不在本层，故不析构。
pub(crate) fn build_scope_drops(
    ctx: &mut TypeContext,
    span: Span,
) -> Result<Vec<HirStmt>, TypeError> {
    let decls = ctx.current_scope_decls();
    let mut drops = Vec::new();
    for (name, _slot, ty) in decls.into_iter().rev() {
        let base = AstExpr::new(ExprKind::Ident(name), span);
        build_drop_glue(ctx, &base, &ty, 0, &mut drops, span)?;
    }
    Ok(drops)
}

/// 字段析构递归深度上限。
///
/// 结构体内嵌自身（`struct A { b: B }` / `struct B { a: A }`）会让 drop glue
/// 无限展开，故设上限；超出即停止（MVP 不处理递归类型的完整析构链）。
const MAX_DROP_DEPTH: usize = 4;

/// Q-M3（SH-P0-8）：为值表达式 `base` 生成类型 `ty` 的析构 glue（drop glue）。
///
/// 顺序与 Rust 一致：**先 `T::drop()`，再按字段逆序递归析构各字段**。
/// 仅具名 struct 参与字段递归（枚举 / 元组 / 内建类型不在 `ctx.structs` 内，
/// 自然终止）；引用不持有所有权，整支跳过。
fn build_drop_glue(
    ctx: &mut TypeContext,
    base: &AstExpr,
    ty: &Type,
    depth: usize,
    out: &mut Vec<HirStmt>,
    span: Span,
) -> Result<(), TypeError> {
    if depth > MAX_DROP_DEPTH || matches!(ty, Type::Ref(..)) {
        return Ok(());
    }
    // 1) 自身 Drop 实现
    if has_drop_impl(ctx, ty) {
        let call = AstExpr::new(
            ExprKind::MethodCall {
                receiver: base.clone(),
                method: "drop".to_string(),
                args: Vec::new(),
                trait_hint: None,
            },
            span,
        );
        let (hir_stmts, _) = crate::check_stmt::check_stmt(ctx, &AstStmt::Semi(call))?;
        out.extend(hir_stmts);
    }
    // 2) 字段递归（drop glue）
    let Type::Named(name, args) = ty else {
        return Ok(());
    };
    let Some(def) = ctx.structs.get(name).cloned() else {
        return Ok(());
    };
    let subst: HashMap<String, Type> = def
        .type_params
        .iter()
        .cloned()
        .zip(args.iter().cloned())
        .collect();
    for (fname, fty) in def.fields.iter().rev() {
        let fty = crate::check_expr::util::substitute(fty, &subst);
        let fexpr = AstExpr::new(
            ExprKind::FieldAccess {
                expr: base.clone(),
                field: fname.clone(),
            },
            span,
        );
        build_drop_glue(ctx, &fexpr, &fty, depth + 1, out, span)?;
    }
    Ok(())
}

/// `&T → dyn Any` 强制转换（G-M2）。
///
/// 与通用 `dyn Trait` 的区别：不查 trait 声明、不填充方法表——vtable 仅保留
/// 3 元槽，其中**槽 0 存具体类型的 type_id**，供 `any_type_id` /
/// `any_downcast_ref` 读取比对（槽 1/2 为 size/align，MVP 置 0）。
fn coerce_to_any(
    ctx: &mut TypeContext,
    data_ptr: HirExpr,
    concrete: &Type,
) -> Result<HirExpr, TypeError> {
    let mut stmts = Vec::new();
    let vt = ctx.fresh_temp();
    stmts.push(HirStmt::new(HirStmtKind::Let{
        name: vt.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: 3,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy()));
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(vt.clone()), Span::dummy())),
        index: 0,
        value: Box::new(HirExpr::new(HirExprKind::IntLiteral(type_id_of(concrete) as i128), Span::dummy())),
        ty: FieldScalar::Int,
    }, Span::dummy())), Span::dummy()));
    for i in 1..3 {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(vt.clone()), Span::dummy())),
            index: i,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()));
    }
    let dyn_var = ctx.fresh_temp();
    stmts.push(HirStmt::new(HirStmtKind::Let{
        name: dyn_var.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: 2,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy()));
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(dyn_var.clone()), Span::dummy())),
        index: 0,
        value: Box::new(data_ptr),
        ty: FieldScalar::Ptr,
    }, Span::dummy())), Span::dummy()));
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(dyn_var.clone()), Span::dummy())),
        index: 1,
        value: Box::new(HirExpr::new(HirExprKind::Variable(vt), Span::dummy())),
        ty: FieldScalar::Ptr,
    }, Span::dummy())), Span::dummy()));
    Ok(HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
        stmts,
        final_expr: Some(HirExpr::new(HirExprKind::Variable(dyn_var), Span::dummy())),
    })), Span::dummy()))
}

/// 读取 `dyn Any` 的运行时类型标识：`FieldGet(FieldGet(x, 1 /*vtable*/), 0)`。
fn read_any_type_id(any: HirExpr) -> HirExpr {
    HirExpr::new(HirExprKind::FieldGet{
        base: Box::new(HirExpr::new(HirExprKind::FieldGet{
            base: Box::new(any),
            index: 1,
            ty: FieldScalar::Ptr,
        }, Span::dummy())),
        index: 0,
        ty: FieldScalar::Int,
    }, Span::dummy())
}

/// 断言实参为 `dyn Any`（G-M3 安全检查的类型前提）。
fn ensure_dyn_any(ty: &Type, who: &str, span: Span) -> Result<(), TypeError> {
    if let Type::Dyn(n) = ty {
        if is_any_trait(n) {
            return Ok(());
        }
    }
    Err(TypeError::Unsupported {
        what: format!("`{who}` 需要 `dyn Any` 实参，实际为 `{ty}`"),
        span,
    })
}

/// `Option` 枚举的槽布局（tag 槽 + payload 槽），与 `check_variant_construct` 一致。
fn option_layout(ctx: &TypeContext) -> (usize, bool) {
    match ctx.lookup_enum("Option") {
        Some(def) => {
            let slots = def.slot_count;
            (slots, enum_instance_by_value(ctx, "Option", slots))
        }
        None => (2, true),
    }
}

/// 构造 `Option` 值块：槽 0 = tag，槽 1 = payload（可选）。
fn make_option_block(
    ctx: &mut TypeContext,
    slots: usize,
    by_value: bool,
    tag: i128,
    payload: Option<HirExpr>,
) -> HirBlock {
    let base = ctx.fresh_temp();
    let mut stmts = vec![HirStmt::new(HirStmtKind::Let{
        name: base.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots,
            by_value,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy())];
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
        index: 0,
        value: Box::new(HirExpr::new(HirExprKind::IntLiteral(tag), Span::dummy())),
        ty: FieldScalar::Int,
    }, Span::dummy())), Span::dummy()));
    if let Some(p) = payload {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(base.clone()), Span::dummy())),
            index: 1,
            value: Box::new(p),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()));
    }
    HirBlock { span: Span::dummy(),
        stmts,
        final_expr: Some(HirExpr::new(HirExprKind::Variable(base), Span::dummy())),
    }
}

/// `any_type_id(x: dyn Any) -> i64`：读取擦除值携带的运行时类型标识（G-M2）。
pub(crate) fn check_any_type_id(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "any_type_id".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let (hir, ty) = infer_expr(ctx, &args[0])?;
    ensure_dyn_any(&ty, "any_type_id", span)?;
    Ok((read_any_type_id(hir), Type::I64))
}

/// `any_downcast_ref::<T>(x: dyn Any) -> Option<&T>`（G-M3 安全向下转换）。
///
/// 展开为 `if type_id(x) == type_id_of(T) { Some(data_ptr as &T) } else { None }`：
/// 类型标识相等才产出具 `&T` 的 `Some`，否则 `None`——无未检查转换，
/// 错误类型的向下转换不会产出悬垂/错误解释的引用。
pub(crate) fn check_any_downcast_ref(
    ctx: &mut TypeContext,
    args: &[AstExpr],
    type_args: &[AstType],
    span: Span,
) -> Result<(HirExpr, Type), TypeError> {
    if args.len() != 1 {
        return Err(TypeError::UnexpectedArgumentCount {
            name: "any_downcast_ref".to_string(),
            expected: 1,
            found: args.len(),
            span,
        });
    }
    let target = match type_args.first() {
        Some(t) => resolve_ast_type(ctx, t, span)?,
        None => {
            return Err(TypeError::Unsupported {
                what: "`any_downcast_ref` 需要显式类型参数：`any_downcast_ref::<T>(x)`".to_string(),
                span,
            })
        }
    };
    let (hir, ty) = infer_expr(ctx, &args[0])?;
    ensure_dyn_any(&ty, "any_downcast_ref", span)?;

    // 绑定到临时变量，避免实参为复杂表达式时重复求值（vtable 读 2 次、数据指针 1 次）。
    let any_var = ctx.fresh_temp();
    let cond = HirExpr::new(HirExprKind::Binary(
        HirBinaryOp::Eq,
        Box::new(read_any_type_id(HirExpr::new(HirExprKind::Variable(any_var.clone()), Span::dummy()))),
        Box::new(HirExpr::new(HirExprKind::IntLiteral(type_id_of(&target) as i128), Span::dummy())),
    ), Span::dummy());
    let (slots, by_value) = option_layout(ctx);
    let data_ptr = HirExpr::new(HirExprKind::FieldGet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(any_var.clone()), Span::dummy())),
        index: 0,
        ty: FieldScalar::Ptr,
    }, Span::dummy());
    let then_block = make_option_block(ctx, slots, by_value, 1, Some(data_ptr));
    let else_block = make_option_block(ctx, slots, by_value, 0, None);
    let ret_ty = Type::Named(
        "Option".to_string(),
        vec![Type::Ref(Box::new(target), Mutability::Immutable)],
    );
    Ok((
        HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
            stmts: vec![HirStmt::new(HirStmtKind::Let{
                name: any_var,
                init: hir,
                mutable: false,
            }, Span::dummy())],
            final_expr: Some(HirExpr::new(HirExprKind::If{
                cond: Box::new(cond),
                then_block: Box::new(then_block),
                else_block: Some(Box::new(else_block)),
            }, Span::dummy())),
        })), Span::dummy()),
        ret_ty,
    ))
}

pub(crate) fn coerce_to_dyn(
    ctx: &mut TypeContext,
    data_ptr: HirExpr,
    concrete: &Type,
    trait_name: &str,
    span: Span,
) -> Result<HirExpr, TypeError> {
    // G-M2（SH-P0-3）：`dyn Any` 类型擦除——内置 trait，不查 trait 声明，
    // vtable 槽 0 存具体类型标识（type_id），无方法表。
    if is_any_trait(trait_name) {
        return coerce_to_any(ctx, data_ptr, concrete);
    }
    let trait_def = ctx.trait_defs.get(trait_name).cloned().ok_or_else(|| {
        TypeError::UndefinedType {
            name: trait_name.to_string(),
            span,
        }
    })?;
    if !trait_def.type_params.is_empty() {
        return Err(TypeError::Unsupported {
            what: format!("`dyn {trait_name}`：泛型 trait 实例化（trait 对象泛型参数规划中）"),
            span,
        });
    }
    // 找 `impl Trait for 具体类型`（MVP：trait/impl 均非泛型，直接按 self_type 精确匹配）
    let impl_def = ctx
        .impl_defs
        .iter()
        .find(|d| d.trait_name.as_deref() == Some(trait_name) && d.self_type == *concrete)
        .cloned()
        .ok_or_else(|| TypeError::Unsupported {
            what: format!(
                "类型 `{concrete}` 未实现 trait `{trait_name}`，无法转换为 `dyn {trait_name}`"
            ),
            span,
        })?;
    if !impl_def.type_params.is_empty() {
        return Err(TypeError::Unsupported {
            what: format!("`dyn {trait_name}`：泛型 impl（`impl<T> {trait_name} for ...`）规划中"),
            span,
        });
    }
    // PC-10：vtable 方法槽按「supertrait 方法在前」线性化填充——保证 `protocol A: B` 时
    // `dyn A` 的 vtable **前缀**与 `dyn B` 一致，从而使 `dyn A → dyn B` 上转零开销复用。
    let trait_full = resolve_trait_def_name(ctx, trait_name);
    let methods = linearize_trait_methods(ctx, &trait_full);
    let n = methods.len();
    let mut stmts = Vec::new();
    // 1) vtable 数组：3 元槽（drop/size/align，MVP = 0）+ N 方法槽
    let vt = ctx.fresh_temp();
    stmts.push(HirStmt::new(HirStmtKind::Let{
        name: vt.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: 3 + n,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy()));
    for i in 0..3 {
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(vt.clone()), Span::dummy())),
            index: i,
            value: Box::new(HirExpr::new(HirExprKind::IntLiteral(0), Span::dummy())),
            ty: FieldScalar::Int,
        }, Span::dummy())), Span::dummy()));
    }
    // 2) 方法表：按线性化顺序填充；方法可声明于父协议，此时从其父协议 impl 取实现。
    let subst = HashMap::new();
    for (i, (owner, m)) in methods.iter().enumerate() {
        let owner_impl = if *owner == trait_full {
            impl_def.clone()
        } else {
            ctx.impl_defs
                .iter()
                .find(|d| {
                    d.trait_name.as_deref() == Some(owner.as_str()) && d.self_type == *concrete
                })
                .cloned()
                .ok_or_else(|| TypeError::Unsupported {
                    what: format!(
                        "类型 `{concrete}` 未实现父协议 `{owner}`，无法构造 `dyn {trait_name}` 的 vtable"
                    ),
                    span,
                })?
        };
        let impl_method = owner_impl
            .methods
            .iter()
            .find(|im| im.sig.name == m.name)
            .ok_or_else(|| TypeError::Unsupported {
                what: format!(
                    "`{owner}` 的 impl for `{concrete}` 缺少方法 `{}`",
                    m.name
                ),
                span,
            })?;
        let fn_name = instantiate_impl_method(ctx, &owner_impl, impl_method, &subst, span)?;
        let m_var = ctx.fresh_temp();
        stmts.push(HirStmt::new(HirStmtKind::Let{
            name: m_var.clone(),
            init: HirExpr::new(HirExprKind::FnPtr(fn_name), Span::dummy()),
            mutable: false,
        }, Span::dummy()));
        stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
            base: Box::new(HirExpr::new(HirExprKind::Variable(vt.clone()), Span::dummy())),
            index: 3 + i,
            value: Box::new(HirExpr::new(HirExprKind::Variable(m_var), Span::dummy())),
            ty: FieldScalar::Ptr,
        }, Span::dummy())), Span::dummy()));
    }
    // 3) 胖指针：槽 0 = 数据指针，槽 1 = vtable 指针
    let dyn_var = ctx.fresh_temp();
    stmts.push(HirStmt::new(HirStmtKind::Let{
        name: dyn_var.clone(),
        init: HirExpr::new(HirExprKind::Alloc{
            slots: 2,
            by_value: false,
            is_strfat: false,
        }, Span::dummy()),
        mutable: false,
    }, Span::dummy()));
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(dyn_var.clone()), Span::dummy())),
        index: 0,
        value: Box::new(data_ptr),
        ty: FieldScalar::Ptr,
    }, Span::dummy())), Span::dummy()));
    stmts.push(HirStmt::new(HirStmtKind::Semi(HirExpr::new(HirExprKind::FieldSet{
        base: Box::new(HirExpr::new(HirExprKind::Variable(dyn_var.clone()), Span::dummy())),
        index: 1,
        value: Box::new(HirExpr::new(HirExprKind::Variable(vt), Span::dummy())),
        ty: FieldScalar::Ptr,
    }, Span::dummy())), Span::dummy()));
    Ok(HirExpr::new(HirExprKind::Block(Box::new(HirBlock { span: Span::dummy(),
        stmts,
        final_expr: Some(HirExpr::new(HirExprKind::Variable(dyn_var), Span::dummy())),
    })), Span::dummy()))
}

pub(crate) fn type_mentions_self(ty: &Type) -> bool {
    match ty {
        // `Self::Item` 关联类型占位（trait 声明收集时无 impl 上下文，
        // 退化为 `Generic("Self::Item")`）同样视为 `Self` 提及。
        Type::Generic(n) => n == "Self" || n.starts_with("Self::"),
        Type::Ref(t, _) | Type::RawPtr(t, _) | Type::Array(t, _) => type_mentions_self(t),
        Type::Named(_, ps) | Type::Tuple(ps) => ps.iter().any(type_mentions_self),
        _ => false,
    }
}
