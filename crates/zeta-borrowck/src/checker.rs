//! 借用检查器：L0 静态所有权验证。

use std::collections::HashMap;

use zeta_hir::{HirBlock, HirExpr, HirItemKind, HirProgram, HirStmt};

use crate::error::BorrowError;

/// 单个绑定的所有权信息。
#[derive(Debug, Clone)]
struct Binding {
    /// 绑定是否可变（`let mut`）。
    mutable: bool,
    /// 绑定是否已被 `transfer` 转移（所有权转出区域）。
    transferred: bool,
}

/// 词法作用域（变量名 → 绑定）。
#[derive(Default)]
struct Scope {
    bindings: HashMap<String, Binding>,
}

/// 借用检查器。
///
/// MVP 阶段检查范围（L0 静态所有权）：
/// - **use-after-move**：变量被 `transfer` 转移所有权后再次使用
///   （Rust E0382 对应；P005 遗留缺口，`test_transfer_then_use_in_region`）；
/// - **不可变绑定赋值**：对 `let x = ...;`（非 `let mut`）的变量赋值
///   （Rust E0384 对应；typecheck 的符号表不记录可变性，此项此前无人检查）；
/// - 区域块值传递例外：`transfer x out of 'r; x` 中 `x` 作为区域块尾值
///   是所有权转出，不视为"使用"（regionck 既有合法用例保持通过）。
///
/// 借用互斥规则（`&` / `&mut`）与生命周期推断依赖引用语法与类型标注，
/// 待 typecheck 支持后启用；`BorrowConflict` / `MoveWhileBorrowed`
/// 为防御性变体，保留构造入口与 Display。
pub struct BorrowChecker {
    scopes: Vec<Scope>,
    errors: Vec<BorrowError>,
}

impl BorrowChecker {
    /// 创建空检查器。
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// 检查整个程序，返回 `Ok(())` 或收集到的错误列表。
    ///
    /// 每个函数独立作用域：参数作为不可变绑定预先注册。
    pub fn check_program(&mut self, program: &HirProgram) -> Result<(), Vec<BorrowError>> {
        for item in &program.items {
            if let HirItemKind::Fn(f) = &item.kind {
                if let Some(body) = &f.body {
                    self.scopes.push(Scope::default());
                    for param in &f.params {
                        self.insert(
                            param.name.clone(),
                            Binding {
                                mutable: false,
                                transferred: false,
                            },
                        );
                    }
                    self.check_block(body, false);
                    self.scopes.pop();
                }
            }
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    /// 在当前作用域注册一个绑定（同名覆盖 = shadowing）。
    fn insert(&mut self, name: String, binding: Binding) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.bindings.insert(name, binding);
        }
    }

    /// 从内到外查找绑定。
    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|s| s.bindings.get(name))
    }

    /// 变量是否已被转移。
    fn is_transferred(&self, name: &str) -> bool {
        self.lookup(name).is_some_and(|b| b.transferred)
    }

    /// 将变量标记为已转移。
    fn mark_transferred(&mut self, name: &str) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(b) = scope.bindings.get_mut(name) {
                b.transferred = true;
                return;
            }
        }
    }

    /// 检查语句块。
    ///
    /// `allow_transfer_pass` 仅在区域块上开启：允许已转移变量以
    /// 直接变量形式作为块尾值（所有权转出，见 [`Self::check_block`]）。
    fn check_block(&mut self, block: &HirBlock, allow_transfer_pass: bool) {
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
        if let Some(expr) = &block.final_expr {
            // transfer 例外：`transfer x out of 'r; x` 中 x 作为块值 =
            // 所有权转出给区域表达式的求值结果，不视为"使用"。
            if allow_transfer_pass {
                if let HirExpr::Variable(name) = expr {
                    if self.is_transferred(name) {
                        return;
                    }
                }
            }
            self.check_expr(expr);
        }
    }

    fn check_stmt(&mut self, stmt: &HirStmt) {
        match stmt {
            HirStmt::Let {
                name,
                init,
                mutable,
            } => {
                // 先检查 init（shadowing 时 init 引用的是旧绑定）
                self.check_expr(init);
                if name != "_" {
                    self.insert(
                        name.clone(),
                        Binding {
                            mutable: *mutable,
                            transferred: false,
                        },
                    );
                }
            }
            HirStmt::Expr(e) | HirStmt::Semi(e) => self.check_expr(e),
        }
    }

    /// 递归检查表达式，维护作用域栈与所有权状态。
    fn check_expr(&mut self, expr: &HirExpr) {
        match expr {
            HirExpr::Variable(name) => {
                if self.is_transferred(name) {
                    self.errors.push(BorrowError::use_after_transfer(name));
                }
            }
            HirExpr::Assign { target, value, .. } => {
                if let Some(b) = self.lookup(target).cloned() {
                    if b.transferred {
                        // 对已转移值的赋值也是使用（Rust：assignment to moved value）
                        self.errors.push(BorrowError::use_after_transfer(target));
                    } else if !b.mutable {
                        self.errors.push(BorrowError::assign_to_immutable(target));
                    }
                }
                self.check_expr(value);
            }
            HirExpr::Binary(_, l, r) => {
                self.check_expr(l);
                self.check_expr(r);
            }
            HirExpr::Unary(_, e) => self.check_expr(e),
            HirExpr::SetLookup { value, members, .. } => {
                self.check_expr(value);
                for m in members {
                    self.check_expr(m);
                }
            }
            HirExpr::RangeCheck {
                value,
                lower,
                upper,
                ..
            } => {
                self.check_expr(value);
                if let Some(l) = lower {
                    self.check_expr(l);
                }
                if let Some(u) = upper {
                    self.check_expr(u);
                }
            }
            HirExpr::If {
                cond,
                then_block,
                else_block,
            } => {
                self.check_expr(cond);
                self.scopes.push(Scope::default());
                self.check_block(then_block, false);
                self.scopes.pop();
                if let Some(eb) = else_block {
                    self.scopes.push(Scope::default());
                    self.check_block(eb, false);
                    self.scopes.pop();
                }
            }
            HirExpr::Block(b) => {
                self.scopes.push(Scope::default());
                self.check_block(b, false);
                self.scopes.pop();
            }
            HirExpr::Call { args, .. } => {
                for a in args {
                    self.check_expr(a);
                }
            }
            HirExpr::While { cond, body } => {
                self.check_expr(cond);
                self.scopes.push(Scope::default());
                self.check_block(body, false);
                self.scopes.pop();
            }
            HirExpr::Loop { body } => {
                self.scopes.push(Scope::default());
                self.check_block(body, false);
                self.scopes.pop();
            }
            HirExpr::Return(e) | HirExpr::Break(e) => {
                if let Some(e) = e {
                    self.check_expr(e);
                }
            }
            HirExpr::Region { body, .. } => {
                self.scopes.push(Scope::default());
                self.check_block(body, true);
                self.scopes.pop();
            }
            HirExpr::InRegion { expr, .. } => self.check_expr(expr),
            HirExpr::Transfer { expr, .. } => {
                // transfer 是所有权簿记（ADR-003：零拷贝、不读取值本身）；
                // 变量被转移后标记，后续使用报 use-after-move。
                if let HirExpr::Variable(name) = expr.as_ref() {
                    if self.is_transferred(name) {
                        self.errors.push(BorrowError::use_after_transfer(name));
                    } else {
                        self.mark_transferred(name);
                    }
                } else {
                    // 非变量表达式（调用结果等）：regionck 报 PartialTransfer，
                    // 此处递归遍历保持一致，不重复报错。
                    self.check_expr(expr);
                }
            }
            // 字面量 / continue / 单元值：无子表达式
            HirExpr::IntLiteral(_)
            | HirExpr::FloatLiteral(_)
            | HirExpr::StringLiteral(_)
            | HirExpr::CharLiteral(_)
            | HirExpr::BoolLiteral(_)
            | HirExpr::Continue
            | HirExpr::Unit => {}
            // 聚合对象构造 / 访问：Alloc 无子表达式；FieldGet / FieldSet 递归检查
            HirExpr::Alloc { .. } => {}
            HirExpr::FieldGet { base, .. } => self.check_expr(base),
            HirExpr::FieldSet { base, value, .. } => {
                self.check_expr(base);
                self.check_expr(value);
            }
            // 索引读取 / 写入：递归检查基址、索引与值表达式
            HirExpr::Index { base, index, .. } => {
                self.check_expr(base);
                self.check_expr(index);
            }
            HirExpr::IndexSet {
                base,
                index,
                value,
                ..
            } => {
                self.check_expr(base);
                self.check_expr(index);
                self.check_expr(value);
            }
            // 引用 / 解引用：递归检查被引用 / 被解引用表达式
            // （`&x` 不转移所有权；`*p = v` 的写入可变性互斥检查待引用
            // 类型标注接入后启用——borrowck 已预留 BorrowConflict 等变体）
            HirExpr::Ref { expr, .. } => self.check_expr(expr),
            HirExpr::Deref { expr, .. } => self.check_expr(expr),
            HirExpr::DerefSet { base, value, .. } => {
                self.check_expr(base);
                self.check_expr(value);
            }
        }
    }
}

impl Default for BorrowChecker {
    fn default() -> Self {
        Self::new()
    }
}
