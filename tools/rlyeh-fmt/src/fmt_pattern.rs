//! fmt_pattern：模式 / 类型 / 参数 / 枚举变体的格式化。
//! （由 lib.rs 拆分而来，保持语义等价）

use super::*;
use crate::fmt_expr::fmt_expr;

// ==================== 模式 / 类型 / 参数 ====================

pub(crate) fn fmt_pattern(p: &AstPattern) -> String {
    match p {
        AstPattern::Ident(name) => name.clone(),
        AstPattern::Wildcard => "_".to_string(),
        AstPattern::Literal(l) => fmt_literal_value(l),
        AstPattern::Tuple(ps, _) => format!(
            "({})",
            ps.iter().map(fmt_pattern).collect::<Vec<_>>().join(", ")
        ),
        AstPattern::Struct(name, fields) => {
            let fields = fields
                .iter()
                .map(|(f, fp)| format!("{}: {}", f, fmt_pattern(fp)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} {{ {} }}", name, fields)
        }
        AstPattern::Enum(name, ps) => format!(
            "{}({})",
            name,
            ps.iter().map(fmt_pattern).collect::<Vec<_>>().join(", ")
        ),
        AstPattern::EnumPath(path, ps) => format!(
            "{}({})",
            path.join("::"),
            ps.iter().map(fmt_pattern).collect::<Vec<_>>().join(", ")
        ),
        AstPattern::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => fmt_range_expr(
            Some(lower),
            Some(upper),
            *lower_inclusive,
            *upper_inclusive,
        ),
        AstPattern::Ref(inner, mut_) => {
            let m = if *mut_ { "mut " } else { "" };
            format!("ref {}{}", m, fmt_pattern(inner))
        }
        AstPattern::Or(alts) => alts
            .iter()
            .map(fmt_pattern)
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

pub(crate) fn fmt_literal_value(l: &LiteralValue) -> String {
    match l {
        LiteralValue::Int(v) => v.to_string(),
        LiteralValue::Float(v) => fmt_float(*v),
        LiteralValue::Str(s) => escape_string(s),
        LiteralValue::Char(c) => escape_char(*c),
        LiteralValue::Bool(b) => b.to_string(),
        LiteralValue::Time {
            hour,
            minute,
            is_pm,
        } => fmt_time(*hour, *minute, *is_pm),
    }
}

pub(crate) fn fmt_range_expr(
    lower: Option<&AstExpr>,
    upper: Option<&AstExpr>,
    lower_inclusive: bool,
    upper_inclusive: bool,
) -> String {
    // P8：省略边界——lower=None 输出空串（无 `<` 前缀），upper=None 仅输出区间运算符
    let lo = match lower {
        Some(l) => {
            let s = fmt_operand(l, PREC_COMPARE, false);
            if lower_inclusive {
                s
            } else {
                format!("<{s}")
            }
        }
        None => String::new(),
    };
    let hi = if upper_inclusive { "..." } else { "..<" };
    let hi_s = match upper {
        Some(u) => format!("{hi}{}", fmt_operand(u, PREC_COMPARE, false)),
        None => hi.to_string(),
    };
    format!("{lo}{hi_s}")
}

pub(crate) fn fmt_type(t: &AstType) -> String {
    match t {
        AstType::Path(name, args) => {
            if args.is_empty() {
                name.clone()
            } else {
                format!(
                    "{}<{}>",
                    name,
                    args.iter().map(fmt_type).collect::<Vec<_>>().join(", ")
                )
            }
        }
        AstType::Ref(inner, mut_) => {
            let m = if *mut_ { "&mut " } else { "&" };
            format!("{}{}", m, fmt_type(inner))
        }
        AstType::RawPtr(inner, is_mut) => {
            let m = if *is_mut { "*mut " } else { "*const " };
            format!("{}{}", m, fmt_type(inner))
        }
        AstType::Dyn(name) => format!("dyn {name}"),
        AstType::Tuple(ts) => format!(
            "({})",
            ts.iter().map(fmt_type).collect::<Vec<_>>().join(", ")
        ),
        AstType::Array(t, size) => match size {
            Some(sz) => format!("[{}; {}]", fmt_type(t), fmt_expr(sz)),
            None => format!("[{}]", fmt_type(t)),
        },
        AstType::Fn(params, ret) => format!(
            "fn({}) -> {}",
            params.iter().map(fmt_type).collect::<Vec<_>>().join(", "),
            fmt_type(ret)
        ),
        // U1：类型联合 `A | B | ...`
        AstType::Union(ts) => ts
            .iter()
            .map(fmt_type)
            .collect::<Vec<_>>()
            .join(" | "),
        AstType::Infer => "_".to_string(),
    }
}

/// 格式化泛型参数列表：`T: Bound1 + Bound2`（U3）。
pub(crate) fn fmt_generics(generics: &[AstTypeParam]) -> String {
    generics
        .iter()
        .map(|p| {
            if p.bounds.is_empty() {
                p.name.clone()
            } else {
                format!("{}: {}", p.name, p.bounds.join(" + "))
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn fmt_param(p: &AstParam) -> String {
    // self 接收者特殊形式（与 parser 的 parse_params 一致）：
    //   self       → `self`
    //   &self      → `&self`（type_ = Ref(Self, false)）
    //   &mut self  → `&mut self`（type_ = Ref(Self, true)）
    if p.name == "self" && p.default.is_none() {
        match &p.type_ {
            AstType::Path(name, args) if name == "Self" && args.is_empty() => {
                return "self".to_string();
            }
            AstType::Ref(inner, is_mut) if matches!(inner.as_ref(), AstType::Path(n, a) if n == "Self" && a.is_empty()) => {
                return if *is_mut {
                    "&mut self".to_string()
                } else {
                    "&self".to_string()
                };
            }
            _ => {}
        }
    }
    let mut out = String::new();
    if p.is_mut {
        out.push_str("mut ");
    }
    out.push_str(&p.name);
    out.push_str(&format!(": {}", fmt_type(&p.type_)));
    if let Some(d) = &p.default {
        out.push_str(&format!(" = {}", fmt_expr(d)));
    }
    out
}

pub(crate) fn fmt_enum_variant(v: &AstEnumVariant) -> String {
    if !v.tuple_fields.is_empty() {
        format!(
            "{}({}),",
            v.name,
            v.tuple_fields.iter().map(fmt_type).collect::<Vec<_>>().join(", ")
        )
    } else if !v.struct_fields.is_empty() {
        let fields = v
            .struct_fields
            .iter()
            .map(|f| format!("{}: {}", f.name, fmt_type(&f.type_)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{} {{ {} }},", v.name, fields)
    } else {
        format!("{},", v.name)
    }
}

