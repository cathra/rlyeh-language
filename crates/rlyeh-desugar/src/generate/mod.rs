//! 状态机生成：struct/impl Future/构造函数 + 引用重写。

use std::collections::{HashMap, HashSet};

use rlyeh_ast::{
    AstBlock, AstExpr, AstFnDecl, AstImplBlock, AstItem, AstParam, AstPattern, AstStmt,
    AstStructDecl, AstStructField, AstType, CompareOp, ExprKind, MatchArm, UnaryOp,
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

/// `Box::new(x)` 表达式（堆分配包装子 future，打破递归无限大小）。
fn box_expr(x: AstExpr, span: Span) -> AstExpr {
    AstExpr::new(
        ExprKind::Call {
            callee: path(&["Box", "new"], span),
            args: vec![x],
            type_args: Vec::new(),
        },
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
    // W1：poll 签名带 `cx: &mut Context`，调用点透传 `&mut *cx`——
    // cx 参数本身是 `&mut Context`，直接 `&mut cx` 是引用再取引用（被禁），
    // 解引用再取址 `&mut *cx` 产生新的 `&mut Context`（U5 解引用目标路径）。
    let cx_ref = AstExpr::new(
        ExprKind::Unary {
            op: UnaryOp::AddrOfMut,
            operand: AstExpr::new(
                ExprKind::Unary {
                    op: UnaryOp::Deref,
                    operand: ident("cx", span),
                },
                span,
            ),
        },
        span,
    );
    AstExpr::new(
        ExprKind::MethodCall {
            receiver,
            method: "poll".to_string(),
            args: vec![cx_ref],
            trait_hint: None,
        },
        span,
    )
}

/// 生成状态机结构体定义，并返回字段规格（供构造器/零值构造复用）。
///
/// `cyclic`：递归环内的 async fn 名集合。对其子 future 槽用 `Box<__Fut_>` 打破
/// 无限大小（`struct __Fut_f { sub: Box<__Fut_f> }`），由类型层两遍收集地基支持。
pub fn gen_struct(
    a: &AnalyzedAsync,
    layout: &HashMap<String, Vec<FieldSpec>>,
    cyclic: &HashSet<String>,
) -> (AstItem, Vec<FieldSpec>) {
    let span = span_of(a);
    let spec = gen_fields(a, layout, cyclic);
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
        // W6：透传泛型参数到 Future 结构体（`struct __Fut_foo<T>`）。
        generics: a.decl.generics.clone(),
        fields,
        derive: Vec::new(),
        span,
    }));
    (item, spec)
}

/// 生成 `impl Future for __Fut_<f>`（poll 状态分派）。
pub fn gen_impl(a: &AnalyzedAsync, cyclic: &HashSet<String>) -> AstItem {
    let span = span_of(a);
    let poll_body = gen_poll_body(a, cyclic);
    let poll_fn = AstFnDecl {
        name: "poll".to_string(),
        generics: Vec::new(),
        params: vec![
            AstParam {
                name: "self".to_string(),
                // `&mut self`：与 parser 对 `fn poll(&mut self)` 的解析保持一致
                // （Ref(Path("Self"), true)），typecheck 对 `Self` 路径按 impl 目标
                // 类型解析接收者，显式 `__Fut_X` 路径会导致接收者字段访问/类型
                // 解析不一致。
                type_: AstType::Ref(Box::new(AstType::Path("Self".to_string(), Vec::new())), true),
                default: None,
                is_mut: false,
                span,
            },
            // W1：poll 签名对齐规划 `fn poll(&mut self, cx: &mut Context)`——
            // `Context` 占位类型（std future.rl），保留参数位、body 不使用。
            AstParam {
                name: "cx".to_string(),
                type_: AstType::Ref(Box::new(AstType::Path("Context".to_string(), Vec::new())), true),
                default: None,
                is_mut: false,
                span,
            },
        ],
        // W1：返回类型 `Poll<Self::Output>`（关联类型，U2）——typecheck 经 impl
        // 的 assoc_types 映射（`type Output = i64`）替换为 `Poll<i64>`。
        return_type: Some(AstType::Path(
            "Poll".to_string(),
            vec![AstType::Path("Self::Output".to_string(), Vec::new())],
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
        // W6：透传泛型参数到 impl（`impl<T> Future for __Fut_foo<T>`）。
        generics: a.decl.generics.clone(),
        // W6：关联类型定义 `type Output = <ret_ty or i64>;`（U2）。
        // `()` 返回沿用 i64（尾值 Ready(0)）；泛型返回 `T` 经单态化替换。
        types: vec![(
            "Output".to_string(),
            a.ret_ty.clone().unwrap_or_else(i64_ty),
        )],
        methods: vec![poll_fn],
        span,
    }))
}

/// 生成构造函数 `fn f(args) -> __Fut_<f>`。
pub fn gen_ctor(
    a: &AnalyzedAsync,
    layout: &HashMap<String, Vec<FieldSpec>>,
    cyclic: &HashSet<String>,
) -> AstItem {
    let span = span_of(a);
    let spec = gen_fields(a, layout, cyclic);
    let fields: Vec<(String, AstExpr)> = spec
        .iter()
        .map(|f| (f.name.clone(), f.ctor_init.clone()))
        .collect();
    // W6：泛型参数名 → AstType（`__Fut_foo<T>` 的 `T`）。
    let gen_args: Vec<AstType> = a
        .decl
        .generics
        .iter()
        .map(|g| AstType::Path(g.name.clone(), Vec::new()))
        .collect();
    let ctor_expr = AstExpr::new(
        ExprKind::StructCtor {
            type_name: vec![fut_ty_name(&a.decl.name)],
            type_args: gen_args.clone(),
            fields,
        },
        span,
    );
    let ret_ty = if gen_args.is_empty() {
        AstType::Path(fut_ty_name(&a.decl.name), Vec::new())
    } else {
        AstType::Path(fut_ty_name(&a.decl.name), gen_args)
    };
    AstItem::FnDecl(Box::new(AstFnDecl {
        name: a.decl.name.clone(),
        // W6：透传泛型参数到构造器（`fn foo<T>(...) -> __Fut_foo<T>`）。
        generics: a.decl.generics.clone(),
        params: a.lifted_params.clone(),
        return_type: Some(ret_ty),
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
///
/// `cyclic`：递归环内 async fn 名。其子 future 槽类型改为 `Box<__Fut_<name>>`
/// （Box 打破无限大小），poll 访问经方法自动剥层，构造/零值用 `Box::new` 包装。
fn gen_fields(a: &AnalyzedAsync, layout: &HashMap<String, Vec<FieldSpec>>, cyclic: &HashSet<String>) -> Vec<FieldSpec> {
    let span = span_of(a);
    let mut spec = Vec::new();

    // state
    spec.push(FieldSpec {
        name: "state".to_string(),
        ty: i64_ty(),
        ctor_init: int_expr(0, span),
        zero_init: int_expr(0, span),
    });

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

    // 跨段变量（段内初始化；构造器零值按类型）
    for (v, ty, _) in &a.lifted_cross {
        let zero = zero_for_type(ty, span);
        spec.push(FieldSpec {
            name: v.clone(),
            ty: ty.clone(),
            ctor_init: zero.clone(),
            zero_init: zero,
        });
    }

    // 内部提升字段（迭代游标等，i64）
    for (v, ty) in &a.lifted_internal {
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

    // 子 future 槽。分两遍：先非递归槽（其零值经 `zero_ctor` 按依赖布局构造，
    // 进入 `spec` 供递归槽零值构造复用），再递归环内槽（`Box<__Fut_>` 打破
    // 无限大小，零值 = `Box::new(__Fut_X { <除自身外所有其他字段零值> })`——
    // 含其他非递归子 future 槽；自身递归 Box 槽由 typecheck 对缺失 Box 字段
    // 放行=空指针占位，首次 poll 求值前不 deref）。
    let mut cyclic_slots: Vec<(String, String)> = Vec::new();
    for (slot, ty_name) in &a.slots {
        let target = ty_name.strip_prefix("__Fut_").unwrap_or(ty_name);
        if cyclic.contains(target) {
            cyclic_slots.push((slot.clone(), ty_name.clone()));
            continue;
        }
        let zero = zero_ctor(ty_name, layout, span);
        spec.push(FieldSpec {
            name: slot.clone(),
            ty: AstType::Path(ty_name.clone(), Vec::new()),
            ctor_init: zero.clone(),
            zero_init: zero,
        });
    }
    // 递归槽零值构造复用的"除自身外所有字段零值"（含非递归子 future 槽）
    let base_zero_fields: Vec<(String, AstExpr)> = spec
        .iter()
        .map(|f| (f.name.clone(), f.zero_init.clone()))
        .collect();
    for (slot, ty_name) in cyclic_slots {
        let inner = AstExpr::new(
            ExprKind::StructCtor {
                type_name: vec![ty_name.clone()],
                type_args: Vec::new(),
                fields: base_zero_fields.clone(),
            },
            span,
        );
        let init = box_expr(inner, span);
        spec.push(FieldSpec {
            name: slot,
            ty: AstType::Path("Box".to_string(), vec![AstType::Path(ty_name, Vec::new())]),
            ctor_init: init.clone(),
            zero_init: init,
        });
    }

    spec
}

/// W2：跨段变量零值按类型（i64 → 0，f64 → 0.0，bool → false，char → '\0'，String → ""）。
fn zero_for_type(ty: &AstType, span: Span) -> AstExpr {
    match ty {
        AstType::Path(n, _) if n == "f64" => AstExpr::new(ExprKind::FloatLiteral(0.0), span),
        AstType::Path(n, _) if n == "bool" => AstExpr::new(ExprKind::BoolLiteral(false), span),
        AstType::Path(n, _) if n == "char" => AstExpr::new(ExprKind::CharLiteral('\0'), span),
        AstType::Path(n, _) if n == "String" => {
            // `String::from("")`（返回 `String`，与字段类型匹配；裸字面量类型为 `string`）
            AstExpr::new(
                ExprKind::Call {
                    callee: AstExpr::new(
                        ExprKind::Path(vec!["String".to_string(), "from".to_string()]),
                        span,
                    ),
                    args: vec![AstExpr::new(ExprKind::StringLiteral(String::new()), span)],
                    type_args: Vec::new(),
                },
                span,
            )
        }
        _ => int_expr(0, span),
    }
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
                type_args: Vec::new(),
                fields: spec.iter().map(|f| (f.name.clone(), f.zero_init.clone())).collect(),
            },
            span,
        ),
        None => {
            // 被依赖者未先生成（拓扑序异常）：退化为 state: 0 单字段
            AstExpr::new(
                ExprKind::StructCtor {
                    type_name: vec![type_name.to_string()],
                    type_args: Vec::new(),
                    fields: vec![("state".to_string(), int_expr(0, span))],
                },
                span,
            )
        }
    }
}

/// 生成 poll 函数体（状态 if 链 + 兜底 Ready(0)）。
///
/// W2：段类型分派——
/// - await 段：状态 2i（首轮询）+ 2i+1（恢复轮询）
/// - 收尾段（`final_expr`/隐式）：状态 2i，return Poll::Ready(尾值 or 0)
/// - 跳转段（`next`）：状态 2i，段尾 `state = 2*next`
/// - 控制流入口段（`inline_jump`）：状态 2i，stmts 内含 state 跳转

/// `self.state = 2 * target_seg;`

/// 跳转段状态：段语句 + `self.state = 2 * next;`。

/// 控制流入口段状态：段语句（内含 state 跳转，不追加）。

/// 收尾段状态：段语句 + `return Poll::Ready(尾值 or 0)`。

/// await 段状态（首次）：段语句 + 子 future 求值 + 首轮询。

/// 恢复段状态：重新轮询子 future。

/// 生成轮询 match（Ready/Pending 两臂）。

/// Ready 分支处理块。W2：非终止 await 段跳转 `state = 2 * seg.next`（段显式后继）。

/// Pending 分支处理块。

/// 重写语句。返回 `None` 表示删除该语句（子 future 源 let——初始化已搬到构造器）。
///
/// - 跨段 let（lifted_cross）→ `self.x = init;`（字段赋值）；
/// - 子 future 源 let → 删除；
/// - 未提升 let → 保留（init 内引用重写）；
/// - 普通 `return v;` → `return Poll::Ready(v);`（v 重写）。

/// 重写表达式：提升变量 Ident → `self.<name>`（函数名/字段名/模式绑定不重写）。

/// 子 future 源变量名集合。

/// 重写块（控制流块内的 let 均为未提升局部，直接保留）。

/// 脱变量：表达式中的标识符替换为 `0`（零值构造中避免悬空引用）。

/// 脱变量块。

/// 脱变量语句。


mod gen_poll;
mod rewrite;
mod devar;

use gen_poll::*;
use rewrite::*;
use devar::*;
