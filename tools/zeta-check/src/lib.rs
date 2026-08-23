//! # zeta-check
//!
//! Zeta 语言静态分析器（lint）。
//!
//! ## 检查规则
//!
//! | 规则 | 级别 | 说明 |
//! |------|------|------|
//! | `parse-error` | error | 源码无法解析 |
//! | `unused-variable` | warning | 块内 `let` 绑定的变量在作用域内从未被引用 |
//! | `constant-condition` | warning | `if`/`while` 条件为恒常字面量 |
//! | `redundant-compare` | warning | `==`/`!=` 两侧均为字面量，结果恒定 |
//! | `unreachable-code` | warning | `return`/`break`/`continue` 之后的语句 |
//!
//! ## 示例
//!
//! ```text
//! file.zeta:3:5: warning[unused-variable]: variable 'x' is never used
//! ```

#![warn(missing_docs)]
#![warn(unsafe_code)]

use zeta_ast::*;

/// 诊断级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// 错误（解析失败等）
    Error,
    /// 警告（lint 规则）
    Warning,
}

impl Level {
    /// 字符串表示（小写）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warning => "warning",
        }
    }
}

/// 静态检查诊断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// 级别
    pub level: Level,
    /// 规则名（`unused-variable` 等）
    pub rule: &'static str,
    /// 行号（从 1 开始）
    pub line: usize,
    /// 列号（从 1 开始）
    pub col: usize,
    /// 人类可读消息
    pub message: String,
}

impl Diagnostic {
    /// 渲染为 `line:col: level[rule]: message` 形式。
    pub fn render(&self) -> String {
        format!(
            "{}:{}: {}[{}]: {}",
            self.line,
            self.col,
            self.level.as_str(),
            self.rule,
            self.message
        )
    }
}

/// 对源码执行静态检查。
///
/// 解析失败返回 `parse-error` 诊断；否则返回 lint 规则诊断。
pub fn check_source(source: &str) -> Vec<Diagnostic> {
    match zeta_parser::parse(source) {
        Ok(prog) => check_program(&prog),
        Err(e) => {
            let span = e.span();
            vec![Diagnostic {
                level: Level::Error,
                rule: "parse-error",
                line: span.line,
                col: span.col,
                message: e.to_string(),
            }]
        }
    }
}

/// 对已解析的 AST 程序执行静态检查。
pub fn check_program(prog: &AstProgram) -> Vec<Diagnostic> {
    let mut checker = Checker {
        scopes: Vec::new(),
        diags: Vec::new(),
    };
    for item in &prog.items {
        checker.walk_item(item);
    }
    checker.diags
}

// ==================== 作用域 / 绑定 ====================

struct Binding {
    name: String,
    span: zeta_lexer::Span,
    used: bool,
}

struct Scope {
    bindings: Vec<Binding>,
}

impl Scope {
    fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }
}

struct Checker {
    scopes: Vec<Scope>,
    diags: Vec<Diagnostic>,
}

impl Checker {
    // ---------- 作用域辅助 ----------

    fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn pop_scope(&mut self) {
        let scope = self.scopes.pop().expect("scope stack non-empty");
        for b in &scope.bindings {
            if !b.used {
                self.diags.push(Diagnostic {
                    level: Level::Warning,
                    rule: "unused-variable",
                    line: b.span.line,
                    col: b.span.col,
                    message: format!("variable '{}' is never used", b.name),
                });
            }
        }
    }

    /// 在当前（最内层）作用域绑定一个名字。
    fn bind(&mut self, name: &str, span: zeta_lexer::Span) {
        self.scopes
            .last_mut()
            .expect("scope exists")
            .bindings
            .push(Binding {
                name: name.to_string(),
                span,
                used: false,
            });
    }

    /// 标记某名字为已使用（从内层到外层查找，命中第一个同名绑定）。
    fn mark_used(&mut self, name: &str) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(b) = scope.bindings.iter_mut().find(|b| b.name == name) {
                b.used = true;
                return;
            }
        }
    }

    /// 模式中绑定所有标识符（`_` 通配跳过）。
    fn bind_pattern(&mut self, p: &AstPattern, span: zeta_lexer::Span) {
        match p {
            AstPattern::Ident(name) => self.bind(name, span),
            AstPattern::Tuple(ps) => {
                for p in ps {
                    self.bind_pattern(p, span);
                }
            }
            AstPattern::Struct(_, fields) => {
                for (_, fp) in fields {
                    self.bind_pattern(fp, span);
                }
            }
            AstPattern::Enum(_, ps) | AstPattern::EnumPath(_, ps) => {
                for p in ps {
                    self.bind_pattern(p, span);
                }
            }
            AstPattern::Ref(inner, _) => self.bind_pattern(inner, span),
            AstPattern::Wildcard | AstPattern::Literal(_) | AstPattern::Range { .. } => {}
        }
    }

    // ---------- 规则：unreachable-code ----------

    /// 判断表达式是否为无条件跳转（return/break/continue）。
    fn is_terminator(e: &AstExpr) -> bool {
        matches!(
            e.kind.as_ref(),
            ExprKind::Return(_) | ExprKind::Break(_) | ExprKind::Continue
        )
    }

    fn walk_block(&mut self, b: &AstBlock) {
        self.push_scope();
        let mut reachable = true;
        for stmt in &b.stmts {
            if !reachable {
                let span = stmt_span(stmt);
                self.diags.push(Diagnostic {
                    level: Level::Warning,
                    rule: "unreachable-code",
                    line: span.line,
                    col: span.col,
                    message: "statement is unreachable".to_string(),
                });
            }
            self.walk_stmt(stmt);
            if let Some(e) = stmt_expr(stmt) {
                if Self::is_terminator(e) {
                    reachable = false;
                }
            }
        }
        if let Some(fe) = &b.final_expr {
            if !reachable {
                self.diags.push(Diagnostic {
                    level: Level::Warning,
                    rule: "unreachable-code",
                    line: fe.span.line,
                    col: fe.span.col,
                    message: "expression is unreachable".to_string(),
                });
            }
            self.walk_expr(fe);
        }
        self.pop_scope();
    }

    // ---------- 规则：constant-condition ----------

    fn is_constant_literal(e: &AstExpr) -> bool {
        matches!(
            e.kind.as_ref(),
            ExprKind::IntLiteral(_)
                | ExprKind::FloatLiteral(_)
                | ExprKind::BoolLiteral(_)
                | ExprKind::CharLiteral(_)
                | ExprKind::StringLiteral(_)
        )
    }

    fn check_condition(&mut self, e: &AstExpr) {
        // 恒常字面量条件（含 `!true`/`!false` 形式）
        let is_const = Self::is_constant_literal(e)
            || matches!(&*e.kind, ExprKind::Unary { op: UnaryOp::Not, operand } if Self::is_constant_literal(operand));
        if is_const {
            self.diags.push(Diagnostic {
                level: Level::Warning,
                rule: "constant-condition",
                line: e.span.line,
                col: e.span.col,
                message: format!(
                    "condition '{}' is a constant, consider simplifying",
                    fmt_short(e)
                ),
            });
        }
    }

    // ---------- 规则：redundant-compare ----------

    fn check_chain(&mut self, e: &AstExpr, elements: &[AstExpr], operators: &[CompareOp]) {
        // `a == b` / `a != b` 且两侧均为字面量 → 结果恒定
        if operators.len() == 1
            && matches!(operators[0], CompareOp::Eq | CompareOp::Ne)
            && elements.len() == 2
            && Self::is_constant_literal(&elements[0])
            && Self::is_constant_literal(&elements[1])
        {
            self.diags.push(Diagnostic {
                level: Level::Warning,
                rule: "redundant-compare",
                line: e.span.line,
                col: e.span.col,
                message: format!(
                    "comparison '{} {} {}' is always the same result",
                    fmt_short(&elements[0]),
                    if operators[0] == CompareOp::Eq {
                        "=="
                    } else {
                        "!="
                    },
                    fmt_short(&elements[1])
                ),
            });
        }
    }

    // ---------- 遍历 ----------

    fn walk_item(&mut self, item: &AstItem) {
        match item {
            AstItem::FnDecl(f) => {
                if let Some(body) = &f.body {
                    // 函数体作用域预填参数（self 接收者不参与未使用检查）
                    self.push_scope();
                    for p in &f.params {
                        if p.name != "self" {
                            self.bind(&p.name, p.span);
                        }
                    }
                    self.walk_block(body);
                    self.pop_scope();
                }
            }
            AstItem::StructDecl(_) | AstItem::EnumDecl(_) | AstItem::UseDecl(_) => {}
            AstItem::TraitDecl(t) => {
                for m in &t.methods {
                    if let Some(body) = &m.body {
                        self.push_scope();
                        for p in &m.params {
                            if p.name != "self" {
                                self.bind(&p.name, p.span);
                            }
                        }
                        self.walk_block(body);
                        self.pop_scope();
                    }
                }
            }
            AstItem::ImplBlock(i) => {
                for m in &i.methods {
                    if let Some(body) = &m.body {
                        self.push_scope();
                        for p in &m.params {
                            if p.name != "self" {
                                self.bind(&p.name, p.span);
                            }
                        }
                        self.walk_block(body);
                        self.pop_scope();
                    }
                }
            }
            AstItem::ModDecl(m) => {
                for item in &m.items {
                    self.walk_item(item);
                }
            }
            AstItem::ConstDecl(c) => self.walk_expr(&c.value),
            AstItem::ActorDecl(a) => {
                for m in &a.methods {
                    if let Some(body) = &m.body {
                        self.push_scope();
                        for p in &m.params {
                            if p.name != "self" {
                                self.bind(&p.name, p.span);
                            }
                        }
                        self.walk_block(body);
                        self.pop_scope();
                    }
                }
            }
            AstItem::MacroDecl(_) => {}
            AstItem::Statement(stmt) => self.walk_stmt(stmt),
        }
    }

    fn walk_stmt(&mut self, s: &AstStmt) {
        match s {
            AstStmt::Let {
                pattern,
                init,
                type_anno: _,
                mutable: _,
            } => {
                // 先求值 init（引用标记），再绑定 pattern（作用域遮蔽语义）
                self.walk_expr(init);
                self.bind_pattern(pattern, stmt_span(s));
            }
            AstStmt::Expr(e) | AstStmt::Semi(e) => self.walk_expr(e),
            AstStmt::Item(item) => self.walk_item(item),
        }
    }

    fn walk_expr(&mut self, e: &AstExpr) {
        match e.kind.as_ref() {
            ExprKind::IntLiteral(_)
            | ExprKind::FloatLiteral(_)
            | ExprKind::StringLiteral(_)
            | ExprKind::CharLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::TimeLiteral { .. }
            | ExprKind::Set(_) => {}
            ExprKind::Block(b) => self.walk_block(b),
            ExprKind::Ident(name) => self.mark_used(name),
            ExprKind::Path(_) => {} // 模块路径，非变量引用
            ExprKind::Range {
                lower,
                upper,
                lower_inclusive: _,
                upper_inclusive: _,
            } => {
                self.walk_expr(lower);
                self.walk_expr(upper);
            }
            ExprKind::Binary { left, right, .. } => {
                self.walk_expr(left);
                self.walk_expr(right);
            }
            ExprKind::Unary { operand, .. } => self.walk_expr(operand),
            ExprKind::ComparisonChain {
                elements,
                operators,
            } => {
                for el in elements {
                    self.walk_expr(el);
                }
                self.check_chain(e, elements, operators);
            }
            ExprKind::InSet { value, set, .. } => {
                self.walk_expr(value);
                for el in set {
                    self.walk_expr(el);
                }
            }
            ExprKind::InRange { value, range, .. } => {
                self.walk_expr(value);
                self.walk_expr(range);
            }
            ExprKind::InRegion { expr, .. } => self.walk_expr(expr),
            ExprKind::Assign { target, value, .. } => {
                self.walk_expr(target);
                self.walk_expr(value);
            }
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                self.check_condition(cond);
                self.walk_expr(cond);
                self.walk_block(then_block);
                if let Some(el) = else_block {
                    self.walk_block(el);
                }
            }
            ExprKind::Match { expr, arms } => {
                self.walk_expr(expr);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        self.walk_expr(g);
                    }
                    self.walk_expr(&arm.body);
                }
            }
            ExprKind::For {
                pattern,
                iterator,
                body,
            } => {
                self.walk_expr(iterator);
                self.push_scope();
                self.bind_pattern(pattern, e.span);
                self.walk_block(body);
                self.pop_scope();
            }
            ExprKind::While { cond, body } => {
                self.check_condition(cond);
                self.walk_expr(cond);
                self.walk_block(body);
            }
            ExprKind::Loop { body } => self.walk_block(body),
            ExprKind::Region {
                name: _,
                options: _,
                body,
            } => self.walk_block(body),
            ExprKind::GcRegion { body } => self.walk_block(body),
            ExprKind::Transfer { expr, .. } => self.walk_expr(expr),
            ExprKind::Call { callee, args, .. } => {
                self.walk_expr(callee);
                for a in args {
                    self.walk_expr(a);
                }
            }
            ExprKind::MethodCall {
                receiver,
                args,
                method: _,
            } => {
                self.walk_expr(receiver);
                for a in args {
                    self.walk_expr(a);
                }
            }
            ExprKind::FieldAccess { expr, .. } => self.walk_expr(expr),
            ExprKind::StructCtor { type_name: _, fields } => {
                for (_, v) in fields {
                    self.walk_expr(v);
                }
            }
            ExprKind::Index { expr, index } => {
                self.walk_expr(expr);
                self.walk_expr(index);
            }
            ExprKind::ArrayLit(elems) => {
                for el in elems {
                    self.walk_expr(el);
                }
            }
            ExprKind::Closure {
                params,
                param_types: _,
                body,
                capture: _,
            } => {
                // 闭包体：新作用域预填闭包参数
                self.push_scope();
                for p in params {
                    self.bind_pattern(p, e.span);
                }
                self.walk_expr(body);
                self.pop_scope();
            }
            ExprKind::Cast { expr, target_type: _ } => self.walk_expr(expr),
            ExprKind::Await(inner) => self.walk_expr(inner),
            ExprKind::Question(inner) => self.walk_expr(inner),
            ExprKind::Return(Some(v)) => self.walk_expr(v),
            ExprKind::Return(None) | ExprKind::Break(None) | ExprKind::Continue => {}
            ExprKind::Break(Some(v)) => self.walk_expr(v),
            ExprKind::Send {
                actor,
                args,
                method: _,
            } => {
                self.walk_expr(actor);
                for a in args {
                    self.walk_expr(a);
                }
            }
            ExprKind::MacroCall { args, .. } => {
                for a in args {
                    self.walk_expr(a);
                }
            }
        }
    }
}

// ==================== 辅助 ====================

fn stmt_span(s: &AstStmt) -> zeta_lexer::Span {
    match s {
        // AstStmt::Let 不携带 span，用初始化表达式的位置近似
        AstStmt::Let { init, .. } => init.span,
        AstStmt::Expr(e) | AstStmt::Semi(e) => e.span,
        AstStmt::Item(_) => zeta_lexer::Span {
            start: 0,
            end: 0,
            line: 0,
            col: 0,
        },
    }
}

fn stmt_expr(s: &AstStmt) -> Option<&AstExpr> {
    match s {
        AstStmt::Expr(e) | AstStmt::Semi(e) => Some(e),
        _ => None,
    }
}

/// 简短渲染表达式（用于诊断消息）。
fn fmt_short(e: &AstExpr) -> String {
    match e.kind.as_ref() {
        ExprKind::IntLiteral(v) => v.to_string(),
        ExprKind::FloatLiteral(v) => v.to_string(),
        ExprKind::BoolLiteral(b) => b.to_string(),
        ExprKind::CharLiteral(c) => format!("'{}'", c),
        ExprKind::StringLiteral(s) => format!("\"{}\"", s),
        ExprKind::Ident(n) => n.clone(),
        ExprKind::Path(p) => p.join("::"),
        _ => "…".to_string(),
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(src: &str) -> Vec<String> {
        check_source(src)
            .into_iter()
            .map(|d| d.render())
            .collect()
    }

    #[test]
    fn parse_error_reported() {
        let d = check_source("fn {");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].level, Level::Error);
        assert_eq!(d[0].rule, "parse-error");
    }

    #[test]
    fn unused_variable() {
        let out = rules("fn main() {\n    let x = 1;\n    println(2);\n}");
        assert_eq!(out.len(), 1, "{:?}", out);
        assert!(out[0].contains("unused-variable"), "{:?}", out);
        assert!(out[0].contains("'x' is never used"), "{:?}", out);
        // AstStmt::Let 无 span，位置取 init 表达式起点
        assert!(out[0].starts_with("2:13"), "{:?}", out);
    }

    #[test]
    fn used_variable_no_warning() {
        let out = rules("fn main() {\n    let x = 1;\n    println(x);\n}");
        assert!(out.is_empty(), "{:?}", out);
    }

    #[test]
    fn shadowed_binding() {
        // 内层遮蔽外层：内层引用只标记内层；外层仍报未使用
        let out = rules("fn main() {\n    let x = 1;\n    if 1 < 2 {\n        let x = 2;\n        println(x);\n    }\n}");
        assert_eq!(out.len(), 1, "{:?}", out);
        assert!(out[0].contains("'x' is never used"), "{:?}", out);
    }

    #[test]
    fn wildcard_not_reported() {
        let out = rules("fn main() {\n    let _ = 1;\n}");
        assert!(out.is_empty(), "{:?}", out);
    }

    #[test]
    fn parameter_usage() {
        // 参数被使用：无警告
        let used = rules("fn add(a: i64, b: i64) -> i64 { a + b }");
        assert!(used.is_empty(), "{:?}", used);
        // 参数未使用：警告
        let unused = rules("fn add(a: i64, b: i64) -> i64 { a }");
        assert_eq!(unused.len(), 1, "{:?}", unused);
        assert!(unused[0].contains("'b' is never used"), "{:?}", unused);
    }

    #[test]
    fn self_receiver_not_reported() {
        let out = rules("actor Counter { value: i64 = 0, pub fn get(&self) -> i64 { self.value } }");
        assert!(out.is_empty(), "{:?}", out);
    }

    #[test]
    fn constant_condition() {
        let out = rules("fn main() {\n    if true { }\n    while false { }\n    if !true { }\n}");
        assert_eq!(out.len(), 3, "{:?}", out);
        assert!(out.iter().all(|d| d.contains("constant-condition")), "{:?}", out);
    }

    #[test]
    fn redundant_compare() {
        let out = rules("fn main() {\n    let a = 1 == 1;\n    let b = 3 != 2;\n    let c = x == 2;\n    println(a);\n    println(b);\n    println(c);\n}");
        assert_eq!(out.len(), 2, "{:?}", out);
        assert!(out[0].contains("redundant-compare"), "{:?}", out);
        assert!(out[0].contains("1 == 1"), "{:?}", out);
        assert!(out[1].contains("redundant-compare"), "{:?}", out);
        assert!(out[1].contains("3 != 2"), "{:?}", out);
    }

    #[test]
    fn unreachable_code() {
        let out = rules("fn main() {\n    return 1;\n    let x = 2;\n}");
        assert_eq!(out.len(), 2, "{:?}", out);
        // 一个 unreachable（let x），一个 unused-variable（x）
        assert!(
            out.iter().any(|d| d.contains("unreachable-code")),
            "{:?}",
            out
        );
        assert!(
            out.iter().any(|d| d.contains("unused-variable")),
            "{:?}",
            out
        );
    }

    #[test]
    fn unreachable_after_break() {
        let out = rules("fn main() {\n    loop {\n        break;\n        let y = 1;\n    }\n}");
        assert!(
            out.iter().any(|d| d.contains("unreachable-code")),
            "{:?}",
            out
        );
    }

    #[test]
    fn clean_code_no_diagnostics() {
        let src = "fn main() {\n    let x = 10;\n    if x < 20 {\n        println(x);\n    } else {\n        println(0);\n    }\n    let y = x + 1;\n    println(y);\n}";
        let out = rules(src);
        assert!(out.is_empty(), "{:?}", out);
    }
}
