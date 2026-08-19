//! 顶层项检查（函数签名收集 + 函数体 / const 检查）。

use zeta_ast::{AstFnDecl, AstItem, AstProgram, AstStructDecl};
use zeta_hir::{HirConstDecl, HirFnDecl, HirItem, HirItemKind, HirParam, HirProgram};
use zeta_lexer::Span;

use crate::check_expr::{check_block, infer_expr, resolve_ast_type};
use crate::context::TypeContext;
use crate::error::TypeError;
use crate::types::{FnSignature, StructDef, Type};

/// 类型检查完整程序。
///
/// 两遍流程：
/// 1. 收集结构体定义、类型别名与所有函数签名（支持函数间互调）；
/// 2. 检查函数体与 const 初始值，生成 HIR。
pub fn typecheck(program: &AstProgram) -> Result<HirProgram, TypeError> {
    let mut ctx = TypeContext::new();
    collect_declarations(&mut ctx, program)?;

    let mut items = Vec::new();
    for item in &program.items {
        if let Some(hir_item) = check_item(&mut ctx, item)? {
            items.push(hir_item);
        }
    }
    Ok(HirProgram { items })
}

/// 第一遍：收集结构体 / 函数签名。
fn collect_declarations(ctx: &mut TypeContext, program: &AstProgram) -> Result<(), TypeError> {
    for item in &program.items {
        match item {
            AstItem::StructDecl(s) => collect_struct(ctx, s)?,
            AstItem::FnDecl(f) => {
                let sig = fn_signature(ctx, f, f.span)?;
                ctx.insert_fn_signature(f.name.clone(), sig);
            }
            _ => {}
        }
    }
    Ok(())
}

/// 第二遍：检查函数体 / const，生成 HIR 项。
pub(crate) fn check_item(
    ctx: &mut TypeContext,
    item: &AstItem,
) -> Result<Option<HirItem>, TypeError> {
    match item {
        AstItem::FnDecl(f) => {
            let body = check_fn_body(ctx, f)?;
            let params = f
                .params
                .iter()
                .map(|p| HirParam {
                    name: p.name.clone(),
                })
                .collect();
            Ok(Some(HirItem {
                name: f.name.clone(),
                kind: HirItemKind::Fn(HirFnDecl { params, body }),
            }))
        }
        AstItem::ConstDecl(c) => {
            let (value, _) = infer_expr(ctx, &c.value)?;
            Ok(Some(HirItem {
                name: c.name.clone(),
                kind: HirItemKind::Const(HirConstDecl { value }),
            }))
        }
        _ => Ok(None),
    }
}

/// 收集结构体字段定义。
fn collect_struct(ctx: &mut TypeContext, s: &AstStructDecl) -> Result<(), TypeError> {
    let mut fields = Vec::with_capacity(s.fields.len());
    for field in &s.fields {
        let ty = resolve_ast_type(ctx, &field.type_, field.span)?;
        fields.push((field.name.clone(), ty));
    }
    ctx.insert_struct(s.name.clone(), StructDef { fields });
    Ok(())
}

/// 从函数声明解析签名。
fn fn_signature(ctx: &TypeContext, f: &AstFnDecl, span: Span) -> Result<FnSignature, TypeError> {
    let mut params = Vec::with_capacity(f.params.len());
    for p in &f.params {
        params.push(resolve_ast_type(ctx, &p.type_, span)?);
    }
    let return_type = match &f.return_type {
        Some(t) => resolve_ast_type(ctx, t, span)?,
        None => Type::Unit,
    };
    Ok(FnSignature {
        params,
        return_type,
    })
}

/// 检查函数体：参数入作用域，检查块，返回类型一致性。
fn check_fn_body(
    ctx: &mut TypeContext,
    f: &AstFnDecl,
) -> Result<Option<zeta_hir::HirBlock>, TypeError> {
    let body = match &f.body {
        Some(b) => b,
        None => {
            // trait 抽象方法允许无函数体
            if f.is_pub {
                return Ok(None);
            }
            return Err(TypeError::MissingFunctionBody {
                name: f.name.clone(),
                span: f.span,
            });
        }
    };

    // 参数进入局部作用域
    let saved = std::mem::take(&mut ctx.variables);
    for p in &f.params {
        let ty = resolve_ast_type(ctx, &p.type_, f.span)?;
        ctx.insert_variable(p.name.clone(), ty);
    }

    // 返回类型检查（签名在收集阶段已存入，此处重新解析以保持一致性）
    let return_type = fn_signature(ctx, f, f.span)?.return_type;

    let (hir_body, body_ty) = check_block(ctx, body)?;

    // 返回类型一致性：函数体类型应兼容声明的返回类型
    if body_ty != Type::Never && !body_ty.compatible_with(&return_type) {
        ctx.variables = saved;
        return Err(TypeError::WrongType {
            expected: return_type.to_string(),
            found: body_ty.to_string(),
            span: f.span,
        });
    }

    ctx.variables = saved;
    Ok(Some(hir_body))
}
