//! 借用检查器：L0 静态所有权验证。

use std::collections::HashMap;

use rlyeh_hir::{HirBlock, HirExpr, HirItemKind, HirProgram, HirStmt};
use rlyeh_lexer::Span;

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

/// 借用种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BorrowKind {
    /// 共享借用（`&x`）：只读，可共存。
    Shared,
    /// 可变借用（`&mut x`）：互斥。
    Mut,
}

/// 一条借用记录。
///
/// 活跃期（NLL 近似）为 `born <= pos <= last_use`：借用创建于语句
/// `born`，其引用变量在语句 `last_use` 最后一次使用后释放。
#[derive(Debug, Clone)]
struct Borrow {
    /// 被借用变量名。
    source: String,
    /// 引用变量名（`let r = &x;` 为 `Some(r)`）；临时借用（`f(&x)` 实参、
    /// 聚合字段内嵌 `&x`）为 `None`，仅创建语句活跃。
    var: Option<String>,
    /// 借用种类。
    kind: BorrowKind,
    /// 创建处全局语句序。
    born: usize,
    /// 引用变量最后使用处全局语句序（临时借用 = `born`）。
    last_use: usize,
}

/// 借用检查器。
///
/// 检查范围（L0 静态所有权 + 引用借用互斥）：
/// - **use-after-move**：变量被 `transfer` 转移所有权后再次使用
///   （Rust E0382 对应；P005 遗留缺口，`test_transfer_then_use_in_region`）；
/// - **不可变绑定赋值**：对 `let x = ...;`（非 `let mut`）的变量赋值
///   （Rust E0384 对应）；
/// - **借用互斥（G1 收尾）**：`&x` / `&mut x` 的别名与可变性互斥——
///   写（赋值）被借用中的变量、`&mut` 与任何活跃借用冲突、`&` 与活跃
///   可变借用冲突、`&mut x` 要求 `x` 为 `let mut`（E0596）、对局部变量
///   的引用逃逸出函数（悬垂，E0597）；
/// - 区域块值传递例外：`transfer x out of 'r; x` 中 `x` 作为区域块尾值
///   是所有权转出，不视为"使用"（regionck 既有合法用例保持通过）。
///
/// 语义说明（与 Rust 的差异，Rlyeh 值语义下有意放宽）：
/// - **读被借用变量不受限**：Rlyeh 的 `&x` 是值槽地址 / 聚合对象指针拷贝，
///   裸指针（`*mut`）别名读写是合法模式（tests/run-pass/raw_ptr.rl），
///   故活跃借用期间读原变量、经引用写原槽均不报错；
/// - **借用活跃期 = 引用变量最后一次使用**（语句粒度 NLL）：借用结束后
///   对原变量的写入/再借用均允许；
/// - **引用经聚合字段传播**（`Wrapper { inner: &a }`）与**方法返回引用**
///   （`String::as_str()`）无法在 HIR 上追踪（HIR 无类型标注），暂不检查
///   字段内引用悬垂；MVP 悬垂检查覆盖 `return` 与函数体块尾值两个出口。
///
/// HIR 子节点不携带源码位置；错误坐标取自查错所在函数的 `HirItem.span`
/// （函数级粒度，合并源码坐标），经 `render(prelude_lines)` 还原为用户坐标。
pub struct BorrowChecker {
    scopes: Vec<Scope>,
    errors: Vec<BorrowError>,
    /// 当前函数内的借用记录。
    borrows: Vec<Borrow>,
    /// 引用变量名 → 使用处全局语句序（预扫描收集，供 NLL 末次使用判定）。
    uses: HashMap<String, Vec<usize>>,
    /// 当前全局语句序（函数内单调递增；预扫描与检查遍历共用）。
    pos: usize,
    /// 当前函数参数名（悬垂判定：借参数不悬垂）。
    param_names: Vec<String>,
    /// 当前函数（查错所在项）的 `HirItem.span`：错误坐标取函数级粒度
    ///（合并源码坐标，L1 余量；经 `render` 减预置行数还原为用户坐标）。
    cur_span: Span,
}

impl BorrowChecker {
    /// 创建空检查器。
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            errors: Vec::new(),
            borrows: Vec::new(),
            uses: HashMap::new(),
            pos: 0,
            param_names: Vec::new(),
            cur_span: Span {
                start: 0,
                end: 0,
                line: 0,
                col: 0,
            },
        }
    }

    /// 检查整个程序，返回 `Ok(())` 或收集到的错误列表。
    ///
    /// 每个函数独立作用域：参数作为不可变绑定预先注册。
    pub fn check_program(&mut self, program: &HirProgram) -> Result<(), Vec<BorrowError>> {
        for item in &program.items {
            if let HirItemKind::Fn(f) = &item.kind {
                if let Some(body) = &f.body {
                    // 函数级状态复位
                    self.cur_span = item.span;
                    self.param_names = f.params.iter().map(|p| p.name.clone()).collect();
                    self.borrows.clear();
                    self.uses.clear();
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
                    // 预扫描：收集引用变量的使用位置（与检查遍历同构，
                    // 语句序一致），供借用末次使用（NLL）判定。
                    self.pos = 0;
                    self.collect_block_uses(body, false);
                    // 检查遍历
                    self.pos = 0;
                    self.check_block(body, false);
                    // 函数体块尾值 = 返回值：悬垂检查
                    if let Some(expr) = &body.final_expr {
                        self.check_dangling_return(expr);
                    }
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

    /// 变量名在当前语句序上的活跃借用（`born <= pos <= last_use`）。
    fn active_borrows(&self, source: &str) -> Vec<&Borrow> {
        self.borrows
            .iter()
            .filter(|b| {
                b.source == source && b.born <= self.pos && self.pos <= b.last_use
            })
            .collect()
    }

    /// 借用创建：冲突检查 + 登记。
    ///
    /// `var`：`let r = &x;` 的引用变量名；表达式内临时借用（`f(&x)`、
    /// 聚合字段内嵌 `&x`）传 `None`（仅当前语句活跃）。
    fn register_borrow(&mut self, var: Option<&str>, source: &str, is_mut: bool) {
        // 可变性互斥 / 别名冲突
        for b in self.active_borrows(source) {
            match (b.kind, is_mut) {
                (BorrowKind::Mut, true) => {
                    self.errors.push(BorrowError::borrow_conflict(format!(
                        "cannot mutably borrow `{source}` because it is already borrowed as mutable"
                    ), self.cur_span));
                    return;
                }
                (BorrowKind::Mut, false) => {
                    self.errors.push(BorrowError::borrow_conflict(format!(
                        "cannot borrow `{source}` as shared because it is already borrowed as mutable"
                    ), self.cur_span));
                    return;
                }
                (BorrowKind::Shared, true) => {
                    self.errors.push(BorrowError::borrow_conflict(format!(
                        "cannot mutably borrow `{source}` because it is already borrowed as shared"
                    ), self.cur_span));
                    return;
                }
                (BorrowKind::Shared, false) => {}
            }
        }
        // `&mut x` 要求 `x` 为 `let mut`（E0596）；全局 / 未绑定（const）宽松跳过
        if is_mut {
            if let Some(b) = self.lookup(source) {
                if !b.mutable {
                    self.errors.push(BorrowError::borrow_mut_immutable(source, self.cur_span));
                    return;
                }
            }
        }
        let kind = if is_mut {
            BorrowKind::Mut
        } else {
            BorrowKind::Shared
        };
        let last_use = var
            .and_then(|v| self.uses.get(v).and_then(|u| u.last()).copied())
            .unwrap_or(self.pos);
        self.borrows.push(Borrow {
            source: source.to_string(),
            var: var.map(|s| s.to_string()),
            kind,
            born: self.pos,
            last_use,
        });
    }

    /// 悬垂检查：返回的引用必须指向参数（或全局），不能是局部变量的引用。
    fn check_dangling_return(&mut self, expr: &HirExpr) {
        match expr {
            HirExpr::Variable(v) => {
                for b in &self.borrows {
                    if b.var.as_deref() == Some(v) && !self.param_names.contains(&b.source) {
                        self.errors.push(BorrowError::dangling_reference(v, self.cur_span));
                        return;
                    }
                }
            }
            HirExpr::Ref { expr: inner, .. } => {
                if let HirExpr::Variable(src) = inner.as_ref() {
                    if !self.param_names.contains(src) {
                        self.errors.push(BorrowError::dangling_reference(src, self.cur_span));
                    }
                }
            }
            // V2-D（2026-08-26）：`s.as_str()` / `s.as_str_range(..)` 的返回是
            // StrFat 构造块（`Alloc{is_strfat}` + FieldSet data/len），悬垂检查需
            // 识别其 data 槽来源：若指向局部 String（非参数），返回 `&str` 悬垂。
            HirExpr::Block(block) | HirExpr::UnsafeBlock(block) => self.check_dangling_strfat_block(block),
            _ => {}
        }
    }

    /// V2-D：检查 StrFat 构造块返回的 data 来源是否为参数。
    ///
    /// `as_str`/`as_str_range` 生成 `Alloc{is_strfat}` + `FieldSet(sf, 0, data)`
    /// 的 StrFat 双槽值；data 槽来自 `FieldGet(base, 0)`（String 的 data 指针）。
    /// 若 `base` 是局部 String（非参数），返回的 `&str` 指向将销毁的局部缓冲 → 悬垂。
    fn check_dangling_strfat_block(&mut self, block: &HirBlock) {
        // 找 StrFat 分配目标 sf
        let mut sf: Option<String> = None;
        let mut field_sets: Vec<&HirStmt> = Vec::new();
        let mut lets: Vec<&HirStmt> = Vec::new();
        for stmt in &block.stmts {
            match stmt {
                HirStmt::Let { init, name, .. } => {
                    if let HirExpr::Alloc {
                        is_strfat: true, ..
                    } = init
                    {
                        sf = Some(name.clone());
                    }
                    lets.push(stmt);
                }
                HirStmt::Semi(HirExpr::FieldSet { base, .. }) => {
                    if let HirExpr::Variable(b) = base.as_ref() {
                        if sf.as_deref() == Some(b) {
                            field_sets.push(stmt);
                        }
                    }
                }
                _ => {}
            }
        }
        let Some(sf_name) = sf else {
            return;
        };
        // 找 FieldSet(sf, 0, data_tmp)：data 槽来源
        let mut data_src: Option<String> = None;
        for stmt in &field_sets {
            if let HirStmt::Semi(HirExpr::FieldSet {
                base,
                index: 0,
                value,
                ..
            }) = stmt
            {
                if let HirExpr::Variable(b) = base.as_ref() {
                    if *b == sf_name {
                        if let HirExpr::Variable(v) = value.as_ref() {
                            data_src = Some(v.clone());
                        }
                    }
                }
            }
        }
        let Some(data_tmp) = data_src else {
            return;
        };
        // 找 Let{name: data_tmp, init: FieldGet{base, 0}}：取 String 来源
        let mut base_src: Option<String> = None;
        for stmt in &lets {
            if let HirStmt::Let { name, init, .. } = stmt {
                if *name == data_tmp {
                    if let HirExpr::FieldGet { base, index: 0, .. } = init {
                        if let HirExpr::Variable(b) = base.as_ref() {
                            base_src = Some(b.clone());
                        }
                    }
                }
            }
        }
        if let Some(src) = base_src {
            if !self.param_names.contains(&src) {
                self.errors
                    .push(BorrowError::dangling_reference(&sf_name, self.cur_span));
            }
        }
    }

    /// 检查语句块。
    ///
    /// `allow_transfer_pass` 仅在区域块上开启：允许已转移变量以
    /// 直接变量形式作为块尾值（所有权转出，见 [`Self::check_block`]）。
    /// 注意：transfer 例外早退使检查阶段语句序比预扫描少 1，为保守方向
    /// （借用活跃期在边界处略延后），不造成漏报。
    fn check_block(&mut self, block: &HirBlock, allow_transfer_pass: bool) {
        for stmt in &block.stmts {
            self.pos += 1;
            self.check_stmt(stmt);
        }
        if let Some(expr) = &block.final_expr {
            self.pos += 1;
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
                // 借用创建特判：`let r = &x;` / `let r = &mut x;` ——
                // 具名借用（引用变量 = r），借用活跃到 r 最后一次使用。
                // 先检查借用（shadowing 时 init 引用的是旧绑定），再注册绑定。
                if let HirExpr::Ref { expr, is_mut, .. } = init {
                    if let HirExpr::Variable(src) = expr.as_ref() {
                        self.register_borrow(Some(name), src, *is_mut);
                    } else {
                        // 非变量源（防御：typecheck 已限制 `&` 目标为变量）
                        self.check_expr(init);
                    }
                } else {
                    self.check_expr(init);
                }
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
                    self.errors.push(BorrowError::use_after_transfer(name, self.cur_span));
                }
            }
            HirExpr::PtrAdd { base, offset, .. } => {
                // 裸指针算术：仅读取 base/offset 指针与偏移值，不产生借用
                self.check_expr(base);
                self.check_expr(offset);
            }
            // U6 Cast IR：`expr as T` 仅读取被转换表达式
            HirExpr::Cast { expr, .. } => self.check_expr(expr),
            HirExpr::Assign { target, value, .. } => {
                // 借用互斥：不能赋值（写）被借用中的变量
                if !self.active_borrows(target).is_empty() {
                    self.errors.push(BorrowError::borrow_conflict(format!(
                        "cannot assign to `{target}` because it is borrowed"
                    ), self.cur_span));
                }
                if let Some(b) = self.lookup(target).cloned() {
                    if b.transferred {
                        // 对已转移值的赋值也是使用（Rust：assignment to moved value）
                        self.errors.push(BorrowError::use_after_transfer(target, self.cur_span));
                    } else if !b.mutable {
                        self.errors.push(BorrowError::assign_to_immutable(target, self.cur_span));
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
            HirExpr::Block(b) | HirExpr::UnsafeBlock(b) => {
                self.scopes.push(Scope::default());
                self.check_block(b, false);
                self.scopes.pop();
            }
            HirExpr::Call { args, .. } => {
                for a in args {
                    self.check_expr(a);
                }
            }
            // 函数地址值：无所有权转移
            HirExpr::FnPtr(_) => {}
            // 间接调用：callee 与实参均视为使用
            HirExpr::CallIndirect { callee, args, .. } => {
                self.check_expr(callee);
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
            HirExpr::Return(e) => {
                if let Some(e) = e {
                    self.check_expr(e);
                    self.check_dangling_return(e);
                }
            }
            HirExpr::Break(e) => {
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
                        self.errors.push(BorrowError::use_after_transfer(name, self.cur_span));
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
            // 引用 / 解引用：
            // - `&x`（非 `let r = &x` 的表达式内出现，如实参 / 聚合字段 / dyn 构造）
            //   为临时借用：冲突检查 + 登记（仅当前语句活跃）。
            // - `*p` 读 / `*p = v` 写经引用进行：读原变量不受限（宽松），
            //   写是借用用途（`&mut` 借出即为此），均不额外检查。
            HirExpr::Ref { expr, is_mut, .. } => {
                if let HirExpr::Variable(src) = expr.as_ref() {
                    self.register_borrow(None, src, *is_mut);
                } else {
                    self.check_expr(expr);
                }
            }
            HirExpr::Deref { expr, .. } => self.check_expr(expr),
            HirExpr::DerefSet { base, value, .. } => {
                self.check_expr(base);
                self.check_expr(value);
            }
        }
    }

    // ---------------- 预扫描（NLL 末次使用收集） ----------------
    // 与 check_block / check_stmt / check_expr 遍历顺序同构，
    // 保证语句序一致；不维护所有权 / 借用状态，仅收集变量使用位置。

    fn collect_block_uses(&mut self, block: &HirBlock, _allow_transfer_pass: bool) {
        for stmt in &block.stmts {
            self.pos += 1;
            self.collect_stmt_uses(stmt);
        }
        if let Some(expr) = &block.final_expr {
            self.pos += 1;
            self.collect_expr_uses(expr);
        }
    }

    fn collect_stmt_uses(&mut self, stmt: &HirStmt) {
        match stmt {
            HirStmt::Let { init, .. } => self.collect_expr_uses(init),
            HirStmt::Expr(e) | HirStmt::Semi(e) => self.collect_expr_uses(e),
        }
    }

    fn collect_expr_uses(&mut self, expr: &HirExpr) {
        match expr {
            HirExpr::Variable(name) => {
                self.uses.entry(name.clone()).or_default().push(self.pos);
            }
            HirExpr::Assign { value, .. } => self.collect_expr_uses(value),
            HirExpr::Binary(_, l, r) => {
                self.collect_expr_uses(l);
                self.collect_expr_uses(r);
            }
            HirExpr::PtrAdd { base, offset, .. } => {
                self.collect_expr_uses(base);
                self.collect_expr_uses(offset);
            }
            HirExpr::Unary(_, e) => self.collect_expr_uses(e),
            HirExpr::SetLookup { value, members, .. } => {
                self.collect_expr_uses(value);
                for m in members {
                    self.collect_expr_uses(m);
                }
            }
            HirExpr::RangeCheck {
                value,
                lower,
                upper,
                ..
            } => {
                self.collect_expr_uses(value);
                if let Some(l) = lower {
                    self.collect_expr_uses(l);
                }
                if let Some(u) = upper {
                    self.collect_expr_uses(u);
                }
            }
            HirExpr::If {
                cond,
                then_block,
                else_block,
            } => {
                self.collect_expr_uses(cond);
                self.collect_block_uses(then_block, false);
                if let Some(eb) = else_block {
                    self.collect_block_uses(eb, false);
                }
            }
            HirExpr::Block(b) | HirExpr::UnsafeBlock(b) => self.collect_block_uses(b, false),
            HirExpr::Call { args, .. } => {
                for a in args {
                    self.collect_expr_uses(a);
                }
            }
            HirExpr::FnPtr(_) => {}
            HirExpr::CallIndirect { callee, args, .. } => {
                self.collect_expr_uses(callee);
                for a in args {
                    self.collect_expr_uses(a);
                }
            }
            HirExpr::While { cond, body } => {
                self.collect_expr_uses(cond);
                self.collect_block_uses(body, false);
            }
            HirExpr::Loop { body } => self.collect_block_uses(body, false),
            HirExpr::Return(e) | HirExpr::Break(e) => {
                if let Some(e) = e {
                    self.collect_expr_uses(e);
                }
            }
            HirExpr::Region { body, .. } => self.collect_block_uses(body, true),
            HirExpr::InRegion { expr, .. } => self.collect_expr_uses(expr),
            HirExpr::Transfer { expr, .. } => self.collect_expr_uses(expr),
            HirExpr::IntLiteral(_)
            | HirExpr::FloatLiteral(_)
            | HirExpr::StringLiteral(_)
            | HirExpr::CharLiteral(_)
            | HirExpr::BoolLiteral(_)
            | HirExpr::Continue
            | HirExpr::Unit
            | HirExpr::Alloc { .. } => {}
            HirExpr::FieldGet { base, .. } => self.collect_expr_uses(base),
            HirExpr::FieldSet { base, value, .. } => {
                self.collect_expr_uses(base);
                self.collect_expr_uses(value);
            }
            HirExpr::Index { base, index, .. } => {
                self.collect_expr_uses(base);
                self.collect_expr_uses(index);
            }
            HirExpr::IndexSet {
                base,
                index,
                value,
                ..
            } => {
                self.collect_expr_uses(base);
                self.collect_expr_uses(index);
                self.collect_expr_uses(value);
            }
            HirExpr::Ref { expr, .. } => self.collect_expr_uses(expr),
            HirExpr::Deref { expr, .. } => self.collect_expr_uses(expr),
            HirExpr::DerefSet { base, value, .. } => {
                self.collect_expr_uses(base);
                self.collect_expr_uses(value);
            }
            // U6 Cast IR：`expr as T` 读取被转换表达式
            HirExpr::Cast { expr, .. } => self.collect_expr_uses(expr),
        }
    }
}

impl Default for BorrowChecker {
    fn default() -> Self {
        Self::new()
    }
}
