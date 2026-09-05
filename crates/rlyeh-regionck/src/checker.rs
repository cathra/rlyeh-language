//! 区域检查器：验证区域的嵌套、对象归属与 `transfer` 合法性。

use std::collections::{HashMap, HashSet};

use rlyeh_hir::{
    HirBlock, HirExpr, HirExprKind, HirItemKind, HirProgram, HirStmt, HirStmtKind,
};
use rlyeh_lexer::Span;

use crate::error::RegionError;

/// 检查过程中的可变状态。
struct CheckState {
    /// 当前活跃区域栈（由 `region` 块进入/退出）。
    active_regions: Vec<String>,
    /// 变量 → 所在区域（由 `in 'r` 记录）。
    allocated: HashMap<String, String>,
    /// 已 transfer 的变量集合（防重复转移）。
    transferred: HashSet<String>,
    /// 收集到的错误。
    errors: Vec<RegionError>,
    /// 当前查错节点的坐标（表达式 / 语句 / 块 / 函数级回退）：由 `check_expr` /
    /// `check_stmt` / `check_block` 在入口处设为被查节点自身的 `span`，取代此前
    /// 函数级 `HirItem.span` 的粗粒度坐标，使报错定位到具体节点。
    cur_span: Span,
}

impl CheckState {
    fn new() -> Self {
        Self {
            active_regions: Vec::new(),
            allocated: HashMap::new(),
            transferred: HashSet::new(),
            errors: Vec::new(),
            cur_span: Span {
                start: 0,
                end: 0,
                line: 0,
                col: 0,
            },
        }
    }

    fn region_in_scope(&self, name: &str) -> bool {
        self.active_regions.contains(&name.to_string())
    }
}

/// 区域检查器。
///
/// MVP 阶段检查范围：
/// - 区域块的嵌套与匿名区域唯一化；
/// - `in 'r` / `transfer ... out of 'r` 引用未定义的区域；
/// - 被 transfer 的对象确实分配在声明的源区域内；
/// - 同一对象不得重复 transfer；
/// - 不能从内层区域转移外层区域的对象（P005 嵌套方向检查）；
/// - 无法静态判定归属的 transfer（调用结果等）直接拒绝（P005 PartialTransfer）。
///
/// 引用逃逸（RegionEscape）的完整分析依赖类型信息，留待后续阶段；
/// `CannotTransferReference` / `UnsizedTransfer` 为防御性变体，
/// 待 HIR 引入引用节点与类型标注后启用。
pub struct RegionChecker {
    state: CheckState,
    anon_counter: usize,
}

impl RegionChecker {
    /// 创建空检查器。
    pub fn new() -> Self {
        Self {
            state: CheckState::new(),
            anon_counter: 0,
        }
    }

    /// 检查整个程序，返回 `Ok(())` 或收集到的错误列表。
    pub fn check_program(&mut self, program: &HirProgram) -> Result<(), Vec<RegionError>> {
        for item in &program.items {
            self.check_item(item);
        }
        if self.state.errors.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.state.errors))
        }
    }

    fn check_item(&mut self, item: &rlyeh_hir::HirItem) {
        if let HirItemKind::Fn(f) = &item.kind {
            if let Some(body) = &f.body {
                self.state.cur_span = item.span;
                self.check_block(body);
            }
        }
    }

    fn check_block(&mut self, block: &HirBlock) {
        self.state.cur_span = block.span;
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
        if let Some(expr) = &block.final_expr {
            self.check_expr(expr);
        }
    }

    fn check_stmt(&mut self, stmt: &HirStmt) {
        self.state.cur_span = stmt.span;
        match &stmt.kind {
            HirStmtKind::Let { name, init, .. } => {
                self.check_expr(init);
                // `let x = expr in 'r`：InRegion 包裹的是初始化表达式本身，
                // 需要在此记录变量 `x` 的归属区域
                if let HirExprKind::InRegion { region, .. } = &(init).kind {
                    if self.state.region_in_scope(region) {
                        self.state.allocated.insert(name.clone(), region.clone());
                    }
                }
            }
            HirStmtKind::Expr(e) | HirStmtKind::Semi(e) => self.check_expr(e),
        }
    }

    /// 递归遍历表达式，维护区域栈与变量归属。
    fn check_expr(&mut self, expr: &HirExpr) {
        self.state.cur_span = expr.span;
        match &expr.kind {
            HirExprKind::Region { name, body, .. } => {
                let key = match name {
                    Some(n) => n.clone(),
                    None => {
                        let key = format!("<anon@{}>", self.anon_counter);
                        self.anon_counter += 1;
                        key
                    }
                };
                self.state.active_regions.push(key.clone());
                self.check_block(body);
                self.state.active_regions.pop();
                // 区域结束后，未 transfer 的区域内变量归属一并清除
                self.state.allocated.retain(|_, r| *r != key);
            }
            HirExprKind::InRegion { expr, region, .. } => {
                self.check_expr(expr);
                if !self.state.region_in_scope(region) {
                    self.state.errors.push(RegionError::not_found(region, self.state.cur_span));
                    return;
                }
                if let HirExprKind::Variable(v) = &(expr.as_ref()).kind {
                    self.state.allocated.insert(v.clone(), region.clone());
                }
            }
            HirExprKind::Transfer { expr, region } => {
                self.check_expr(expr);
                match &expr.as_ref().kind {
                    HirExprKind::Variable(v) => self.check_transfer(v, region),
                    // 非变量表达式（调用结果、复合表达式等）无法静态判定其归属区域，
                    // 也不存在"已分配于区域"的对象可转移（P005：PartialTransfer）。
                    other => {
                        let _ = other;
                        self.state.errors.push(RegionError::partial_transfer(
                            "cannot statically determine the owning region of the transferred value",
                            self.state.cur_span,
                        ));
                    }
                }
            }
            HirExprKind::Assign { value, .. } => self.check_expr(value),
            HirExprKind::Binary(_, l, r) => {
                self.check_expr(l);
                self.check_expr(r);
            }
            HirExprKind::Unary(_, e) => self.check_expr(e),
            HirExprKind::SetLookup { value, .. } => self.check_expr(value),
            HirExprKind::RangeCheck {
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
            HirExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                self.check_expr(cond);
                self.check_block(then_block);
                if let Some(eb) = else_block {
                    self.check_block(eb);
                }
            }
            HirExprKind::Block(b) | HirExprKind::UnsafeBlock(b) => self.check_block(b),
            HirExprKind::While { cond, body } => {
                self.check_expr(cond);
                self.check_block(body);
            }
            HirExprKind::Loop { body } => self.check_block(body),
            HirExprKind::Call { args, .. } => {
                for a in args {
                    self.check_expr(a);
                }
            }
            // 函数地址值：无区域归属
            HirExprKind::FnPtr(_) => {}
            // 间接调用：callee 与实参递归检查
            HirExprKind::CallIndirect { callee, args, .. } => {
                self.check_expr(callee);
                for a in args {
                    self.check_expr(a);
                }
            }
            HirExprKind::Return(e) | HirExprKind::Break(e) => {
                if let Some(e) = e {
                    self.check_expr(e);
                }
            }
            // 字面量 / 变量引用 / continue / 单元值：无子表达式
            HirExprKind::IntLiteral(_)
            | HirExprKind::FloatLiteral(_)
            | HirExprKind::StringLiteral(_)
            | HirExprKind::CharLiteral(_)
            | HirExprKind::BoolLiteral(_)
            | HirExprKind::Variable(_)
            | HirExprKind::Continue
            | HirExprKind::Unit => {}
            // 聚合对象构造 / 访问：Alloc 无子表达式；FieldGet / FieldSet 递归检查
            HirExprKind::Alloc { .. } => {}
            HirExprKind::FieldGet { base, .. } => self.check_expr(base),
            HirExprKind::FieldSet { base, value, .. } => {
                self.check_expr(base);
                self.check_expr(value);
            }
            // 索引读取 / 写入：递归检查基址、索引与值表达式
            HirExprKind::Index { base, index, .. } => {
                self.check_expr(base);
                self.check_expr(index);
            }
            HirExprKind::IndexSet {
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
            HirExprKind::Ref { expr, .. } => self.check_expr(expr),
            HirExprKind::PtrAdd { base, offset, .. } => {
                self.check_expr(base);
                self.check_expr(offset);
            }
            // U6 Cast IR：`expr as T` 仅检查被转换表达式
            HirExprKind::Cast { expr, .. } => self.check_expr(expr),
            HirExprKind::Deref { expr, .. } => self.check_expr(expr),
            HirExprKind::DerefSet { base, value, .. } => {
                self.check_expr(base);
                self.check_expr(value);
            }
        }
    }

    /// 校验一次 `transfer v out of 'r`：
    /// 1. 区域 `'r` 必须当前可见；
    /// 2. `v` 不得被重复转移；
    /// 3. `v` 必须分配在 `'r` 内；
    /// 4. `'r` 必须是当前最内层活跃区域（不能从内层转移外层区域对象，P005）。
    fn check_transfer(&mut self, v: &str, region: &str) {
        if !self.state.region_in_scope(region) {
            self.state.errors.push(RegionError::not_found(region, self.state.cur_span));
            return;
        }
        if self.state.transferred.contains(v) {
            self.state.errors.push(RegionError::double_transfer(v, self.state.cur_span));
            return;
        }
        match self.state.allocated.get(v) {
            Some(r) if r == region => {
                // 嵌套方向检查：transfer 必须写在源区域的直接作用域内，
                // 栈顶不是源区域说明从更内层区域转移外层区域对象。
                if self.state.active_regions.last().map(String::as_str) != Some(region) {
                    self.state
                        .errors
                        .push(RegionError::outer_region_transfer(format!(
                            "object `{v}` belongs to region `'{region}`, \
                             but the transfer occurs inside a nested region; \
                             move the transfer into `'{region}` directly"
                        ), self.state.cur_span));
                    return;
                }
                self.state.transferred.insert(v.to_string());
            }
            Some(other) => {
                self.state
                    .errors
                    .push(RegionError::invalid_transfer(format!(
                        "object `{v}` is allocated in region `'{other}`, not `'{region}`"
                    ), self.state.cur_span));
            }
            None => {
                self.state
                    .errors
                    .push(RegionError::invalid_transfer(format!(
                        "object `{v}` is not allocated in any visible region"
                    ), self.state.cur_span));
            }
        }
    }
}

impl Default for RegionChecker {
    fn default() -> Self {
        Self::new()
    }
}
