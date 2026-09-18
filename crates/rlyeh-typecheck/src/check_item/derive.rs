//! trait derive 宏展开（0.2.0-C）。
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
//! 实现对应 trait（如 `T: Clone`，MVP 未强制校验 bound，违反时在调用点报出）。

use rlyeh_ast::{
    AstExpr, AstFnDecl, AstImplBlock, AstParam, AstStmt, AstStructDecl, AstStructField, AstType,
    BinaryOp, CompareOp, ExprKind,
};
use rlyeh_lexer::Span;

use crate::context::TypeContext;
use crate::error::TypeError;

/// 受支持的 derive trait 名 → 规范 trait 全名。
///
/// 未知名（如 `Serialize` / `Deserialize`，serde 路径）返回 `None` 被忽略，
/// 保持既有「宽松忽略」行为（不报错、不生成 impl）。
fn canonical_trait(name: &str) -> Option<&'static str> {
    match name {
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
            trait_hint: None,
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
    if is_scalar_ast(ty) || matches!(ty, AstType::Ref(_, _)) {
        // 标量 / 引用直接拷贝（无需 .clone()）
        self_field(field, span)
    } else {
        // 其余（`String` / 嵌套 struct/enum / 泛型字段）调用 `.clone()`
        AstExpr::new(
            ExprKind::MethodCall {
                receiver: self_field(field, span),
                method: "clone".to_string(),
                args: vec![],
                trait_hint: None,
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
            type_: AstType::Ref(Box::new(st.clone()), false),
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
        trait_name: Some("Clone".to_string()),
        type_name: s.name.clone(),
        generics: s.generics.clone(),
        trait_type_args: vec![],
        extra_traits: vec![],
        types: vec![],
        methods: vec![clone_fn],
        span,
    }
}

// ---------- Copy（标记 trait，无方法） ----------

fn build_copy_impl(s: &AstStructDecl, _prefix: &str, span: Span) -> AstImplBlock {
    // `Copy` 是零方法标记 trait：`#[derive(Copy)]` 展开为 `impl Copy for T {}`，
    // 供泛型约束 `T: Copy` 经 `type_implements_trait` 命中（与 Rust 语义对齐）。
    AstImplBlock {
        trait_name: Some("Copy".to_string()),
        type_name: s.name.clone(),
        generics: s.generics.clone(),
        trait_type_args: vec![],
        extra_traits: vec![],
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
                type_: AstType::Ref(Box::new(st.clone()), false),
                default: None,
                is_mut: false,
                span,
            },
            AstParam {
                name: "other".to_string(),
                type_: AstType::Ref(Box::new(st.clone()), false),
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
        trait_name: Some("PartialEq".to_string()),
        type_name: s.name.clone(),
        generics: s.generics.clone(),
        trait_type_args: vec![],
        extra_traits: vec![],
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
                    trait_hint: None,
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
                trait_hint: None,
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
                type_: AstType::Ref(Box::new(st.clone()), false),
                default: None,
                is_mut: false,
                span,
            },
            AstParam {
                name: "f".to_string(),
                type_: AstType::Ref(
                    Box::new(AstType::Path("fmt::Formatter".to_string(), vec![])),
                    true,
                ),
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
        trait_name: Some("fmt::Debug".to_string()),
        type_name: s.name.clone(),
        generics: s.generics.clone(),
        trait_type_args: vec![],
        extra_traits: vec![],
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
        let Some(trait_full) = canonical_trait(name) else {
            // 未知 derive 名（serde 等）：忽略
            continue;
        };
        let impl_block = match trait_full {
            "Clone" => build_clone_impl(s, prefix, span),
            "PartialEq" => build_partialeq_impl(s, prefix, span),
            "Copy" => build_copy_impl(s, prefix, span),
            "fmt::Debug" => build_debug_impl(s, prefix, span),
            _ => continue,
        };
        super::collect_impl(ctx, &impl_block, prefix)?;
    }
    Ok(())
}
