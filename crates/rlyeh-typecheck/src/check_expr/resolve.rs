//! 表达式检查子模块：符号解析与关联投影。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use super::*;

pub(super) fn resolve_callable(ctx: &TypeContext, name: &str) -> String {
    // `r#` 原始标识符：显式引用根命名空间（如 fs 模块内 `r#rename(...)`
    // 引用根 extern `rename`，不被 `fs::rename` 遮蔽）。去前缀后跳过
    // 模块内优先 / 用户遮蔽，直接绑定根命名空间符号（注册名已归一化）。
    if let Some(base) = name.strip_prefix("r#") {
        return base.to_string();
    }
    // 模块内函数引用：优先绑定当前模块内定义（`prefix::name`），
    // 避免子模块内部裸名调用被全局同名函数（含用户顶层覆盖）劫持。
    // 例：std `io.rl` 内部 `read(0, tmp, 256)` 必须绑定 `io::read`，
    // 用户顶层 `fn read(p: &i64)` 只影响用户自己的裸名调用。
    if !name.contains("::") && !ctx.module_prefix.is_empty() {
        let full = format!("{}::{}", ctx.module_prefix, name);
        if ctx.fn_signatures.contains_key(&full) {
            return full;
        }
    }
    // 顶层用户代码：被遮蔽的用户声明优先（std 原名保留供模块内部绑定）
    if !name.contains("::") && ctx.module_prefix.is_empty() {
        if let Some(m) = ctx.fn_shadow_of.get(name) {
            return m.clone();
        }
    }
    if ctx.fn_signatures.contains_key(name) {
        return name.to_string();
    }
    // 兜底：模块前缀 + 裸名（模块内自由函数互相引用）
    if !name.contains("::") && !ctx.module_prefix.is_empty() {
        let full = format!("{}::{}", ctx.module_prefix, name);
        if ctx.fn_signatures.contains_key(&full) {
            return full;
        }
    }
    // 经 use 别名（含 `pub use` 重导出的多级链，如 `r → outer::revealed →
    // inner::secret`）传递追踪到完整符号名；`resolve_full_name` 已做传递解析。
    ctx.resolve_full_name(name)
        .unwrap_or_else(|| name.to_string())
}

pub(super) fn split_variant_path(ctx: &TypeContext, resolved: &str) -> Option<(String, String)> {
    if let Some((en, vr)) = resolved.rsplit_once("::") {
        let en_full = ctx
            .resolve_full_name(en)
            .unwrap_or_else(|| en.to_string());
        if ctx.enum_defs.contains_key(&en_full) {
            return Some((en_full, vr.to_string()));
        }
    }
    ctx.resolve_variant(None, resolved)
}

pub(crate) fn resolve_ast_type(
    ctx: &TypeContext,
    ty: &AstType,
    span: Span,
) -> Result<Type, TypeError> {
    match ty {
        AstType::Path(name, args) => {
            // `Self::Item`：关联类型引用（U2）——impl 收集时查当前 assoc_types
            // 映射替换为具体类型；trait 声明收集时（无 impl 上下文）退化为
            // 占位 `Type::Generic("Self::Item")`（仅作记录，不参与实例化替换）。
            if let Some(member) = name.strip_prefix("Self::") {
                if let Some(t) = ctx.assoc_types.get(member) {
                    return Ok(t.clone());
                }
                return Ok(Type::Generic(name.clone()));
            }
            // W4/V3-A4 补全：关联类型投影 `F::Item` / `F::Output`。
            // - base 为当前泛型参数时：产出 `Type::AssocProjection`，实例化时替换求值。
            // - base 为已命名具体类型（如 `Range::Item`）时：立即经 `eval_assoc_projection`
            //   查该类型的 trait impl 的关联类型求值（V3-A4）。
            if args.is_empty() {
                if let Some((base, member)) = name.rsplit_once("::") {
                    let is_generic_param = ctx.type_params.iter().any(|p| p == base)
                        || ctx.generic_subst.contains_key(base);
                    if is_generic_param {
                        // base 为泛型参数：模板收集期 type_params 含 base；实例化期
                        // `cloned.generics` 被清空（type_params=[]）但 generic_subst 含 base
                        //（instantiate_generic_fn 克隆后解析签名），两者任一命中即视为投影。
                        return resolve_assoc_projection(ctx, base, member, span);
                    }
                    // base 为命名具体类型：`Range::Item` 立即求值（查 impl 的 assoc_types）。
                    let base_full = ctx
                        .resolve_full_name(base)
                        .unwrap_or_else(|| base.to_string());
                    if let Some(resolved) =
                        eval_assoc_projection(ctx, &Type::Named(base_full, vec![]), member)
                    {
                        return Ok(resolved);
                    }
                }
            }
            // `str`：字符串类型关键字（`&str` 引用切片类型的一部分；G2）
            if name == "str" && args.is_empty() {
                return Ok(Type::Str);
            }
            // 带泛型实参的类型也解析完整名（use 别名 / 模块前缀），与 args 为空时
            // 一致（S1 修复：`Poll<i64>` 注解与 `Poll::Ready` 构造的路径一致性——
            // 裸名不做别名展开会产生 `Poll` vs `future::Poll` 的错配）。
            if args.is_empty() {
                ctx.resolve_named_type(name, span)
            } else {
                // 带泛型实参的类型也解析完整名（use 别名 / 模块前缀），与 args 为空
                // 时一致（S1 修复：`Poll<i64>` 注解与 `Poll::Ready` 构造的路径一致性——
                // 裸名不做别名展开会产生 `Poll` vs `future::Poll` 的错配）。
                let full = ctx
                    .resolve_full_name(name)
                    .unwrap_or_else(|| name.to_string());
                let mut resolved = Vec::with_capacity(args.len());
                for a in args {
                    resolved.push(resolve_ast_type(ctx, a, span)?);
                }
                Ok(Type::Named(full, resolved))
            }
        }
        AstType::Ref(inner, is_mut) => {
            let inner = resolve_ast_type(ctx, inner, span)?;
            // B-6（P3）：`#[memory(gc)]` 模块内引用默认映射为 `Gc<T>`（L3），
            // 指针图 / 树结构免写 `&`/`&mut`（`next: &Node` ≡ `next: Gc<Node>`）。
            if ctx.in_gc_module() {
                return Ok(Type::Named("Gc".to_string(), vec![inner]));
            }
            let m = if *is_mut {
                Mutability::Mutable
            } else {
                Mutability::Immutable
            };
            Ok(Type::Ref(Box::new(inner), m))
        }
        AstType::RawPtr(inner, is_mut) => {
            let inner = resolve_ast_type(ctx, inner, span)?;
            Ok(Type::RawPtr(Box::new(inner), *is_mut))
        }
        // trait 对象（H4）：`dyn Trait` → `Type::Dyn(完整 trait 名)`。
        // 布局为 2 槽胖指针（数据指针 + vtable 指针），转换与调用见
        // `coerce_to_dyn` / `check_method_call` 的 Dyn 分支。
        AstType::Dyn(name) => {
            // G-M2（SH-P0-3）：`dyn Any` 为编译器内置的类型擦除标签，不要求
            // trait 声明存在。统一归一为 `Any`，使 `is_any_trait` 在各处
            // （coerce_to_dyn / any_type_id / any_downcast_ref）稳定匹配。
            if is_any_trait(name) {
                return Ok(Type::Dyn("Any".to_string()));
            }
            // 复用 resolve_trait_key：支持裸名 / 模块前缀 / use 别名 / `::Name` 结尾
            // 定位（与 collect_impl 一致）。
            // P7d-1（2026-08-29）：若未找到且当前正在收集同名 trait（自引用 trait，
            // 如 `trait Error { fn source(&self) -> Option<&dyn Error> }`），回退到自身。
            if let Some(full) = ctx.resolve_trait_key(name) {
                Ok(Type::Dyn(full))
            } else if let Some(cur) = &ctx.collecting_trait {
                let bare = name.rsplit("::").next().unwrap_or(name);
                let cur_bare = cur.rsplit("::").next().unwrap_or(cur);
                if bare == cur_bare {
                    Ok(Type::Dyn(cur.clone()))
                } else {
                    Err(TypeError::UndefinedType {
                        name: name.clone(),
                        span,
                    })
                }
            } else {
                Err(TypeError::UndefinedType {
                    name: name.clone(),
                    span,
                })
            }
        }
        AstType::Tuple(ts) => {
            // X4：空元组 `()` → 单元类型 `Type::Unit`（`Result<(), FmtError>` 的 `()`
            // 作为泛型实参可解析；空 tuple 值构造见 `ExprKind::Unit`）。
            if ts.is_empty() {
                return Ok(Type::Unit);
            }
            let mut resolved = Vec::with_capacity(ts.len());
            for t in ts {
                resolved.push(resolve_ast_type(ctx, t, span)?);
            }
            Ok(Type::Tuple(resolved))
        }
        AstType::Array(inner, size) => {
            let inner = resolve_ast_type(ctx, inner, span)?;
            match size {
                // 定长数组 `[T; N]`：大小仅支持整数字面量（MVP）
                Some(e) => match *e.kind {
                    ExprKind::IntLiteral(v) if v >= 0 => Ok(Type::Array(Box::new(inner), v as usize)),
                    _ => Err(TypeError::Unsupported {
                        what: "数组大小非常量整数字面量在 MVP 阶段".to_string(),
                        span,
                    }),
                },
                // 切片 `[T]`（无大小）→ 切片类型 `&[T]` / `&mut [T]` 的元素类型
                None => Ok(Type::Slice(Box::new(inner))),
            }
        }
        AstType::Fn(params, ret) => {
            let params = params
                .iter()
                .map(|p| resolve_ast_type(ctx, p, span))
                .collect::<Result<Vec<_>, _>>()?;
            let ret = resolve_ast_type(ctx, ret, span)?;
            Ok(Type::Fn(Box::new(FnSignature {
                param_spans: vec![span; params.len()],
                params,
                return_type: ret,
            })))
        }
        // U1：类型联合 `A | B | ...`——解析各成员后校验两两互不相交
        AstType::Union(members) => {
            let resolved = members
                .iter()
                .map(|m| resolve_ast_type(ctx, m, span))
                .collect::<Result<Vec<_>, _>>()?;
            check_union_disjoint(&resolved, span)?;
            Ok(Type::Union(resolved))
        }
        AstType::Infer => Ok(Type::Infer),
    }
}

/// U1：校验类型联合成员**两两互不相交**（"受限制"的核心约束）。
///
/// 互不相交保证 tag 判别无歧义、`match` 收窄安全。不同具名类型 / 枚举成员
/// 视为不相交（MVP 不引入子类型）。冲突时报 `UnionMembersNotDisjoint`。
fn check_union_disjoint(members: &[Type], span: Span) -> Result<(), TypeError> {
    for (i, a) in members.iter().enumerate() {
        for b in &members[i + 1..] {
            if let Some(why) = union_overlap_reason(a, b) {
                return Err(TypeError::UnionMembersNotDisjoint {
                    first: a.to_string(),
                    second: b.to_string(),
                    why,
                    span,
                });
            }
        }
    }
    Ok(())
}

/// 判定两个联合成员是否重叠：返回 `Some(冲突原因)` 表示重叠（不相交返回 `None`）。
fn union_overlap_reason(a: &Type, b: &Type) -> Option<String> {
    if a == b {
        return Some("重复成员".to_string());
    }
    // 数值类型互通（`is_numeric()` 双向兼容；含 `i64 | isize` 的平台相关重叠）
    if a.is_numeric() && b.is_numeric() {
        return Some("数值类型互通（宽度 / 平台相关重叠）".to_string());
    }
    // 引用可变性重叠：`&T | &mut T`（`&mut T` 可降级为 `&T`，判别有歧义）
    if let (Type::Ref(ia, _), Type::Ref(ib, _)) = (a, b) {
        if ia == ib {
            return Some("引用可变性重叠（`&T` 与 `&mut T`）".to_string());
        }
    }
    // 裸指针可变性重叠：`*const T | *mut T`
    if let (Type::RawPtr(ia, _), Type::RawPtr(ib, _)) = (a, b) {
        if ia == ib {
            return Some("裸指针可变性重叠（`*const T` 与 `*mut T`）".to_string());
        }
    }
    // 嵌套联合：展开后任一成员与另一侧重叠，即视为重叠
    if let (Type::Union(us), other) | (other, Type::Union(us)) = (a, b) {
        for u in us {
            if let Some(why) = union_overlap_reason(u, other) {
                return Some(format!("嵌套联合成员重叠（{why}）"));
            }
        }
    }
    None
}

pub(super) fn resolve_assoc_projection(
    ctx: &TypeContext,
    base: &str,
    member: &str,
    _span: Span,
) -> Result<Type, TypeError> {
    if let Some(concrete) = ctx.generic_subst.get(base) {
        if let Some(t) = eval_assoc_projection(ctx, concrete, member) {
            return Ok(t);
        }
        // base 已实例化但找不到该类型的关联类型实现：保留投影（防御性，
        // 通常实例化前必能求值）。
        return Ok(Type::AssocProjection {
            base: Box::new(concrete.clone()),
            assoc: member.to_string(),
        });
    }
    Ok(Type::AssocProjection {
        base: Box::new(Type::Generic(base.to_string())),
        assoc: member.to_string(),
    })
}

pub(super) fn eval_assoc_projection(ctx: &TypeContext, base: &Type, member: &str) -> Option<Type> {
    // U8 补全：按 self_type 名匹配（忽略泛型实参——泛型 impl `impl<T> Future
    // for GenOut<T>` 的 self_type 实参为 `Generic("T")`，与具体实例实参 `String`
    // 不相等，`compatible_with` 会误判不匹配），再用 base 具体实参替换 assoc_types
    // 里的 `Generic("T")` 占位求值。
    let Type::Named(base_name, base_args) = base else {
        // base 非 Named（引用等）：退回按名 + 宽松匹配
        for imp in &ctx.impl_defs {
            if imp.self_type.compatible_with(base) {
                for (name, ty) in &imp.assoc_types {
                    if name == member {
                        return Some(ty.clone());
                    }
                }
            }
        }
        return None;
    };
    for imp in &ctx.impl_defs {
        let Type::Named(impl_name, self_args) = &imp.self_type else {
            continue;
        };
        if impl_name != base_name {
            continue;
        }
        // 绑泛型参数：self_args[i] = Generic(tp) → base_args[i]
        let mut subst: HashMap<String, Type> = HashMap::new();
        for (i, tp) in imp.type_params.iter().enumerate() {
            if i < self_args.len()
                && i < base_args.len()
                && self_args[i] == Type::Generic(tp.clone())
            {
                subst.insert(tp.clone(), base_args[i].clone());
            }
        }
        for (name, ty) in &imp.assoc_types {
            if name == member {
                let resolved = if subst.is_empty() {
                    ty.clone()
                } else {
                    substitute(ty, &subst)
                };
                return Some(resolved);
            }
        }
    }
    None
}
