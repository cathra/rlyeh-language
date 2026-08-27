//! 顶层项检查（函数签名收集 + 函数体 / const 检查 + 模块 / use 支持
//! + enum / trait / impl 收集）。

use rlyeh_ast::{
    AstActorDecl, AstEnumDecl, AstFnDecl, AstImplBlock, AstItem, AstModDecl, AstProgram,
    AstStructDecl, AstTraitDecl, AstUseDecl,
};
use rlyeh_hir::{
    FieldScalar, HirBinaryOp, HirBlock, HirConstDecl, HirExpr, HirFnDecl, HirItem, HirItemKind,
    HirParam, HirProgram, HirStmt,
};
use rlyeh_lexer::Span;

use crate::check_expr::{
    check_block, check_block_inner, fix_deferred_closure_with_sig, infer_expr, resolve_ast_type,
    try_closure_value_as_fn,
};
use crate::context::{FnTemplate, TypeContext};
use crate::error::TypeError;
use crate::types::{
    field_scalar_of, EnumDef, FnSignature, ImplDef, ImplMethod, MethodSig, Mutability, StructDef,
    TraitDef, Type, VariantDef,
};

/// 类型检查完整程序。
///
/// 两遍流程：
/// 1. 收集结构体 / 枚举 / trait / impl 定义、类型别名与所有函数签名
///    （支持函数间互调），并注册 `import` 导入别名；
/// 2. 检查函数体与 const 初始值，生成 HIR（模块项以 `mod::item` 扁平化命名）。
///
/// 泛型函数 / 泛型方法在调用点实例化，实例化产生的函数项追加到输出末尾。
pub fn typecheck(program: &AstProgram) -> Result<HirProgram, TypeError> {
    typecheck_with_region_hints(program, &Default::default())
}

/// 类型检查完整程序，并注入 L3 PGO 回灌提示（区域名 → 推荐初始容量）。
///
/// 无提示时等价于 [`typecheck`]；`rlyeh build --profile` 编译路径使用本入口。
pub fn typecheck_with_region_hints(
    program: &AstProgram,
    region_hints: &std::collections::HashMap<String, usize>,
) -> Result<HirProgram, TypeError> {
    let mut ctx = TypeContext::new();
    ctx.region_hints = region_hints.clone();
    collect_declarations(&mut ctx, program)?;
    // 第二遍：所有 struct 名注册后解析字段（支持自引用/前向引用递归类型）。
    resolve_all_struct_fields(&mut ctx, program)?;

    let mut items = Vec::new();
    for item in &program.items {
        check_item(&mut ctx, item, "", &mut items)?;
    }
    // 泛型实例化产生的函数项追加到末尾
    items.append(&mut ctx.mono_items);
    Ok(HirProgram { items })
}

/// 拼接模块前缀与名称（`mod::name`），顶层直接返回原名。
fn full_name(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}::{name}")
    }
}

/// 第一遍：收集结构体 / 函数签名 / use 导入（递归处理嵌套模块）。
fn collect_declarations(ctx: &mut TypeContext, program: &AstProgram) -> Result<(), TypeError> {
    for item in &program.items {
        collect_item_decls(ctx, item, "")?;
    }
    Ok(())
}

/// 收集单个项的声明（带模块前缀）。
fn collect_item_decls(
    ctx: &mut TypeContext,
    item: &AstItem,
    prefix: &str,
) -> Result<(), TypeError> {
    match item {
        AstItem::StructDecl(s) => collect_struct(ctx, s, prefix)?,
        AstItem::FnDecl(f) => {
            if !f.generics.is_empty() {
                // 泛型函数注册为模板，调用点按实参实例化
                let sig = fn_signature(ctx, f, f.span)?;
                ctx.fn_templates.insert(
                    full_name(prefix, &f.name),
                    FnTemplate {
                        name: f.name.clone(),
                        type_params: f.generics.iter().map(|p| p.name.clone()).collect(),
                        bounds: f
                            .generics
                            .iter()
                            .filter(|p| !p.bounds.is_empty())
                            .map(|p| (p.name.clone(), p.bounds.clone()))
                            .collect(),
                        sig,
                        ast: (**f).clone(),
                    },
                );
            } else {
                let sig = fn_signature(ctx, f, f.span)?;
                let full = full_name(prefix, &f.name);
                // 顶层裸名与已注册函数（std 预置根函数 / 用户先声明者）重名：
                // 签名不同 → 后声明者 mangle 重命名（`read@shadow<N>`），先声明者保留原名。
                // std 根函数（如 extern `read`）被用户顶层 `fn read` 遮蔽时，
                // std 原名留在表中供 std 模块内部裸名调用绑定；用户顶层代码
                // 经 `fn_shadow_of` 绑定自身版本；下游符号名因此唯一不冲突。
                // 签名相同 → 视为无害重声明（FFI 惯用法：用户重复 extern 声明
                // 同一符号，如 `extern fn __rlyeh_target_os() -> i32`），保留原名。
                if prefix.is_empty()
                    && ctx.fn_signatures.get(&full).is_some_and(|existing| existing != &sig)
                {
                    let seq = ctx.fn_shadow_seq.get(&f.name).copied().unwrap_or(0) + 1;
                    ctx.fn_shadow_seq.insert(f.name.clone(), seq);
                    let mangled = format!("{}@shadow{}", f.name, seq);
                    ctx.fn_decl_shadow.insert((f.span.start, f.span.end), mangled.clone());
                    ctx.fn_shadow_of.insert(f.name.clone(), mangled.clone());
                    ctx.insert_fn_signature(mangled, sig);
                } else {
                    ctx.insert_fn_signature(full, sig);
                }
            }
        }
        AstItem::EnumDecl(e) => collect_enum(ctx, e, prefix)?,
        AstItem::TraitDecl(t) => collect_trait(ctx, t, prefix)?,
        AstItem::ImplBlock(imp) => collect_impl(ctx, imp, prefix)?,
        AstItem::ActorDecl(a) => collect_actor(ctx, a, prefix)?,
        AstItem::ModDecl(m) => {
            let new_prefix = full_name(prefix, &m.name);
            // Q3a 修复：模块内符号（struct/trait/impl）的短名解析须感知模块前缀。
            // 模块内 trait/impl 方法签名在收集阶段即 resolve_ast_type（如
            // `fmt/module.rl` 的 `trait Display { fn fmt(&self, f: &mut Formatter) }`），
            // 此时文件后部的 use 段尚未注册 use_aliases，须按 `mod::Name` 前缀回退。
            let old_prefix = std::mem::replace(&mut ctx.module_prefix, new_prefix.clone());
            for inner in &m.items {
                collect_item_decls(ctx, inner, &new_prefix)?;
            }
            ctx.module_prefix = old_prefix;
        }
        AstItem::UseDecl(u) => register_use(ctx, u)?,
        // 收集阶段注册模块常量（供函数体 / 其它 const 引用）
        AstItem::ConstDecl(c) => {
            let (value, ty) = infer_expr(ctx, &c.value)?;
            ctx.insert_constant(full_name(prefix, &c.name), value, ty);
        }
        _ => {}
    }
    Ok(())
}

/// 将 typecheck 类型序列化为 extern 签名类型名（LIR 侧解析为 `LirType`）。
/// MVP 仅支持标量类型与单元类型；引用参数视为标量指针（按值传递数字）。
pub(crate) fn type_to_extern_name(ty: &Type) -> String {
    match ty {
        Type::I8 => "i8".into(),
        Type::I16 => "i16".into(),
        Type::I32 => "i32".into(),
        Type::I64 => "i64".into(),
        Type::ISize => "isize".into(),
        Type::U8 => "u8".into(),
        Type::U16 => "u16".into(),
        Type::U32 => "u32".into(),
        Type::U64 => "u64".into(),
        Type::USize => "usize".into(),
        Type::F32 => "f32".into(),
        Type::F64 => "f64".into(),
        Type::Bool => "bool".into(),
        Type::Char => "char".into(),
        Type::Unit => "()".into(),
        Type::Ref(inner, _) => format!("&{inner}"),
        t => t.to_string(),
    }
}

/// 注册 use 导入别名（`use path::to::item [as alias];`）。
///
/// MVP 限制：路径从根开始解析；暂不支持 glob 导入（`use a::*;`）。
fn register_use(ctx: &mut TypeContext, u: &AstUseDecl) -> Result<(), TypeError> {
    // `r#` 前缀（关键字转义 / 根命名空间显式引用标记）在符号注册时归一化，
    // 与 extern 声明注册名保持一致（`use r#rename` → 目标 "rename"）。
    let norm = |s: &str| s.strip_prefix("r#").unwrap_or(s).to_string();
    let path = u
        .path
        .iter()
        .map(|s| norm(s))
        .collect::<Vec<String>>()
        .join("::");
    if u.path.last().map(String::as_str) == Some("*") {
        return Err(TypeError::Unsupported {
            what: "glob 导入 use a::*".to_string(),
            span: u.span,
        });
    }
    let local = match &u.alias {
        Some(a) => norm(a),
        None => u.path.last().map(|s| norm(s)).unwrap_or_default(),
    };
    ctx.insert_use_alias(local, path);
    Ok(())
}

/// 第二遍：检查函数体 / const，生成 HIR 项（递归处理嵌套模块）。
pub(crate) fn check_item(
    ctx: &mut TypeContext,
    item: &AstItem,
    prefix: &str,
    out: &mut Vec<HirItem>,
) -> Result<(), TypeError> {
    // K4：GC 运行时 extern 声明（程序级去重，有 item 即生成；
    // `Gc::new` / `gc_region` 依赖，未使用也无 harm——LLVM declare 未引用符号不报错）
    emit_gc_runtime_externs(ctx, out);
    match item {
        AstItem::FnDecl(f) => {
            // 泛型函数仅在调用点实例化（无调用则不生成代码）
            if !f.generics.is_empty() {
                return Ok(());
            }
            // 函数体中的裸名（函数 / 常量）优先解析到当前模块
            let old_prefix = std::mem::replace(&mut ctx.module_prefix, prefix.to_string());
            let body = check_fn_body(ctx, f)?;
            ctx.module_prefix = old_prefix;
            let params = f
                .params
                .iter()
                .map(|p| HirParam {
                    name: p.name.clone(),
                })
                .collect();
            // extern 声明：序列化签名（参数类型名 + 返回类型名）供 LIR 解析
            let extern_sig = if f.is_extern {
                let sig = crate::check_item::fn_signature_with_self(ctx, f, None, f.span)?;
                Some((
                    sig.params
                        .iter()
                        .map(type_to_extern_name)
                        .collect::<Vec<_>>(),
                    type_to_extern_name(&sig.return_type),
                ))
            } else {
                None
            };
            // 被遮蔽重命名的声明按收集阶段的记录使用 mangle 名（两遍按 Span 对齐），
            // 保证 HIR 符号名与 fn_signatures / 调用解析结果一致。
            let item_name = ctx
                .fn_decl_shadow
                .get(&(f.span.start, f.span.end))
                .cloned()
                .unwrap_or_else(|| full_name(prefix, &f.name));
            out.push(HirItem {
                name: item_name,
                kind: HirItemKind::Fn(HirFnDecl {
                    params,
                    body,
                    is_extern: f.is_extern,
                    extern_sig,
                }),
            });
        }
        AstItem::ConstDecl(c) => {
            let old_prefix = std::mem::replace(&mut ctx.module_prefix, prefix.to_string());
            let (value, ty) = infer_expr(ctx, &c.value)?;
            ctx.module_prefix = old_prefix;
            ctx.insert_constant(full_name(prefix, &c.name), value.clone(), ty);
            out.push(HirItem {
                name: full_name(prefix, &c.name),
                kind: HirItemKind::Const(HirConstDecl { value }),
            });
        }
        AstItem::ModDecl(m) => {
            let new_prefix = full_name(prefix, &m.name);
            // 与收集阶段一致：模块内短名解析感知模块前缀（Q3a）
            let old_prefix = std::mem::replace(&mut ctx.module_prefix, new_prefix.clone());
            for inner in &m.items {
                check_item(ctx, inner, &new_prefix, out)?;
            }
            ctx.module_prefix = old_prefix;
        }
        // actor：展开为状态初始化函数 + 方法函数 + dispatch handle + runtime extern 声明
        AstItem::ActorDecl(a) => expand_actor(ctx, a, prefix, out)?,
        // use 导入在收集阶段（第一遍）已注册；其余项 MVP 阶段不生成 HIR
        AstItem::UseDecl(_) | AstItem::StructDecl(_) | AstItem::TraitDecl(_)
        | AstItem::ImplBlock(_) | AstItem::EnumDecl(_)
        | AstItem::MacroDecl(_) | AstItem::Statement(_) => {}
    }
    Ok(())
}

/// 收集结构体字段定义。
///
/// 泛型结构体（`struct Vec<T>`）在解析字段类型时，`T` 应处于类型参数作用域内
/// （与 `collect_enum` / `collect_impl` 保持一致）。
/// 收集 struct 声明（第一遍：仅注册名与泛型参数，字段类型延迟到
/// `resolve_all_struct_fields` 第二遍解析——支持 struct 自引用 / 前向引用
/// （如 `struct Node { next: Box<Node> }` 递归类型，为 W6 递归 async fn
/// 提供类型层地基）。

/// 第二遍：解析所有 struct 字段类型（所有 struct 名已注册，支持自引用/前向引用）。
///
/// 在 `collect_declarations` 完成后、`check_item` 之前调用。遍历顶层与嵌套
/// 模块的 struct 声明，重建 `type_params`/`module_prefix` 上下文后解析字段。


/// 收集 actor 定义（查重 + 字段/方法 MVP 限制校验 + 注册）。

/// MVP 阶段 actor 字段允许的标量类型。

/// actor 展开（check 阶段）：生成状态初始化函数 + 方法函数 + dispatch handle
/// 与 `rlyeh_actor_*` runtime extern 声明，全部为普通 HirItem，
/// 下游 MIR / LIR / codegen 复用现有机制。

/// 递归构造 dispatch：`if kind == i { return m_i(self, a, b, c); } else { ... }`，
/// 方法耗尽时尾表达式为 `-1`（未命中方法 → 崩溃信号 u64::MAX）。

/// 检查 actor 方法体：`self` 隐式绑定状态指针（类型为 actor 名），
/// 参数入作用域，检查块，返回类型一致性（方法与生成函数均返回 i64）。

/// 生成 `rlyeh_actor_*` runtime extern 声明（程序级去重，多 actor 只生成一份）。

/// 生成 `rlyeh_gc_*` runtime extern 声明（程序级去重，K4 追踪 GC）。
///
/// 类型名：`Ptr` 经 LIR `parse_extern_type` 解析为指针（未知名默认 Ptr）；
/// `rlyeh_gc_alloc` 返回堆块基址（指针），`rlyeh_gc_escape` 接收 Gc 对象指针。

/// 收集枚举定义（变体 + 字段类型 + 对象槽数布局）。

/// 收集 trait 定义（抽象方法签名）。

/// 收集 impl 块（inherent 或 trait impl），方法保存原始 AST 供调用点实例化。

/// 仅收集函数签名（不检查函数体），供增量编译提取模块接口。
///
/// 递归处理嵌套模块（符号名带 `mod::` 前缀）。输出按函数名排序，
/// 保证接口哈希稳定。

/// 递归收集模块内的类型声明（结构体 / 枚举 / trait / impl）。
///
/// 模块前缀与 `collect_item_decls` 一致地**逐级累积**（`io::error::IoErrorKind`），
/// 否则 `impl` 方法签名在收集时经 `resolve_ast_type` 解析 `mod::Type` 引用会
/// 因注册名缺前缀（`error::IoErrorKind`）而报 UndefinedType（S1 修复，2026-08）。


/// 递归收集模块内的结构体。
/// 递归收集模块内的函数签名。

/// 从函数声明解析签名（泛型参数名在签名内解析为 [`Type::Generic`]；
/// 若上下文存在泛型替换表则解析为替换后的具体类型）。

/// 解析函数 / 方法签名；`self_ty` 提供时，名为 `self` 的参数直接取该类型
/// （用于 impl 方法实例化，`&self` 的引用层级在传入前已确定）。

/// 检查函数体：参数入作用域，检查块，返回类型一致性。

/// 检查函数体（支持 `self` 参数，供 impl 方法实例化使用）。


mod collect;
mod actor;
mod fn_sig;

use collect::*;
use actor::*;
use fn_sig::*;

// 对外 API：被 lib.rs / check_expr / check_stmt 经 `crate::check_item::` 访问
pub use fn_sig::collect_fn_signatures;
pub(crate) use fn_sig::{fn_signature_with_self, check_fn_body_with_self};
