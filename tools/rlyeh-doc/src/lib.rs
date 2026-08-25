//! # rlyeh-doc
//!
//! Rlyeh 语言文档生成器：从源码提取 `///` 文档注释（`//!` 为文件级注释），
//! 与 AST 顶层项按源码位置关联，渲染为 Markdown 文档。
//!
//! ## 关联规则
//!
//! - `///` 连续行合并为一个文档块；若它（允许以空行 / 普通注释间隔）紧跟在
//!   某个顶层项之前，则与该项关联。
//! - `//!` 行作为文件级文档，渲染在标题下方。
//! - 无文档注释的项仍会出现在文档中（仅签名 + 位置），保证 API 目录完整。

#![warn(missing_docs)]
#![warn(unsafe_code)]

use std::collections::BTreeSet;

use rlyeh_ast::*;

/// 文档生成选项。
#[derive(Debug, Clone, Default)]
pub struct DocOptions {
    /// 文档标题（缺省为 `Rlyeh 文档`）。
    pub title: Option<String>,
}

/// 文档注释块。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocKind {
    /// `//!` 文件级注释
    File,
    /// `///` 项注释
    Item,
}

#[derive(Debug)]
struct DocBlock {
    kind: DocKind,
    /// 起始行号（1 起；当前用于测试与调试）
    #[allow(dead_code)]
    start_line: usize,
    /// 结束行号（1 起）
    end_line: usize,
    /// 已去除 `///`/`//!` 前缀并合并的正文
    text: String,
}

/// 扫描结果。
#[derive(Debug, Default)]
struct DocScan {
    blocks: Vec<DocBlock>,
}

/// 解析源码并生成 Markdown 文档。
///
/// 解析失败（语法错误）时返回 `Err`。
pub fn doc_source(source: &str, options: &DocOptions) -> Result<String, String> {
    let program = rlyeh_parser::parse(source).map_err(|e| e.to_string())?;
    Ok(doc_program(&program, source, options))
}

/// 从已解析的 AST 生成 Markdown 文档。
///
/// `source` 为原始源码，用于提取 `///` / `//!` 注释与代码行位置。
pub fn doc_program(program: &AstProgram, source: &str, options: &DocOptions) -> String {
    let scan = scan_doc_comments(source);
    let code_lines = collect_code_lines(source);
    let title = options
        .title
        .clone()
        .unwrap_or_else(|| "Rlyeh 文档".to_string());

    let mut out = String::new();
    out.push_str(&format!("# {title}\n\n"));

    // 文件级文档
    for b in scan.blocks.iter().filter(|b| b.kind == DocKind::File) {
        out.push_str(&b.text);
        out.push('\n');
    }
    if scan.blocks.iter().any(|b| b.kind == DocKind::File) {
        out.push('\n');
    }

    // 目录
    out.push_str("## 目录\n\n");
    for item in &program.items {
        out.push_str(&format!(
            "- {} · `{}`\n",
            category_label(item),
            item_title(item)
        ));
    }
    out.push_str("\n---\n\n");

    // 主体：按类别分组，类内保持源码顺序
    let mut funcs: Vec<&AstItem> = Vec::new();
    let mut structs: Vec<&AstItem> = Vec::new();
    let mut enums: Vec<&AstItem> = Vec::new();
    let mut traits: Vec<&AstItem> = Vec::new();
    let mut impls: Vec<&AstItem> = Vec::new();
    let mut actors: Vec<&AstItem> = Vec::new();
    let mut consts: Vec<&AstItem> = Vec::new();
    let mut mods: Vec<&AstItem> = Vec::new();
    let mut others: Vec<&AstItem> = Vec::new();

    for item in &program.items {
        match category(item) {
            "函数" => funcs.push(item),
            "结构体" => structs.push(item),
            "枚举" => enums.push(item),
            "Trait" => traits.push(item),
            "impl" => impls.push(item),
            "Actor" => actors.push(item),
            "常量" => consts.push(item),
            "模块" => mods.push(item),
            _ => others.push(item),
        }
    }

    render_section(&mut out, "函数", &funcs, &scan, &code_lines);
    render_section(&mut out, "结构体", &structs, &scan, &code_lines);
    render_section(&mut out, "枚举", &enums, &scan, &code_lines);
    render_section(&mut out, "Trait", &traits, &scan, &code_lines);
    render_section(&mut out, "impl", &impls, &scan, &code_lines);
    render_section(&mut out, "Actor", &actors, &scan, &code_lines);
    render_section(&mut out, "常量与静态量", &consts, &scan, &code_lines);
    render_section(&mut out, "模块", &mods, &scan, &code_lines);
    render_section(&mut out, "其他项", &others, &scan, &code_lines);

    out
}

/// 渲染一个分组小节。
fn render_section(
    out: &mut String,
    heading: &str,
    items: &[&AstItem],
    scan: &DocScan,
    code_lines: &BTreeSet<usize>,
) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("## {heading}\n\n"));
    for item in items {
        render_item(out, item, scan, code_lines);
    }
}

/// 渲染单个顶层项。
fn render_item(out: &mut String, item: &AstItem, scan: &DocScan, code_lines: &BTreeSet<usize>) {
    let span = item_span(item);
    let doc = find_doc_for(scan, code_lines, span.line);

    out.push_str(&format!("### `{}`\n\n", item_title(item)));
    out.push_str("```rlyeh\n");
    out.push_str(&item_signature(item));
    out.push_str("\n```\n\n");
    out.push_str(&format!("> 位置: {}:{}\n\n", span.line, span.col));
    if let Some(doc) = doc {
        out.push_str(&doc.text);
        out.push('\n');
        out.push('\n');
    }

    // 聚合类型的成员列表
    match item {
        AstItem::TraitDecl(t) if !t.methods.is_empty() => {
            out.push_str("抽象方法:\n\n");
            for m in &t.methods {
                out.push_str(&format!("- `{}`\n", fn_signature(m)));
            }
            out.push('\n');
        }
        AstItem::ImplBlock(i) if !i.methods.is_empty() => {
            out.push_str("方法:\n\n");
            for m in &i.methods {
                out.push_str(&format!("- `{}`\n", fn_signature(m)));
            }
            out.push('\n');
        }
        AstItem::ActorDecl(a) => {
            if !a.fields.is_empty() {
                out.push_str("字段:\n\n");
                for f in &a.fields {
                    out.push_str(&format!(
                        "- `{}: {}`{}\n",
                        f.name,
                        fmt_type(&f.type_),
                        if f.is_pub { " · pub" } else { "" }
                    ));
                }
                out.push('\n');
            }
            if !a.methods.is_empty() {
                out.push_str("方法:\n\n");
                for m in &a.methods {
                    out.push_str(&format!("- `{}`\n", fn_signature(m)));
                }
                out.push('\n');
            }
        }
        AstItem::ModDecl(m) if !m.external && !m.items.is_empty() => {
            out.push_str("子项:\n\n");
            for sub in &m.items {
                out.push_str(&format!("- {} · `{}`\n", category_label(sub), item_title(sub)));
            }
            out.push('\n');
        }
        _ => {}
    }
}

/// 项的分类标签（用于目录与分组）。
fn category(item: &AstItem) -> &'static str {
    match item {
        AstItem::FnDecl(_) => "函数",
        AstItem::StructDecl(_) => "结构体",
        AstItem::EnumDecl(_) => "枚举",
        AstItem::TraitDecl(_) => "Trait",
        AstItem::ImplBlock(_) => "impl",
        AstItem::ModDecl(_) => "模块",
        AstItem::UseDecl(_) => "其他项",
        AstItem::ConstDecl(_) => "常量",
        AstItem::ActorDecl(_) => "Actor",
        AstItem::MacroDecl(_) => "其他项",
        AstItem::Statement(_) => "其他项",
    }
}

/// 与 [`category`] 对应的中文标签。
fn category_label(item: &AstItem) -> &'static str {
    category(item)
}

/// 项的简短标题（`fn foo` / `struct Point<T>` / `actor Counter`）。
fn item_title(item: &AstItem) -> String {
    match item {
        AstItem::FnDecl(f) => {
            let mut s = String::new();
            if f.is_pub {
                s.push_str("pub ");
            }
            if f.is_async {
                s.push_str("async ");
            }
            if f.is_extern {
                s.push_str("extern ");
            }
            s.push_str("fn ");
            s.push_str(&f.name);
            if !f.generics.is_empty() {
                s.push_str(&format!("<{}>", f.generics.join(", ")));
            }
            s
        }
        AstItem::StructDecl(s) => {
            let mut t = format!("struct {}", s.name);
            if !s.generics.is_empty() {
                t.push_str(&format!("<{}>", s.generics.join(", ")));
            }
            t
        }
        AstItem::EnumDecl(e) => {
            let mut t = format!("enum {}", e.name);
            if !e.generics.is_empty() {
                t.push_str(&format!("<{}>", e.generics.join(", ")));
            }
            t
        }
        AstItem::TraitDecl(t) => {
            let mut s = format!("trait {}", t.name);
            if !t.generics.is_empty() {
                s.push_str(&format!("<{}>", t.generics.join(", ")));
            }
            s
        }
        AstItem::ImplBlock(i) => {
            let mut s = String::from("impl ");
            if let Some(t) = &i.trait_name {
                s.push_str(&format!("{t} for "));
            }
            s.push_str(&i.type_name);
            if !i.generics.is_empty() {
                s.push_str(&format!("<{}>", i.generics.join(", ")));
            }
            s
        }
        AstItem::ModDecl(m) => format!("mod {}{}", m.name, if m.external { ";" } else { "" }),
        AstItem::UseDecl(u) => {
            let mut s = format!("use {}", u.path.join("::"));
            if let Some(a) = &u.alias {
                s.push_str(&format!(" as {a}"));
            }
            s
        }
        AstItem::ConstDecl(c) => format!(
            "{} {}",
            if c.is_static { "static" } else { "const" },
            c.name
        ),
        AstItem::ActorDecl(a) => format!("actor {}", a.name),
        AstItem::MacroDecl(m) => {
            let mut s = format!("macro {}", m.name);
            if !m.params.is_empty() {
                s.push_str(&format!("({})", m.params.join(", ")));
            }
            s
        }
        AstItem::Statement(_) => "顶层语句".to_string(),
    }
}

/// 项的完整签名（多行渲染时的代码块内容）。
pub fn item_signature(item: &AstItem) -> String {
    match item {
        AstItem::FnDecl(f) => fn_signature(f),
        AstItem::StructDecl(s) => struct_signature(s),
        AstItem::EnumDecl(e) => enum_signature(e),
        AstItem::TraitDecl(t) => {
            let mut s = format!("trait {}", t.name);
            if !t.generics.is_empty() {
                s.push_str(&format!("<{}>", t.generics.join(", ")));
            }
            if !t.methods.is_empty() {
                s.push_str(" {\n");
                for m in &t.methods {
                    for line in fn_signature(m).split('\n') {
                        s.push_str("    ");
                        s.push_str(line);
                        s.push('\n');
                    }
                }
                s.push('}');
            }
            s
        }
        AstItem::ImplBlock(i) => {
            let mut s = String::from("impl ");
            if let Some(t) = &i.trait_name {
                s.push_str(&format!("{t} for "));
            }
            s.push_str(&i.type_name);
            if !i.generics.is_empty() {
                s.push_str(&format!("<{}>", i.generics.join(", ")));
            }
            if !i.methods.is_empty() {
                s.push_str(" {\n");
                for m in &i.methods {
                    for line in fn_signature(m).split('\n') {
                        s.push_str("    ");
                        s.push_str(line);
                        s.push('\n');
                    }
                }
                s.push('}');
            }
            s
        }
        AstItem::ModDecl(m) => {
            let mut s = format!("mod {}", m.name);
            if m.external {
                s.push(';');
            } else if !m.items.is_empty() {
                s.push_str(" {\n");
                for sub in &m.items {
                    s.push_str("    ");
                    s.push_str(&item_title(sub).replace('\n', "\n    "));
                    s.push('\n');
                }
                s.push('}');
            }
            s
        }
        AstItem::UseDecl(u) => {
            let mut s = format!("use {}", u.path.join("::"));
            if let Some(a) = &u.alias {
                s.push_str(&format!(" as {a}"));
            }
            s.push(';');
            s
        }
        AstItem::ConstDecl(c) => {
            let mut s = if c.is_static {
                "static ".to_string()
            } else {
                "const ".to_string()
            };
            s.push_str(&c.name);
            if let Some(t) = &c.type_ {
                s.push_str(&format!(": {}", fmt_type(t)));
            }
            s
        }
        AstItem::ActorDecl(a) => {
            let mut s = format!("actor {}", a.name);
            if !a.fields.is_empty() {
                s.push_str(" {\n");
                for f in &a.fields {
                    s.push_str(&format!(
                        "    {}: {}{}\n",
                        f.name,
                        fmt_type(&f.type_),
                        if f.default.is_some() { " = ..." } else { "" }
                    ));
                }
                for m in &a.methods {
                    for line in fn_signature(m).split('\n') {
                        s.push_str("    ");
                        s.push_str(line);
                        s.push('\n');
                    }
                }
                s.push('}');
            }
            s
        }
        AstItem::MacroDecl(m) => {
            let mut s = format!("macro {}", m.name);
            if !m.params.is_empty() {
                s.push_str(&format!("({})", m.params.join(", ")));
            }
            s
        }
        AstItem::Statement(_) => item_title(item),
    }
}

/// 结构体签名。
fn struct_signature(s: &AstStructDecl) -> String {
    let mut out = format!("struct {}", s.name);
    if !s.generics.is_empty() {
        out.push_str(&format!("<{}>", s.generics.join(", ")));
    }
    if !s.fields.is_empty() {
        out.push_str(" { ");
        out.push_str(
            &s.fields
                .iter()
                .map(|f| {
                    format!(
                        "{}{}: {}",
                        if f.is_pub { "pub " } else { "" },
                        f.name,
                        fmt_type(&f.type_)
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        out.push_str(" }");
    }
    out
}

/// 枚举签名。
fn enum_signature(e: &AstEnumDecl) -> String {
    let mut out = format!("enum {}", e.name);
    if !e.generics.is_empty() {
        out.push_str(&format!("<{}>", e.generics.join(", ")));
    }
    if !e.variants.is_empty() {
        out.push_str(" { ");
        out.push_str(
            &e.variants
                .iter()
                .map(|v| {
                    let mut s = v.name.clone();
                    if !v.tuple_fields.is_empty() {
                        s.push_str(&format!(
                            "({})",
                            v.tuple_fields.iter().map(fmt_type).collect::<Vec<_>>().join(", ")
                        ));
                    } else if !v.struct_fields.is_empty() {
                        s.push_str(&format!(
                            " {{ {} }}",
                            v.struct_fields
                                .iter()
                                .map(|f| format!("{}: {}", f.name, fmt_type(&f.type_)))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    s
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        out.push_str(" }");
    }
    out
}

/// 函数签名（含修饰符）。
pub fn fn_signature(f: &AstFnDecl) -> String {
    let mut out = String::new();
    if f.is_pub {
        out.push_str("pub ");
    }
    if f.is_async {
        out.push_str("async ");
    }
    if f.is_extern {
        out.push_str("extern ");
    }
    out.push_str("fn ");
    out.push_str(&f.name);
    if !f.generics.is_empty() {
        out.push_str(&format!("<{}>", f.generics.join(", ")));
    }
    out.push('(');
    out.push_str(
        &f.params
            .iter()
            .map(fmt_param)
            .collect::<Vec<_>>()
            .join(", "),
    );
    out.push(')');
    if let Some(ret) = &f.return_type {
        out.push_str(&format!(" -> {}", fmt_type(ret)));
    }
    out
}

/// 参数格式化（`self` / `&self` / `&mut self` 特判）。
fn fmt_param(p: &AstParam) -> String {
    if p.name == "self" && p.default.is_none() {
        return match &p.type_ {
            AstType::Ref(inner, true) if is_self_type(inner) => "&mut self".to_string(),
            AstType::Ref(inner, false) if is_self_type(inner) => "&self".to_string(),
            _ if is_self_type(&p.type_) => "self".to_string(),
            _ => format!("self: {}", fmt_type(&p.type_)),
        };
    }
    let mut out = if p.is_mut {
        format!("mut {}", p.name)
    } else {
        p.name.clone()
    };
    out.push_str(": ");
    out.push_str(&fmt_type(&p.type_));
    out
}

/// 判断类型是否为 `Self` / `Self<T>`。
fn is_self_type(t: &AstType) -> bool {
    matches!(t, AstType::Path(n, _) if n == "Self")
}

/// 类型格式化。
pub fn fmt_type(t: &AstType) -> String {
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
            let base = fmt_type(inner);
            if *mut_ {
                format!("&mut {base}")
            } else {
                format!("&{base}")
            }
        }
        AstType::RawPtr(inner, is_mut) => {
            let base = fmt_type(inner);
            if *is_mut {
                format!("*mut {base}")
            } else {
                format!("*const {base}")
            }
        }
        AstType::Dyn(name) => format!("dyn {name}"),
        AstType::Tuple(ts) => format!("({})", ts.iter().map(fmt_type).collect::<Vec<_>>().join(", ")),
        AstType::Array(inner, len) => match len {
            Some(_) => format!("[{}; ...]", fmt_type(inner)),
            None => format!("[{}]", fmt_type(inner)),
        },
        AstType::Fn(params, ret) => format!(
            "fn({}) -> {}",
            params.iter().map(fmt_type).collect::<Vec<_>>().join(", "),
            fmt_type(ret)
        ),
        AstType::Infer => "_".to_string(),
    }
}

/// 顶层项的位置。
fn item_span(item: &AstItem) -> &rlyeh_lexer::Span {
    match item {
        AstItem::FnDecl(f) => &f.span,
        AstItem::StructDecl(s) => &s.span,
        AstItem::EnumDecl(e) => &e.span,
        AstItem::TraitDecl(t) => &t.span,
        AstItem::ImplBlock(i) => &i.span,
        AstItem::ModDecl(m) => &m.span,
        AstItem::UseDecl(u) => &u.span,
        AstItem::ConstDecl(c) => &c.span,
        AstItem::ActorDecl(a) => &a.span,
        AstItem::MacroDecl(m) => &m.span,
        AstItem::Statement(s) => stmt_span(s),
    }
}

/// 语句位置（`Let` 无 span，用初始化表达式位置近似）。
fn stmt_span(s: &AstStmt) -> &rlyeh_lexer::Span {
    match s {
        AstStmt::Let { init, .. } => &init.span,
        AstStmt::Expr(e) => &e.span,
        AstStmt::Semi(e) => &e.span,
        AstStmt::Item(i) => item_span(i),
    }
}

/// 收集所有"代码行"（非空、非注释行）的行号。
fn collect_code_lines(source: &str) -> BTreeSet<usize> {
    let mut set = BTreeSet::new();
    for (idx, raw) in source.lines().enumerate() {
        let t = raw.trim_start();
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        set.insert(idx + 1);
    }
    set
}

/// 扫描源码，提取 `///` 与 `//!` 文档注释块。
fn scan_doc_comments(source: &str) -> DocScan {
    let mut scan = DocScan::default();
    let mut cur: Option<DocBlock> = None;

    for (idx, raw) in source.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw.trim_start();
        let lead = raw.len() - trimmed.len();

        let (kind, body) = if trimmed.strip_prefix("///").is_some() {
            (DocKind::Item, raw.get(lead + 3..).unwrap_or(""))
        } else if trimmed.strip_prefix("//!").is_some() {
            (DocKind::File, raw.get(lead + 3..).unwrap_or(""))
        } else {
            // 非注释行：终结当前块
            if let Some(b) = cur.take() {
                scan.blocks.push(b);
            }
            continue;
        };
        let body = body.strip_prefix(' ').unwrap_or(body);

        if let Some(b) = cur.as_mut() {
            if b.kind == kind && b.end_line == line_no - 1 {
                b.text.push('\n');
                b.text.push_str(body);
                b.end_line = line_no;
                continue;
            }
        }
        // 新块（或块类型切换）
        if let Some(b) = cur.take() {
            scan.blocks.push(b);
        }
        cur = Some(DocBlock {
            kind,
            start_line: line_no,
            end_line: line_no,
            text: body.to_string(),
        });
    }
    if let Some(b) = cur.take() {
        scan.blocks.push(b);
    }
    scan
}

/// 为指定项行号查找关联的 `///` 文档块。
///
/// 规则：最近的 `///` 块，且该块结束行与项起始行之间不含任何代码行。
fn find_doc_for<'a>(
    scan: &'a DocScan,
    code_lines: &BTreeSet<usize>,
    item_line: usize,
) -> Option<&'a DocBlock> {
    scan.blocks
        .iter()
        .filter(|b| b.kind == DocKind::Item && b.end_line < item_line)
        .rev()
        .find(|b| (b.end_line + 1..item_line).all(|l| !code_lines.contains(&l)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_merges_adjacent_item_docs() {
        let src = "/// a\n/// b\nfn f() {}\n";
        let scan = scan_doc_comments(src);
        assert_eq!(scan.blocks.len(), 1);
        assert_eq!(scan.blocks[0].kind, DocKind::Item);
        assert_eq!(scan.blocks[0].text, "a\nb");
        assert_eq!(scan.blocks[0].start_line, 1);
        assert_eq!(scan.blocks[0].end_line, 2);
    }

    #[test]
    fn scan_separates_file_and_item_docs() {
        let src = "//! module doc\n\n/// item doc\nfn f() {}\n";
        let scan = scan_doc_comments(src);
        assert_eq!(scan.blocks.len(), 2);
        assert_eq!(scan.blocks[0].kind, DocKind::File);
        assert_eq!(scan.blocks[1].kind, DocKind::Item);
    }

    #[test]
    fn associate_skips_blank_and_comment_lines() {
        let src = "/// doc\n// plain\n\nfn f() {}\n";
        let scan = scan_doc_comments(src);
        let code = collect_code_lines(src);
        let doc = find_doc_for(&scan, &code, 4);
        assert!(doc.is_some());
        assert_eq!(doc.unwrap().text, "doc");
    }

    #[test]
    fn associate_rejects_code_between_doc_and_item() {
        let src = "/// doc\nlet x = 1;\nfn f() {}\n";
        let scan = scan_doc_comments(src);
        let code = collect_code_lines(src);
        assert!(find_doc_for(&scan, &code, 3).is_none());
    }

    #[test]
    fn signature_self_param() {
        let prog = rlyeh_parser::parse("impl Foo { fn get(&self) -> i64 { 1 } }").unwrap();
        let AstItem::ImplBlock(i) = &prog.items[0] else {
            panic!("expected impl");
        };
        assert_eq!(fn_signature(&i.methods[0]), "fn get(&self) -> i64");
    }
}
