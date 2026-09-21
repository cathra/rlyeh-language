//! protocol derive 宏展开（0.2.0-C）。
//!
//! 为带 `#[derive(Clone / PartialEq / Debug)]` 的结构体自动合成对应的 `impl`
//! 块，并复用既有 `collect_impl` 注册（方法体在调用点实例化，零新增 IR 节点）。
//!
//! 受支持：struct 的 `Clone` / `PartialEq` / `Copy` / `Debug`。
//! - `Clone`：逐字段拷贝（标量 / 引用直接拷贝，其余调用 `.clone()`）。
//! - `PartialEq`：逐字段 `==` 链；`==`/`!=` 经 `comparison` 落点为 `a.eq(&b)`。
//! - `Debug`：构造 `Name { f0: <Debug>, ... }` 形式（标量走 `int_to_string` /
//!   `float_to_string`，String 走 `write_str`，嵌套类型走 `.fmt(f)`）。
//!
//! 已知限制：仅 struct 受支持（enum 待办）；generic struct 的字段类型须自身
//! 实现对应 protocol（如 `T: Clone`，MVP 未强制校验 bound，违反时在调用点报出）。

use rlyeh_ast::{
    AstAttr, AstEnumDecl, AstExpr, AstFnDecl, AstImplBlock, AstParam, AstPattern, AstStmt,
    AstStructDecl, AstStructField, AstType, BinaryOp, CompareOp, ExprKind, MatchArm,
};
use rlyeh_lexer::Span;

use crate::context::TypeContext;
use crate::error::TypeError;

/// 受支持的 derive protocol 名 → 规范 protocol 全名。
///
/// 未知名（如 `Serialize` / `Deserialize`，serde 路径）返回 `None` 被忽略，
/// 保持既有「宽松忽略」行为（不报错、不生成 impl）。
fn canonical_protocol(name: &str) -> Option<&'static str> {
    match name {
        // EH-5（0.2.0-AA，2026-09-21）：`#[derive(Error)]` + `#[error("..")]` /
        // `#[from]` / `#[source]` 属性。
        "Error" => Some("Error"),
        "Clone" => Some("Clone"),
        "PartialEq" => Some("PartialEq"),
        "Copy" => Some("Copy"),
        "Debug" => Some("fmt::Debug"),
        _ => None,
    }
}

/// 拼接模块前缀与名称（与 `mod.rs::full_name` 一致）。
fn qual(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}::{name}")
    }
}

/// struct 的类型（含泛型实参）AST。
fn struct_type(s: &AstStructDecl, prefix: &str) -> AstType {
    let type_args: Vec<AstType> = s
        .generics
        .iter()
        .map(|g| AstType::Path(g.name.clone(), vec![]))
        .collect();
    AstType::Path(qual(prefix, &s.name), type_args)
}

// ---------- AST 构造辅助 ----------

fn ident(name: &str, span: Span) -> AstExpr {
    AstExpr::new(ExprKind::Ident(name.to_string()), span)
}

fn self_field(field: &str, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::FieldAccess {
            expr: ident("self", span),
            field: field.to_string(),
        },
        span,
    )
}

fn other_field(field: &str, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::FieldAccess {
            expr: ident("other", span),
            field: field.to_string(),
        },
        span,
    )
}

fn call_expr(callee: &str, args: Vec<AstExpr>, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::Call {
            callee: ident(callee, span),
            args,
            type_args: vec![],
        },
        span,
    )
}

fn method_call(receiver: &str, method: &str, args: Vec<AstExpr>, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::MethodCall {
            receiver: ident(receiver, span),
            method: method.to_string(),
            args,
            protocol_hint: None,
        },
        span,
    )
}

fn write_str_stmt(receiver: &str, s: &str, span: Span) -> AstStmt {
    AstStmt::Semi(method_call(
        receiver,
        "write_str",
        vec![AstExpr::new(ExprKind::StringLiteral(s.to_string()), span)],
        span,
    ))
}

fn result_ok_unit(span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(
                ExprKind::Path(vec!["Result".to_string(), "Ok".to_string()]),
                span,
            ),
            args: vec![AstExpr::new(ExprKind::Unit, span)],
            type_args: vec![],
        },
        span,
    )
}

// ---------- 字段类型判定 ----------

fn is_scalar_ast(ty: &AstType) -> bool {
    matches!(
        ty,
        AstType::Path(
            n,
            _
        ) if matches!(
            n.as_str(),
            "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize"
                | "f32" | "f64" | "bool" | "char"
        )
    )
}

fn is_char_ast(ty: &AstType) -> bool {
    matches!(ty, AstType::Path(n, _) if n == "char")
}

fn is_string_ast(ty: &AstType) -> bool {
    matches!(ty, AstType::Path(n, _) if n == "String")
}

// ---------- Clone ----------

fn clone_field_expr(field: &str, ty: &AstType, span: Span) -> AstExpr {
    if is_scalar_ast(ty) || matches!(ty, AstType::Ref(_, _, _)) {
        // 标量 / 引用直接拷贝（无需 .clone()）
        self_field(field, span)
    } else {
        // 其余（`String` / 嵌套 struct/enum / 泛型字段）调用 `.clone()`
        AstExpr::new(
            ExprKind::MethodCall {
                receiver: self_field(field, span),
                method: "clone".to_string(),
                args: vec![],
                protocol_hint: None,
            },
            span,
        )
    }
}

fn build_clone_impl(s: &AstStructDecl, prefix: &str, span: Span) -> AstImplBlock {
    let st = struct_type(s, prefix);
    let mut fields = Vec::with_capacity(s.fields.len());
    for f in &s.fields {
        fields.push((f.name.clone(), clone_field_expr(&f.name, &f.type_, span)));
    }
    let clone_fn = AstFnDecl {
        name: "clone".to_string(),
        generics: s.generics.clone(),
        params: vec![AstParam {
            name: "self".to_string(),
            type_: AstType::Ref(Box::new(st.clone()), false, None),
            default: None,
            is_mut: false,
            span,
        }],
        return_type: Some(st.clone()),
        body: Some(rlyeh_ast::AstBlock {
            stmts: vec![],
            final_expr: Some(AstExpr::new(
                ExprKind::StructCtor {
                    type_name: qual(prefix, &s.name)
                        .split("::")
                        .map(String::from)
                        .collect(),
                    type_args: s
                        .generics
                        .iter()
                        .map(|g| AstType::Path(g.name.clone(), vec![]))
                        .collect(),
                    fields,
                    base: None,
                },
                span,
            )),
            span,
        }),
        is_pub: false,
        is_async: false,
        is_extern: false,
        span,
    };
    AstImplBlock {
        protocol_name: Some("Clone".to_string()),
        type_name: s.name.clone(),
        generics: s.generics.clone(),
        // self 类型实参留空 → typecheck 回退为 impl 泛型参数重建（沿用旧行为）。
        self_type_args: vec![],
        protocol_type_args: vec![],
        extra_protocols: vec![],
        types: vec![],
        methods: vec![clone_fn],
        span,
    }
}

// ---------- Copy（标记 protocol，无方法） ----------

fn build_copy_impl(s: &AstStructDecl, _prefix: &str, span: Span) -> AstImplBlock {
    // `Copy` 是零方法标记 protocol：`#[derive(Copy)]` 展开为 `impl Copy for T {}`，
    // 供泛型约束 `T: Copy` 经 `type_implements_protocol` 命中（与 Rust 语义对齐）。
    AstImplBlock {
        protocol_name: Some("Copy".to_string()),
        type_name: s.name.clone(),
        generics: s.generics.clone(),
        // self 类型实参留空 → typecheck 回退为 impl 泛型参数重建（沿用旧行为）。
        self_type_args: vec![],
        protocol_type_args: vec![],
        extra_protocols: vec![],
        types: vec![],
        methods: vec![],
        span,
    }
}

// ---------- PartialEq ----------

fn build_partialeq_impl(s: &AstStructDecl, prefix: &str, span: Span) -> AstImplBlock {
    let st = struct_type(s, prefix);
    let mut acc: Option<AstExpr> = None;
    for f in &s.fields {
        let cmp = AstExpr::new(
            ExprKind::ComparisonChain {
                elements: vec![self_field(&f.name, span), other_field(&f.name, span)],
                operators: vec![CompareOp::Eq],
            },
            span,
        );
        acc = Some(match acc {
            None => cmp,
            Some(a) => AstExpr::new(
                ExprKind::Binary {
                    op: BinaryOp::And,
                    left: a,
                    right: cmp,
                },
                span,
            ),
        });
    }
    let final_expr = acc.unwrap_or_else(|| AstExpr::new(ExprKind::BoolLiteral(true), span));
    let eq_fn = AstFnDecl {
        name: "eq".to_string(),
        generics: s.generics.clone(),
        params: vec![
            AstParam {
                name: "self".to_string(),
                type_: AstType::Ref(Box::new(st.clone()), false, None),
                default: None,
                is_mut: false,
                span,
            },
            AstParam {
                name: "other".to_string(),
                type_: AstType::Ref(Box::new(st.clone()), false, None),
                default: None,
                is_mut: false,
                span,
            },
        ],
        return_type: Some(AstType::Path("bool".to_string(), vec![])),
        body: Some(rlyeh_ast::AstBlock {
            stmts: vec![],
            final_expr: Some(final_expr),
            span,
        }),
        is_pub: false,
        is_async: false,
        is_extern: false,
        span,
    };
    AstImplBlock {
        protocol_name: Some("PartialEq".to_string()),
        type_name: s.name.clone(),
        generics: s.generics.clone(),
        // self 类型实参留空 → typecheck 回退为 impl 泛型参数重建（沿用旧行为）。
        self_type_args: vec![],
        protocol_type_args: vec![],
        extra_protocols: vec![],
        types: vec![],
        methods: vec![eq_fn],
        span,
    }
}

// ---------- Debug ----------

fn debug_field_expr(f: &AstStructField, span: Span) -> AstExpr {
    let sf = self_field(&f.name, span);
    if is_scalar_ast(&f.type_) {
        match &f.type_ {
            AstType::Path(n, _) if n == "f32" || n == "f64" => {
                method_call("f", "write_str", vec![call_expr("float_to_string", vec![sf], span)], span)
            }
            AstType::Path(n, _) if n == "bool" => AstExpr::new(
                ExprKind::If {
                    cond: sf,
                    then_block: rlyeh_ast::AstBlock {
                        stmts: vec![write_str_stmt("f", "true", span)],
                        final_expr: None,
                        span,
                    },
                    else_block: Some(rlyeh_ast::AstBlock {
                        stmts: vec![write_str_stmt("f", "false", span)],
                        final_expr: None,
                        span,
                    }),
                },
                span,
            ),
            _ if is_char_ast(&f.type_) => AstExpr::new(
                ExprKind::MethodCall {
                    receiver: sf,
                    method: "fmt".to_string(),
                    args: vec![ident("f", span)],
                    protocol_hint: None,
                },
                span,
            ),
            // 整型：int_to_string(self.field)
            _ => method_call("f", "write_str", vec![call_expr("int_to_string", vec![sf], span)], span),
        }
    } else if is_string_ast(&f.type_) {
        // String 字段：write_str(self.field)（write_str 接收 String）
        method_call("f", "write_str", vec![sf], span)
    } else {
        // 嵌套类型：self.field.fmt(f)
        AstExpr::new(
            ExprKind::MethodCall {
                receiver: sf,
                method: "fmt".to_string(),
                args: vec![ident("f", span)],
                protocol_hint: None,
            },
            span,
        )
    }
}

fn build_debug_impl(s: &AstStructDecl, prefix: &str, span: Span) -> AstImplBlock {
    let st = struct_type(s, prefix);
    let mut stmts = Vec::new();
    stmts.push(write_str_stmt("f", &s.name, span));
    stmts.push(write_str_stmt("f", " { ", span));
    let n = s.fields.len();
    for (i, f) in s.fields.iter().enumerate() {
        stmts.push(write_str_stmt("f", &format!("{}: ", f.name), span));
        stmts.push(AstStmt::Semi(debug_field_expr(f, span)));
        if i + 1 < n {
            stmts.push(write_str_stmt("f", ", ", span));
        }
    }
    stmts.push(write_str_stmt("f", " }", span));
    let fmt_fn = AstFnDecl {
        name: "fmt".to_string(),
        generics: s.generics.clone(),
        params: vec![
            AstParam {
                name: "self".to_string(),
                type_: AstType::Ref(Box::new(st.clone()), false, None),
                default: None,
                is_mut: false,
                span,
            },
            AstParam {
                name: "f".to_string(),
                type_: AstType::Ref(
                    Box::new(AstType::Path("fmt::Formatter".to_string(), vec![])),
                    true, None),
                default: None,
                is_mut: false,
                span,
            },
        ],
        return_type: Some(AstType::Path(
            "Result".to_string(),
            vec![
                AstType::Tuple(vec![]),
                AstType::Path("fmt::FmtError".to_string(), vec![]),
            ],
        )),
        body: Some(rlyeh_ast::AstBlock {
            stmts,
            final_expr: Some(result_ok_unit(span)),
            span,
        }),
        is_pub: false,
        is_async: false,
        is_extern: false,
        span,
    };
    AstImplBlock {
        protocol_name: Some("fmt::Debug".to_string()),
        type_name: s.name.clone(),
        generics: s.generics.clone(),
        // self 类型实参留空 → typecheck 回退为 impl 泛型参数重建（沿用旧行为）。
        self_type_args: vec![],
        protocol_type_args: vec![],
        extra_protocols: vec![],
        types: vec![],
        methods: vec![fmt_fn],
        span,
    }
}

/// 为带 `#[derive(...)]` 的结构体展开并注册合成 impl。
///
/// 在 `collect_item_decls` 第一遍收集阶段调用：结构体自身已注册，此处为每个
/// 受支持的 derive 名合成一个 `AstImplBlock` 并走 `collect_impl`（与手写 impl
/// 完全一致），方法体在调用点实例化。
pub(crate) fn expand_derives_for_struct(
    ctx: &mut TypeContext,
    s: &AstStructDecl,
    prefix: &str,
) -> Result<(), TypeError> {
    if s.derive.is_empty() {
        return Ok(());
    }
    let span = s.span;
    for name in &s.derive {
        let Some(protocol_full) = canonical_protocol(name) else {
            // 未知 derive 名（serde 等）：忽略
            continue;
        };
        let impl_block = match protocol_full {
            "Clone" => build_clone_impl(s, prefix, span),
            "PartialEq" => build_partialeq_impl(s, prefix, span),
            "Copy" => build_copy_impl(s, prefix, span),
            "fmt::Debug" => build_debug_impl(s, prefix, span),
            // EH-5：`Error` → `message()`（`#[error("模板")]`）+ `source()`（`#[source]`）。
            "Error" => build_error_impl_struct(s, prefix, span)?,
            _ => continue,
        };
        super::collect_impl(ctx, &impl_block, prefix)?;
        // EH-5：`#[from]` 字段 → 另生成 `impl X: From<T>`。
        if protocol_full == "Error" {
            if let Some(from_impl) = build_from_impl_struct(s, prefix, span)? {
                super::collect_impl(ctx, &from_impl, prefix)?;
            }
        }
    }
    Ok(())
}

/// EH-5：枚举 derive 展开（目前仅 `Error`）。
///
/// 与 struct 版同构：`message()` 逐变体 `match`（各变体模板取变体级 `#[error("..")]`，
/// 缺失时回退到 enum 项级模板）；`#[from]` 出现在「仅含单个元组字段的变体」时生成
/// `impl X: From<T>`（构造该变体）。
pub(crate) fn expand_derives_for_enum(
    ctx: &mut TypeContext,
    e: &AstEnumDecl,
    prefix: &str,
) -> Result<(), TypeError> {
    if e.derive.is_empty() {
        return Ok(());
    }
    let span = e.span;
    for name in &e.derive {
        let Some(protocol_full) = canonical_protocol(name) else {
            continue;
        };
        if protocol_full != "Error" {
            // 枚举的 Clone / PartialEq / Debug derive 尚未实现（MVP 仅 Error）。
            continue;
        }
        let impl_block = build_error_impl_enum(e, prefix, span)?;
        super::collect_impl(ctx, &impl_block, prefix)?;
        for from_impl in build_from_impl_enum(e, prefix, span)? {
            super::collect_impl(ctx, &from_impl, prefix)?;
        }
    }
    Ok(())
}

// ==================== EH-5：Error derive ====================

/// 取属性列表中 `#[error("模板")]` 的模板原文。
fn error_template(attrs: &[AstAttr]) -> Option<String> {
    attrs
        .iter()
        .find(|a| a.name == "error")
        .and_then(|a| a.value.clone())
}

/// 属性列表是否含标记属性（`#[from]` / `#[source]`）。
fn has_attr(attrs: &[AstAttr], name: &str) -> bool {
    attrs.iter().any(|a| a.name == name)
}

/// `String::from(lit)`。
fn string_from_lit(s: &str, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::Call {
            callee: AstExpr::new(
                ExprKind::Path(vec!["String".to_string(), "from".to_string()]),
                span,
            ),
            args: vec![AstExpr::new(ExprKind::StringLiteral(s.to_string()), span)],
            type_args: vec![],
        },
        span,
    )
}

/// `Option::None`。
fn option_none(span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::Path(vec!["Option".to_string(), "None".to_string()]),
        span,
    )
}

/// `Option<&dyn Error>`（`Error::source` 的返回类型）。
fn option_dyn_error_ty() -> AstType {
    AstType::Path(
        "Option".to_string(),
        vec![AstType::Ref(
            Box::new(AstType::Dyn("Error".to_string())),
            false,
            None,
        )],
    )
}

/// 模板占位符的字段值 → `String` 表达式。
///
/// MVP 支持：整型（`int_to_string`）、`f32`/`f64`（`float_to_string`）、`bool`
/// （`if` 取 `"true"`/`"false"`）、`char`（`int_to_string`，码点）、`String`（原样）。
/// 其余类型报 `Unsupported`（嵌套错误类型请改用 `#[source]` 表达，不参与消息插值）。
fn placeholder_string_expr(
    field: &str,
    ty: &AstType,
    is_self: bool,
    span: Span,
) -> Result<AstExpr, TypeError> {
    let v = if is_self {
        self_field(field, span)
    } else {
        ident(field, span)
    };
    if is_string_ast(ty) {
        return Ok(v);
    }
    if matches!(ty, AstType::Path(n, _) if n == "f32" || n == "f64") {
        return Ok(call_expr("float_to_string", vec![v], span));
    }
    if matches!(ty, AstType::Path(n, _) if n == "bool") {
        return Ok(AstExpr::new(
            ExprKind::If {
                cond: v,
                then_block: rlyeh_ast::AstBlock {
                    stmts: vec![],
                    final_expr: Some(string_from_lit("true", span)),
                    span,
                },
                else_block: Some(rlyeh_ast::AstBlock {
                    stmts: vec![],
                    final_expr: Some(string_from_lit("false", span)),
                    span,
                }),
            },
            span,
        ));
    }
    if is_scalar_ast(ty) {
        // 整型 + char（char 以码点整数参与消息；与 Debug 的 `int_to_string` 一致）
        return Ok(call_expr("int_to_string", vec![v], span));
    }
    Err(TypeError::Unsupported {
        what: format!(
            "`#[error(\"..\")]` 模板占位符 `{{{field}}}`：该字段类型不支持消息插值\
             （MVP 支持整型 / f64 / bool / char / String；嵌套错误类型请用 `#[source]`）"
        ),
        span,
    })
}

/// 消息模板占位符所引用的字段。
///
/// `key` 为模板中的引用名（`{0}` → `f0`；`{name}` → `name`）；`bind` 为**生成的绑定
/// 标识符**——枚举各臂须**逐变体唯一**（LIR 局部变量按名索引，同名跨类型复用会报
/// 「变量 `x` 类型冲突」，见 `docs/std-lib.md` §12 已知限制）。
struct TmplField {
    key: String,
    bind: String,
    ty: AstType,
    /// `true`：`bind` 为 `self.<bind>`（struct 字段访问）；
    /// `false`：`bind` 为 `match` 臂绑定的裸标识符（enum 变体负载）。
    is_self: bool,
}

/// 消息模板 → `String` 表达式（`String::from("lit") + <占位> + …`）。
///
/// 占位符语法：`{0}`/`{1}`…（元组字段，键 `f0`/`f1`…）与 `{name}`（具名字段）。
/// 未闭合的 `{` 按字面输出（宽松处理）；无对应字段 / 类型不可插值时报 `Unsupported`。
fn message_from_template(
    tmpl: &str,
    fields: &[TmplField],
    span: Span,
) -> Result<AstExpr, TypeError> {
    let mut parts: Vec<AstExpr> = Vec::new();
    let mut lit = String::new();
    let mut it = tmpl.chars().peekable();
    while let Some(c) = it.next() {
        if c != '{' {
            lit.push(c);
            continue;
        }
        let mut name = String::new();
        let mut closed = false;
        for c2 in it.by_ref() {
            if c2 == '}' {
                closed = true;
                break;
            }
            name.push(c2);
        }
        if !closed {
            lit.push('{');
            lit.push_str(&name);
            continue;
        }
        // `{0}` → 键 `f0`（元组字段）；`{name}` → 键 `name`（具名字段）
        let key = if !name.is_empty() && name.chars().all(|c| c.is_ascii_digit()) {
            format!("f{name}")
        } else {
            name.clone()
        };
        let Some(f) = fields.iter().find(|f| f.key == key) else {
            return Err(TypeError::Unsupported {
                what: format!("`#[error(\"..\")]` 模板占位符 `{{{name}}}` 在该类型上无对应字段"),
                span,
            });
        };
        if !lit.is_empty() {
            parts.push(string_from_lit(&lit, span));
            lit.clear();
        }
        parts.push(placeholder_string_expr(&f.bind, &f.ty, f.is_self, span)?);
    }
    if !lit.is_empty() {
        parts.push(string_from_lit(&lit, span));
    }
    if parts.is_empty() {
        return Ok(string_from_lit("", span));
    }
    let mut rest = parts.into_iter();
    let mut acc = rest.next().expect("parts 非空");
    for p in rest {
        acc = AstExpr::new(
            ExprKind::Binary {
                op: BinaryOp::Add,
                left: acc,
                right: p,
            },
            span,
        );
    }
    Ok(acc)
}

/// `source()` 方法体：首个 `#[source]` 字段 → `self.<f>.source()`；无 → `Option::None`。
///
/// 多个 `#[source]` 字段报 `Unsupported`（错误链为单链）。
fn source_body(fields: &[(String, &[AstAttr])], span: Span) -> Result<AstExpr, TypeError> {
    let marked: Vec<&String> = fields
        .iter()
        .filter(|(_, a)| has_attr(a, "source"))
        .map(|(n, _)| n)
        .collect();
    match marked.as_slice() {
        [] => Ok(option_none(span)),
        [f] => Ok(AstExpr::new(
            ExprKind::MethodCall {
                receiver: self_field(f, span),
                method: "source".to_string(),
                args: vec![],
                protocol_hint: None,
            },
            span,
        )),
        _ => Err(TypeError::Unsupported {
            what: "`#[derive(Error)]`：`#[source]` 只能标注一个字段（错误链为单链）"
                .to_string(),
            span,
        }),
    }
}

/// `&self` 参数（derive 生成的协议方法统一签名）。
fn ref_self_param(st: &AstType, span: Span) -> AstParam {
    AstParam {
        name: "self".to_string(),
        type_: AstType::Ref(Box::new(st.clone()), false, None),
        default: None,
        is_mut: false,
        span,
    }
}

/// struct 的 `Error` 实现：`message()` + `source()`。
fn build_error_impl_struct(
    s: &AstStructDecl,
    prefix: &str,
    span: Span,
) -> Result<AstImplBlock, TypeError> {
    let st = struct_type(s, prefix);
    let Some(tmpl) = error_template(&s.attrs) else {
        return Err(TypeError::Unsupported {
            what: "`#[derive(Error)]`：缺少 `#[error(\"消息模板\")]`（标注在 struct 上）"
                .to_string(),
            span,
        });
    };
    let fields: Vec<TmplField> = s
        .fields
        .iter()
        .map(|f| TmplField {
            key: f.name.clone(),
            bind: f.name.clone(),
            ty: f.type_.clone(),
            is_self: true,
        })
        .collect();
    let msg = message_from_template(&tmpl, &fields, span)?;
    let msg_fn = AstFnDecl {
        name: "message".to_string(),
        generics: s.generics.clone(),
        params: vec![ref_self_param(&st, span)],
        return_type: Some(AstType::Path("String".to_string(), vec![])),
        body: Some(rlyeh_ast::AstBlock {
            stmts: vec![],
            final_expr: Some(msg),
            span,
        }),
        is_pub: false,
        is_async: false,
        is_extern: false,
        span,
    };
    let field_attrs: Vec<(String, &[AstAttr])> = s
        .fields
        .iter()
        .map(|f| (f.name.clone(), f.attrs.as_slice()))
        .collect();
    let src = source_body(&field_attrs, span)?;
    let src_fn = AstFnDecl {
        name: "source".to_string(),
        generics: s.generics.clone(),
        params: vec![ref_self_param(&st, span)],
        return_type: Some(option_dyn_error_ty()),
        body: Some(rlyeh_ast::AstBlock {
            stmts: vec![],
            final_expr: Some(src),
            span,
        }),
        is_pub: false,
        is_async: false,
        is_extern: false,
        span,
    };
    Ok(AstImplBlock {
        protocol_name: Some("Error".to_string()),
        type_name: s.name.clone(),
        generics: s.generics.clone(),
        self_type_args: vec![],
        protocol_type_args: vec![],
        extra_protocols: vec![],
        types: vec![],
        methods: vec![msg_fn, src_fn],
        span,
    })
}

/// enum 的 `Error` 实现：`message()` 逐变体 `match`；`source()` 恒 `Option::None`
/// （MVP：变体级 `#[source]` 待办，嵌套来源请改用 struct 包装）。
fn build_error_impl_enum(
    e: &AstEnumDecl,
    prefix: &str,
    span: Span,
) -> Result<AstImplBlock, TypeError> {
    let st = AstType::Path(
        qual(prefix, &e.name),
        e.generics
            .iter()
            .map(|g| AstType::Path(g.name.clone(), vec![]))
            .collect(),
    );
    let fallback = error_template(&e.attrs);
    let mut arms: Vec<MatchArm> = Vec::with_capacity(e.variants.len());
    for (vi, v) in e.variants.iter().enumerate() {
        let Some(tmpl) = error_template(&v.attrs).or_else(|| fallback.clone()) else {
            return Err(TypeError::Unsupported {
                what: format!(
                    "`#[derive(Error)]`：变体 `{}::{}` 缺少 `#[error(\"消息模板\")]`\
                     （变体级或 enum 项级至少一处）",
                    e.name, v.name
                ),
                span: v.span,
            });
        };
        // 字段列表 + 模式子项。绑定名**逐变体唯一**（`__err<vi>_f<j>` / `__err<vi>_<name>`）：
        // 同一 `match` 的各臂共享函数级局部变量命名空间，`f0` 在 String 臂与 i64 臂
        // 复用会触发 LIR「变量类型冲突」（见 `docs/std-lib.md` §12）。
        let mut fields: Vec<TmplField> = Vec::new();
        let mut subs: Vec<AstPattern> = Vec::new();
        for (i, t) in v.tuple_fields.iter().enumerate() {
            let bind = format!("__err{vi}_f{i}");
            fields.push(TmplField {
                key: format!("f{i}"),
                bind: bind.clone(),
                ty: t.clone(),
                is_self: false,
            });
            subs.push(AstPattern::Ident(bind));
        }
        let mut path: Vec<String> = qual(prefix, &e.name)
            .split("::")
            .map(String::from)
            .collect();
        path.push(v.name.clone());
        let pattern = if v.struct_fields.is_empty() {
            AstPattern::EnumPath(path, subs)
        } else {
            let mut named: Vec<(String, AstPattern)> = Vec::new();
            for sf in &v.struct_fields {
                let bind = format!("__err{vi}_{}", sf.name);
                fields.push(TmplField {
                    key: sf.name.clone(),
                    bind: bind.clone(),
                    ty: sf.type_.clone(),
                    is_self: false,
                });
                named.push((sf.name.clone(), AstPattern::Ident(bind)));
            }
            AstPattern::EnumStructPath(path, named)
        };
        arms.push(MatchArm {
            pattern,
            guard: None,
            body: message_from_template(&tmpl, &fields, span)?,
            span: v.span,
        });
    }
    let msg_fn = AstFnDecl {
        name: "message".to_string(),
        generics: e.generics.clone(),
        params: vec![ref_self_param(&st, span)],
        return_type: Some(AstType::Path("String".to_string(), vec![])),
        body: Some(rlyeh_ast::AstBlock {
            stmts: vec![],
            final_expr: Some(AstExpr::new(
                ExprKind::Match {
                    expr: ident("self", span),
                    arms,
                },
                span,
            )),
            span,
        }),
        is_pub: false,
        is_async: false,
        is_extern: false,
        span,
    };
    let src_fn = AstFnDecl {
        name: "source".to_string(),
        generics: e.generics.clone(),
        params: vec![ref_self_param(&st, span)],
        return_type: Some(option_dyn_error_ty()),
        body: Some(rlyeh_ast::AstBlock {
            stmts: vec![],
            final_expr: Some(option_none(span)),
            span,
        }),
        is_pub: false,
        is_async: false,
        is_extern: false,
        span,
    };
    Ok(AstImplBlock {
        protocol_name: Some("Error".to_string()),
        type_name: e.name.clone(),
        generics: e.generics.clone(),
        self_type_args: vec![],
        protocol_type_args: vec![],
        extra_protocols: vec![],
        types: vec![],
        methods: vec![msg_fn, src_fn],
        span,
    })
}

/// struct 的 `#[from]` → `impl X: From<T>`（要求该字段是 struct 的**唯一**字段）。
fn build_from_impl_struct(
    s: &AstStructDecl,
    prefix: &str,
    span: Span,
) -> Result<Option<AstImplBlock>, TypeError> {
    let marked: Vec<&AstStructField> = s.fields.iter().filter(|f| has_attr(&f.attrs, "from")).collect();
    let Some(f) = marked.first() else {
        return Ok(None);
    };
    if s.fields.len() != 1 {
        return Err(TypeError::Unsupported {
            what: format!(
                "`#[from]` 要求 struct `{}` 仅有一个字段（经 `From` 只能构造该字段）",
                s.name
            ),
            span: f.span,
        });
    }
    let st = struct_type(s, prefix);
    let from_fn = AstFnDecl {
        name: "from".to_string(),
        generics: s.generics.clone(),
        params: vec![AstParam {
            name: "v".to_string(),
            type_: f.type_.clone(),
            default: None,
            is_mut: false,
            span,
        }],
        return_type: Some(st),
        body: Some(rlyeh_ast::AstBlock {
            stmts: vec![],
            final_expr: Some(AstExpr::new(
                ExprKind::StructCtor {
                    type_name: qual(prefix, &s.name).split("::").map(String::from).collect(),
                    type_args: s
                        .generics
                        .iter()
                        .map(|g| AstType::Path(g.name.clone(), vec![]))
                        .collect(),
                    fields: vec![(f.name.clone(), ident("v", span))],
                    base: None,
                },
                span,
            )),
            span,
        }),
        is_pub: false,
        is_async: false,
        is_extern: false,
        span,
    };
    Ok(Some(AstImplBlock {
        protocol_name: Some("From".to_string()),
        type_name: s.name.clone(),
        generics: s.generics.clone(),
        self_type_args: vec![],
        protocol_type_args: vec![f.type_.clone()],
        extra_protocols: vec![],
        types: vec![],
        methods: vec![from_fn],
        span,
    }))
}

/// enum 的 `#[from]` → `impl X: From<T>`（要求该变体**仅含一个元组字段**）。
fn build_from_impl_enum(
    e: &AstEnumDecl,
    prefix: &str,
    span: Span,
) -> Result<Vec<AstImplBlock>, TypeError> {
    let mut impls = Vec::new();
    for v in &e.variants {
        if !has_attr(&v.attrs, "from") {
            continue;
        }
        if v.tuple_fields.len() != 1 {
            return Err(TypeError::Unsupported {
                what: format!(
                    "`#[from]` 要求变体 `{}::{}` 仅含一个元组字段，实际 {} 个",
                    e.name,
                    v.name,
                    v.tuple_fields.len()
                ),
                span: v.span,
            });
        }
        let fty = v.tuple_fields[0].clone();
        let st = AstType::Path(
            qual(prefix, &e.name),
            e.generics
                .iter()
                .map(|g| AstType::Path(g.name.clone(), vec![]))
                .collect(),
        );
        let mut path: Vec<String> = qual(prefix, &e.name).split("::").map(String::from).collect();
        path.push(v.name.clone());
        let from_fn = AstFnDecl {
            name: "from".to_string(),
            generics: e.generics.clone(),
            params: vec![AstParam {
                name: "v".to_string(),
                type_: fty.clone(),
                default: None,
                is_mut: false,
                span,
            }],
            return_type: Some(st),
            body: Some(rlyeh_ast::AstBlock {
                stmts: vec![],
                final_expr: Some(AstExpr::new(
                    ExprKind::Call {
                        callee: AstExpr::new(ExprKind::Path(path), span),
                        args: vec![ident("v", span)],
                        type_args: vec![],
                    },
                    span,
                )),
                span,
            }),
            is_pub: false,
            is_async: false,
            is_extern: false,
            span,
        };
        impls.push(AstImplBlock {
            protocol_name: Some("From".to_string()),
            type_name: e.name.clone(),
            generics: e.generics.clone(),
            self_type_args: vec![],
            protocol_type_args: vec![fty],
            extra_protocols: vec![],
            types: vec![],
            methods: vec![from_fn],
            span,
        });
    }
    Ok(impls)
}
