//! 状态机生成：struct/impl Future/构造函数 + 引用重写。

use std::collections::{HashMap, HashSet};

use rlyeh_ast::{
    AstBlock, AstExpr, AstFnDecl, AstImplBlock, AstItem, AstParam, AstPattern, AstStmt,
    AstStructDecl, AstStructField, AstType, CompareOp, ExprKind, MatchArm,
};
use rlyeh_lexer::Span;

use crate::analyze::{AnalyzedAsync, AwaitInfo, AwaitTarget, Segment};

/// 结构体字段规格（含构造器初值与零值）。
pub struct FieldSpec {
    /// 字段名
    pub name: String,
    /// 字段类型
    pub ty: AstType,
    /// 构造函数字段初值
    pub ctor_init: AstExpr,
    /// 零值（内层槽零值构造用）
    pub zero_init: AstExpr,
}

fn span_of(a: &AnalyzedAsync) -> Span {
    a.decl.span
}

fn i64_ty() -> AstType {
    AstType::Path("i64".to_string(), Vec::new())
}

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

fn path(names: &[&str], span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::Path(names.iter().map(|s| s.to_string()).collect()),
        span,
    )
}

fn poll_ready(arg: AstExpr, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::Call {
            callee: path(&["Poll", "Ready"], span),
            args: vec![arg],
            type_args: Vec::new(),
        },
        span,
    )
}

fn poll_pending(span: Span) -> AstExpr {
    path(&["Poll", "Pending"], span)
}

fn ret_expr(e: AstExpr, span: Span) -> AstExpr {
    AstExpr::new(ExprKind::Return(Some(e)), span)
}

fn state_eq(k: i64, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::ComparisonChain {
            elements: vec![self_field("state", span), int_expr(k as i128, span)],
            operators: vec![CompareOp::Eq],
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

fn poll_call(receiver: AstExpr, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::MethodCall {
            receiver,
            method: "poll".to_string(),
            args: Vec::new(),
        },
        span,
    )
}

/// 生成状态机结构体定义，并返回字段规格（供构造器/零值构造复用）。
pub fn gen_struct(
    a: &AnalyzedAsync,
    layout: &HashMap<String, Vec<FieldSpec>>,
) -> (AstItem, Vec<FieldSpec>) {
    let span = span_of(a);
    let spec = gen_fields(a, layout);
    let fields: Vec<AstStructField> = spec
        .iter()
        .map(|f| AstStructField {
            name: f.name.clone(),
            type_: f.ty.clone(),
            is_pub: false,
            span,
        })
        .collect();
    let item = AstItem::StructDecl(Box::new(AstStructDecl {
        name: fut_ty_name(&a.decl.name),
        generics: Vec::new(),
        fields,
        derive: Vec::new(),
        span,
    }));
    (item, spec)
}

/// 生成 `impl Future for __Fut_<f>`（poll 状态分派）。
pub fn gen_impl(a: &AnalyzedAsync) -> AstItem {
    let span = span_of(a);
    let poll_body = gen_poll_body(a);
    let poll_fn = AstFnDecl {
        name: "poll".to_string(),
        generics: Vec::new(),
        params: vec![AstParam {
            name: "self".to_string(),
            // `&mut self`：与 parser 对 `fn poll(&mut self)` 的解析保持一致
            // （Ref(Path("Self"), true)），typecheck 对 `Self` 路径按 impl 目标
            // 类型解析接收者，显式 `__Fut_X` 路径会导致接收者字段访问/类型
            // 解析不一致。
            type_: AstType::Ref(Box::new(AstType::Path("Self".to_string(), Vec::new())), true),
            default: None,
            is_mut: false,
            span,
        }],
        return_type: Some(AstType::Path(
            "Poll".to_string(),
            vec![i64_ty()],
        )),
        body: Some(poll_body),
        is_pub: false,
        is_async: false,
        is_extern: false,
        span,
    };
    AstItem::ImplBlock(Box::new(AstImplBlock {
        trait_name: Some("Future".to_string()),
        type_name: fut_ty_name(&a.decl.name),
        generics: Vec::new(),
        methods: vec![poll_fn],
        span,
    }))
}

/// 生成构造函数 `fn f(args) -> __Fut_<f>`。
pub fn gen_ctor(
    a: &AnalyzedAsync,
    layout: &HashMap<String, Vec<FieldSpec>>,
) -> AstItem {
    let span = span_of(a);
    let spec = gen_fields(a, layout);
    let fields: Vec<(String, AstExpr)> = spec
        .iter()
        .map(|f| (f.name.clone(), f.ctor_init.clone()))
        .collect();
    let ctor_expr = AstExpr::new(
        ExprKind::StructCtor {
            type_name: vec![fut_ty_name(&a.decl.name)],
            fields,
        },
        span,
    );
    AstItem::FnDecl(Box::new(AstFnDecl {
        name: a.decl.name.clone(),
        generics: Vec::new(),
        params: a.lifted_params.clone(),
        return_type: Some(AstType::Path(fut_ty_name(&a.decl.name), Vec::new())),
        body: Some(AstBlock {
            stmts: Vec::new(),
            final_expr: Some(ctor_expr),
            span,
        }),
        is_pub: a.decl.is_pub,
        is_async: false,
        is_extern: false,
        span,
    }))
}

fn fut_ty_name(fname: &str) -> String {
    format!("__Fut_{fname}")
}

/// 生成状态机结构体字段规格。
fn gen_fields(a: &AnalyzedAsync, layout: &HashMap<String, Vec<FieldSpec>>) -> Vec<FieldSpec> {
    let span = span_of(a);
    let mut spec = Vec::new();

    // state
    spec.push(FieldSpec {
        name: "state".to_string(),
        ty: i64_ty(),
        ctor_init: int_expr(0, span),
        zero_init: int_expr(0, span),
    });

    // 子 future 槽
    for (slot, ty_name) in &a.slots {
        let zero = zero_ctor(ty_name, layout, span);
        spec.push(FieldSpec {
            name: slot.clone(),
            ty: AstType::Path(ty_name.clone(), Vec::new()),
            ctor_init: zero.clone(),
            zero_init: zero,
        });
    }

    // 参数（构造器传参；零值为 0）
    for p in &a.lifted_params {
        spec.push(FieldSpec {
            name: p.name.clone(),
            ty: p.type_.clone(),
            ctor_init: ident(&p.name, span),
            zero_init: int_expr(0, span),
        });
    }

    // await 结果变量（i64）
    for v in &a.lifted_await_results {
        spec.push(FieldSpec {
            name: v.clone(),
            ty: i64_ty(),
            ctor_init: int_expr(0, span),
            zero_init: int_expr(0, span),
        });
    }

    // 跨段 i64 变量（段内初始化；构造器 0）
    for (v, ty, _) in &a.lifted_cross {
        spec.push(FieldSpec {
            name: v.clone(),
            ty: ty.clone(),
            ctor_init: int_expr(0, span),
            zero_init: int_expr(0, span),
        });
    }

    // 子 future 源变量（初始化搬到构造器；零值脱变量）
    for (v, ty, init) in &a.lifted_future_sources {
        spec.push(FieldSpec {
            name: v.clone(),
            ty: ty.clone(),
            ctor_init: init.clone(),
            zero_init: devar(init),
        });
    }

    spec
}

/// 零值构造：`__Fut_X { <字段零值> }`（字段零值来自目标布局的 `zero_init`）。
fn zero_ctor(type_name: &str, layout: &HashMap<String, Vec<FieldSpec>>, span: Span) -> AstExpr {
    // layout 键为 async fn 名（如 `g`），`type_name` 形如 `__Fut_g`。
    let key = type_name
        .strip_prefix("__Fut_")
        .unwrap_or(type_name)
        .to_string();
    match layout.get(&key).or_else(|| layout.get(type_name)) {
        Some(spec) => AstExpr::new(
            ExprKind::StructCtor {
                type_name: vec![type_name.to_string()],
                fields: spec.iter().map(|f| (f.name.clone(), f.zero_init.clone())).collect(),
            },
            span,
        ),
        None => {
            // 被依赖者未先生成（拓扑序异常）：退化为 state: 0 单字段
            AstExpr::new(
                ExprKind::StructCtor {
                    type_name: vec![type_name.to_string()],
                    fields: vec![("state".to_string(), int_expr(0, span))],
                },
                span,
            )
        }
    }
}

/// 生成 poll 函数体（状态 if 链 + 兜底 Ready(0)）。
fn gen_poll_body(a: &AnalyzedAsync) -> AstBlock {
    let span = span_of(a);
    let mut stmts = Vec::new();
    let n = a.segments.len();
    for (i, seg) in a.segments.iter().enumerate() {
        if i + 1 == n {
            // 收尾段：状态 2i，return Poll::Ready(尾值)
            stmts.push(state_if_last(a, seg, i as i64));
        } else {
            // await 段：状态 2i（首轮询）+ 状态 2i+1（恢复轮询）
            stmts.push(state_if_await(a, seg, i as i64));
            stmts.push(resume_if(a, seg, i as i64));
        }
    }
    AstBlock {
        stmts,
        final_expr: Some(poll_ready(int_expr(0, span), span)),
        span,
    }
}

/// 收尾段状态：段语句 + `return Poll::Ready(尾值 or 0)`。
fn state_if_last(a: &AnalyzedAsync, seg: &Segment, k: i64) -> AstStmt {
    let span = span_of(a);
    let fs = future_sources(a);
    let mut block_stmts: Vec<AstStmt> = seg
        .stmts
        .iter()
        .filter_map(|s| rewrite_stmt(s, &a.lifted, &fs, span))
        .collect();
    let tail = match &seg.final_expr {
        Some(e) => rewrite_expr(e, &a.lifted, span),
        None => int_expr(0, span),
    };
    block_stmts.push(AstStmt::Semi(ret_expr(poll_ready(tail, span), span)));
    // 与 parser 对 `if` 语句的解析一致：`AstStmt::Semi(ExprKind::If)`。
    // `Expr(If)` 会改变块尾值语义，导致后续语句/兜底 final_expr 行为异常。
    AstStmt::Semi(AstExpr::new(
        ExprKind::If {
            // 段 k 的状态约定为 2k（await 段 k=0 → state 0，其 Ready 推进到 k+2）；
            // 尾段（最后一个 segment）为 2k。
            cond: state_eq(2 * k, span),
            then_block: AstBlock {
                stmts: block_stmts,
                final_expr: None,
                span,
            },
            else_block: None,
        },
        span,
    ))
}

/// await 段状态（首次）：段语句 + 子 future 求值 + 首轮询。
fn state_if_await(a: &AnalyzedAsync, seg: &Segment, k: i64) -> AstStmt {
    let span = span_of(a);
    let ai = seg.await_info.as_ref().expect("await segment");
    let fs = future_sources(a);
    let mut block_stmts: Vec<AstStmt> = seg
        .stmts
        .iter()
        .filter_map(|s| rewrite_stmt(s, &a.lifted, &fs, span))
        .collect();

    // 子 future 求值（async fn 调用才需要：求值结果存入槽）
    let receiver = match ai.target() {
        AwaitTarget::AsyncFnCall { callee, args } => {
            let slot = ai.slot().expect("slot for async fn call");
            let call = AstExpr::new(
                ExprKind::Call {
                    callee: ident(callee, span),
                    args: args
                        .iter()
                        .map(|arg| rewrite_expr(arg, &a.lifted, span))
                        .collect(),
                    type_args: Vec::new(),
                },
                span,
            );
            block_stmts.push(assign(self_field(slot, span), call, span));
            self_field(slot, span)
        }
        AwaitTarget::IdentVar { var } => self_field(var, span),
    };

    // 首轮询
    block_stmts.push(poll_match(a, ai, receiver, k, false));

    AstStmt::Semi(AstExpr::new(
        ExprKind::If {
            // await 段 k 的首轮询状态为 2k
            cond: state_eq(2 * k, span),
            then_block: AstBlock {
                stmts: block_stmts,
                final_expr: None,
                span,
            },
            else_block: None,
        },
        span,
    ))
}

/// 恢复段状态：重新轮询子 future。
fn resume_if(a: &AnalyzedAsync, seg: &Segment, k: i64) -> AstStmt {
    let span = span_of(a);
    let ai = seg.await_info.as_ref().expect("await segment");
    let receiver = match ai.target() {
        AwaitTarget::AsyncFnCall { .. } => {
            self_field(ai.slot().expect("slot for async fn call"), span)
        }
        AwaitTarget::IdentVar { var } => self_field(var, span),
    };
    let block_stmts = vec![poll_match(a, ai, receiver, k, true)];
    AstStmt::Semi(AstExpr::new(
        ExprKind::If {
            // await 段 k 的恢复轮询状态为 2k+1
            cond: state_eq(2 * k + 1, span),
            then_block: AstBlock {
                stmts: block_stmts,
                final_expr: None,
                span,
            },
            else_block: None,
        },
        span,
    ))
}

/// 生成轮询 match（Ready/Pending 两臂）。
fn poll_match(
    a: &AnalyzedAsync,
    ai: &AwaitInfo,
    receiver: AstExpr,
    k: i64,
    resume: bool,
) -> AstStmt {
    let span = span_of(a);
    let arms = vec![
        MatchArm {
            pattern: AstPattern::EnumPath(
                vec!["Poll".to_string(), "Ready".to_string()],
                vec![AstPattern::Ident("__v".to_string())],
            ),
            guard: None,
            body: AstExpr::new(ExprKind::Block(ready_block(a, ai, k)), span),
            span,
        },
        MatchArm {
            pattern: AstPattern::EnumPath(
                vec!["Poll".to_string(), "Pending".to_string()],
                Vec::new(),
            ),
            guard: None,
            body: AstExpr::new(ExprKind::Block(pending_block(k, resume, span)), span),
            span,
        },
    ];
    AstStmt::Semi(AstExpr::new(
        ExprKind::Match {
            expr: poll_call(receiver, span),
            arms,
        },
        span,
    ))
}

/// Ready 分支处理块。
fn ready_block(a: &AnalyzedAsync, ai: &AwaitInfo, k: i64) -> AstBlock {
    let span = span_of(a);
    match ai {
        AwaitInfo::LetBind { var, .. } => AstBlock {
            stmts: vec![
                assign(self_field(var, span), ident("__v", span), span),
                assign(
                    self_field("state", span),
                    int_expr((2 * k + 2) as i128, span),
                    span,
                ),
            ],
            final_expr: None,
            span,
        },
        AwaitInfo::ExprStmt { .. } => AstBlock {
            stmts: vec![assign(
                self_field("state", span),
                int_expr((2 * k + 2) as i128, span),
                span,
            )],
            final_expr: None,
            span,
        },
        AwaitInfo::ReturnVal { .. } | AwaitInfo::TailValue { .. } => AstBlock {
            stmts: vec![AstStmt::Semi(ret_expr(
                poll_ready(ident("__v", span), span),
                span,
            ))],
            final_expr: None,
            span,
        },
    }
}

/// Pending 分支处理块。
fn pending_block(k: i64, resume: bool, span: Span) -> AstBlock {
    let mut stmts = Vec::new();
    if !resume {
        stmts.push(assign(
            self_field("state", span),
            int_expr((2 * k + 1) as i128, span),
            span,
        ));
    }
    stmts.push(AstStmt::Semi(ret_expr(poll_pending(span), span)));
    AstBlock {
        stmts,
        final_expr: None,
        span,
    }
}

/// 重写语句。返回 `None` 表示删除该语句（子 future 源 let——初始化已搬到构造器）。
///
/// - 跨段 let（lifted_cross）→ `self.x = init;`（字段赋值）；
/// - 子 future 源 let → 删除；
/// - 未提升 let → 保留（init 内引用重写）；
/// - 普通 `return v;` → `return Poll::Ready(v);`（v 重写）。
fn rewrite_stmt(
    stmt: &AstStmt,
    lifted: &HashSet<String>,
    future_sources: &HashSet<String>,
    span: Span,
) -> Option<AstStmt> {
    match stmt {
        AstStmt::Let {
            pattern,
            type_anno,
            init,
            mutable,
        } => {
            let span = init.span;
            if let AstPattern::Ident(x) = pattern {
                if future_sources.contains(x) {
                    return None;
                }
                if lifted.contains(x) {
                    // 跨段变量：let → self.x = init
                    return Some(AstStmt::Semi(AstExpr::new(
                        ExprKind::Assign {
                            target: self_field(x, span),
                            op: rlyeh_ast::AssignOp::Assign,
                            value: rewrite_expr(init, lifted, span),
                        },
                        span,
                    )));
                }
            }
            // 未提升：保留 let
            Some(AstStmt::Let {
                pattern: pattern.clone(),
                type_anno: type_anno.clone(),
                init: rewrite_expr(init, lifted, span),
                mutable: *mutable,
            })
        }
        AstStmt::Semi(e) | AstStmt::Expr(e) => {
            let mut e2 = rewrite_expr(e, lifted, span);
            // 普通 return → return Poll::Ready(v)
            if let ExprKind::Return(val) = &mut *e2.kind {
                let v = match val.take() {
                    Some(v) => v,
                    None => int_expr(0, span),
                };
                *val = Some(poll_ready(v, span));
            }
            match stmt {
                AstStmt::Semi(_) => Some(AstStmt::Semi(e2)),
                _ => Some(AstStmt::Expr(e2)),
            }
        }
        other => Some(other.clone()),
    }
}

/// 重写表达式：提升变量 Ident → `self.<name>`（函数名/字段名/模式绑定不重写）。
fn rewrite_expr(e: &AstExpr, lifted: &HashSet<String>, span: Span) -> AstExpr {
    match &*e.kind {
        ExprKind::Ident(name) => {
            if lifted.contains(name) {
                self_field(name, span)
            } else {
                e.clone()
            }
        }
        ExprKind::Set(items) => AstExpr::new(
            ExprKind::Set(items.iter().map(|i| rewrite_expr(i, lifted, span)).collect()),
            span,
        ),
        ExprKind::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => AstExpr::new(
            ExprKind::Range {
                lower: rewrite_expr(lower, lifted, span),
                upper: rewrite_expr(upper, lifted, span),
                lower_inclusive: *lower_inclusive,
                upper_inclusive: *upper_inclusive,
            },
            span,
        ),
        ExprKind::Binary { op, left, right } => AstExpr::new(
            ExprKind::Binary {
                op: *op,
                left: rewrite_expr(left, lifted, span),
                right: rewrite_expr(right, lifted, span),
            },
            span,
        ),
        ExprKind::Unary { op, operand } => AstExpr::new(
            ExprKind::Unary {
                op: *op,
                operand: rewrite_expr(operand, lifted, span),
            },
            span,
        ),
        ExprKind::ComparisonChain { elements, operators } => AstExpr::new(
            ExprKind::ComparisonChain {
                elements: elements
                    .iter()
                    .map(|el| rewrite_expr(el, lifted, span))
                    .collect(),
                operators: operators.clone(),
            },
            span,
        ),
        ExprKind::InSet { value, set, negated } => AstExpr::new(
            ExprKind::InSet {
                value: rewrite_expr(value, lifted, span),
                set: set.iter().map(|s| rewrite_expr(s, lifted, span)).collect(),
                negated: *negated,
            },
            span,
        ),
        ExprKind::InRange {
            value,
            range,
            negated,
        } => AstExpr::new(
            ExprKind::InRange {
                value: rewrite_expr(value, lifted, span),
                range: rewrite_expr(range, lifted, span),
                negated: *negated,
            },
            span,
        ),
        ExprKind::InRegion { expr, region } => AstExpr::new(
            ExprKind::InRegion {
                expr: rewrite_expr(expr, lifted, span),
                region: region.clone(),
            },
            span,
        ),
        ExprKind::Assign { target, op, value } => AstExpr::new(
            ExprKind::Assign {
                target: rewrite_expr(target, lifted, span),
                op: *op,
                value: rewrite_expr(value, lifted, span),
            },
            span,
        ),
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => AstExpr::new(
            ExprKind::If {
                cond: rewrite_expr(cond, lifted, span),
                then_block: rewrite_block(then_block, lifted, span),
                else_block: else_block
                    .as_ref()
                    .map(|b| rewrite_block(b, lifted, span)),
            },
            span,
        ),
        ExprKind::Match { expr, arms } => AstExpr::new(
            ExprKind::Match {
                expr: rewrite_expr(expr, lifted, span),
                arms: arms
                    .iter()
                    .map(|arm| MatchArm {
                        pattern: arm.pattern.clone(),
                        guard: arm
                            .guard
                            .as_ref()
                            .map(|g| rewrite_expr(g, lifted, span)),
                        body: rewrite_expr(&arm.body, lifted, span),
                        span: arm.span,
                    })
                    .collect(),
            },
            span,
        ),
        ExprKind::For {
            pattern,
            iterator,
            body,
        } => AstExpr::new(
            ExprKind::For {
                pattern: pattern.clone(),
                iterator: rewrite_expr(iterator, lifted, span),
                body: rewrite_block(body, lifted, span),
            },
            span,
        ),
        ExprKind::While { cond, body } => AstExpr::new(
            ExprKind::While {
                cond: rewrite_expr(cond, lifted, span),
                body: rewrite_block(body, lifted, span),
            },
            span,
        ),
        ExprKind::Loop { body } => AstExpr::new(
            ExprKind::Loop {
                body: rewrite_block(body, lifted, span),
            },
            span,
        ),
        ExprKind::Region {
            name,
            options,
            body,
        } => AstExpr::new(
            ExprKind::Region {
                name: name.clone(),
                options: options.clone(),
                body: rewrite_block(body, lifted, span),
            },
            span,
        ),
        ExprKind::GcRegion { body } => AstExpr::new(
            ExprKind::GcRegion {
                body: rewrite_block(body, lifted, span),
            },
            span,
        ),
        ExprKind::Transfer { expr, region } => AstExpr::new(
            ExprKind::Transfer {
                expr: rewrite_expr(expr, lifted, span),
                region: region.clone(),
            },
            span,
        ),
        ExprKind::Call {
            callee,
            args,
            type_args,
        } => AstExpr::new(
            ExprKind::Call {
                callee: match &*callee.kind {
                    // 函数名/枚举路径不重写
                    ExprKind::Ident(_) | ExprKind::Path(_) => callee.clone(),
                    _ => rewrite_expr(callee, lifted, span),
                },
                args: args
                    .iter()
                    .map(|arg| rewrite_expr(arg, lifted, span))
                    .collect(),
                type_args: type_args.clone(),
            },
            span,
        ),
        ExprKind::MacroCall { name, args } => AstExpr::new(
            ExprKind::MacroCall {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| rewrite_expr(arg, lifted, span))
                    .collect(),
            },
            span,
        ),
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => AstExpr::new(
            ExprKind::MethodCall {
                receiver: rewrite_expr(receiver, lifted, span),
                method: method.clone(),
                args: args
                    .iter()
                    .map(|arg| rewrite_expr(arg, lifted, span))
                    .collect(),
            },
            span,
        ),
        ExprKind::FieldAccess { expr, field } => AstExpr::new(
            ExprKind::FieldAccess {
                expr: rewrite_expr(expr, lifted, span),
                field: field.clone(),
            },
            span,
        ),
        ExprKind::StructCtor { type_name, fields } => AstExpr::new(
            ExprKind::StructCtor {
                type_name: type_name.clone(),
                fields: fields
                    .iter()
                    .map(|(k, v)| (k.clone(), rewrite_expr(v, lifted, span)))
                    .collect(),
            },
            span,
        ),
        ExprKind::Index { expr, index } => AstExpr::new(
            ExprKind::Index {
                expr: rewrite_expr(expr, lifted, span),
                index: rewrite_expr(index, lifted, span),
            },
            span,
        ),
        ExprKind::ArrayLit(items) => AstExpr::new(
            ExprKind::ArrayLit(
                items
                    .iter()
                    .map(|i| rewrite_expr(i, lifted, span))
                    .collect(),
            ),
            span,
        ),
        ExprKind::Closure {
            params,
            param_types,
            body,
            capture,
        } => AstExpr::new(
            ExprKind::Closure {
                params: params.clone(),
                param_types: param_types.clone(),
                body: rewrite_expr(body, lifted, span),
                capture: *capture,
            },
            span,
        ),
        ExprKind::Cast {
            expr,
            target_type,
        } => AstExpr::new(
            ExprKind::Cast {
                expr: rewrite_expr(expr, lifted, span),
                target_type: target_type.clone(),
            },
            span,
        ),
        ExprKind::Block(block) => AstExpr::new(
            ExprKind::Block(rewrite_block(block, lifted, span)),
            span,
        ),
        ExprKind::Return(Some(v)) => AstExpr::new(
            ExprKind::Return(Some(rewrite_expr(v, lifted, span))),
            span,
        ),
        ExprKind::Question(inner) => AstExpr::new(
            ExprKind::Question(Box::new(rewrite_expr(inner, lifted, span))),
            span,
        ),
        ExprKind::Break(Some(v)) => AstExpr::new(
            ExprKind::Break(Some(rewrite_expr(v, lifted, span))),
            span,
        ),
        ExprKind::Send {
            actor,
            method,
            args,
        } => AstExpr::new(
            ExprKind::Send {
                actor: rewrite_expr(actor, lifted, span),
                method: method.clone(),
                args: args
                    .iter()
                    .map(|arg| rewrite_expr(arg, lifted, span))
                    .collect(),
            },
            span,
        ),
        // 其余无子表达式或已由上层处理
        _ => e.clone(),
    }
}

/// 子 future 源变量名集合。
fn future_sources(a: &AnalyzedAsync) -> HashSet<String> {
    a.lifted_future_sources
        .iter()
        .map(|(v, _, _)| v.clone())
        .collect()
}

/// 重写块（控制流块内的 let 均为未提升局部，直接保留）。
fn rewrite_block(block: &AstBlock, lifted: &HashSet<String>, span: Span) -> AstBlock {
    AstBlock {
        stmts: block
            .stmts
            .iter()
            .map(|s| rewrite_stmt(s, lifted, &HashSet::new(), span).expect("non-top-level stmt"))
            .collect(),
        final_expr: block
            .final_expr
            .as_ref()
            .map(|fe| rewrite_expr(fe, lifted, span)),
        span: block.span,
    }
}

/// 脱变量：表达式中的标识符替换为 `0`（零值构造中避免悬空引用）。
fn devar(e: &AstExpr) -> AstExpr {
    match &*e.kind {
        ExprKind::Ident(_) => AstExpr::new(ExprKind::IntLiteral(0), e.span),
        ExprKind::Set(items) => AstExpr::new(
            ExprKind::Set(items.iter().map(devar).collect()),
            e.span,
        ),
        ExprKind::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => AstExpr::new(
            ExprKind::Range {
                lower: devar(lower),
                upper: devar(upper),
                lower_inclusive: *lower_inclusive,
                upper_inclusive: *upper_inclusive,
            },
            e.span,
        ),
        ExprKind::Binary { op, left, right } => AstExpr::new(
            ExprKind::Binary {
                op: *op,
                left: devar(left),
                right: devar(right),
            },
            e.span,
        ),
        ExprKind::Unary { op, operand } => AstExpr::new(
            ExprKind::Unary {
                op: *op,
                operand: devar(operand),
            },
            e.span,
        ),
        ExprKind::ComparisonChain { elements, operators } => AstExpr::new(
            ExprKind::ComparisonChain {
                elements: elements.iter().map(devar).collect(),
                operators: operators.clone(),
            },
            e.span,
        ),
        ExprKind::InSet { value, set, negated } => AstExpr::new(
            ExprKind::InSet {
                value: devar(value),
                set: set.iter().map(devar).collect(),
                negated: *negated,
            },
            e.span,
        ),
        ExprKind::InRange {
            value,
            range,
            negated,
        } => AstExpr::new(
            ExprKind::InRange {
                value: devar(value),
                range: devar(range),
                negated: *negated,
            },
            e.span,
        ),
        ExprKind::InRegion { expr, region } => AstExpr::new(
            ExprKind::InRegion {
                expr: devar(expr),
                region: region.clone(),
            },
            e.span,
        ),
        ExprKind::Assign { target, op, value } => AstExpr::new(
            ExprKind::Assign {
                target: devar(target),
                op: *op,
                value: devar(value),
            },
            e.span,
        ),
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => AstExpr::new(
            ExprKind::If {
                cond: devar(cond),
                then_block: devar_block(then_block),
                else_block: else_block.as_ref().map(devar_block),
            },
            e.span,
        ),
        ExprKind::Match { expr, arms } => AstExpr::new(
            ExprKind::Match {
                expr: devar(expr),
                arms: arms
                    .iter()
                    .map(|arm| MatchArm {
                        pattern: arm.pattern.clone(),
                        guard: arm.guard.as_ref().map(|g| devar(g)),
                        body: devar(&arm.body),
                        span: arm.span,
                    })
                    .collect(),
            },
            e.span,
        ),
        ExprKind::For {
            pattern,
            iterator,
            body,
        } => AstExpr::new(
            ExprKind::For {
                pattern: pattern.clone(),
                iterator: devar(iterator),
                body: devar_block(body),
            },
            e.span,
        ),
        ExprKind::While { cond, body } => AstExpr::new(
            ExprKind::While {
                cond: devar(cond),
                body: devar_block(body),
            },
            e.span,
        ),
        ExprKind::Loop { body } => AstExpr::new(
            ExprKind::Loop {
                body: devar_block(body),
            },
            e.span,
        ),
        ExprKind::Region {
            name,
            options,
            body,
        } => AstExpr::new(
            ExprKind::Region {
                name: name.clone(),
                options: options.clone(),
                body: devar_block(body),
            },
            e.span,
        ),
        ExprKind::GcRegion { body } => AstExpr::new(
            ExprKind::GcRegion {
                body: devar_block(body),
            },
            e.span,
        ),
        ExprKind::Transfer { expr, region } => AstExpr::new(
            ExprKind::Transfer {
                expr: devar(expr),
                region: region.clone(),
            },
            e.span,
        ),
        ExprKind::Call {
            callee,
            args,
            type_args,
        } => AstExpr::new(
            ExprKind::Call {
                callee: match &*callee.kind {
                    ExprKind::Ident(_) | ExprKind::Path(_) => callee.clone(),
                    _ => devar(callee),
                },
                args: args.iter().map(devar).collect(),
                type_args: type_args.clone(),
            },
            e.span,
        ),
        ExprKind::MacroCall { name, args } => AstExpr::new(
            ExprKind::MacroCall {
                name: name.clone(),
                args: args.iter().map(devar).collect(),
            },
            e.span,
        ),
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => AstExpr::new(
            ExprKind::MethodCall {
                receiver: devar(receiver),
                method: method.clone(),
                args: args.iter().map(devar).collect(),
            },
            e.span,
        ),
        ExprKind::FieldAccess { expr, field } => AstExpr::new(
            ExprKind::FieldAccess {
                expr: devar(expr),
                field: field.clone(),
            },
            e.span,
        ),
        ExprKind::StructCtor { type_name, fields } => AstExpr::new(
            ExprKind::StructCtor {
                type_name: type_name.clone(),
                fields: fields.iter().map(|(k, v)| (k.clone(), devar(v))).collect(),
            },
            e.span,
        ),
        ExprKind::Index { expr, index } => AstExpr::new(
            ExprKind::Index {
                expr: devar(expr),
                index: devar(index),
            },
            e.span,
        ),
        ExprKind::ArrayLit(items) => AstExpr::new(
            ExprKind::ArrayLit(items.iter().map(devar).collect()),
            e.span,
        ),
        ExprKind::Closure {
            params,
            param_types,
            body,
            capture,
        } => AstExpr::new(
            ExprKind::Closure {
                params: params.clone(),
                param_types: param_types.clone(),
                body: devar(body),
                capture: *capture,
            },
            e.span,
        ),
        ExprKind::Cast {
            expr,
            target_type,
        } => AstExpr::new(
            ExprKind::Cast {
                expr: devar(expr),
                target_type: target_type.clone(),
            },
            e.span,
        ),
        ExprKind::Block(block) => AstExpr::new(
            ExprKind::Block(devar_block(block)),
            e.span,
        ),
        ExprKind::Return(Some(v)) => AstExpr::new(
            ExprKind::Return(Some(devar(v))),
            e.span,
        ),
        ExprKind::Question(inner) => AstExpr::new(
            ExprKind::Question(Box::new(devar(inner))),
            e.span,
        ),
        ExprKind::Break(Some(v)) => AstExpr::new(
            ExprKind::Break(Some(devar(v))),
            e.span,
        ),
        ExprKind::Send {
            actor,
            method,
            args,
        } => AstExpr::new(
            ExprKind::Send {
                actor: devar(actor),
                method: method.clone(),
                args: args.iter().map(devar).collect(),
            },
            e.span,
        ),
        _ => e.clone(),
    }
}

/// 脱变量块。
fn devar_block(block: &AstBlock) -> AstBlock {
    AstBlock {
        stmts: block.stmts.iter().map(devar_stmt).collect(),
        final_expr: block.final_expr.as_ref().map(|fe| devar(fe)),
        span: block.span,
    }
}

/// 脱变量语句。
fn devar_stmt(stmt: &AstStmt) -> AstStmt {
    match stmt {
        AstStmt::Let {
            pattern,
            type_anno,
            init,
            mutable,
        } => AstStmt::Let {
            pattern: pattern.clone(),
            type_anno: type_anno.clone(),
            init: devar(init),
            mutable: *mutable,
        },
        AstStmt::Semi(e) => AstStmt::Semi(devar(e)),
        AstStmt::Expr(e) => AstStmt::Expr(devar(e)),
        other => other.clone(),
    }
}
