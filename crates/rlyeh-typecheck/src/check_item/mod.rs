//! 顶层项检查（函数签名收集 + 函数体 / const 检查 + 模块 / use 支持
//! + enum / protocol / impl 收集）。

use rlyeh_ast::{
    AstActorDecl, AstEnumDecl, AstFnDecl, AstImplBlock, AstItem, AstModDecl, AstProgram,
    AstStructDecl, AstProtocolDecl, AstUseDecl, AstUseMember,
};
use rlyeh_hir::{
    FieldScalar, HirBinaryOp, HirBlock, HirConstDecl, HirExpr, HirExprKind, HirFnDecl, HirItem,
    HirItemKind, HirParam, HirProgram, HirStmt,
};
use rlyeh_lexer::Span;

use crate::check_expr::{
    check_block, check_block_inner, fix_deferred_closure_with_sig, infer_expr, resolve_ast_type,
    try_closure_value_as_fn,
};
use crate::context::{FnTemplate, TypeContext};
use crate::error::TypeError;
use crate::{ConstValue, GlobalDecl};
use crate::Warning;
use crate::types::{
    field_scalar_of, EnumDef, FnSignature, ImplDef, ImplMethod, MethodSig, Mutability, StructDef,
    ProtocolDef, Type, VariantDef,
};

/// 类型检查完整程序。
///
/// 两遍流程：
/// 1. 收集结构体 / 枚举 / protocol / impl 定义、类型别名与所有函数签名
///    （支持函数间互调），并注册 `import` 导入别名；
/// 2. 检查函数体与 const 初始值，生成 HIR（模块项以 `mod::item` 扁平化命名）。
///
/// 泛型函数 / 泛型方法在调用点实例化，实例化产生的函数项追加到输出末尾。
pub fn typecheck(program: &AstProgram) -> Result<(HirProgram, Vec<Warning>), TypeError> {
    typecheck_with_region_hints(program, &Default::default(), 0, crate::VisibilityMode::Off)
        .map(|(hir, w, _globals)| (hir, w))
}

/// 类型检查完整程序，并注入 L3 PGO 回灌提示（区域名 → 推荐初始容量）。
///
/// 无提示时等价于 [`typecheck`]；`rlyeh build --profile` 编译路径使用本入口。
pub fn typecheck_with_region_hints(
    program: &AstProgram,
    region_hints: &std::collections::HashMap<String, usize>,
    prelude_len: usize,
    visibility: crate::VisibilityMode,
) -> Result<(HirProgram, Vec<Warning>, Vec<GlobalDecl>), TypeError> {
    let mut ctx = TypeContext::new();
    ctx.region_hints = region_hints.clone();
    ctx.prelude_len = prelude_len;
    ctx.visibility = visibility;
    collect_declarations(&mut ctx, program)?;
    // PC-4：父协议一致性校验（`protocol A: B` → 实现 A 的类型必须同时实现 B）。
    crate::check_expr::validate_superprotocols(&ctx)?;
    // 第二遍：所有 struct 名注册后解析字段（支持自引用/前向引用递归类型）。
    resolve_all_struct_fields(&mut ctx, program)?;

    let mut items = Vec::new();
    // 全局变量（`static` / `static mut`）声明：收集为 GlobalDecl 透传至 codegen。
    let mut globals: Vec<GlobalDecl> = Vec::new();
    for item in &program.items {
        check_item(&mut ctx, item, "", &mut items, &mut globals)?;
    }
    // B：全部收集完成后延迟校验 import 目标符号 / 模块是否存在（规避模块收集时序误报）。
    verify_imports(&mut ctx)?;
    // 泛型实例化产生的函数项追加到末尾
    items.append(&mut ctx.mono_items);
    Ok((HirProgram { items }, ctx.warnings.clone(), globals))
}

/// 拼接模块前缀与名称（`mod::name`），顶层直接返回原名。
fn full_name(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}::{name}")
    }
}

/// 将编译期常量表达式求值为 [`crate::ConstValue`]，用于 `static` 初始值发射。
/// 仅支持字面量及其算术组合（i64 / f64 / bool / char）；其余返回 `None`。
pub(crate) fn eval_const_value(expr: &HirExpr) -> Option<ConstValue> {
    use rlyeh_hir::{HirBinaryOp, HirUnaryOp};
    match &expr.kind {
        HirExprKind::IntLiteral(i) => Some(ConstValue::I64(*i as i64)),
        HirExprKind::FloatLiteral(f) => Some(ConstValue::F64(*f)),
        HirExprKind::BoolLiteral(b) => Some(ConstValue::Bool(*b)),
        HirExprKind::CharLiteral(c) => Some(ConstValue::Char(*c as i8)),
        HirExprKind::Unary(op, e) => {
            let v = eval_const_value(e)?;
            match (*op, v) {
                (HirUnaryOp::Neg, ConstValue::I64(x)) => Some(ConstValue::I64(-x)),
                (HirUnaryOp::Neg, ConstValue::F64(x)) => Some(ConstValue::F64(-x)),
                _ => None,
            }
        }
        HirExprKind::Binary(op, a, b) => {
            let av = eval_const_value(a)?;
            let bv = eval_const_value(b)?;
            match (*op, av, bv) {
                (HirBinaryOp::Add, ConstValue::I64(x), ConstValue::I64(y)) => {
                    Some(ConstValue::I64(x.wrapping_add(y)))
                }
                (HirBinaryOp::Sub, ConstValue::I64(x), ConstValue::I64(y)) => {
                    Some(ConstValue::I64(x.wrapping_sub(y)))
                }
                (HirBinaryOp::Mul, ConstValue::I64(x), ConstValue::I64(y)) => {
                    Some(ConstValue::I64(x.wrapping_mul(y)))
                }
                (HirBinaryOp::Div, ConstValue::I64(x), ConstValue::I64(y)) => {
                    Some(ConstValue::I64(x.wrapping_div(y)))
                }
                (HirBinaryOp::Add, ConstValue::F64(x), ConstValue::F64(y)) => Some(ConstValue::F64(x + y)),
                (HirBinaryOp::Sub, ConstValue::F64(x), ConstValue::F64(y)) => Some(ConstValue::F64(x - y)),
                (HirBinaryOp::Mul, ConstValue::F64(x), ConstValue::F64(y)) => Some(ConstValue::F64(x * y)),
                (HirBinaryOp::Div, ConstValue::F64(x), ConstValue::F64(y)) => Some(ConstValue::F64(x / y)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// 收集声明：分三遍以正确处理类型别名与函数签名间的引用顺序。
///
/// - 第一遍：结构体 / 枚举 / protocol / impl / actor / use / const / 模块
///   （**不含**函数签名与类型别名），先登记全部类型名。
/// - 第二遍：类型别名（不动点循环支持别名→别名前向引用；目标可引用第一遍
///   已登记的类型名）。
/// - 第三遍：函数签名（此时别名已登记，签名中的别名类型可正确解析）。
fn collect_declarations(ctx: &mut TypeContext, program: &AstProgram) -> Result<(), TypeError> {
    // 第一遍：结构 / 枚举 / protocol / impl / actor / use / const / 模块
    // （不含 fn 签名与 type 别名）
    for item in &program.items {
        collect_item_decls(ctx, item, "")?;
    }
    // 第二遍：类型别名（支持别名→别名前向引用；目标可引用第一遍已登记类型名）
    collect_type_aliases_pass(ctx, &program.items, "")?;
    // 第三遍：函数签名（此时别名已登记，签名中的别名类型可正确解析）
    for item in &program.items {
        collect_fn_sigs_pass(ctx, item, "")?;
    }
    Ok(())
}

/// 第三遍：收集顶层函数签名（类型别名已在第二遍登记，签名中的别名可正确解析）。
///
/// 仅处理顶层 `fn`；模块内函数签名由各自模块收集路径处理（保持既有行为不变）。
/// 逻辑与原 `collect_item_decls` 的 `FnDecl` 分支一致，仅拆出以调整收集时序。
fn collect_fn_sigs_pass(ctx: &mut TypeContext, item: &AstItem, prefix: &str) -> Result<(), TypeError> {
    match item {
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
                    ctx.insert_fn_signature(mangled.clone(), sig);
                    if f.is_extern {
                        ctx.extern_fns.insert(mangled);
                    }
                } else {
                    ctx.insert_fn_signature(full.clone(), sig);
                    if f.is_extern {
                        ctx.extern_fns.insert(full);
                    }
                }
                if f.is_pub {
                    ctx.pub_symbols.insert(full_name(prefix, &f.name));
                }
            }
        }
        AstItem::ModDecl(m) => {
            // 登记模块内函数签名，使模块内跨函数调用（如 `io::file::open` 调 `io::base::c_str`）
            // 能在 check_item 阶段解析（M0：补齐模块函数签名登记）。
            let new_prefix = full_name(prefix, &m.name);
            collect_mod_fn_sigs(ctx, m, &new_prefix, &mut Vec::new())?;
        }
        _ => {}
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
        AstItem::StructDecl(s) => {
            collect_struct(ctx, s, prefix)?;
            expand_derives_for_struct(ctx, s, prefix)?;
            if s.is_pub {
                ctx.pub_symbols.insert(full_name(prefix, &s.name));
            }
        }
        // fn 签名收集延后至第三遍（collect_fn_sigs_pass），确保类型别名已登记，
        // 避免签名中的别名（如 `fn f() -> Int`）因别名尚未收集而报 undefined type。
        AstItem::FnDecl(_) => {}
        AstItem::EnumDecl(e) => {
            collect_enum(ctx, e, prefix)?;
            if e.is_pub {
                // 枚举为 `pub` 时，枚举名及其全部变体（Rust 语义：变体继承枚举可见性）
                // 均对外可达，登记枚举全名与每个变体的 `Enum::Variant` 全名。
                let base = full_name(prefix, &e.name);
                ctx.pub_symbols.insert(base.clone());
                for v in &e.variants {
                    ctx.pub_symbols.insert(format!("{base}::{}", v.name));
                }
            }
        }
        AstItem::ProtocolDecl(t) => {
            collect_protocol(ctx, t, prefix)?;
            if t.is_pub {
                ctx.pub_symbols.insert(full_name(prefix, &t.name));
            }
        }
        AstItem::ImplBlock(imp) => collect_impl(ctx, imp, prefix)?,
        AstItem::ActorDecl(a) => {
            collect_actor(ctx, a, prefix)?;
            if a.is_pub {
                ctx.pub_symbols.insert(full_name(prefix, &a.name));
            }
        }
        AstItem::TypeAlias(_) => {}
        AstItem::ModDecl(m) => {
            let new_prefix = full_name(prefix, &m.name);
            // 登记已声明模块路径（供 `resolve_import_path` 区分相对子模块导入与
            // 跨模块绝对路径导入；在声明即登记，不依赖符号收集时机）。
            ctx.modules.insert(new_prefix.clone());
            if m.is_pub {
                ctx.pub_module_prefixes.insert(new_prefix.clone());
            }
            // B-6：登记 `#[memory(gc)]` 模块前缀，供引用→Gc 默认映射判定
            if m.memory.as_deref() == Some("gc") {
                ctx.gc_modules.insert(new_prefix.clone());
            }
            // Q3a 修复：模块内符号（struct/protocol/impl）的短名解析须感知模块前缀。
            // 模块内 protocol/impl 方法签名在收集阶段即 resolve_ast_type（如
            // `fmt/module.rl` 的 `protocol Display { fn fmt(&self, f: &mut Formatter) }`），
            // 此时文件后部的 use 段尚未注册 use_aliases，须按 `mod::Name` 前缀回退。
            let old_prefix = std::mem::replace(&mut ctx.module_prefix, new_prefix.clone());
            for inner in &m.items {
                collect_item_decls(ctx, inner, &new_prefix)?;
            }
            ctx.module_prefix = old_prefix;
        }
        AstItem::UseDecl(u) => register_use(ctx, u, prefix)?,
        // 收集阶段注册模块常量 / 全局变量（供函数体 / 其它 const 引用）
        AstItem::ConstDecl(c) => {
            let (value, ty) = infer_expr(ctx, &c.value)?;
            if c.is_static {
                // `static` / `static mut`：登记为全局变量（data 段符号），
                // 而非可内联常量；引用处经 codegen 的全局名表发射 `@name` 读写。
                ctx.insert_global(full_name(prefix, &c.name), ty, c.is_mut);
            } else {
                ctx.insert_constant(full_name(prefix, &c.name), value, ty);
            }
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
        Type::Ref(inner, _, _) => format!("&{inner}"),
        t => t.to_string(),
    }
}

/// 注册 use 导入别名（`use path::to::item [as alias];` / 组导入 `a::{b, c}` /
/// glob 导入 `a::*` / `pub use` 重导出）。
///
/// `prefix` 为当前模块前缀（`mod math` 内为 `"math"`），用于 `pub use` 重导出时
/// 登记 `prefix::local → 目标全名`，使外部模块 `import math::local` 可经
/// `resolve_full_name` 的别名链解析到真实符号。
fn register_use(
    ctx: &mut TypeContext,
    u: &AstUseDecl,
    prefix: &str,
) -> Result<(), TypeError> {
    // `r#` 前缀（关键字转义 / 根命名空间显式引用标记）在符号注册时归一化，
    // 与 extern 声明注册名保持一致（`use r#rename` → 目标 "rename"）。
    let norm = |s: &str| s.strip_prefix("r#").unwrap_or(s).to_string();
    // 登记一条 `local → full`；`pub` 时额外登记 `prefix::local → full` 重导出。
    let register_one = |ctx: &mut TypeContext, local: String, full: String, prefix: &str, is_pub: bool| -> Result<(), TypeError> {
        // 冲突：本地名已被「非 pub」显式 import 绑定到不同目标。`pub import` 重导出链
        // 不计冲突（标准库大量 `pub import` 重导出同名符号，沿用旧版 last-wins 行为）。
        if let Some(prev) = ctx.use_aliases.get(&local) {
            let prev_pub = ctx.explicit_import_sources.get(&local).copied().unwrap_or(true);
            if *prev != full && !prev_pub && !is_pub {
                return Err(TypeError::NameConflict {
                    name: local.clone(),
                    span: u.span,
                });
            }
        }
        ctx.insert_use_alias(local.clone(), full.clone());
        if is_pub {
            ctx.insert_use_alias(full_name(prefix, &local), full.clone());
        }
        ctx.record_explicit_import(local.clone(), is_pub);
        ctx.import_alias_spans.push((local, full, u.span));
        Ok(())
    };
    if let Some(members) = &u.group {
        let base = u
            .path
            .iter()
            .map(|s| norm(s))
            .collect::<Vec<String>>()
            .join("::");
        let base = resolve_import_path(ctx, prefix, &base);
        register_use_group(ctx, members, &base, prefix, u.is_pub, u.span)?;
        return Ok(());
    }
    if u.path.last().map(String::as_str) == Some("*") {
        // glob 导入 `a::*`：枚举 `a` 的直接子项（不含 `a::b::` 嵌套），逐一定位全名。
        let base = resolve_import_path(
            ctx,
            prefix,
            &u.path
                .iter()
                .take(u.path.len() - 1)
                .map(|s| norm(s))
                .collect::<Vec<String>>()
                .join("::"),
        );
        let prefix2 = base.clone();
        let mut members: Vec<String> = Vec::new();
        for k in ctx.fn_signatures.keys() {
            if let Some(rest) = k.strip_prefix(&format!("{prefix2}::")) {
                if !rest.contains("::") {
                    members.push(rest.to_string());
                }
            }
        }
        for k in ctx.structs.keys() {
            if let Some(rest) = k.strip_prefix(&format!("{prefix2}::")) {
                if !rest.contains("::") {
                    members.push(rest.to_string());
                }
            }
        }
        for k in ctx.enum_defs.keys() {
            if let Some(rest) = k.strip_prefix(&format!("{prefix2}::")) {
                if !rest.contains("::") {
                    members.push(rest.to_string());
                }
            }
        }
        for k in ctx.constants.keys() {
            if let Some(rest) = k.strip_prefix(&format!("{prefix2}::")) {
                if !rest.contains("::") {
                    members.push(rest.to_string());
                }
            }
        }
        for k in ctx.actors.keys() {
            if let Some(rest) = k.strip_prefix(&format!("{prefix2}::")) {
                if !rest.contains("::") {
                    members.push(rest.to_string());
                }
            }
        }
        for m in members {
            let full = format!("{base}::{m}");
            // 记录 glob 来源（≥2 即歧义）；插入仅首次生效（显式 / 先到者优先，不遮蔽）。
            ctx.glob_exports.entry(m.clone()).or_default().push(base.clone());
            if !ctx.use_aliases.contains_key(&m) {
                ctx.insert_use_alias(m.clone(), full.clone());
                if u.is_pub {
                    ctx.insert_use_alias(full_name(prefix, &m), full.clone());
                }
            }
            ctx.import_alias_spans.push((m.clone(), full.clone(), u.span));
        }
        return Ok(());
    }
    let path = u
        .path
        .iter()
        .map(|s| norm(s))
        .collect::<Vec<String>>()
        .join("::");
    let local = match &u.alias {
        Some(a) => norm(a),
        None => u.path.last().map(|s| norm(s)).unwrap_or_default(),
    };
    let full = resolve_import_path(ctx, prefix, &path);
    register_one(ctx, local, full, prefix, u.is_pub)?;
    Ok(())
}

/// 把导入路径归一为**完整符号名**（`use_aliases` 的目标）。
///
/// - `prefix` 为空（顶层）：路径原样即完整名（如 `time::duration::Duration`）；
/// - `prefix` 非空（模块内）：
///   - 若路径首段是 `prefix` 的**已声明子模块**（见 `TypeContext::modules`），按相对
///     本模块解析——`import base::OpenMode;` 归一为 `io::base::OpenMode`；
///   - 否则视为引用**外部 / 顶层模块**的绝对路径（如 `module pkg` 内
///     `import helper::bump;`，`helper` 是顶层模块，`pkg::helper` 不存在）→ `helper::bump`。
///   - 已写明绝对路径（首段与本模块同名）时原样保留。
///
/// 相对目标是否存在**不依赖符号收集时机**——仅依据模块声明（`modules` 在 `module X;`
/// 即登记），故 `io::base::OpenMode` 在 `OpenMode` 枚举收集前即可正确归一，避免
/// 别名链 `OpenMode → base::OpenMode` 指向不存在符号（io 模块拆分回归）。
fn resolve_import_path(ctx: &TypeContext, prefix: &str, path: &str) -> String {
    if prefix.is_empty() || path == prefix || path.starts_with(&format!("{prefix}::")) {
        return path.to_string();
    }
    let rel = format!("{prefix}::{path}");
    let first_seg = path.split("::").next().unwrap_or(path);
    if ctx.modules.contains(&format!("{prefix}::{first_seg}")) {
        rel
    } else {
        path.to_string()
    }
}

/// 递归登记组导入成员（`import a::{b::{x, y}, c}`）。
///
/// 嵌套子组 `name::{ ... }` 仅将 `name` 作为新前缀下钻，叶子名（最内层成员）才入
/// 作用域；`pub` 重导出时同样只重导出叶子名（与 Rust `use a::b::{x, y}` 仅暴露
/// `x`/`y` 一致）。`r#` 前缀在注册时归一化。
fn register_use_group(
    ctx: &mut TypeContext,
    members: &[AstUseMember],
    base: &str,
    prefix: &str,
    is_pub: bool,
    span: Span,
) -> Result<(), TypeError> {
    let norm = |s: &str| s.strip_prefix("r#").unwrap_or(s).to_string();
    for mem in members {
        let m = norm(&mem.name);
        if let Some(nested) = &mem.nested {
            // `name::{ ... }`：name 作为新前缀递归，仅叶子名入作用域
            let child_base = format!("{base}::{m}");
            register_use_group(ctx, nested, &child_base, prefix, is_pub, span)?;
        } else {
            let full = format!("{base}::{m}");
            let local = match &mem.alias {
                Some(a) => norm(a),
                None => m.clone(),
            };
            // 冲突：本地名已被「非 pub」显式 import 绑定到不同目标（同 register_one 语义）。
            if let Some(prev) = ctx.use_aliases.get(&local) {
                let prev_pub = ctx.explicit_import_sources.get(&local).copied().unwrap_or(true);
                if *prev != full && !prev_pub && !is_pub {
                    return Err(TypeError::NameConflict {
                        name: local.clone(),
                        span,
                    });
                }
            }
            ctx.insert_use_alias(local.clone(), full.clone());
            if is_pub {
                ctx.insert_use_alias(full_name(prefix, &local), full.clone());
            }
            ctx.record_explicit_import(local.clone(), is_pub);
            ctx.import_alias_spans.push((local, full, span));
        }
    }
    Ok(())
}

/// B：全部收集完成后延迟校验 import 目标是否存在。
///
/// 收集阶段只登记别名、不校验（模块 / 符号登记顺序不确定，提前校验会误伤标准库等
/// 「import 早于所引用模块登记」的合法用例）。此处所有符号表与模块前缀均已就绪，
/// 逐条检查 `import_alias_spans`；目标符号 / 模块均不存在则报 `NameNotFound`（带候选）。
fn verify_imports(ctx: &mut TypeContext) -> Result<(), TypeError> {
    for (_local, full, span) in &ctx.import_alias_spans {
        if import_target_known(ctx, full) {
            continue;
        }
        return Err(TypeError::NameNotFound {
            name: full.clone(),
            candidates: ctx.name_candidates(full),
            span: *span,
        });
    }
    Ok(())
}

/// import 目标 `full` 是否存在：命中任一符号表，或首段 / 全路径为已知模块前缀。
fn import_target_known(ctx: &TypeContext, full: &str) -> bool {
    if ctx.structs.contains_key(full)
        || ctx.fn_signatures.contains_key(full)
        || ctx.fn_templates.contains_key(full)
        || ctx.enum_defs.contains_key(full)
        || ctx.constants.contains_key(full)
        || ctx.actors.contains_key(full)
        || ctx.protocol_defs.contains_key(full)
        || ctx.modules.contains(full)
    {
        return true;
    }
    // 目标落在某已知模块内（首段为该模块前缀）；成员是否真实存在留给使用处校验。
    let first = full.split("::").next().unwrap_or(full);
    ctx.modules.contains(first)
}

/// 第二遍：检查函数体 / const，生成 HIR 项（递归处理嵌套模块）。
/// `globals` 收集 `static` / `static mut` 声明，透传至 codegen 发射 data 段符号。
pub(crate) fn check_item(
    ctx: &mut TypeContext,
    item: &AstItem,
    prefix: &str,
    out: &mut Vec<HirItem>,
    globals: &mut Vec<GlobalDecl>,
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
                .map(|p| HirParam { span: Span::dummy(),
                    name: p.name.clone(),
                    is_ref: matches!(&p.type_, rlyeh_ast::AstType::Ref(..)),
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
                span: f.span,
            });
        }
        AstItem::ConstDecl(c) => {
            let old_prefix = std::mem::replace(&mut ctx.module_prefix, prefix.to_string());
            let (value, ty) = infer_expr(ctx, &c.value)?;
            ctx.module_prefix = old_prefix;
            if c.is_static {
                // `static` / `static mut`：收集为全局声明，发射 data 段符号；
                // 不发射 HIR const 项（codegen 经 LirProgram.globals 处理）。
                match eval_const_value(&value) {
                    Some(init) => globals.push(GlobalDecl {
                        name: full_name(prefix, &c.name),
                        type_: ty,
                        is_mut: c.is_mut,
                        init,
                    }),
                    None => {
                        return Err(TypeError::NonConstStaticInit {
                            name: c.name.clone(),
                            span: c.span,
                        })
                    }
                }
            } else {
                ctx.insert_constant(full_name(prefix, &c.name), value.clone(), ty);
                out.push(HirItem {
                    name: full_name(prefix, &c.name),
                    kind: HirItemKind::Const(HirConstDecl { value }),
                    span: c.span,
                });
            }
        }
        AstItem::ModDecl(m) => {
            let new_prefix = full_name(prefix, &m.name);
            // B-6：登记 `#[memory(gc)]` 模块前缀，供引用→Gc 默认映射判定
            if m.memory.as_deref() == Some("gc") {
                ctx.gc_modules.insert(new_prefix.clone());
            }
            // 与收集阶段一致：模块内短名解析感知模块前缀（Q3a）
            let old_prefix = std::mem::replace(&mut ctx.module_prefix, new_prefix.clone());
            for inner in &m.items {
                check_item(ctx, inner, &new_prefix, out, globals)?;
            }
            ctx.module_prefix = old_prefix;
        }
        // actor：展开为状态初始化函数 + 方法函数 + dispatch handle + runtime extern 声明
        AstItem::ActorDecl(a) => expand_actor(ctx, a, prefix, out)?,
        // use 导入在收集阶段（第一遍）已注册；其余项 MVP 阶段不生成 HIR
        AstItem::UseDecl(_) | AstItem::StructDecl(_) | AstItem::ProtocolDecl(_)
        | AstItem::ImplBlock(_) | AstItem::EnumDecl(_)
        | AstItem::MacroDecl(_) | AstItem::TypeAlias(_) | AstItem::Statement(_) => {}
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

/// 收集 protocol 定义（抽象方法签名）。

/// 收集 impl 块（inherent 或 protocol impl），方法保存原始 AST 供调用点实例化。

/// 仅收集函数签名（不检查函数体），供增量编译提取模块接口。
///
/// 递归处理嵌套模块（符号名带 `mod::` 前缀）。输出按函数名排序，
/// 保证接口哈希稳定。

/// 递归收集模块内的类型声明（结构体 / 枚举 / protocol / impl）。
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
mod derive;

use collect::*;
use actor::*;
use fn_sig::*;
use derive::expand_derives_for_struct;

// 对外 API：被 lib.rs / check_expr / check_stmt 经 `crate::check_item::` 访问
pub use fn_sig::collect_fn_signatures;
pub(crate) use fn_sig::{fn_signature_with_self, check_fn_body_with_self};
