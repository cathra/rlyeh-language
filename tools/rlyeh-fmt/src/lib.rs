//! # rlyeh-fmt
//!
//! Rlyeh 语言代码格式化器：解析源码为 AST，再按统一规范重新打印。
//!
//! ## 格式化约定
//!
//! - 缩进 4 空格；顶层项之间空一行。
//! - `fn`/`if`/`match`/`for`/`while`/`loop`/`region` 等块一律换行展开。
//! - 表达式按优先级括号重建，保证语义不变（宁可多括号不丢括号）。
//! - 字符串/字符按 lexer 支持的转义规则重新编码（`\"`、`\\`、`\n`、`\xNN`）。
//! - 注释不保留（MVP 基于 AST 重建，源码注释在词法阶段即被丢弃）。
//!
//! 格式化输出保证幂等：对同一 AST 连续格式化结果不变，且格式化结果可再次解析。

#![warn(missing_docs)]
#![warn(unsafe_code)]

use rlyeh_ast::*;

/// 格式化选项。
#[derive(Debug, Clone, Copy)]
pub struct FmtOptions {
    /// 缩进宽度（空格数），默认 4。
    pub indent_width: usize,
}

impl Default for FmtOptions {
    fn default() -> Self {
        Self { indent_width: 4 }
    }
}

/// 解析并格式化源码，解析失败返回错误信息。
pub fn format_source(source: &str) -> Result<String, String> {
    format_source_with_options(source, &FmtOptions::default())
}

/// 解析并格式化源码（自定义选项）。
pub fn format_source_with_options(source: &str, opts: &FmtOptions) -> Result<String, String> {
    match rlyeh_parser::parse(source) {
        Ok(prog) => Ok(format_program_with_options(&prog, opts)),
        Err(e) => Err(e.to_string()),
    }
}

/// 格式化已解析的 AST 程序。
pub fn format_program(prog: &AstProgram) -> String {
    format_program_with_options(prog, &FmtOptions::default())
}

/// 格式化已解析的 AST 程序（自定义选项）。
pub fn format_program_with_options(prog: &AstProgram, opts: &FmtOptions) -> String {
    let mut p = Printer::new(opts.indent_width);
    p.print_program(prog);
    p.finish()
}

// ==================== Printer ====================

struct Printer {
    buf: String,
    indent: usize,
    width: usize,
}

impl Printer {
    fn new(width: usize) -> Self {
        Self {
            buf: String::new(),
            indent: 0,
            width,
        }
    }

    /// 收尾：去掉行尾空白，保证以单个换行结尾。
    fn finish(mut self) -> String {
        while self.buf.ends_with('\n') {
            self.buf.pop();
        }
        self.buf.push('\n');
        self.buf
    }

    /// 输出一行（带当前缩进）。
    fn line(&mut self, s: &str) {
        if !s.is_empty() {
            for _ in 0..self.indent * self.width {
                self.buf.push(' ');
            }
            self.buf.push_str(s);
        }
        self.buf.push('\n');
    }

    /// 在当前行尾部追加文本（不换行），供「`let x = ` + 多行块」使用。
    fn open(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    /// 空行。
    fn blank(&mut self) {
        self.buf.push('\n');
    }

    /// 缩进一级执行闭包。
    fn with_indent(&mut self, f: impl FnOnce(&mut Self)) {
        self.indent += 1;
        f(self);
        self.indent -= 1;
    }
}

// ==================== 程序 / 顶层项 ====================

impl Printer {
    fn print_program(&mut self, prog: &AstProgram) {
        for (i, item) in prog.items.iter().enumerate() {
            if i > 0 {
                self.blank();
            }
            self.print_item(item);
        }
    }

    fn print_item(&mut self, item: &AstItem) {
        match item {
            AstItem::FnDecl(f) => self.print_fn_decl(f, true),
            AstItem::StructDecl(s) => self.print_struct_decl(s),
            AstItem::EnumDecl(e) => self.print_enum_decl(e),
            AstItem::TraitDecl(t) => self.print_trait_decl(t),
            AstItem::ImplBlock(i) => self.print_impl_block(i),
            AstItem::ModDecl(m) => self.print_mod_decl(m),
            AstItem::UseDecl(u) => self.print_use_decl(u),
            AstItem::ConstDecl(c) => self.print_const_decl(c),
            AstItem::ActorDecl(a) => self.print_actor_decl(a),
            AstItem::MacroDecl(m) => {
                let params = if m.params.is_empty() {
                    String::new()
                } else {
                    format!("({})", m.params.join(", "))
                };
                self.line(&format!("macro {}{} {{", m.name, params));
                for l in m.body.lines() {
                    self.line(l);
                }
                self.line("}");
            }
            AstItem::Statement(stmt) => self.print_stmt(stmt),
        }
    }

    fn print_fn_decl(&mut self, f: &AstFnDecl, allow_pub: bool) {
        let mut head = String::new();
        if allow_pub && f.is_pub {
            head.push_str("pub ");
        }
        if f.is_extern {
            head.push_str("extern ");
        }
        if f.is_async {
            head.push_str("async ");
        }
        head.push_str("fn ");
        head.push_str(&f.name);
        if !f.generics.is_empty() {
            head.push_str(&format!("<{}>", fmt_generics(&f.generics)));
        }
        head.push('(');
        let params = f
            .params
            .iter()
            .map(fmt_param)
            .collect::<Vec<_>>()
            .join(", ");
        head.push_str(&params);
        head.push(')');
        if let Some(rt) = &f.return_type {
            head.push_str(&format!(" -> {}", fmt_type(rt)));
        }
        match &f.body {
            Some(block) => {
                self.line(&format!("{} {{", head));
                self.with_indent(|p| p.print_block_body(block));
                self.line("}");
            }
            None => self.line(&format!("{};", head)),
        }
    }

    fn print_struct_decl(&mut self, s: &AstStructDecl) {
        let mut head = format!("struct {}", s.name);
        if !s.generics.is_empty() {
            head.push_str(&format!("<{}>", fmt_generics(&s.generics)));
        }
        head.push_str(" {");
        let fields = s
            .fields
            .iter()
            .map(|f| {
                let vis = if f.is_pub { "pub " } else { "" };
                format!("{}{}: {}", vis, f.name, fmt_type(&f.type_))
            })
            .collect::<Vec<_>>()
            .join(", ");
        self.line(&format!("{} {} }}", head, fields));
    }

    fn print_enum_decl(&mut self, e: &AstEnumDecl) {
        let mut head = format!("enum {}", e.name);
        if !e.generics.is_empty() {
            head.push_str(&format!("<{}>", fmt_generics(&e.generics)));
        }
        self.line(&format!("{} {{", head));
        self.with_indent(|p| {
            for v in &e.variants {
                p.line(&fmt_enum_variant(v));
            }
        });
        self.line("}");
    }

    fn print_trait_decl(&mut self, t: &AstTraitDecl) {
        let mut head = format!("trait {}", t.name);
        if !t.generics.is_empty() {
            head.push_str(&format!("<{}>", fmt_generics(&t.generics)));
        }
        self.line(&format!("{} {{", head));
        self.with_indent(|p| {
            for m in &t.methods {
                p.print_fn_decl(m, false);
            }
        });
        self.line("}");
    }

    fn print_impl_block(&mut self, i: &AstImplBlock) {
        let mut head = String::from("impl");
        if !i.generics.is_empty() {
            head.push_str(&format!("<{}>", fmt_generics(&i.generics)));
        }
        head.push(' ');
        match &i.trait_name {
            Some(t) => head.push_str(&format!("{} for {}", t, i.type_name)),
            None => head.push_str(&i.type_name),
        }
        self.line(&format!("{} {{", head));
        self.with_indent(|p| {
            for m in &i.methods {
                p.print_fn_decl(m, false);
            }
        });
        self.line("}");
    }

    fn print_mod_decl(&mut self, m: &AstModDecl) {
        if m.external {
            self.line(&format!("mod {};", m.name));
            return;
        }
        self.line(&format!("mod {} {{", m.name));
        self.with_indent(|p| {
            for item in &m.items {
                p.print_item(item);
            }
        });
        self.line("}");
    }

    fn print_use_decl(&mut self, u: &AstUseDecl) {
        let path = u.path.join("::");
        match &u.alias {
            Some(a) => self.line(&format!("use {} as {};", path, a)),
            None => self.line(&format!("use {};", path)),
        }
    }

    fn print_const_decl(&mut self, c: &AstConstDecl) {
        let kw = if c.is_static { "static" } else { "const" };
        let mut head = format!("{} {}", kw, c.name);
        if let Some(t) = &c.type_ {
            head.push_str(&format!(": {}", fmt_type(t)));
        }
        head.push_str(" = ");
        self.print_expr_value(&head, &c.value, ";");
    }

    fn print_actor_decl(&mut self, a: &AstActorDecl) {
        self.line(&format!("actor {} {{", a.name));
        self.with_indent(|p| {
            for f in &a.fields {
                let vis = if f.is_pub { "pub " } else { "" };
                let mut line = format!("{}{}: {}", vis, f.name, fmt_type(&f.type_));
                if let Some(d) = &f.default {
                    line.push_str(&format!(" = {}", fmt_expr(d)));
                }
                line.push(',');
                p.line(&line);
            }
            if !a.fields.is_empty() && !a.methods.is_empty() {
                p.blank();
            }
            for m in &a.methods {
                p.print_fn_decl(m, true);
            }
        });
        self.line("}");
    }
}

// ==================== 块 / 语句 ====================

impl Printer {
    fn print_block_body(&mut self, b: &AstBlock) {
        for stmt in &b.stmts {
            self.print_stmt(stmt);
        }
        if let Some(fe) = &b.final_expr {
            self.print_expr_value("", fe, "");
        }
    }

    fn print_stmt(&mut self, s: &AstStmt) {
        match s {
            AstStmt::Let {
                pattern,
                type_anno,
                init,
                mutable,
            } => {
                let mut head = String::from("let ");
                if *mutable {
                    head.push_str("mut ");
                }
                head.push_str(&fmt_pattern(pattern));
                if let Some(t) = type_anno {
                    head.push_str(&format!(": {}", fmt_type(t)));
                }
                head.push_str(" = ");
                self.print_expr_value(&head, init, ";");
            }
            // 注意：parser 中 `expr;` → AstStmt::Expr（分号被吃掉）；
            // 块类表达式无分号跟在下一条语句前 → AstStmt::Semi（本无分号）。
            AstStmt::Expr(e) => self.print_expr_value("", e, ";"),
            AstStmt::Semi(e) => self.print_expr_value("", e, ""),
            AstStmt::Item(item) => self.print_item(item),
        }
    }

    /// 打印一个「值位置」的表达式：块类表达式多行展开（前缀 + 块 + 后缀），
    /// 其余表达式单行（前缀 + 单行 + 后缀）。
    fn print_expr_value(&mut self, prefix: &str, e: &AstExpr, suffix: &str) {
        if is_block_like(e) {
            self.open(prefix);
            self.print_block_like(e, suffix);
        } else {
            self.line(&format!("{}{}{}", prefix, fmt_expr(e), suffix));
        }
    }

    /// 多行打印块类表达式（If/Match/For/While/Loop/Region/Block），结尾 `}` 后附 suffix。
    fn print_block_like(&mut self, e: &AstExpr, suffix: &str) {
        match e.kind.as_ref() {
            ExprKind::Block(b) => {
                self.line("{");
                self.with_indent(|p| p.print_block_body(b));
                self.line(&format!("}}{}", suffix));
            }
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                self.line(&format!("if {} {{", fmt_expr(cond)));
                self.with_indent(|p| p.print_block_body(then_block));
                match else_block {
                    Some(el) => {
                        self.line("} else {");
                        self.with_indent(|p| p.print_block_body(el));
                        self.line(&format!("}}{}", suffix));
                    }
                    None => self.line(&format!("}}{}", suffix)),
                }
            }
            ExprKind::Match { expr, arms } => {
                self.line(&format!("match {} {{", fmt_expr(expr)));
                self.with_indent(|p| {
                    for arm in arms {
                        let mut line = fmt_pattern(&arm.pattern);
                        if let Some(g) = &arm.guard {
                            line.push_str(&format!(" if {}", fmt_expr(g)));
                        }
                        line.push_str(&format!(" => {}", fmt_expr(&arm.body)));
                        line.push(',');
                        p.line(&line);
                    }
                });
                self.line(&format!("}}{}", suffix));
            }
            ExprKind::For {
                pattern,
                iterator,
                body,
            } => {
                self.line(&format!(
                    "for {} in {} {{",
                    fmt_pattern(pattern),
                    fmt_expr(iterator)
                ));
                self.with_indent(|p| p.print_block_body(body));
                self.line(&format!("}}{}", suffix));
            }
            ExprKind::While { cond, body } => {
                self.line(&format!("while {} {{", fmt_expr(cond)));
                self.with_indent(|p| p.print_block_body(body));
                self.line(&format!("}}{}", suffix));
            }
            ExprKind::Loop { body } => {
                self.line("loop {");
                self.with_indent(|p| p.print_block_body(body));
                self.line(&format!("}}{}", suffix));
            }
            ExprKind::GcRegion { body } => {
                self.line("gc_region {");
                self.with_indent(|p| p.print_block_body(body));
                self.line(&format!("}}{}", suffix));
            }
            ExprKind::Region {
                name,
                options,
                body,
            } => {
                let mut head = String::from("region");
                if let Some(n) = name {
                    head.push_str(&format!(" '{}", n));
                }
                if options.adaptive {
                    head.push_str(" adaptive");
                }
                if options.exact {
                    head.push_str(" exact");
                }
                if let Some(sz) = options.size {
                    head.push_str(&format!(" with_size({})", sz));
                }
                if options.allow_growth {
                    if let Some(g) = options.growth_factor {
                        head.push_str(&format!(" allow_growth(growth_factor={})", g));
                    }
                } else {
                    head.push_str(" allow_growth(false)");
                }
                self.line(&format!("{} {{", head));
                self.with_indent(|p| p.print_block_body(body));
                self.line(&format!("}}{}", suffix));
            }
            // 其余（不应出现）退化为单行
            _ => self.line(&format!("{}{}", fmt_expr(e), suffix)),
        }
    }
}

// ==================== 表达式（单行重建） ====================

/// 判断表达式是否为块类（需要多行展开）。
fn is_block_like(e: &AstExpr) -> bool {
    matches!(
        e.kind.as_ref(),
        ExprKind::Block(_)
            | ExprKind::If { .. }
            | ExprKind::Match { .. }
            | ExprKind::For { .. }
            | ExprKind::While { .. }
            | ExprKind::Loop { .. }
            | ExprKind::Region { .. }
            | ExprKind::GcRegion { .. }
    )
}

/// 表达式 → 单行字符串（块类压缩为 `{ ... }` 单行形式）。
fn fmt_expr(e: &AstExpr) -> String {
    match e.kind.as_ref() {
        ExprKind::IntLiteral(v) => v.to_string(),
        ExprKind::FloatLiteral(v) => fmt_float(*v),
        ExprKind::StringLiteral(s) => escape_string(s),
        ExprKind::CharLiteral(c) => escape_char(*c),
        ExprKind::BoolLiteral(b) => b.to_string(),
        ExprKind::TimeLiteral {
            hour,
            minute,
            is_pm,
        } => fmt_time(*hour, *minute, *is_pm),
        ExprKind::Unit => "()".to_string(),
        ExprKind::Ident(name) => name.clone(),
        ExprKind::Path(seg) => seg.join("::"),
        ExprKind::Set(elems) => format!(
            "({})",
            elems.iter().map(fmt_expr).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => fmt_range_expr(lower, upper, *lower_inclusive, *upper_inclusive),
        ExprKind::Binary { op, left, right } => {
            let p = bin_prec(*op);
            let l = fmt_operand(left, p, false);
            let r = fmt_operand(right, p, true);
            format!("{} {} {}", l, bin_op_str(*op), r)
        }
        ExprKind::Unary { op, operand } => {
            let s = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
                UnaryOp::Deref => "*",
                UnaryOp::AddrOf => "&",
                UnaryOp::AddrOfMut => "&mut ",
            };
            format!("{}{}", s, fmt_operand(operand, PREC_UNARY, false))
        }
        ExprKind::ComparisonChain {
            elements,
            operators,
        } => {
            let mut out = String::new();
            for (i, op) in operators.iter().enumerate() {
                out.push_str(&fmt_operand(&elements[i], PREC_COMPARE, false));
                out.push(' ');
                out.push_str(cmp_op_str(*op));
                out.push(' ');
            }
            out.push_str(&fmt_operand(
                elements.last().unwrap(),
                PREC_COMPARE,
                false,
            ));
            out
        }
        ExprKind::InSet {
            value,
            set,
            negated,
        } => {
            let v = fmt_operand(value, PREC_COMPARE, false);
            let elems = set.iter().map(fmt_expr).collect::<Vec<_>>().join(", ");
            if *negated {
                format!("{} not in ({})", v, elems)
            } else {
                format!("{} in ({})", v, elems)
            }
        }
        ExprKind::InRange {
            value,
            range,
            negated,
        } => {
            let v = fmt_operand(value, PREC_COMPARE, false);
            if *negated {
                format!("{} not in {}", v, fmt_expr(range))
            } else {
                format!("{} in {}", v, fmt_expr(range))
            }
        }
        ExprKind::InRegion { expr, region } => {
            format!("{} in '{}", fmt_expr(expr), region)
        }
        ExprKind::Assign { target, op, value } => {
            let t = fmt_operand(target, PREC_ASSIGN, false);
            let v = fmt_operand(value, PREC_ASSIGN, true);
            format!("{} {} {}", t, assign_op_str(*op), v)
        }
        ExprKind::If { .. }
        | ExprKind::Match { .. }
        | ExprKind::For { .. }
        | ExprKind::While { .. 
        | ExprKind::Loop { .. }
        | ExprKind::Region { .. }
        | ExprKind::GcRegion { .. } => fmt_expr_compact_block(e),
        ExprKind::Transfer { expr, region } => {
            format!("transfer {} out of '{}", fmt_expr(expr), region)
        }
        ExprKind::Call { callee, args, .. } => {
            let c = fmt_operand(callee, PREC_POSTFIX, false);
            let a = args.iter().map(fmt_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", c, a)
        }
        ExprKind::MacroCall { name, args } => {
            let a = args.iter().map(fmt_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", name, a)
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
            trait_hint: _,
        } => {
            let r = fmt_operand(receiver, PREC_POSTFIX, false);
            let a = args.iter().map(fmt_expr).collect::<Vec<_>>().join(", ");
            format!("{}.{}({})", r, method, a)
        }
        ExprKind::FieldAccess { expr, field } => {
            format!(
                "{}.{}",
                fmt_operand(expr, PREC_POSTFIX, false),
                field
            )
        }
        ExprKind::StructCtor {
            type_name,
            type_args,
            fields,
        } => {
            // 泛型实参 MVP 不美化输出（`Foo<T> { .. }` 原样保留路径，实参暂略）
            let _ = type_args;
            let fields = fields
                .iter()
                .map(|(n, v)| format!("{}: {}", n, fmt_expr(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} {{ {} }}", type_name.join("::"), fields)
        }
        ExprKind::Index { expr, index } => format!(
            "{}[{}]",
            fmt_operand(expr, PREC_POSTFIX, false),
            fmt_expr(index)
        ),
        ExprKind::ArrayLit(elems) => format!(
            "[{}]",
            elems.iter().map(fmt_expr).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Closure {
            params,
            param_types,
            body,
            capture,
        } => {
            let cap = match capture {
                CaptureMode::Move => "move ",
                CaptureMode::Borrow => "",
            };
            // 参数类型注解 `|x: i64, y|`：有注解的参数拼上类型
            let ps = params
                .iter()
                .zip(param_types.iter())
                .map(|(p, t)| match t {
                    Some(ty) => format!("{}: {}", fmt_pattern(p), fmt_type(ty)),
                    None => fmt_pattern(p),
                })
                .collect::<Vec<_>>()
                .join(", ");
            let b = if is_block_like(body) {
                fmt_expr_compact_block(body)
            } else {
                fmt_expr(body)
            };
            format!("{}|{}| {}", cap, ps, b)
        }
        ExprKind::Cast {
            expr,
            target_type,
        } => format!(
            "{} as {}",
            fmt_operand(expr, PREC_CAST, false),
            fmt_type(target_type)
        ),
        ExprKind::Await(inner) => format!(
            "{}.await",
            fmt_operand(inner, PREC_POSTFIX, false)
        ),
        ExprKind::Block(b) => fmt_block_compact(b),
        ExprKind::Question(inner) => format!("{}?", fmt_operand(inner, PREC_POSTFIX, false)),
        ExprKind::Return(Some(v)) => format!("return {}", fmt_operand(v, PREC_ASSIGN, false)),
        ExprKind::Return(None) => "return".to_string(),
        ExprKind::Break(Some(v)) => format!("break {}", fmt_operand(v, PREC_ASSIGN, false)),
        ExprKind::Break(None) => "break".to_string(),
        ExprKind::Continue => "continue".to_string(),
        ExprKind::Send {
            actor,
            method,
            args,
        } => {
            let a = args.iter().map(fmt_expr).collect::<Vec<_>>().join(", ");
            format!("send {}.{}({})", fmt_expr(actor), method, a)
        }
    }
}

/// 块类表达式在单行位置时的压缩表示（如闭包体）。
fn fmt_expr_compact_block(e: &AstExpr) -> String {
    match e.kind.as_ref() {
        ExprKind::Block(b) => fmt_block_compact(b),
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            let t = fmt_block_compact(then_block);
            match else_block {
                Some(el) => format!("if {} {} else {}", fmt_expr(cond), t, fmt_block_compact(el)),
                None => format!("if {} {}", fmt_expr(cond), t),
            }
        }
        ExprKind::Match { expr, arms } => {
            let arms = arms
                .iter()
                .map(|a| {
                    let mut s = fmt_pattern(&a.pattern);
                    if let Some(g) = &a.guard {
                        s.push_str(&format!(" if {}", fmt_expr(g)));
                    }
                    format!("{} => {}", s, fmt_expr(&a.body))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("match {} {{ {} }}", fmt_expr(expr), arms)
        }
        ExprKind::For {
            pattern,
            iterator,
            body,
        } => format!(
            "for {} in {} {}",
            fmt_pattern(pattern),
            fmt_expr(iterator),
            fmt_block_compact(body)
        ),
        ExprKind::While { cond, body } => {
            format!("while {} {}", fmt_expr(cond), fmt_block_compact(body))
        }
        ExprKind::Loop { body } => format!("loop {}", fmt_block_compact(body)),
        ExprKind::Region {
            name,
            options,
            body,
        } => {
            let mut head = String::from("region");
            if let Some(n) = name {
                head.push_str(&format!(" '{}", n));
            }
            if options.adaptive {
                head.push_str(" adaptive");
            }
            format!("{} {}", head, fmt_block_compact(body))
        }
        ExprKind::GcRegion { body } => {
            format!("gc_region {}", fmt_block_compact(body))
        }
        _ => fmt_expr(e),
    }
}

fn fmt_block_compact(b: &AstBlock) -> String {
    let mut parts: Vec<String> = b
        .stmts
        .iter()
        .map(fmt_stmt_compact)
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(fe) = &b.final_expr {
        parts.push(fmt_expr(fe));
    }
    if parts.is_empty() {
        "{}".to_string()
    } else {
        format!("{{ {} }}", parts.join("; "))
    }
}

fn fmt_stmt_compact(s: &AstStmt) -> String {
    match s {
        AstStmt::Let {
            pattern,
            type_anno,
            init,
            mutable,
        } => {
            let mut out = String::from("let ");
            if *mutable {
                out.push_str("mut ");
            }
            out.push_str(&fmt_pattern(pattern));
            if let Some(t) = type_anno {
                out.push_str(&format!(": {}", fmt_type(t)));
            }
            out.push_str(&format!(" = {};", fmt_expr(init)));
            out
        }
        AstStmt::Expr(e) => format!("{};", fmt_expr(e)),
        AstStmt::Semi(e) => fmt_expr(e),
        AstStmt::Item(_) => String::new(),
    }
}

// ==================== 优先级与括号 ====================

const PREC_ASSIGN: usize = 1;
const PREC_OR: usize = 2;
const PREC_AND: usize = 3;
const PREC_BITOR: usize = 4;
const PREC_BITXOR: usize = 5;
const PREC_BITAND: usize = 6;
const PREC_COMPARE: usize = 7;
const PREC_SHIFT: usize = 8;
const PREC_ADD: usize = 9;
const PREC_MUL: usize = 10;
const PREC_CAST: usize = 11;
const PREC_UNARY: usize = 12;
const PREC_POSTFIX: usize = 13;
const PREC_ATOM: usize = 14;

fn bin_prec(op: BinaryOp) -> usize {
    match op {
        BinaryOp::Or => PREC_OR,
        BinaryOp::And => PREC_AND,
        BinaryOp::BitOr => PREC_BITOR,
        BinaryOp::BitXor => PREC_BITXOR,
        BinaryOp::BitAnd => PREC_BITAND,
        BinaryOp::Shl | BinaryOp::Shr => PREC_SHIFT,
        BinaryOp::Add | BinaryOp::Sub => PREC_ADD,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => PREC_MUL,
    }
}

/// 表达式的绑定优先级（数值越大绑定越紧）。
fn prec(e: &AstExpr) -> usize {
    match e.kind.as_ref() {
        ExprKind::Assign { .. } | ExprKind::Return(_) | ExprKind::Break(_) => PREC_ASSIGN,
        ExprKind::Question(_) => PREC_POSTFIX,
        ExprKind::Binary { op, .. } => bin_prec(*op),
        ExprKind::ComparisonChain { .. }
        | ExprKind::InSet { .. }
        | ExprKind::InRange { .. }
        | ExprKind::InRegion { .. } => PREC_COMPARE,
        ExprKind::Cast { .. } => PREC_CAST,
        ExprKind::Unary { .. } => PREC_UNARY,
        ExprKind::Call { .. }
        | ExprKind::MethodCall { .. }
        | ExprKind::FieldAccess { .. }
        | ExprKind::Index { .. }
        | ExprKind::Await(_)
        | ExprKind::Send { .. } => PREC_POSTFIX,
        _ => PREC_ATOM,
    }
}

/// 在父优先级上下文中打印子表达式：需要时加括号保证语义不变。
fn fmt_operand(child: &AstExpr, parent_prec: usize, right: bool) -> String {
    let cp = prec(child);
    if cp < parent_prec || (right && cp == parent_prec) {
        format!("({})", fmt_expr(child))
    } else {
        fmt_expr(child)
    }
}

// ==================== 模式 / 类型 / 参数 ====================

fn fmt_pattern(p: &AstPattern) -> String {
    match p {
        AstPattern::Ident(name) => name.clone(),
        AstPattern::Wildcard => "_".to_string(),
        AstPattern::Literal(l) => fmt_literal_value(l),
        AstPattern::Tuple(ps) => format!(
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
        } => fmt_range_expr(lower, upper, *lower_inclusive, *upper_inclusive),
        AstPattern::Ref(inner, mut_) => {
            let m = if *mut_ { "mut " } else { "" };
            format!("ref {}{}", m, fmt_pattern(inner))
        }
    }
}

fn fmt_literal_value(l: &LiteralValue) -> String {
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

fn fmt_range_expr(
    lower: &AstExpr,
    upper: &AstExpr,
    lower_inclusive: bool,
    upper_inclusive: bool,
) -> String {
    let lo = if lower_inclusive {
        fmt_operand(lower, PREC_COMPARE, false)
    } else {
        format!("<{}", fmt_operand(lower, PREC_COMPARE, false))
    };
    let hi = if upper_inclusive { "..." } else { "..<" };
    format!("{}{}{}", lo, hi, fmt_operand(upper, PREC_COMPARE, false))
}

fn fmt_type(t: &AstType) -> String {
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
        AstType::Infer => "_".to_string(),
    }
}

/// 格式化泛型参数列表：`T: Bound1 + Bound2`（U3）。
fn fmt_generics(generics: &[AstTypeParam]) -> String {
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

fn fmt_param(p: &AstParam) -> String {
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

fn fmt_enum_variant(v: &AstEnumVariant) -> String {
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

// ==================== 运算符字符串 ====================

fn bin_op_str(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

fn cmp_op_str(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Lt => "<",
        CompareOp::Le => "<=",
        CompareOp::Gt => ">",
        CompareOp::Ge => ">=",
        CompareOp::Eq => "==",
        CompareOp::Ne => "!=",
    }
}

fn assign_op_str(op: AssignOp) -> &'static str {
    match op {
        AssignOp::Assign => "=",
        AssignOp::AddAssign => "+=",
        AssignOp::SubAssign => "-=",
        AssignOp::MulAssign => "*=",
        AssignOp::DivAssign => "/=",
    }
}

// ==================== 字面量转义 / 格式化 ====================

fn fmt_float(v: f64) -> String {
    let s = format!("{}", v);
    // 保证浮点字面量可被 lexer 识别为浮点（带小数点或指数）
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{}.0", s)
    }
}

fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        push_escaped(&mut out, c);
    }
    out.push('"');
    out
}

fn escape_char(c: char) -> String {
    let mut out = String::with_capacity(4);
    out.push('\'');
    push_escaped(&mut out, c);
    out.push('\'');
    out
}

fn push_escaped(out: &mut String, c: char) {
    match c {
        '"' => out.push_str("\\\""),
        '\\' => out.push_str("\\\\"),
        '\'' => out.push_str("\\'"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        c if (c as u32) < 0x20 => {
            out.push_str(&format!("\\x{:02x}", c as u32));
        }
        c => out.push(c),
    }
}

fn fmt_time(hour: u8, minute: u8, is_pm: bool) -> String {
    match (minute == 0, is_pm) {
        (true, false) => match hour {
            0 => "12am".to_string(),
            h @ 1..=11 => format!("{}am", h),
            h => format!("{}:00", h), // 24 小时制整点写法
        },
        (true, true) => match hour {
            12 => "12pm".to_string(),
            h => format!("{}pm", h - 12),
        },
        (false, false) => match hour {
            0 => format!("12:{:02}am", minute),
            h @ 1..=11 => format!("{}:{:02}am", h, minute),
            h => format!("{}:{:02}", h, minute),
        },
        (false, true) => match hour {
            12 => format!("12:{:02}pm", minute),
            h => format!("{}:{:02}pm", h - 12, minute),
        },
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str) -> String {
        format_source(src).expect("parse should succeed")
    }

    #[test]
    fn roundtrip_reparse() {
        let cases = [
            "fn main() -> i64 { let x = 1; x + 2 }",
            "pub fn add(a: i64, b: i64 = 2) -> i64 { a + b }",
            "struct Point { x: i64, y: i64 }",
            "enum Shape { Circle(f64), Rect { w: f64, h: f64 } }",
            "trait Area { fn area(&self) -> f64; }",
            "impl Area for Shape { fn area(&self) -> f64 { 1.0 } }",
            "mod math { pub const PI: f64 = 3.14; }",
            "use math::PI;",
            "const MAX: i64 = 100;",
            "actor Counter { value: i64 = 0, pub fn inc() -> i64 { self.value } }",
            "if 0 < x < 10 { x } else { 0 }",
            "let y = x in (1, 3, 5);",
            "let r = x in 0..<10;",
            "let z = if a { 1 } else { 2 };",
            "match s { Shape::Circle(r) => r, _ => 0.0 }",
            "for i in 0..<10 { println(i); }",
            "while x > 0 { x = x - 1; }",
            "loop { break; }",
            "region 'r { let d = make() in 'r; }",
            "let p = Point { x: 3, y: 4 };",
            "let a = [1, 2, 3];",
            "let t = (a + b) * c;",
            "let u = a << (b + 1);",
            "let v = x as i64;",
            "let s = \"he\\\"llo\\nworld\";",
            "let ch = '\\n';",
            "if hour in (9am...6pm) {}",
            "send counter.increment(1);",
            "let r = make() in 'r;",
            "fn f() -> i64 { return 3; }",
        ];
        for src in cases {
            let out = fmt(src);
            let reparsed = rlyeh_parser::parse(&out);
            assert!(reparsed.is_ok(), "reparse failed for {:?}:\n{}", src, out);
        }
    }

    #[test]
    fn idempotent() {
        let src = "fn main() -> i64 {\n    let x = 1 + 2 * 3;\n    x\n}";
        let once = fmt(src);
        let twice = fmt(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn top_level_blank_line() {
        let out = fmt("fn a() -> i64 { 1 }fn b() -> i64 { 2 }");
        assert!(out.contains("}\n\nfn b"), "expected blank line:\n{}", out);
    }

    #[test]
    fn operator_precedence_preserved() {
        // (a + b) * c 必须保留括号
        let out = fmt("let t = (a + b) * c;");
        assert!(out.contains("(a + b) * c"), "{}", out);
        // a - (b - c) 必须保留括号
        let out = fmt("let t = a - (b - c);");
        assert!(out.contains("a - (b - c)"), "{}", out);
    }

    #[test]
    fn float_keeps_decimal() {
        // 浮点字面量必须带小数点，保证再次词法分析仍为浮点
        assert_eq!(fmt_float(3.0), "3.0");
        assert_eq!(fmt_float(0.5), "0.5");
        assert_eq!(fmt_float(-2.0), "-2.0");
        assert_eq!(
            fmt("fn f() -> f64 { 3.0 }").trim(),
            "fn f() -> f64 {\n    3.0\n}"
        );
    }

    #[test]
    fn string_escaped() {
        let out = fmt("let s = \"a\\\"b\\nc\";");
        assert!(out.contains("\"a\\\"b\\nc\""), "{}", out);
    }

    #[test]
    fn time_literals() {
        assert_eq!(fmt_time(9, 0, false), "9am");
        assert_eq!(fmt_time(18, 0, true), "6pm");
        assert_eq!(fmt_time(0, 0, false), "12am");
        assert_eq!(fmt_time(12, 0, true), "12pm");
        assert_eq!(fmt_time(9, 30, false), "9:30am");
        assert_eq!(fmt_time(22, 0, false), "22:00");
        assert_eq!(fmt_time(13, 5, true), "1:05pm");
    }

    #[test]
    fn parse_error_reported() {
        let err = format_source("fn {");
        assert!(err.is_err());
    }

    #[test]
    fn actor_and_region() {
        let src = "actor Counter { value: i64 = 0, pub fn increment(amount: i64) -> i64 { self.value += amount; self.value } }";
        let out = fmt(src);
        assert!(out.contains("actor Counter {"), "{}", out);
        assert!(out.contains("value: i64 = 0,"), "{}", out);
        assert!(out.contains("pub fn increment(amount: i64) -> i64 {"), "{}", out);
    }
}
