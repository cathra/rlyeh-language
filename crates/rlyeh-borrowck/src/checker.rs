//! 借用检查器：L0 静态所有权验证。

use std::collections::HashMap;

use rlyeh_hir::{
    FieldScalar, HirBlock, HirExpr, HirExprKind, HirItemKind, HirProgram, HirStmt, HirStmtKind,
};
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

/// 引用存放位置（T-7）：根变量名 + 字段/索引访问路径。
///
/// 用于追踪「引用存进聚合字段/数组元素」这一此前漏检的内嵌引用
/// （`let w = Wrapper { inner: &x }; *w.inner`），把字段读写位置与
/// referent 关联起来。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Place {
    /// 根变量名（如 `w`，来自 `ref_root`）。
    root: String,
    /// 访问路径步：`Field(i)` = 第 `i` 槽（字段/元组元素/枚举负载），
    /// `Index` = 数组/切片索引槽。
    steps: Vec<PlaceStep>,
}

/// 位置访问步（T-7）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PlaceStep {
    /// 聚合对象槽位。
    Field(usize),
    /// 数组 / 切片索引槽。
    Index,
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
    /// 创建处源码位置（SH-P2-6 L2 相关位置标注：冲突时回指此处）。
    span: Span,
    /// 引用是否逃逸（创建时据 `is_escaping_root(source)` 快照，T-2）：
    /// 源为局部/按值参数时 `true`（悬垂风险），源为引用参数/全局时为 `false`。
    /// 在创建时刻求值，避免后续源绑定出作用域后 `is_escaping_root` 误判为否。
    escapes: bool,
    /// 块级区间模型（T-6）：本借用所属块的序号；活跃期 = `[born, block_ends[block_id])`
    /// （活跃到最近块/region 边界，取代 NLL 近似 `last_use`）。
    block_id: usize,
    /// 内嵌引用存放位置（T-7）：当引用被存入字段/索引用 `FieldSet` / `IndexSet`
    /// （如 `Wrapper { inner: &x }`）时记录其 place（根变量 + 字段/索引路径），
    /// 供解引用该字段时回溯 referent、校验悬垂。普通具名/临时借用为 `None`。
    embedded_place: Option<Place>,
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
///   字段内引用悬垂；MVP 悬垂检查覆盖 `return`、函数体块尾值与**引用别名**
///   （`let s = r;` / `s = r` / `let s = { ...; r }` 拷贝传播，T-2）三个出口。
/// - **region 边界悬垂（T-3）**：引用指向 `region 'r {}` 内创建的值，区域在块尾
///   批量释放后该引用仍被使用（或作为区域块尾值逃逸到区外变量）→ `DanglingReference`。
///   由 `region_local_stack` 记录区内局部变量，在 region 退出时扫描「生于区内、
///   指向 region 局部、却活到区外」的引用；`let r = region 'r { &x }` 在
///   `check_stmt` 落点单独处理区尾引用逃逸。
///
/// HIR 子节点（表达式 / 语句 / 块）现已携带源 `Span`；错误坐标取自查错节点
/// 自身的 `span`（表达式 / 语句 / 块级粒度，合并源码坐标），经 `render`
/// 还原为用户坐标（取代此前函数级 `HirItem.span` 的粗粒度坐标）。
/// 解析 `&expr` 的「引用根变量」：沿 `FieldGet` / `Index` 链下钻到底层变量名
///（如 `&x.a.b[0].c` → `x`）。用于悬垂判定（lang-defects #9）。
fn ref_root(expr: &HirExpr) -> Option<String> {
    match &expr.kind {
        HirExprKind::Variable(name) => Some(name.clone()),
        HirExprKind::FieldGet { base, .. } => ref_root(base),
        HirExprKind::Index { base, .. } => ref_root(base),
        _ => None,
    }
}



/// 借用检查器。
pub struct BorrowChecker {
    scopes: Vec<Scope>,
    errors: Vec<BorrowError>,
    /// 当前函数内的借用记录。
    borrows: Vec<Borrow>,
    /// 引用变量名 → 使用处全局语句序（预扫描收集，供 NLL 末次使用判定）。
    uses: HashMap<String, Vec<usize>>,
    /// 变量名 → 其各次 `let` 定义处的全局语句序（预扫描收集，供同名遮蔽时按绑定实例
    /// 裁剪 `last_use`，修复「uses 按名索引不区分遮蔽」缺陷，RFC §9.8）。
    defs: HashMap<String, Vec<usize>>,
    /// 块级区间模型（T-6）：块序号 → 块尾语句序；栈顶为当前最近块。
    block_ends: HashMap<usize, usize>,
    block_stack: Vec<usize>,
    block_seq: usize,
    /// 当前全局语句序（函数内单调递增；预扫描与检查遍历共用）。
    pos: usize,
    /// 当前函数参数名（悬垂判定：借参数不悬垂）。
    param_names: Vec<String>,
    /// 当前函数**引用参数**名（`&T` / `&mut T` / `&self` / `&mut self`）：
    /// 指向调用方内存，返回 `&param.field` 合法（不悬垂）。由 `HirParam::is_ref`
    /// 填充（lang-defects #9 修复：区分按值/按引用参数）。
    ref_param_names: Vec<String>,
    /// 当前查错节点的坐标（表达式 / 语句 / 块 / 函数级回退）：由 `check_expr` /
    /// `check_stmt` / `check_block` 在入口处设为被查节点自身的 `span`，取代此前
    /// 函数级 `HirItem.span` 的粗粒度坐标，使报错定位到具体节点。
    cur_span: Span,
    /// region 局部变量栈（与词法 `scopes` 平行）：每个 region 层级一份，记录
    /// 在该 region 内声明、将随 region 批量释放而失效的局部变量名（T-3，供
    /// region 边界悬垂判定；不包含参数 / 全局 / region 外的绑定）。
    region_local_stack: Vec<Vec<String>>,
    /// region 退出时暂存的局部变量名（供 `let r = region 'r { &x }` 这类
    /// 「区尾引用逃逸到区外变量」在 `check_stmt` 落点处复用，T-3）。
    recent_region_locals: Option<Vec<String>>,
    /// 变量名 → 存储根变量别名（T-7）：结构体字面量 desugar 为
    /// `let __tmp = alloc; __tmp.field = &x; let w = __tmp`，字段写入侧 base 是临时
    /// 变量 `__tmp` 而读写侧用 `w`，经本表把 `w` 解析回 `__tmp` 以消除 place 根不一致。
    aliases: HashMap<String, String>,
}

impl BorrowChecker {
    /// 创建空检查器。
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            errors: Vec::new(),
            borrows: Vec::new(),
            uses: HashMap::new(),
            defs: HashMap::new(),
            block_ends: HashMap::new(),
            block_stack: Vec::new(),
            block_seq: 0,
            pos: 0,
            param_names: Vec::new(),
            ref_param_names: Vec::new(),
            cur_span: Span {
                start: 0,
                end: 0,
                line: 0,
                col: 0,
            },
            region_local_stack: Vec::new(),
            recent_region_locals: None,
            aliases: HashMap::new(),
        }
    }

    /// T-7：若 `value` 是引用且 `base` 可解析为字段/索引存放位置，登记一条内嵌引用借用，
    /// 使后续解引用该字段（或经 `let r = w.inner` 具名化）时能回溯 referent、校验悬垂
    /// （含 region 边界扫描 T-3 复用）。`step` 为本次写入的槽步（`Field(i)` / `Index`）。
    fn record_embedded_borrow(&mut self, base: &HirExpr, step: PlaceStep, value: &HirExpr) {
        if let HirExprKind::Ref { expr, is_mut, .. } = &(value).kind {
            if let (Some(mut place), Some(referent)) = (self.place_of(base), ref_root(expr)) {
                place.steps.push(step);
                let kind = if *is_mut {
                    BorrowKind::Mut
                } else {
                    BorrowKind::Shared
                };
                self.borrows.push(Borrow {
                    source: referent.clone(),
                    var: None,
                    kind,
                    born: self.pos,
                    last_use: self.pos,
                    span: self.cur_span,
                    escapes: self.is_escaping_root(&referent),
                    block_id: *self.block_stack.last().unwrap_or(&0),
                    embedded_place: Some(place),
                });
            }
        }
    }

    /// T-7：解引用「持有引用的字段/索引槽」（`ty`/`elem == Ptr`）时，回溯内嵌引用的
    /// referent，校验其是否已 `transfer`（move 出作用域 → 悬垂，BC005），并把该内嵌
    /// 借用的末次使用推进到当前语句序，使 region 边界扫描（T-3）能捕获「referent 为
    /// region 局部、内嵌引用活到区外」的悬垂。
    fn check_embedded_deref(&mut self, base: &HirExpr) {
        let is_ref_slot = match &base.kind {
            HirExprKind::FieldGet { ty, .. } => *ty == FieldScalar::Ptr,
            HirExprKind::Index { elem, .. } => *elem == FieldScalar::Ptr,
            _ => false,
        };
        if !is_ref_slot {
            return;
        }
        let Some(place) = self.place_of(base) else {
            return;
        };
        // 先以不可变遍历定位内嵌借用，避免在持有 `&mut Borrow` 时调用 `&self` 方法
        // （E0502）；定位后再推进 `last_use` 并据 referent 是否转移判定悬垂。
        let mut hit: Option<(usize, bool)> = None;
        for (i, b) in self.borrows.iter().enumerate() {
            if b.embedded_place.as_ref() == Some(&place) {
                hit = Some((i, self.is_transferred(&b.source)));
                break;
            }
        }
        if let Some((i, transferred)) = hit {
            self.borrows[i].last_use = self.pos;
            if transferred {
                let src = self.borrows[i].source.clone();
                self.errors
                    .push(BorrowError::dangling_reference(&src, self.cur_span));
            }
        }
    }

    /// 表达式求值后的「最终变量」（T-7）：用于 `let w = <expr>` 时把 `w` 关联到
    /// 其底层存储变量。覆盖 `Variable`、`InRegion { .. }`、块末表达式等形态，
    /// 从而识别结构体字面量 desugar 出的 `let w = <__tmp>` 别名。
    fn final_var(&self, expr: &HirExpr) -> Option<String> {
        match &expr.kind {
            HirExprKind::Variable(v) => Some(v.clone()),
            HirExprKind::InRegion { expr, .. } => self.final_var(expr),
            HirExprKind::Block(block) => block
                .final_expr
                .as_ref()
                .and_then(|e| self.final_var(e)),
            _ => None,
        }
    }

    /// 变量名 → 其「存储根」变量（T-7）：结构体字面量 desugar 为
    /// `let __tmp = alloc; __tmp.field = &x; let w = __tmp`，字段写入的 base 是
    /// 临时变量 `__tmp` 而后续读写用 `w`，故需经别名链把 `w` 解析回 `__tmp`
    /// （存储根），使内嵌引用的 place 在写入侧与读取侧一致。
    fn resolve_root(&self, name: &str) -> String {
        let mut cur = name.to_string();
        let mut seen = std::collections::HashSet::new();
        while let Some(next) = self.aliases.get(&cur) {
            if !seen.insert(cur.clone()) {
                break; // 环保护
            }
            cur = next.clone();
        }
        cur
    }

    /// 从 HIR 表达式抽取引用存放位置（T-7）；仅对变量 / 字段读取 / 索引读取
    /// 有效，其它（字面量 / 调用等）返回 `None`。变量根经 [`Self::resolve_root`]
    /// 解析为存储根，闭合结构体字面量临时变量别名缺口。
    fn place_of(&self, expr: &HirExpr) -> Option<Place> {
        match &expr.kind {
            HirExprKind::Variable(v) => Some(Place {
                root: self.resolve_root(v),
                steps: Vec::new(),
            }),
            HirExprKind::FieldGet { base, index, .. } => {
                let mut p = self.place_of(base)?;
                p.steps.push(PlaceStep::Field(*index));
                Some(p)
            }
            HirExprKind::Index { base, .. } => {
                let mut p = self.place_of(base)?;
                p.steps.push(PlaceStep::Index);
                Some(p)
            }
            _ => None,
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
                    self.ref_param_names = f
                        .params
                        .iter()
                        .filter(|p| p.is_ref)
                        .map(|p| p.name.clone())
                        .collect();
                    self.borrows.clear();
                    self.uses.clear();
                    self.defs.clear();
                    self.block_stack.clear();
                    self.block_ends.clear();
                    self.block_seq = 0;
                    self.region_local_stack.clear();
                    self.recent_region_locals = None;
                    self.aliases.clear();
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

    /// 变量名在当前语句序上的活跃借用（T-6 块级区间：`born <= pos < block_ends[block_id]`）。
    ///
    /// 跳过「内嵌引用借用」（`embedded_place.is_some()`，见 T-7）：这类借用是引用
    /// 经字段/索引存放时产生的，仅用于悬垂检测（BC005），其 `source` 指向被引用的
    /// 根变量（如 `&mut self` 存进 guard 字段）。若参与冲突判定会误伤标准库
    /// `DerefMut` / `lock_guard` 等把 `&mut self` 存入内部字段的正常代码，产生虚假
    /// BC003。普通临时借用（`var: None, embedded_place: None`，如 `f(&x)`）仍纳入判定。
    fn active_borrows(&self, source: &str) -> Vec<&Borrow> {
        self.borrows
            .iter()
            .filter(|b| {
                let end = self.block_ends.get(&b.block_id).copied().unwrap_or(usize::MAX);
                b.source == source
                    && b.born <= self.pos
                    && self.pos < end
                    && b.embedded_place.is_none()
            })
            .collect()
    }

    /// 借用创建：冲突检查 + 登记。
    ///
    /// `var`：`let r = &x;` 的引用变量名；表达式内临时借用（`f(&x)`、
    /// 聚合字段内嵌 `&x`）传 `None`（仅当前语句活跃）。
    fn register_borrow(&mut self, var: Option<&str>, source: &str, is_mut: bool, check_mut: bool) {
        // 可变性互斥 / 别名冲突
        for b in self.active_borrows(source) {
            match (b.kind, is_mut) {
                (BorrowKind::Mut, true) => {
                    self.errors.push(
                        BorrowError::borrow_conflict(
                            format!(
                                "cannot mutably borrow `{source}` because it is already borrowed as mutable"
                            ),
                            self.cur_span,
                        )
                        .with_related(vec![(b.span, "先前借用创建于此".to_string())]),
                    );
                    return;
                }
                (BorrowKind::Mut, false) => {
                    self.errors.push(
                        BorrowError::borrow_conflict(
                            format!(
                                "cannot borrow `{source}` as shared because it is already borrowed as mutable"
                            ),
                            self.cur_span,
                        )
                        .with_related(vec![(b.span, "先前借用创建于此".to_string())]),
                    );
                    return;
                }
                (BorrowKind::Shared, true) => {
                    self.errors.push(
                        BorrowError::borrow_conflict(
                            format!(
                                "cannot mutably borrow `{source}` because it is already borrowed as shared"
                            ),
                            self.cur_span,
                        )
                        .with_related(vec![(b.span, "先前借用创建于此".to_string())]),
                    );
                    return;
                }
                (BorrowKind::Shared, false) => {}
            }
        }
        // `&mut x` 要求 `x` 为 `let mut`（E0596）；全局 / 未绑定（const）宽松跳过。
        // `check_mut=false` 用于字段/索引引用：字段级 `&mut`（含 `&mut self.field`）
        // 由 check_expr 既有路径处理，此处仅登记借用做悬垂判定，跳过可变性检查（#9 修复）。
        if is_mut && check_mut {
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
            .map(|v| self.last_use_for(v, self.pos))
            .unwrap_or(self.pos);
        self.borrows.push(Borrow {
            source: source.to_string(),
            var: var.map(|s| s.to_string()),
            kind,
            born: self.pos,
            last_use,
            span: self.cur_span,
            escapes: self.is_escaping_root(source),
            block_id: *self.block_stack.last().unwrap_or(&0),
            embedded_place: None,
        });
    }

    /// 从初始化 / 赋值表达式抽取「最终求值为的引用变量名」：
    /// - `Variable(v)` → `Some(v)`（直接引用变量拷贝）；
    /// - `Block` / `UnsafeBlock` 且块尾值为 `Variable(v)` → `Some(v)`（块值引用传播，T-2）；
    /// - 其它 → `None`。
    fn ref_source_var(&self, expr: &HirExpr) -> Option<String> {
        match &expr.kind {
            HirExprKind::Variable(v) => Some(v.clone()),
            HirExprKind::Block(b) | HirExprKind::UnsafeBlock(b) => {
                if let Some(fe) = &b.final_expr {
                    if let HirExprKind::Variable(v) = &fe.kind {
                        return Some(v.clone());
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// 引用变量拷贝（`let b = a;` / `b = a;` / `let b = { ...; a }` 且 `a` 为引用变量）：
    /// 为 `to` 登记一条与 `from` 同源的借用，使引用存活边经拷贝传播，闭合别名逃逸缺口（T-2）。
    ///
    /// 若 `from` 非引用变量（无任何以其为 `var` 的借用记录），则无操作——普通值拷贝不影响借用图。
    fn copy_borrow(&mut self, to: &str, from: &str) {
        let Some((source, kind, escapes)) = self
            .borrows
            .iter()
            .filter(|b| b.var.as_deref() == Some(from))
            .max_by_key(|b| b.born)
            .map(|b| (b.source.clone(), b.kind, b.escapes))
        else {
            return;
        };
        let last_use = self.last_use_for(to, self.pos);
        self.borrows.push(Borrow {
            source,
            var: Some(to.to_string()),
            kind,
            born: self.pos,
            last_use,
            span: self.cur_span,
            escapes,
            block_id: *self.block_stack.last().unwrap_or(&0),
            embedded_place: None,
        });
    }

    /// 计算「具名引用变量 `var` 的某次绑定（定义于 `born`）」的有效末次使用位置。
    ///
    /// 修复 RFC §9.8 缺陷：原实现直接取 `uses[var].last()`（按名索引），在**同名遮蔽**
    /// 时会被后续绑定的使用位置污染，导致 `last_use` 越过 region 边界误报 `DanglingReference`。
    /// 此处仅取「`>= born` 且 `< 下一次同名重定义`」区间内的使用位置最大值，使每个绑定实例
    /// 的活跃期互不干扰。无遮蔽时 `next_def = ∞`，等价于原 `uses[var].last()`（零行为变化）。
    fn last_use_for(&self, var: &str, born: usize) -> usize {
        let next_def = self
            .defs
            .get(var)
            .and_then(|ds| ds.iter().filter(|&&d| d > born).min().copied())
            .unwrap_or(usize::MAX);
        self.uses
            .get(var)
            .and_then(|us| {
                us.iter()
                    .filter(|&&u| u >= born && u < next_def)
                    .max()
                    .copied()
            })
            .unwrap_or(born)
    }

    /// 引用根变量 `root` 指向的存储是否「逃出当前函数帧即悬垂」。
    ///
    /// - 引用参数（`&self` 等，`ref_param_names`）：指向调用方内存 → 不悬垂；
    /// - 全局 / const（既非局部绑定也非参数）→ 生命周期静态 → 不悬垂；
    /// - 局部 `let` 绑定 / 按值参数（`self` 按值等）→ 帧销毁即失效 → 悬垂。
    fn is_escaping_root(&self, root: &str) -> bool {
        if self.ref_param_names.iter().any(|n| n == root) {
            return false;
        }
        let is_local_or_param =
            self.lookup(root).is_some() || self.param_names.iter().any(|n| n == root);
        if !is_local_or_param {
            return false;
        }
        true
    }

    /// 悬垂检查：返回的引用必须指向参数（或全局），不能是局部变量的引用。
    /// （`fn f() -> &i64 { let x = 1; &x }`，Rust E0597 对应）。
    fn check_dangling_return(&mut self, expr: &HirExpr) {
        match &expr.kind {
            HirExprKind::Variable(v) => {
                for b in &self.borrows {
                    if b.var.as_deref() == Some(v) && b.escapes {
                        self.errors.push(BorrowError::dangling_reference(v, self.cur_span));
                        return;
                    }
                }
            }
            HirExprKind::Ref { expr: inner, .. } => {
                // #9：沿字段/索引链解析引用根变量；按值参数 / 局部绑定逃逸出
                // 函数帧即悬垂，引用参数（`&self` 等）指向调用方内存则合法。
                if let Some(root) = ref_root(inner) {
                    if self.is_escaping_root(&root) {
                        self.errors.push(BorrowError::dangling_reference(&root, self.cur_span));
                    }
                }
            }
            // V2-D（2026-08-26）：`s.as_str()` / `s.as_str_range(..)` 的返回是
            // StrFat 构造块（`Alloc{is_strfat}` + FieldSet data/len），悬垂检查需
            // 识别其 data 槽来源：若指向局部 String（非参数），返回 `&str` 悬垂。
            HirExprKind::Block(block) | HirExprKind::UnsafeBlock(block) => self.check_dangling_strfat_block(block),
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
            match &stmt.kind {
                HirStmtKind::Let { init, name, .. } => {
                    if let HirExprKind::Alloc {
                        is_strfat: true, ..
                    } = &(init).kind {
                        sf = Some(name.clone());
                    }
                    lets.push(stmt);
                }
                HirStmtKind::Semi(e) => {
                    if let HirExprKind::FieldSet { base, .. } = &e.kind {
                        if let HirExprKind::Variable(b) = &(base.as_ref()).kind {
                            if sf.as_deref() == Some(b) {
                                field_sets.push(stmt);
                            }
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
            if let HirStmtKind::Semi(e) = &(stmt).kind {
                if let HirExprKind::FieldSet {
                    base,
                    index: 0,
                    value,
                    ..
                } = &e.kind {
                    if let HirExprKind::Variable(b) = &(base.as_ref()).kind {
                        if *b == sf_name {
                            if let HirExprKind::Variable(v) = &(value.as_ref()).kind {
                                data_src = Some(v.clone());
                            }
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
            if let HirStmtKind::Let { name, init, .. } = &(stmt).kind {
                if *name == data_tmp {
                    if let HirExprKind::FieldGet { base, index: 0, .. } = &(init).kind {
                        if let HirExprKind::Variable(b) = &(base.as_ref()).kind {
                            base_src = Some(b.clone());
                        }
                    }
                }
            }
        }
        if let Some(src) = base_src {
            if self.is_escaping_root(&src) {
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
        self.cur_span = block.span;
        self.block_seq += 1;
        let block_id = self.block_seq;
        self.block_stack.push(block_id);
        for stmt in &block.stmts {
            self.pos += 1;
            self.check_stmt(stmt);
        }
        if let Some(expr) = &block.final_expr {
            self.pos += 1;
            // transfer 例外：`transfer x out of 'r; x` 中 x 作为块值 =
            // 所有权转出给区域表达式的求值结果，不视为"使用"。
            if allow_transfer_pass {
                if let HirExprKind::Variable(name) = &(expr).kind {
                    if self.is_transferred(name) {
                        self.block_ends.insert(block_id, self.pos);
                        self.block_stack.pop();
                        return;
                    }
                }
            }
            self.check_expr(expr);
        }
        self.block_ends.insert(block_id, self.pos);
        self.block_stack.pop();
    }

    fn check_stmt(&mut self, stmt: &HirStmt) {
        self.cur_span = stmt.span;
        match &stmt.kind {
            HirStmtKind::Let {
                name,
                init,
                mutable,
            } => {
                // 借用创建特判：`let r = &x;` / `let r = &mut x;` ——
                // 具名借用（引用变量 = r），借用活跃到 r 最后一次使用。
                // 先检查借用（shadowing 时 init 引用的是旧绑定），再注册绑定。
                // T-3：`let r = region 'r { &x }` —— region 块尾值是引用，逃逸出
                // region（其指向值在 region 退出时批量释放）→ 悬垂。在此处分流：
                // 递归检查 region 体（消费 recent_region_locals），再以区外变量 `r`
                // 之名登记同源借用并据「referent 是否 region 局部」判定悬垂。
                if let HirExprKind::Region { body, .. } = &(init).kind {
                    if let Some(final_expr) = &body.final_expr {
                        if let HirExprKind::Ref { expr, is_mut, .. } = &final_expr.kind {
                            if let Some(src) = ref_root(expr) {
                                self.check_expr(init);
                                let _boundary = self.pos;
                                let mut dangling = false;
                                if let Some(locals) = self.recent_region_locals.take() {
                                    // 区尾引用指向 region 局部：无论外部是否使用，该引用
                                    // 值已逃逸出 region（其指向值在 region 退出时释放）→ 悬垂。
                                    if locals.iter().any(|n| n == &src) {
                                        dangling = true;
                                    }
                                }
                                if dangling {
                                    // 区尾引用已逃逸出 region：直接报错，不再登记
                                    // 具名借用（避免后续 `return r` 二次捕获叠加报错）。
                                    self.errors.push(BorrowError::dangling_reference(
                                        &src,
                                        self.cur_span,
                                    ));
                                } else {
                                    // 以区外变量 `r` 之名登记同源借用：其 `escapes`
                                    // 据 referent 是否逃逸帧判定，故后续若 `r` 被
                                    // return 仍可经由 check_dangling_return 二次捕获。
                                    self.register_borrow(Some(name), &src, *is_mut, *is_mut);
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
                                return;
                            }
                        }
                    }
                }
                // T-7：引用经聚合字段传播（`let r = w.inner;`）。若该字段持有引用
                // （ty/elem == Ptr）且已在 FieldSet/IndexSet 登记内嵌借用，则把
                // 该内嵌借用转为具名借用 `r`（复用既有悬垂/冲突/region 边界扫描），
                // 闭合「字段中存储的引用」追踪缺口。
                if let HirExprKind::FieldGet { ty, .. } | HirExprKind::Index { elem: ty, .. } =
                    &(init).kind
                {
                    if *ty == FieldScalar::Ptr {
                        if let Some(place) = self.place_of(&init) {
                            let idx = self
                                .borrows
                                .iter()
                                .position(|b| b.embedded_place.as_ref() == Some(&place));
                            if let Some(idx) = idx {
                                // 把该内嵌借用转为具名借用 `r`：先以不可变遍历定位，
                                // 释出借用后再改字段，避免持有 `&mut Borrow` 时调用
                                // `&self` 方法（E0502）。
                                let src = self.borrows[idx].source.clone();
                                let last_use = self.last_use_for(name, self.pos);
                                let block_id = *self.block_stack.last().unwrap_or(&0);
                                let escapes = self.is_escaping_root(&src);
                                self.borrows[idx].var = Some(name.clone());
                                self.borrows[idx].embedded_place = None;
                                self.borrows[idx].last_use = last_use;
                                self.borrows[idx].block_id = block_id;
                                self.borrows[idx].escapes = escapes;
                                if name != "_" {
                                    self.insert(
                                        name.clone(),
                                        Binding {
                                            mutable: *mutable,
                                            transferred: false,
                                        },
                                    );
                                }
                                return;
                            }
                        }
                    }
                }
                if let HirExprKind::Ref { expr, is_mut, .. } = &(init).kind {
                    match &(expr.as_ref()).kind {
                        HirExprKind::Variable(src) => {
                            // 直接变量引用：完整借用检查（含可变性 E0596）
                            self.register_borrow(Some(name), src, *is_mut, true);
                        }
                        _ => {
                            if let Some(src) = ref_root(expr) {
                                // 字段/索引引用：仅登记用于悬垂判定，跳过可变性检查
                                // （字段级 &mut 由 check_expr 既有路径处理，#9 修复避免误报）
                                self.register_borrow(Some(name), &src, *is_mut, false);
                            } else {
                                // 非变量源（防御：typecheck 已限制 `&` 目标为变量）
                                self.check_expr(init);
                            }
                        }
                    }
                } else {
                    self.check_expr(init);
                    // 引用变量拷贝传播（T-2）：`let b = a;` / `let b = { ...; a }`
                    // 且 `a` 为引用变量时，为 `b` 登记与 `a` 同源的借用，闭合别名逃逸缺口。
                    if let Some(from) = self.ref_source_var(init) {
                        self.copy_borrow(name, &from);
                    }
                }
                if name != "_" {
                    self.insert(
                        name.clone(),
                        Binding {
                            mutable: *mutable,
                            transferred: false,
                        },
                    );
                    // T-3：若处于某个 region 内，该绑定随 region 批量释放，
                    // 登记为 region 局部，供 region 边界悬垂判定。
                    if let Some(top) = self.region_local_stack.last_mut() {
                        top.push(name.clone());
                    }
                    // T-7：别名登记——`let w = Wrapper { .. }` 等构造 desugar 为
                    // `let __tmp = alloc; __tmp.field = &x; let w = __tmp`，需把 `w` 关联到
                    // 其底层存储变量 `__tmp`，使字段读写侧的 place 根一致（见 `resolve_root`）。
                    // 跳过引用变量别名（`let r = a`，由 copy_borrow / T-2 处理），避免误伤
                    // 既有拷贝传播。
                    if let Some(rhs) = self.final_var(init) {
                        if rhs != *name
                            && !self.borrows.iter().any(|b| b.var.as_deref() == Some(rhs.as_str()))
                        {
                            self.aliases.insert(name.clone(), self.resolve_root(&rhs));
                        }
                    }
                }
            }
            HirStmtKind::Expr(e) | HirStmtKind::Semi(e) => self.check_expr(e),
        }
    }

    /// 递归检查表达式，维护作用域栈与所有权状态。
    fn check_expr(&mut self, expr: &HirExpr) {
        self.cur_span = expr.span;
        match &expr.kind {
            HirExprKind::Variable(name) => {
                if self.is_transferred(name) {
                    self.errors.push(BorrowError::use_after_transfer(name, self.cur_span));
                }
            }
            HirExprKind::PtrAdd { base, offset, .. } => {
                // 裸指针算术：仅读取 base/offset 指针与偏移值，不产生借用
                self.check_expr(base);
                self.check_expr(offset);
            }
            // U6 Cast IR：`expr as T` 仅读取被转换表达式
            HirExprKind::Cast { expr, .. } => self.check_expr(expr),
            HirExprKind::Assign { target, value, .. } => {
                // 借用互斥：不能赋值（写）被借用中的变量
                let active = self.active_borrows(target);
                if !active.is_empty() {
                    let related: Vec<(Span, String)> = active
                        .first()
                        .map(|b| vec![(b.span, "该变量在此处被借用".to_string())])
                        .unwrap_or_default();
                    self.errors.push(
                        BorrowError::borrow_conflict(
                            format!("cannot assign to `{target}` because it is borrowed"),
                            self.cur_span,
                        )
                        .with_related(related),
                    );
                }
                if let Some(b) = self.lookup(target).cloned() {
                    if b.transferred {
                        // 对已转移值的赋值也是使用（Rust：assignment to moved value）
                        self.errors.push(BorrowError::use_after_transfer(target, self.cur_span));
                    } else if !b.mutable {
                        self.errors.push(BorrowError::assign_to_immutable(target, self.cur_span));
                    }
                }
                // T-3：`r = &x`（区内赋值到区外引用变量）直接为 `r` 登记以 `x`
                // 为源的具名借用，使 region 边界悬垂扫描能捕获「引用随 `r` 逃逸出
                // region」的路径（与 `let r = &x` 同构，仅目标为既有变量）。
                if let HirExprKind::Ref { expr, is_mut, .. } = &(value).kind {
                    match &(expr.as_ref()).kind {
                        HirExprKind::Variable(src) => {
                            self.register_borrow(Some(target), src, *is_mut, true);
                        }
                        _ => {
                            if let Some(src) = ref_root(expr) {
                                self.register_borrow(Some(target), &src, *is_mut, false);
                            }
                        }
                    }
                }
                self.check_expr(value);
                // 引用变量拷贝传播（T-2）：`b = a;` 且 `a` 为引用变量时，
                // `b` 继承 `a` 的借用源，闭合别名逃逸缺口。
                if let Some(from) = self.ref_source_var(value) {
                    self.copy_borrow(target, &from);
                }
            }
            HirExprKind::Binary(_, l, r) => {
                self.check_expr(l);
                self.check_expr(r);
            }
            HirExprKind::Unary(_, e) => self.check_expr(e),
            HirExprKind::SetLookup { value, members, .. } => {
                self.check_expr(value);
                for m in members {
                    self.check_expr(m);
                }
            }
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
                self.scopes.push(Scope::default());
                self.check_block(then_block, false);
                self.scopes.pop();
                if let Some(eb) = else_block {
                    self.scopes.push(Scope::default());
                    self.check_block(eb, false);
                    self.scopes.pop();
                }
            }
            HirExprKind::Block(b) | HirExprKind::UnsafeBlock(b) => {
                self.scopes.push(Scope::default());
                self.check_block(b, false);
                self.scopes.pop();
            }
            HirExprKind::Call { args, .. } => {
                for a in args {
                    self.check_expr(a);
                }
            }
            // 函数地址值：无所有权转移
            HirExprKind::FnPtr(_) => {}
            // 间接调用：callee 与实参均视为使用
            HirExprKind::CallIndirect { callee, args, .. } => {
                self.check_expr(callee);
                for a in args {
                    self.check_expr(a);
                }
            }
            HirExprKind::While { cond, body } => {
                self.check_expr(cond);
                self.scopes.push(Scope::default());
                self.check_block(body, false);
                self.scopes.pop();
            }
            HirExprKind::Loop { body } => {
                self.scopes.push(Scope::default());
                self.check_block(body, false);
                self.scopes.pop();
            }
            HirExprKind::Return(e) => {
                if let Some(e) = e {
                    self.check_expr(e);
                    self.check_dangling_return(e);
                }
            }
            HirExprKind::Break(e) => {
                if let Some(e) = e {
                    self.check_expr(e);
                }
            }
            HirExprKind::Region { body, .. } => {
                self.scopes.push(Scope::default());
                self.region_local_stack.push(Vec::new());
                let start = self.pos;
                self.check_block(body, true);
                let boundary = self.pos;
                let locals = self.region_local_stack.pop().unwrap();
                self.scopes.pop();
                // T-3：region 边界悬垂扫描——生于区内、指向 region 局部、却活到
                // 区外的引用（`r = &x` 在区内赋值到区外变量等路径；区尾引用逃逸
                // 由 `let r = region 'r { &x }` 在 check_stmt 落点单独处理）。
                for b in &self.borrows {
                    if b.born > start
                        && b.born <= boundary
                        && b.last_use > boundary
                        && locals.iter().any(|n| n == &b.source)
                    {
                        self.errors.push(BorrowError::dangling_reference(&b.source, b.span));
                    }
                }
                self.recent_region_locals = Some(locals);
            }
            HirExprKind::InRegion { expr, .. } => self.check_expr(expr),
            HirExprKind::Transfer { expr, .. } => {
                // transfer 是所有权簿记（ADR-003：零拷贝、不读取值本身）；
                // 变量被转移后标记，后续使用报 use-after-move。
                if let HirExprKind::Variable(name) = &(expr.as_ref()).kind {
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
            HirExprKind::IntLiteral(_)
            | HirExprKind::FloatLiteral(_)
            | HirExprKind::StringLiteral(_)
            | HirExprKind::CharLiteral(_)
            | HirExprKind::BoolLiteral(_)
            | HirExprKind::Continue
            | HirExprKind::Unit => {}
            // 聚合对象构造 / 访问：Alloc 无子表达式；FieldGet / FieldSet 递归检查
            HirExprKind::Alloc { .. } => {}
            HirExprKind::FieldGet { base, .. } => self.check_expr(base),
            HirExprKind::FieldSet { base, index, value, .. } => {
                self.record_embedded_borrow(base, PlaceStep::Field(*index), value);
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
                self.record_embedded_borrow(base, PlaceStep::Index, value);
                self.check_expr(base);
                self.check_expr(index);
                self.check_expr(value);
            }
            // 引用 / 解引用：
            // - `&x`（非 `let r = &x` 的表达式内出现，如实参 / 聚合字段 / dyn 构造）
            //   为临时借用：冲突检查 + 登记（仅当前语句活跃）。
            // - `*p` 读 / `*p = v` 写经引用进行：读原变量不受限（宽松），
            //   写是借用用途（`&mut` 借出即为此），均不额外检查。
            HirExprKind::Ref { expr, is_mut, .. } => {
                match &(expr.as_ref()).kind {
                    HirExprKind::Variable(src) => {
                        self.register_borrow(None, src, *is_mut, true);
                    }
                    _ => {
                        if let Some(src) = ref_root(expr) {
                            self.register_borrow(None, &src, *is_mut, false);
                        } else {
                            self.check_expr(expr);
                        }
                    }
                }
            }
            HirExprKind::Deref { expr, .. } => {
                self.check_embedded_deref(expr);
                self.check_expr(expr);
            }
            HirExprKind::DerefSet { base, value, .. } => {
                self.check_embedded_deref(base);
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
        match &stmt.kind {
            HirStmtKind::Let { name, init, .. } => {
                self.defs.entry(name.clone()).or_default().push(self.pos);
                self.collect_expr_uses(init);
            }
            HirStmtKind::Expr(e) | HirStmtKind::Semi(e) => self.collect_expr_uses(e),
        }
    }

    fn collect_expr_uses(&mut self, expr: &HirExpr) {
        match &expr.kind {
            HirExprKind::Variable(name) => {
                self.uses.entry(name.clone()).or_default().push(self.pos);
            }
            HirExprKind::Assign { value, .. } => self.collect_expr_uses(value),
            HirExprKind::Binary(_, l, r) => {
                self.collect_expr_uses(l);
                self.collect_expr_uses(r);
            }
            HirExprKind::PtrAdd { base, offset, .. } => {
                self.collect_expr_uses(base);
                self.collect_expr_uses(offset);
            }
            HirExprKind::Unary(_, e) => self.collect_expr_uses(e),
            HirExprKind::SetLookup { value, members, .. } => {
                self.collect_expr_uses(value);
                for m in members {
                    self.collect_expr_uses(m);
                }
            }
            HirExprKind::RangeCheck {
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
            HirExprKind::If {
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
            HirExprKind::Block(b) | HirExprKind::UnsafeBlock(b) => self.collect_block_uses(b, false),
            HirExprKind::Call { args, .. } => {
                for a in args {
                    self.collect_expr_uses(a);
                }
            }
            HirExprKind::FnPtr(_) => {}
            HirExprKind::CallIndirect { callee, args, .. } => {
                self.collect_expr_uses(callee);
                for a in args {
                    self.collect_expr_uses(a);
                }
            }
            HirExprKind::While { cond, body } => {
                self.collect_expr_uses(cond);
                self.collect_block_uses(body, false);
            }
            HirExprKind::Loop { body } => self.collect_block_uses(body, false),
            HirExprKind::Return(e) | HirExprKind::Break(e) => {
                if let Some(e) = e {
                    self.collect_expr_uses(e);
                }
            }
            HirExprKind::Region { body, .. } => self.collect_block_uses(body, true),
            HirExprKind::InRegion { expr, .. } => self.collect_expr_uses(expr),
            HirExprKind::Transfer { expr, .. } => self.collect_expr_uses(expr),
            HirExprKind::IntLiteral(_)
            | HirExprKind::FloatLiteral(_)
            | HirExprKind::StringLiteral(_)
            | HirExprKind::CharLiteral(_)
            | HirExprKind::BoolLiteral(_)
            | HirExprKind::Continue
            | HirExprKind::Unit
            | HirExprKind::Alloc { .. } => {}
            HirExprKind::FieldGet { base, .. } => self.collect_expr_uses(base),
            HirExprKind::FieldSet { base, value, .. } => {
                self.collect_expr_uses(base);
                self.collect_expr_uses(value);
            }
            HirExprKind::Index { base, index, .. } => {
                self.collect_expr_uses(base);
                self.collect_expr_uses(index);
            }
            HirExprKind::IndexSet {
                base,
                index,
                value,
                ..
            } => {
                self.collect_expr_uses(base);
                self.collect_expr_uses(index);
                self.collect_expr_uses(value);
            }
            HirExprKind::Ref { expr, .. } => self.collect_expr_uses(expr),
            HirExprKind::Deref { expr, .. } => self.collect_expr_uses(expr),
            HirExprKind::DerefSet { base, value, .. } => {
                self.collect_expr_uses(base);
                self.collect_expr_uses(value);
            }
            // U6 Cast IR：`expr as T` 读取被转换表达式
            HirExprKind::Cast { expr, .. } => self.collect_expr_uses(expr),
        }
    }
}

impl Default for BorrowChecker {
    fn default() -> Self {
        Self::new()
    }
}
