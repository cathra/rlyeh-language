//! async fn 分析：切段（按 await 点）、提升变量、子 future 槽、依赖收集。

use std::collections::{HashMap, HashSet};

use zeta_ast::{AstBlock, AstExpr, AstFnDecl, AstParam, AstPattern, AstStmt, AstType, ExprKind};
use zeta_lexer::Span;

use crate::DesugarError;

/// 状态机段：一段直线语句 + 末尾的 await 点（最后一段无 await）。
#[derive(Debug, Clone)]
pub struct Segment {
    /// 段主体语句（不含 await）
    pub stmts: Vec<AstStmt>,
    /// 本段末尾的 await（最后一段为 `None`）
    pub await_info: Option<AwaitInfo>,
    /// 尾表达式（仅最后一段，无 await 时）
    pub final_expr: Option<AstExpr>,
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
    /// 状态段
    pub segments: Vec<Segment>,
    /// 提升的参数（全提升，类型 i64）
    pub lifted_params: Vec<AstParam>,
    /// 跨段 i64 变量（段内初始化为 `self.x = init`）
    pub lifted_cross: Vec<(String, AstType, AstExpr)>,
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
    type_anno: Option<AstType>,
    /// 初始化表达式
    init: AstExpr,
}

/// 分析上下文（跨切段累积）。
struct Ctx<'a> {
    async_names: &'a HashSet<String>,
    slots: Vec<(String, String)>,
    await_results: Vec<String>,
    future_sources: Vec<(String, AstType, AstExpr)>,
    let_defs: HashMap<String, LetInfo>,
    seg_uses: Vec<HashSet<String>>,
    cross: Vec<(String, AstType, AstExpr)>,
    lifted: HashSet<String>,
    deps: Vec<String>,
    span: Span,
}

/// 分析一个 async fn，得到状态机规划。
pub fn analyze_async_fn(
    decl: &AstFnDecl,
    async_names: &HashSet<String>,
) -> Result<AnalyzedAsync, DesugarError> {
    let span = decl.span;
    if !decl.generics.is_empty() {
        return Err(DesugarError::Unsupported {
            what: format!("async fn `{}` 不支持泛型（MVP）", decl.name),
            span,
        });
    }
    if decl.is_extern {
        return Err(DesugarError::Unsupported {
            what: format!("extern async fn `{}` 不支持（MVP）", decl.name),
            span,
        });
    }

    // 参数：全部提升，类型限 i64
    let mut lifted_params = Vec::new();
    for p in &decl.params {
        if !is_i64_ty(&p.type_) {
            return Err(DesugarError::Unsupported {
                what: format!("async fn `{}` 参数 `{}` 限 i64（MVP）", decl.name, p.name),
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

    // 返回类型：i64 或 ()
    let body_ret_unit = match &decl.return_type {
        None => true,
        Some(AstType::Path(n, _)) if n == "()" => true,
        Some(AstType::Path(n, args)) if n == "i64" && args.is_empty() => false,
        _ => {
            return Err(DesugarError::Unsupported {
                what: format!("async fn `{}` 返回类型限 i64 或 ()（MVP）", decl.name),
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
        slots: Vec::new(),
        await_results: Vec::new(),
        future_sources: Vec::new(),
        let_defs: HashMap::new(),
        seg_uses: Vec::new(),
        cross: Vec::new(),
        lifted: HashSet::new(),
        deps: Vec::new(),
        span,
    };
    for p in &lifted_params {
        ctx.lifted.insert(p.name.clone());
    }

    let segments = split_segments(body, &mut ctx)?;

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
                let ty = info.type_anno.clone().ok_or_else(|| {
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

    // 跨段变量：定义段 < 使用段 → 提升（限 i64）
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
            let ty = info.type_anno.clone().or_else(|| infer_ast_type(&info.init)).ok_or_else(|| {
                DesugarError::Unsupported {
                    what: format!("跨 await 变量 `{var}` 类型无法确定，请添加类型注解（MVP）"),
                    span,
                }
            })?;
            if !is_i64_ty(&ty) {
                return Err(DesugarError::Unsupported {
                    what: format!("跨 await 变量 `{var}` 限 i64（MVP）"),
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
        segments,
        lifted_params,
        lifted_cross: ctx.cross,
        lifted_await_results: ctx.await_results,
        lifted_future_sources: ctx.future_sources,
        slots: ctx.slots,
        deps: ctx.deps,
        lifted: ctx.lifted,
    })
}

/// 按 await 点切分 body 为状态段，同时收集变量使用集与嵌套 await 校验。
fn split_segments(body: &AstBlock, ctx: &mut Ctx) -> Result<Vec<Segment>, DesugarError> {
    let mut segments = Vec::new();
    let mut cur_stmts: Vec<AstStmt> = Vec::new();
    let mut cur_uses: HashSet<String> = HashSet::new();
    let mut seg_idx = 0usize;

    for stmt in &body.stmts {
        match stmt {
            AstStmt::Let {
                pattern: AstPattern::Ident(x),
                type_anno,
                init,
                mutable: _,
            } => {
                let span = init.span;
                if let ExprKind::Await(inner) = &*init.kind {
                    // let v = e.await;
                    let inner = (**inner).clone();
                    let (target, slot) = resolve_target(&inner, ctx)?;
                    scan_expr(&inner, &mut cur_uses).map_err(|_| DesugarError::Unsupported {
                        what: "await 表达式内禁止嵌套 await（MVP）".to_string(),
                        span,
                    })?;
                    ctx.await_results.push(x.clone());
                    ctx.lifted.insert(x.clone());
                    segments.push(Segment {
                        stmts: std::mem::take(&mut cur_stmts),
                        await_info: Some(AwaitInfo::LetBind {
                            var: x.clone(),
                            target,
                            slot,
                        }),
                        final_expr: None,
                    });
                    ctx.seg_uses.push(std::mem::take(&mut cur_uses));
                    seg_idx += 1;
                } else {
                    // 普通 let
                    ctx.let_defs.insert(
                        x.clone(),
                        LetInfo {
                            def_seg: seg_idx,
                            type_anno: type_anno.clone(),
                            init: init.clone(),
                        },
                    );
                    scan_expr(init, &mut cur_uses).map_err(|_| DesugarError::Unsupported {
                        what: "await 仅支持直线代码（let 初始化/语句/return/尾表达式位置），且控制流块内不支持 await（MVP）"
                            .to_string(),
                        span,
                    })?;
                    cur_stmts.push(stmt.clone());
                }
            }
            AstStmt::Semi(e) | AstStmt::Expr(e) => match &*e.kind {
                ExprKind::Await(inner) => {
                    // e.await;
                    let inner = (**inner).clone();
                    let (target, slot) = resolve_target(&inner, ctx)?;
                    scan_expr(&inner, &mut cur_uses).map_err(|_| DesugarError::Unsupported {
                        what: "await 表达式内禁止嵌套 await（MVP）".to_string(),
                        span: e.span,
                    })?;
                    segments.push(Segment {
                        stmts: std::mem::take(&mut cur_stmts),
                        await_info: Some(AwaitInfo::ExprStmt { target, slot }),
                        final_expr: None,
                    });
                    ctx.seg_uses.push(std::mem::take(&mut cur_uses));
                    seg_idx += 1;
                }
                ExprKind::Return(Some(inner)) if matches!(&*inner.kind, ExprKind::Await(_)) => {
                    // return e.await;
                    let inner = match &*inner.kind {
                        ExprKind::Await(i) => (**i).clone(),
                        _ => unreachable!(),
                    };
                    let (target, slot) = resolve_target(&inner, ctx)?;
                    scan_expr(&inner, &mut cur_uses).map_err(|_| DesugarError::Unsupported {
                        what: "await 表达式内禁止嵌套 await（MVP）".to_string(),
                        span: e.span,
                    })?;
                    segments.push(Segment {
                        stmts: std::mem::take(&mut cur_stmts),
                        await_info: Some(AwaitInfo::ReturnVal { target, slot }),
                        final_expr: None,
                    });
                    ctx.seg_uses.push(std::mem::take(&mut cur_uses));
                    seg_idx += 1;
                }
                _ => {
                    scan_expr(e, &mut cur_uses).map_err(|_| DesugarError::Unsupported {
                        what: "await 仅支持直线代码（let 初始化/语句/return/尾表达式位置），且控制流块内不支持 await（MVP）"
                            .to_string(),
                        span: e.span,
                    })?;
                    cur_stmts.push(stmt.clone());
                }
            },
            other => {
                return Err(DesugarError::Unsupported {
                    what: format!("async fn 体内不支持嵌套项声明（MVP）：{other:?}"),
                    span: ctx.span,
                })
            }
        }
    }

    match &body.final_expr {
        Some(e) if matches!(&*e.kind, ExprKind::Await(_)) => {
            let inner = match &*e.kind {
                ExprKind::Await(i) => (**i).clone(),
                _ => unreachable!(),
            };
            let (target, slot) = resolve_target(&inner, ctx)?;
            scan_expr(&inner, &mut cur_uses).map_err(|_| DesugarError::Unsupported {
                what: "await 表达式内禁止嵌套 await（MVP）".to_string(),
                span: e.span,
            })?;
            segments.push(Segment {
                stmts: std::mem::take(&mut cur_stmts),
                await_info: Some(AwaitInfo::TailValue { target, slot }),
                final_expr: None,
            });
            ctx.seg_uses.push(std::mem::take(&mut cur_uses));
            // 尾 await 之后不再有代码：补一个空收尾段（无 await，Ready(0) 不可达）
            segments.push(Segment {
                stmts: Vec::new(),
                await_info: None,
                final_expr: None,
            });
            ctx.seg_uses.push(HashSet::new());
        }
        Some(e) => {
            scan_expr(e, &mut cur_uses).map_err(|_| DesugarError::Unsupported {
                what: "await 仅支持直线代码（let 初始化/语句/return/尾表达式位置），且控制流块内不支持 await（MVP）"
                    .to_string(),
                span: e.span,
            })?;
            segments.push(Segment {
                stmts: std::mem::take(&mut cur_stmts),
                await_info: None,
                final_expr: Some(e.clone()),
            });
            ctx.seg_uses.push(std::mem::take(&mut cur_uses));
        }
        None => {
            segments.push(Segment {
                stmts: std::mem::take(&mut cur_stmts),
                await_info: None,
                final_expr: None,
            });
            ctx.seg_uses.push(std::mem::take(&mut cur_uses));
        }
    }

    Ok(segments)
}

/// 解析 await 目标：async fn 调用（分配槽）或变量（子 future 源）。
fn resolve_target(
    inner: &AstExpr,
    ctx: &mut Ctx,
) -> Result<(AwaitTarget, Option<String>), DesugarError> {
    match &*inner.kind {
        ExprKind::Call {
            callee,
            args,
            type_args,
        } => {
            if let ExprKind::Ident(name) = &*callee.kind {
                if ctx.async_names.contains(name) {
                    if !type_args.is_empty() {
                        return Err(DesugarError::Unsupported {
                            what: format!("await 目标 `{name}` 不支持泛型调用（MVP）"),
                            span: inner.span,
                        });
                    }
                    let slot = format!("__fut_{}", ctx.slots.len());
                    ctx.slots
                        .push((slot.clone(), format!("__Fut_{}", name)));
                    if !ctx.deps.contains(name) {
                        ctx.deps.push(name.clone());
                    }
                    return Ok((
                        AwaitTarget::AsyncFnCall {
                            callee: name.clone(),
                            args: args.clone(),
                        },
                        Some(slot),
                    ));
                }
            }
            Err(DesugarError::Unsupported {
                what: "await 目标须为 async fn 调用 `f(args)` 或带类型注解的变量 `x`（MVP）"
                    .to_string(),
                span: inner.span,
            })
        }
        ExprKind::Ident(var) => Ok((AwaitTarget::IdentVar { var: var.clone() }, None)),
        _ => Err(DesugarError::Unsupported {
            what: "await 目标须为 async fn 调用 `f(args)` 或带类型注解的变量 `x`（MVP）"
                .to_string(),
            span: inner.span,
        }),
    }
}

/// 扫描表达式：收集变量引用；发现嵌套/非法位置 await 返回 `Err`。
fn scan_expr(e: &AstExpr, uses: &mut HashSet<String>) -> Result<(), ()> {
    match &*e.kind {
        ExprKind::Ident(name) => {
            uses.insert(name.clone());
            Ok(())
        }
        ExprKind::IntLiteral(_)
        | ExprKind::FloatLiteral(_)
        | ExprKind::StringLiteral(_)
        | ExprKind::CharLiteral(_)
        | ExprKind::BoolLiteral(_)
        | ExprKind::TimeLiteral { .. }
        | ExprKind::Path(_) => Ok(()),
        ExprKind::Set(items) => items.iter().try_for_each(|i| scan_expr(i, uses)),
        ExprKind::Range { lower, upper, .. } => {
            scan_expr(lower, uses)?;
            scan_expr(upper, uses)
        }
        ExprKind::Binary { left, right, .. } => {
            scan_expr(left, uses)?;
            scan_expr(right, uses)
        }
        ExprKind::Unary { operand, .. } => scan_expr(operand, uses),
        ExprKind::ComparisonChain { elements, .. } => {
            elements.iter().try_for_each(|el| scan_expr(el, uses))
        }
        ExprKind::InSet { value, set, .. } => {
            scan_expr(value, uses)?;
            set.iter().try_for_each(|s| scan_expr(s, uses))
        }
        ExprKind::InRange { value, range, .. } => {
            scan_expr(value, uses)?;
            scan_expr(range, uses)
        }
        ExprKind::InRegion { expr, .. } => scan_expr(expr, uses),
        ExprKind::Assign { target, value, .. } => {
            scan_expr(target, uses)?;
            scan_expr(value, uses)
        }
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            scan_expr(cond, uses)?;
            scan_block(then_block, uses)?;
            if let Some(eb) = else_block {
                scan_block(eb, uses)?;
            }
            Ok(())
        }
        ExprKind::Match { expr, arms } => {
            scan_expr(expr, uses)?;
            for arm in arms {
                scan_expr(&arm.body, uses)?;
            }
            Ok(())
        }
        ExprKind::For {
            iterator, body, ..
        } => {
            scan_expr(iterator, uses)?;
            scan_block(body, uses)
        }
        ExprKind::While { cond, body } => {
            scan_expr(cond, uses)?;
            scan_block(body, uses)
        }
        ExprKind::Loop { body } => scan_block(body, uses),
        ExprKind::Region { body, .. } => scan_block(body, uses),
        ExprKind::GcRegion { body } => scan_block(body, uses),
        ExprKind::Transfer { expr, .. } => scan_expr(expr, uses),
        ExprKind::Call {
            callee, args, ..
        } => {
            // callee 为函数名/枚举路径，不收集；其余保守扫描
            match &*callee.kind {
                ExprKind::Ident(_) | ExprKind::Path(_) => {}
                _ => scan_expr(callee, uses)?,
            }
            args.iter().try_for_each(|a| scan_expr(a, uses))
        }
        ExprKind::MacroCall { args, .. } => args.iter().try_for_each(|a| scan_expr(a, uses)),
        ExprKind::MethodCall {
            receiver,
            args,
            ..
        } => {
            scan_expr(receiver, uses)?;
            args.iter().try_for_each(|a| scan_expr(a, uses))
        }
        ExprKind::FieldAccess { expr, .. } => scan_expr(expr, uses),
        ExprKind::StructCtor { fields, .. } => {
            fields.iter().try_for_each(|(_, v)| scan_expr(v, uses))
        }
        ExprKind::Index { expr, index } => {
            scan_expr(expr, uses)?;
            scan_expr(index, uses)
        }
        ExprKind::ArrayLit(items) => items.iter().try_for_each(|i| scan_expr(i, uses)),
        ExprKind::Closure { body, .. } => scan_expr(body, uses),
        ExprKind::Cast { expr, .. } => scan_expr(expr, uses),
        ExprKind::Await(_) => Err(()),
        ExprKind::Block(block) => scan_block(block, uses),
        ExprKind::Return(Some(inner)) => scan_expr(inner, uses),
        ExprKind::Return(None) => Ok(()),
        ExprKind::Question(inner) => scan_expr(inner, uses),
        ExprKind::Break(Some(v)) => scan_expr(v, uses),
        ExprKind::Break(None) | ExprKind::Continue => Ok(()),
        ExprKind::Send {
            actor, args, ..
        } => {
            scan_expr(actor, uses)?;
            args.iter().try_for_each(|a| scan_expr(a, uses))
        }
    }
}

/// 扫描块：语句 + 尾表达式。
fn scan_block(block: &AstBlock, uses: &mut HashSet<String>) -> Result<(), ()> {
    for stmt in &block.stmts {
        match stmt {
            AstStmt::Let { init, .. } => scan_expr(init, uses)?,
            AstStmt::Semi(e) | AstStmt::Expr(e) => scan_expr(e, uses)?,
            AstStmt::Item(_) => {
                return Err(());
            }
        }
    }
    if let Some(fe) = &block.final_expr {
        scan_expr(fe, uses)?;
    }
    Ok(())
}

/// 类型是否为 `i64`。
fn is_i64_ty(ty: &AstType) -> bool {
    matches!(ty, AstType::Path(n, args) if n == "i64" && args.is_empty())
}

/// 浅推断字面量类型（跨 await 变量无注解时）。
fn infer_ast_type(e: &AstExpr) -> Option<AstType> {
    match &*e.kind {
        ExprKind::IntLiteral(_) => Some(AstType::Path("i64".into(), Vec::new())),
        ExprKind::FloatLiteral(_) => Some(AstType::Path("f64".into(), Vec::new())),
        ExprKind::BoolLiteral(_) => Some(AstType::Path("bool".into(), Vec::new())),
        ExprKind::CharLiteral(_) => Some(AstType::Path("char".into(), Vec::new())),
        ExprKind::StringLiteral(_) => Some(AstType::Path("String".into(), Vec::new())),
        _ => None,
    }
}
