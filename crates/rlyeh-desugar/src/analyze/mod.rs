//! async fn 分析：控制流图展开切段（W2）、提升变量、子 future 槽、依赖收集。
//!
//! W2（2026-08-25）架构升级：状态机从「直线切段」扩展为「控制流图展开」——
//! 函数体（含 if/match/loop/for/while 子块）按 await 点递归展开为扁平段列表，
//! 每段携带显式后继（`next`）实现跳转/回跳；表达式中间的嵌套 await 经临时变量
//! 提取为独立段；跨 await 变量类型从 i64 扩展到 f64/bool/char/String。

use std::collections::{HashMap, HashSet};

use rlyeh_ast::{
    AstBlock, AstExpr, AstFnDecl, AstParam, AstPattern, AstStmt, AstType, BinaryOp, CompareOp,
    ExprKind, MatchArm, SpannedAstType,
};
use rlyeh_lexer::Span;

use crate::DesugarError;

/// 函数收尾哨兵（展开完成后回填为收尾段编号）。
pub(crate) const FN_END: usize = usize::MAX;

/// 状态机段：一段同步直线语句 + 末尾 await 点（或跳转/收尾）。
#[derive(Debug, Clone)]
pub struct Segment {
    /// 段主体语句（不含 await；控制流入口段内含 state 跳转语句）
    pub stmts: Vec<AstStmt>,
    /// 本段末尾的 await
    pub await_info: Option<AwaitInfo>,
    /// 尾表达式（收尾段）
    pub final_expr: Option<AstExpr>,
    /// 段尾/Ready 跳转目标段编号（`FN_END` = 函数收尾，展开完成后回填）
    pub next: Option<usize>,
    /// stmts 内含 state 跳转（控制流入口段，不追加段尾赋值）
    pub inline_jump: bool,
}

impl Segment {
    /// 是否隐式收尾段（无语句、无 await、无尾表达式：`Ready(0)`）。
    pub fn is_tail(&self) -> bool {
        self.await_info.is_none()
            && self.final_expr.is_none()
            && !self.inline_jump
            && self.next.is_none()
    }
}

/// await 在语句中的形态。
#[derive(Debug, Clone)]
pub enum AwaitInfo {
    /// `let v = e.await;`
    LetBind {
        /// await 结果绑定变量（提升为字段）
        var: String,
        /// await 目标
        target: AwaitTarget,
        /// 子 future 槽名（async fn 调用）；`IdentVar` 无新槽
        slot: Option<String>,
    },
    /// `e.await;`
    ExprStmt {
        /// await 目标
        target: AwaitTarget,
        /// 子 future 槽名
        slot: Option<String>,
    },
    /// `return e.await;`
    ReturnVal {
        /// await 目标
        target: AwaitTarget,
        /// 子 future 槽名
        slot: Option<String>,
    },
    /// 块尾表达式 `e.await`
    TailValue {
        /// await 目标
        target: AwaitTarget,
        /// 子 future 槽名
        slot: Option<String>,
    },
}

/// await 目标（S1c MVP 限两种）。
#[derive(Debug, Clone)]
pub enum AwaitTarget {
    /// `f(args)`：同一编译单元内 async fn 直接调用（子 future 存新槽）
    AsyncFnCall {
        /// 被调 async fn 名
        callee: String,
        /// 实参
        args: Vec<AstExpr>,
    },
    /// `x`：带类型注解的 let 变量（子 future 源，变量本身提升）
    IdentVar {
        /// 变量名
        var: String,
    },
}

impl AwaitInfo {
    /// 提取目标（供统一轮询代码生成）
    pub fn target(&self) -> &AwaitTarget {
        match self {
            AwaitInfo::LetBind { target, .. }
            | AwaitInfo::ExprStmt { target, .. }
            | AwaitInfo::ReturnVal { target, .. }
            | AwaitInfo::TailValue { target, .. } => target,
        }
    }

    /// 提取槽名
    pub fn slot(&self) -> Option<&str> {
        match self {
            AwaitInfo::LetBind { slot, .. }
            | AwaitInfo::ExprStmt { slot, .. }
            | AwaitInfo::ReturnVal { slot, .. }
            | AwaitInfo::TailValue { slot, .. } => slot.as_deref(),
        }
    }
}

/// 单个 async fn 的完整状态机规划（供生成阶段使用）。
#[derive(Debug, Clone)]
pub struct AnalyzedAsync {
    /// 原 async fn 声明（构造器基座：名/参数/span）
    pub decl: AstFnDecl,
    /// 返回类型是否为 `()`（`i64` 则 `false`）
    pub body_ret_unit: bool,
    /// 返回类型 AstType（None = `()`；Some = i64 或泛型参数 `T`，供 `type Output`）
    pub ret_ty: Option<AstType>,
    /// 状态段（扁平列表，段尾显式跳转）
    pub segments: Vec<Segment>,
    /// 提升的参数（全提升，类型 i64）
    pub lifted_params: Vec<AstParam>,
    /// 跨段变量（段内初始化为 `self.x = init`）
    pub lifted_cross: Vec<(String, AstType, AstExpr)>,
    /// 内部提升字段（迭代游标等，类型 i64，零值 0）
    pub lifted_internal: Vec<(String, AstType)>,
    /// await 结果变量（i64，Ready 分支赋值）
    pub lifted_await_results: Vec<String>,
    /// 子 future 源变量（初始化表达式搬到构造函数）
    pub lifted_future_sources: Vec<(String, AstType, AstExpr)>,
    /// 子 future 槽：槽名 → 目标状态机类型名
    pub slots: Vec<(String, String)>,
    /// 依赖的 async fn 名（去重，拓扑排序用）
    pub deps: Vec<String>,
    /// 全部提升名（引用重写用）
    pub lifted: HashSet<String>,
}

/// 普通 let 变量的定义信息。
struct LetInfo {
    /// 定义段序号
    def_seg: usize,
    /// 类型注解（可选）
    type_anno: Option<SpannedAstType>,
    /// 初始化表达式
    init: AstExpr,
}

/// 分析上下文（跨切段累积）。
struct Ctx<'a> {
    async_names: &'a HashSet<String>,
    /// W6：泛型 async fn 名集合（作为子 future await 暂不支持，独立 block_on 支持）。
    generic_async: &'a HashSet<String>,
    slots: Vec<(String, String)>,
    await_results: Vec<String>,
    future_sources: Vec<(String, AstType, AstExpr)>,
    let_defs: HashMap<String, LetInfo>,
    seg_uses: Vec<HashSet<String>>,
    cross: Vec<(String, AstType, AstExpr)>,
    internal: Vec<(String, AstType)>,
    lifted: HashSet<String>,
    deps: Vec<String>,
    temp_seq: usize,
    span: Span,
}

// ============ AST 构造助手 ============

fn int_expr(v: i128, span: Span) -> AstExpr {
    AstExpr::new(ExprKind::IntLiteral(v), span)
}

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

fn assign(target: AstExpr, value: AstExpr, span: Span) -> AstStmt {
    AstStmt::Semi(AstExpr::new(
        ExprKind::Assign {
            target,
            op: rlyeh_ast::AssignOp::Assign,
            value,
        },
        span,
    ))
}

/// 函数收尾哨兵（展开完成后回填为收尾段编号）。
///
/// state 跳转目标若为 `FN_END`（控制流汇合到函数收尾），展开时先写入哨兵值，
/// 待收尾段编号确定后统一回填为 `2 * end`。
pub(crate) const FN_END_TARGET: i128 = -1;

/// 控制流汇合占位哨兵（`CONT_PENDING` 段编号的 state 跳转值，回填为 `2 * ph`）。
const CONT_PENDING_TARGET: i128 = -2;

/// `self.state = 2 * target_seg;`（FN_END / CONT_PENDING 时写入哨兵值，展开后回填）。
fn state_assign(target_seg: usize, span: Span) -> AstStmt {
    let v = if target_seg == FN_END {
        FN_END_TARGET
    } else if target_seg == CONT_PENDING {
        CONT_PENDING_TARGET
    } else {
        (2 * target_seg) as i128
    };
    assign(self_field("state", span), int_expr(v, span), span)
}

/// `if cond { then_stmts } else { else_stmts }`（if 表达式，用作语句）。
fn mk_if(cond: AstExpr, then_stmts: Vec<AstStmt>, else_stmts: Vec<AstStmt>, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::If {
            cond,
            then_block: AstBlock {
                stmts: then_stmts,
                final_expr: None,
                span,
            },
            else_block: Some(AstBlock {
                stmts: else_stmts,
                final_expr: None,
                span,
            }),
        },
        span,
    )
}

/// 二元算术表达式（如 `lo + 1`）。
fn binop(op: BinaryOp, left: AstExpr, right: AstExpr, span: Span) -> AstExpr {
    AstExpr::new(ExprKind::Binary { op, left, right }, span)
}

/// 比较链 `a < b` / `a <= b`。
fn cmp(op: CompareOp, a: AstExpr, b: AstExpr, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::ComparisonChain {
            elements: vec![a, b],
            operators: vec![op],
        },
        span,
    )
}

/// 落段：当前累积直线语句 → 段（next = 紧邻下一段）。返回段编号（无内容返回 None）。

/// 把「当前段语句 + 尾表达式」合并为收尾段（无 next、final_expr 非空）。
///
/// 供函数尾表达式无 await 且当前段已积累语句时使用：避免把 `let r = ..; r`
/// 拆成「跳转段 + 独立收尾段」，使段内局部变量 `r` 在尾表达式可见（否则跨段
/// 不可见报 undefined，且需多余提升）。cur_uses 与尾表达式 uses 合并记入本段。

/// 无 uses 收集版落段（for 展开等）。

/// push 一个 await 段（next = 紧邻或 None；terminal = ReturnVal/TailValue 不跳转）。

// ============ 分析入口 ============

/// 分析一个 async fn，得到状态机规划。
pub fn analyze_async_fn(
    decl: &AstFnDecl,
    async_names: &HashSet<String>,
    generic_async: &HashSet<String>,
) -> Result<AnalyzedAsync, DesugarError> {
    let span = decl.span;
    // W6：泛型参数名集合（`async fn foo<T>(...)` 中 `T` 参与参数/返回类型，
    // 透传到 Future 结构体/impl/构造器，由 typecheck 单态化解析）。
    let gen_names: HashSet<String> = decl.generics.iter().map(|g| g.name.clone()).collect();
    if decl.is_extern {
        return Err(DesugarError::Unsupported {
            what: format!("extern async fn `{}` 不支持（MVP）", decl.name),
            span,
        });
    }

    // 参数：全部提升，类型限 i64 或泛型参数
    let mut lifted_params = Vec::new();
    for p in &decl.params {
        if !is_i64_ty(&p.type_) && !is_generic_ty(&p.type_, &gen_names) {
            return Err(DesugarError::Unsupported {
                what: format!(
                    "async fn `{}` 参数 `{}` 限 i64 或泛型参数（MVP）",
                    decl.name, p.name
                ),
                span: p.span,
            });
        }
        if p.default.is_some() {
            return Err(DesugarError::Unsupported {
                what: format!("async fn `{}` 参数 `{}` 不支持默认值（MVP）", decl.name, p.name),
                span: p.span,
            });
        }
        lifted_params.push(p.clone());
    }

    // 返回类型：i64 / 泛型参数 / ()；`ret_ty` 供 `type Output`（None = ()）
    let (body_ret_unit, ret_ty) = match &decl.return_type {
        None => (true, None),
        Some(AstType::Path(n, _)) if n == "()" => (true, None),
        Some(AstType::Path(n, args)) if n == "i64" && args.is_empty() => {
            (false, Some(AstType::Path("i64".to_string(), Vec::new())))
        }
        Some(AstType::Path(n, args)) if args.is_empty() && gen_names.contains(n) => {
            (false, Some(AstType::Path(n.clone(), Vec::new())))
        }
        _ => {
            return Err(DesugarError::Unsupported {
                what: format!("async fn `{}` 返回类型限 i64、泛型参数或 ()（MVP）", decl.name),
                span,
            })
        }
    };

    let body = decl.body.as_ref().ok_or_else(|| DesugarError::Unsupported {
        what: format!("async fn `{}` 缺少函数体", decl.name),
        span,
    })?;

    let mut ctx = Ctx {
        async_names,
        generic_async,
        slots: Vec::new(),
        await_results: Vec::new(),
        future_sources: Vec::new(),
        let_defs: HashMap::new(),
        seg_uses: Vec::new(),
        cross: Vec::new(),
        internal: Vec::new(),
        lifted: HashSet::new(),
        deps: Vec::new(),
        temp_seq: 0,
        span,
    };
    for p in &lifted_params {
        ctx.lifted.insert(p.name.clone());
    }

    let segments = expand_fn_body(body, &mut ctx)?;

    // 子 future 源变量：校验定义 + 类型注解；初始化搬到构造函数
    for seg in &segments {
        if let Some(ai) = &seg.await_info {
            if let AwaitTarget::IdentVar { var } = ai.target() {
                if ctx.lifted.contains(var) {
                    continue; // 已提升（避免重复）
                }
                let info = ctx.let_defs.get(var).ok_or_else(|| {
                    DesugarError::Unsupported {
                        what: format!("await 目标变量 `{var}` 未定义"),
                        span,
                    }
                })?;
                let ty = info
                    .type_anno
                    .clone()
                    .map(|s| s.ty)
                    .ok_or_else(|| {
                    DesugarError::Unsupported {
                        what: format!("await 目标变量 `{var}` 须带类型注解（MVP）"),
                        span,
                    }
                })?;
                // 初始化表达式仅可引用参数（搬入构造函数）
                let mut init_uses = HashSet::new();
                let _ = scan_expr(&info.init, &mut init_uses);
                let params: HashSet<&str> = lifted_params.iter().map(|p| p.name.as_str()).collect();
                for u in &init_uses {
                    if !params.contains(u.as_str()) {
                        return Err(DesugarError::Unsupported {
                            what: format!(
                                "await 目标变量 `{var}` 的初始化表达式仅可引用参数（MVP），引用了 `{u}`"
                            ),
                            span,
                        });
                    }
                }
                ctx.lifted.insert(var.clone());
                ctx.future_sources.push((var.clone(), ty, info.init.clone()));
            }
        }
    }

    // 跨段变量：定义段 < 使用段 → 提升（限可提升类型）
    for (var, info) in &ctx.let_defs {
        if ctx.lifted.contains(var) {
            continue;
        }
        let mut lift = false;
        for (i, uses) in ctx.seg_uses.iter().enumerate() {
            if i > info.def_seg && uses.contains(var) {
                lift = true;
                break;
            }
        }
        if lift {
            let ty = info
                .type_anno
                .clone()
                .map(|s| s.ty)
                .or_else(|| infer_ast_type(&info.init))
                .ok_or_else(|| DesugarError::Unsupported {
                    what: format!("跨 await 变量 `{var}` 类型无法确定，请添加类型注解（MVP）"),
                    span,
                })?;
            if !is_liftable_ty(&ty) {
                return Err(DesugarError::Unsupported {
                    what: format!(
                        "跨 await 变量 `{var}` 限 i64/f64/bool/char/String（W2 MVP，引用类型规划中）"
                    ),
                    span,
                });
            }
            ctx.lifted.insert(var.clone());
            ctx.cross.push((var.clone(), ty, info.init.clone()));
        }
    }

    Ok(AnalyzedAsync {
        decl: decl.clone(),
        body_ret_unit,
        ret_ty,
        segments,
        lifted_params,
        lifted_cross: ctx.cross,
        lifted_internal: ctx.internal,
        lifted_await_results: ctx.await_results,
        lifted_future_sources: ctx.future_sources,
        slots: ctx.slots,
        deps: ctx.deps,
        lifted: ctx.lifted,
    })
}

// ============ 控制流图展开 ============

/// 展开函数体为扁平段列表（末段为收尾段）。
fn expand_fn_body(body: &AstBlock, ctx: &mut Ctx) -> Result<Vec<Segment>, DesugarError> {
    let mut out = Vec::new();
    let (first, _) = expand_block(ctx, &mut out, body, FN_END)?;
    let _ = first;
    // 回填 FN_END → 收尾段编号（末段）
    let end = out.len() - 1;
    for seg in &mut out {
        if seg.next == Some(FN_END) {
            seg.next = Some(end);
        }
    }
    // 回填 stmts 内联的 state 跳转哨兵 `self.state = -1` → `self.state = 2 * end`
    for seg in &mut out {
        for stmt in &mut seg.stmts {
            replace_state_sentinel_stmt(stmt, end);
        }
    }
    Ok(out)
}

/// 递归将语句内 `self.state = FN_END_TARGET` 赋值替换为 `self.state = 2 * end`。
fn replace_state_sentinel_stmt(stmt: &mut AstStmt, end: usize) {
    match stmt {
        AstStmt::Semi(e) | AstStmt::Expr(e) => replace_state_sentinel_expr(e, end),
        AstStmt::Let { init, .. } => replace_state_sentinel_expr(init, end),
        AstStmt::Item(_) => {}
    }
}

/// 递归将表达式内 state 赋值哨兵替换为真实跳转值。
fn replace_state_sentinel_expr(e: &mut AstExpr, end: usize) {
    // 直接匹配 `self.state = -1`
    if let ExprKind::Assign { target, value, .. } = &mut *e.kind {
        if let ExprKind::FieldAccess { expr, field } = &*target.kind {
            if field == "state"
                && matches!(&*expr.kind, ExprKind::Ident(name) if name == "self")
                && matches!(&*value.kind, ExprKind::IntLiteral(v) if *v == FN_END_TARGET)
            {
                *value.kind = ExprKind::IntLiteral((2 * end) as i128);
                return;
            }
        }
    }
    // 递归遍历子表达式
    let children: Vec<&mut AstExpr> = expr_children_mut(e);
    for child in children {
        replace_state_sentinel_expr(child, end);
    }
}

/// 收集表达式的可变子表达式（仅控制流/块/赋值等含嵌套表达式结构的节点）。
fn expr_children_mut(e: &mut AstExpr) -> Vec<&mut AstExpr> {
    match &mut *e.kind {
        ExprKind::Assign { value, .. } => vec![value],
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            let mut v = vec![cond];
            v.extend(then_block.stmts.iter_mut().filter_map(as_expr_stmt_mut));
            if let Some(eb) = else_block {
                v.extend(eb.stmts.iter_mut().filter_map(as_expr_stmt_mut));
            }
            v
        }
        ExprKind::Match { expr, arms } => {
            let mut v = vec![expr];
            for arm in arms {
                v.push(&mut arm.body);
            }
            v
        }
        ExprKind::Block(block) | ExprKind::UnsafeBlock(block) | ExprKind::TryBlock(block) => {
            let mut v = Vec::new();
            for s in &mut block.stmts {
                if let AstStmt::Semi(e) | AstStmt::Expr(e) = s {
                    v.push(e);
                }
            }
            v
        }
        _ => Vec::new(),
    }
}

/// 从语句借出表达式（Semi/Expr），供遍历。
fn as_expr_stmt_mut(s: &mut AstStmt) -> Option<&mut AstExpr> {
    match s {
        AstStmt::Semi(e) | AstStmt::Expr(e) => Some(e),
        _ => None,
    }
}

/// 展开一个块。`cont` = 块汇合后的后继段（`FN_END` = 函数收尾）。
///
/// 返回 `(首段编号, 末段编号)`（空块为 `(None, None)`）。
#[allow(clippy::too_many_arguments)]
fn expand_block(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    block: &AstBlock,
    cont: usize,
) -> Result<(Option<usize>, Option<usize>), DesugarError> {
    let mut cur_stmts: Vec<AstStmt> = Vec::new();
    let mut cur_uses: HashSet<String> = HashSet::new();
    let mut first: Option<usize> = None;
    let mut last: Option<usize> = None;

    for stmt in &block.stmts {
        match stmt {
            // —— `let v = e.await;` ——
            AstStmt::Let {
                pattern: AstPattern::Ident(x),
                init,
                ..
            } if matches!(&*init.kind, ExprKind::Await(_)) => {
                flush(ctx, out, &mut cur_stmts, &mut cur_uses, &mut first, &mut last);
                let inner = match &*init.kind {
                    ExprKind::Await(i) => (**i).clone(),
                    _ => unreachable!(),
                };
                let (target, slot) = resolve_target(&inner, ctx)?;
                check_no_nested_await(&inner, init.span)?;
                scan_expr(&inner, &mut cur_uses)?;
                ctx.await_results.push(x.clone());
                ctx.lifted.insert(x.clone());
                push_await(
                    ctx,
                    out,
                    &mut cur_uses,
                    AwaitInfo::LetBind {
                        var: x.clone(),
                        target,
                        slot,
                    },
                    false,
                    &mut first,
                    &mut last,
                );
            }

            // —— `let v = <含 await 的复合表达式>;` ——
            AstStmt::Let {
                pattern: AstPattern::Ident(x),
                type_anno,
                init,
                mutable,
            } => {
                if contains_await_expr(init) {
                    return Err(DesugarError::Unsupported {
                        what: format!(
                            "W2 MVP：`let` 初始化表达式中仅支持顶层 `e.await` 形态的嵌套 await，\
                             复合表达式（控制流/调用/运算内）请先拆分（`{x}` 处）"
                        ),
                        span: init.span,
                    });
                }
                ctx.let_defs.insert(
                    x.clone(),
                    LetInfo {
                        def_seg: out.len(),
                        type_anno: type_anno.clone(),
                        init: init.clone(),
                    },
                );
                scan_expr(init, &mut cur_uses)?;
                cur_stmts.push(AstStmt::Let {
                    pattern: AstPattern::Ident(x.clone()),
                    type_anno: type_anno.clone(),
                    init: init.clone(),
                    mutable: *mutable,
                });
            }

            // —— 其它 let 模式 ——
            AstStmt::Let { init, .. } => {
                if contains_await_expr(init) {
                    return Err(DesugarError::Unsupported {
                        what: "W2 MVP：await 语句限 `let v = e.await;` 简单绑定模式（其它模式暂不支持）"
                            .to_string(),
                        span: init.span,
                    });
                }
                scan_expr(init, &mut cur_uses)?;
                cur_stmts.push(stmt.clone());
            }

            // —— 表达式语句 / return / 控制流 ——
            AstStmt::Semi(e) | AstStmt::Expr(e) => {
                // `e.await;`
                if let ExprKind::Await(inner) = &*e.kind {
                    flush(ctx, out, &mut cur_stmts, &mut cur_uses, &mut first, &mut last);
                    let inner2 = (**inner).clone();
                    let (target, slot) = resolve_target(&inner2, ctx)?;
                    check_no_nested_await(&inner2, e.span)?;
                    scan_expr(&inner2, &mut cur_uses)?;
                    push_await(
                        ctx,
                        out,
                        &mut cur_uses,
                        AwaitInfo::ExprStmt { target, slot },
                        false,
                        &mut first,
                        &mut last,
                    );
                    continue;
                }
                // `return e.await;`
                if let ExprKind::Return(Some(inner)) = &*e.kind {
                    if let ExprKind::Await(inner_aw) = &*inner.kind {
                        flush(ctx, out, &mut cur_stmts, &mut cur_uses, &mut first, &mut last);
                        let inner2 = (**inner_aw).clone();
                        let (target, slot) = resolve_target(&inner2, ctx)?;
                        check_no_nested_await(&inner2, e.span)?;
                        scan_expr(&inner2, &mut cur_uses)?;
                        push_await(
                            ctx,
                            out,
                            &mut cur_uses,
                            AwaitInfo::ReturnVal { target, slot },
                            true,
                            &mut first,
                            &mut last,
                        );
                        continue;
                    }
                }
                // break / continue：展开路径中不支持（含 await 控制流内）
                if matches!(&*e.kind, ExprKind::Break(_) | ExprKind::Continue) {
                    return Err(DesugarError::Unsupported {
                        what: "W2 MVP：含 await 的控制流块内 break/continue 暂不支持（规划中）"
                            .to_string(),
                        span: e.span,
                    });
                }
                // 控制流表达式（含 await）→ 展开
                if is_control_flow_expr(e) {
                    if contains_await_expr(e) {
                        flush(ctx, out, &mut cur_stmts, &mut cur_uses, &mut first, &mut last);
                        expand_control_stmt(ctx, out, e, cont, &mut first, &mut last)?;
                        continue;
                    }
                    scan_expr(e, &mut cur_uses)?;
                    cur_stmts.push(stmt.clone());
                    continue;
                }
                // 普通表达式语句：嵌套 await → 提取临时变量
                if contains_await_expr(e) {
                    let e2 = extract_expr_awaits(ctx, out, e, &mut cur_uses, &mut first, &mut last)?;
                    // 提取后重新收集变量使用（extract 内部只收集 await 目标，
                    // 外层表达式的 target/其余操作数需在此补充，避免跨段变量漏提升）
                    scan_expr(&e2, &mut cur_uses)?;
                    cur_stmts.push(AstStmt::Semi(e2));
                    continue;
                }
                scan_expr(e, &mut cur_uses)?;
                cur_stmts.push(stmt.clone());
            }

            // —— 其它 ——
            other => {
                return Err(DesugarError::Unsupported {
                    what: format!("async fn 体内不支持嵌套项声明（MVP）：{other:?}"),
                    span: ctx.span,
                })
            }
        }
    }

    // 尾表达式
    match &block.final_expr {
        Some(e) => {
            if let ExprKind::Await(inner) = &*e.kind {
                if cont != FN_END {
                    return Err(DesugarError::Unsupported {
                        what: "W2 MVP：控制流块尾表达式含 await 暂不支持（await 须为语句形态）"
                            .to_string(),
                        span: e.span,
                    });
                }
                flush(ctx, out, &mut cur_stmts, &mut cur_uses, &mut first, &mut last);
                let inner2 = (**inner).clone();
                let (target, slot) = resolve_target(&inner2, ctx)?;
                check_no_nested_await(&inner2, e.span)?;
                scan_expr(&inner2, &mut cur_uses)?;
                push_await(
                    ctx,
                    out,
                    &mut cur_uses,
                    AwaitInfo::TailValue { target, slot },
                    true,
                    &mut first,
                    &mut last,
                );
            } else if contains_await_expr(e) {
                return Err(DesugarError::Unsupported {
                    what: "W2 MVP：函数尾表达式中嵌套 await（非顶层 `e.await` 形态）暂不支持，请先拆分"
                        .to_string(),
                    span: e.span,
                });
            } else {
                if cont != FN_END {
                    // 子块尾值（无 await）：作为表达式语句并入末段（值被丢弃）
                    cur_stmts.push(AstStmt::Expr(e.clone()));
                    flush(ctx, out, &mut cur_stmts, &mut cur_uses, &mut first, &mut last);
                    let idx = out.len();
                    // 尾表达式段：扫描 `e` 使用的变量记入本段 seg_uses，使未提升局部
                    // 变量（如 `let r = ..; r`）在后续段的跨段使用能被识别并提升，
                    // 避免跳转段定义的局部变量在收尾段不可见（undefined）。
                    let mut tail_uses = HashSet::new();
                    scan_expr(e, &mut tail_uses)?;
                    out.push(Segment {
                        stmts: Vec::new(),
                        await_info: None,
                        final_expr: Some(e.clone()),
                        next: None,
                        inline_jump: false,
                    });
                    ctx.seg_uses.push(tail_uses);
                    if first.is_none() {
                        first = Some(idx);
                    }
                    last = Some(idx);
                } else if !cur_stmts.is_empty() {
                    // 函数尾表达式 + 当前段已积累语句：合并为收尾段
                    // （`let r = ..; r` 同段，局部变量 `r` 尾表达式可见）
                    flush_tail(ctx, out, &mut cur_stmts, &mut cur_uses, e.clone(), &mut first, &mut last);
                } else {
                    flush(ctx, out, &mut cur_stmts, &mut cur_uses, &mut first, &mut last);
                    let idx = out.len();
                    let mut tail_uses = HashSet::new();
                    scan_expr(e, &mut tail_uses)?;
                    out.push(Segment {
                        stmts: Vec::new(),
                        await_info: None,
                        final_expr: Some(e.clone()),
                        next: None,
                        inline_jump: false,
                    });
                    ctx.seg_uses.push(tail_uses);
                    if first.is_none() {
                        first = Some(idx);
                    }
                    last = Some(idx);
                }
            }
        }
        None => {
            if cont == FN_END {
                flush(ctx, out, &mut cur_stmts, &mut cur_uses, &mut first, &mut last);
                let idx = out.len();
                out.push(Segment {
                    stmts: Vec::new(),
                    await_info: None,
                    final_expr: None,
                    next: None,
                    inline_jump: false,
                });
                ctx.seg_uses.push(HashSet::new());
                if first.is_none() {
                    first = Some(idx);
                }
                last = Some(idx);
            } else {
                flush(ctx, out, &mut cur_stmts, &mut cur_uses, &mut first, &mut last);
            }
        }
    }

    // 块末段回填：可继续段（非终止）→ 跳 cont
    if let Some(ls) = last {
        let terminal = match &out[ls].await_info {
            Some(AwaitInfo::ReturnVal { .. }) | Some(AwaitInfo::TailValue { .. }) => true,
            Some(_) => false,
            None => out[ls].final_expr.is_some() || out[ls].is_tail() || out[ls].inline_jump,
        };
        if !terminal {
            out[ls].next = Some(cont);
        }
    }

    Ok((first, last))
}

// ============ 控制流展开 ============

/// 控制流表达式（语句形态展开候选）。

/// 控制流分支「汇合点」占位哨兵（展开期表示「控制流之后的紧邻段」，
/// 展开完成后回填为真实编号）。
const CONT_PENDING: usize = usize::MAX - 1;

/// 展开一条含 await 的控制流语句（块内位置）。
///
/// 控制流分支以 `CONT_PENDING` 占位，展开完成、控制流段就位后，在控制流
/// 段末尾追加一「汇合占位段」`ph`（真实编号，恒在控制流段之后、非初始段 0），
/// 并把所有 `CONT_PENDING` 跳转回填为 `ph`。`ph.next` 指向控制流之后的
/// 第一段（后续语句/尾表达式）；若控制流为块内最后一条，则由 expand_block
/// 末尾回填 `ph.next = cont`。这样 `if x { await } else { await }` 之后的
/// 语句才能被正确顺序执行，且初始 `state=0` 不会被 `ph` 占用。
#[allow(clippy::too_many_arguments)]

/// 递归将语句内 `self.state = CONT_PENDING` 赋值替换为 `self.state = 2 * ph`。

/// 递归将表达式内 state 赋值哨兵替换为真实跳转值。

/// 展开控制流语句（If/Match/For/While/Loop；Region 等含 await 报错）。
#[allow(clippy::too_many_arguments)]

/// 展开 `if cond { then } else { else }`（`else if` 链由 parser 解析为 else 块内 If 语句，
/// 经 expand_block 自然递归展开）。
#[allow(clippy::too_many_arguments)]

/// 展开 `loop { body }`。

/// 展开 `while cond { body }`。

// ============ for / match 展开 ============

/// 展开 `for pattern in iterator { body }`（W2 MVP：仅数值区间 Range）。
#[allow(clippy::too_many_arguments)]

/// 展开 `match expr { arms }`。

/// 模式是否含绑定变量（Ident/Wildcard 绑定；容器模式递归）。

// ============ await 目标解析 ============

/// 解析 await 目标：async fn 调用或带注解 let 变量。

// ============ 变量使用扫描 / await 检测 ============

/// 表达式是否含 await（递归）。

/// 检查表达式内无嵌套 await。

/// 递归收集表达式中的标识符使用；遇 Await 返回 Err（用于 await 检测）。

/// 递归收集块内表达式使用。

/// 语句使用扫描。

// ============ 类型判断 ============

/// 类型是否为 i64。

/// W6：类型是否为泛型参数名（`T` 在 `async fn foo<T>(...)` 中）。

/// W2：跨 await 可提升类型。

/// 从初始化表达式推断类型（字面量级）。

// ============ 嵌套 await 提取 ============

/// 递归提取表达式中的嵌套 await 为独立段（临时变量 `__w_N`），返回替换后的表达式。


mod expand;
mod scan;
mod segment;

use expand::*;
use scan::*;
use segment::*;
