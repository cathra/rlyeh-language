//! # rlyeh-desugar
//!
//! Rlyeh 语言 async/await 状态机 desugar（S1c，2026-08）。
//!
//! 将顶层 `async fn` 编译为三个普通项（AST → AST 转换，parse 后、typecheck
//! 前执行，下游各阶段无感知）：
//!
//! - `struct __Fut_<f>`：状态机结构体（`state: i64` + 子 future 槽 + 提升字段）；
//! - `impl Future for __Fut_<f>`：`poll` 方法按状态分派执行，遇到子 future
//!   `Pending` 时保存状态并返回 `Poll::Pending`，下次 `poll` 从保存状态恢复；
//! - `fn f(args) -> __Fut_<f>`：构造函数（返回初始状态机）。
//!
//! ## MVP 范围（std-lib.md §10.2 / mvp-gaps-plan.md S1c）
//!
//! - `async fn` 限顶层、非泛型、返回 `i64`（`-> ()` 映射 `Ready(0)`），参数限 `i64`；
//! - `.await` 目标限两种：同一编译单元内 async fn 的直接调用
//!   `f(args).await`，或带类型注解的 let 变量 `x.await`（其类型须实现
//!   `Future`，MVP 由用户代码 `impl Future for X` 提供）；
//! - `.await` 仅限直线代码：`let v = e.await;` / `e.await;` / `return e.await;`
//!   / 块尾表达式 `e.await`；`if`/`while`/`loop`/`for`/`match`/`region`/闭包
//!   等控制流内出现 `.await` 报错；await 表达式内禁止嵌套 await；
//! - 跨 await 的局部变量：await 结果变量与跨段 `i64` 变量提升为状态机字段
//!   （段内初始化赋 `self.x = init`）；被 await 的变量（子 future 源）提升，
//!   其初始化表达式搬到构造函数；参数一律提升；
//! - 递归 async fn（`f` 的 await 目标含 `f` 自身或其依赖环）报错。

#![warn(missing_docs)]
#![warn(unsafe_code)]

mod analyze;
mod generate;
mod guard;

use std::collections::{HashMap, HashSet};

use thiserror::Error;
use rlyeh_ast::{AstFnDecl, AstImplBlock, AstItem, AstProgram, AstType, AstTypeParam};
use rlyeh_lexer::Span;

pub use analyze::{AnalyzedAsync, AwaitInfo, Segment};

/// desugar 阶段错误。
#[derive(Debug, Error)]
pub enum DesugarError {
    /// MVP 范围之外的语法/语义（await 位置、跨 await 变量类型、递归等）。
    #[error("S1c async/await 状态机 desugar 限制：{what}")]
    Unsupported {
        /// 错误描述
        what: String,
        /// 源码位置
        span: Span,
    },
}

impl DesugarError {
    /// 错误位置。
    pub fn span(&self) -> Span {
        match self {
            DesugarError::Unsupported { span, .. } => *span,
        }
    }
}

/// `scan_expr` 的 await 检测错误（`Err(())`）统一转为 Unsupported。
///
/// 该占位错误仅作为兜底；调用点通常会在 `?` 前已构造更精确、带位置的错误。
impl From<()> for DesugarError {
    fn from(_: ()) -> Self {
        DesugarError::Unsupported {
            what: "表达式内含 await（扫描失败）".to_string(),
            span: Span {
                start: 0,
                end: 0,
                line: 1,
                col: 1,
            },
        }
    }
}

/// 对程序执行 async/await 状态机 desugar（原地修改 items）。
///
/// 无 async fn 时原样返回；有则按依赖拓扑序分析并生成
/// struct/impl/构造器，替换原 async fn 项。
pub fn desugar_program(program: &mut AstProgram) -> Result<(), DesugarError> {
    // PC-1：声明点一致性 + 类型体内联成员 → 既有 impl 结构（先于 guard，使其可见方法体）
    lower_type_conformance(program)?;

    // P2：MutexGuard 作用域守卫自动解锁注入（独立 pass，先于 async desugar）
    guard::inject_guard_unlocks(program);

    // 1. 收集顶层 async fn 声明
    let async_names: HashSet<String> = program
        .items
        .iter()
        .filter_map(|item| match item {
            AstItem::FnDecl(f) if f.is_async && !f.is_extern => Some(f.name.clone()),
            _ => None,
        })
        .collect();
    if async_names.is_empty() {
        return Ok(());
    }

    let mut decls: HashMap<String, AstFnDecl> = HashMap::new();
    // W6：泛型 async fn 名集合（作为子 future await 暂不支持）。
    let mut generic_async: HashSet<String> = HashSet::new();
    for item in &program.items {
        if let AstItem::FnDecl(f) = item {
            if f.is_async && !f.is_extern {
                decls.insert(f.name.clone(), f.as_ref().clone());
                if !f.generics.is_empty() {
                    generic_async.insert(f.name.clone());
                }
            }
        }
    }

    // 2. 逐个分析（拿到依赖 + 完整状态机规划）
    let mut analyzed: Vec<AnalyzedAsync> = Vec::new();
    for (name, decl) in &decls {
        let a = analyze::analyze_async_fn(decl, &async_names, &generic_async)?;
        debug_assert_eq!(&a.decl.name, name);
        analyzed.push(a);
    }

    // 3. 依赖拓扑排序（被依赖者在前）；W6 支持递归环，`cyclic` 记录环内 async fn 名
    let mut visited: HashSet<String> = HashSet::new();
    let mut ordering: Vec<String> = Vec::new();
    let mut cyclic: HashSet<String> = HashSet::new();
    for a in &analyzed {
        topo_sort(&a.decl.name, &analyzed, &mut visited, &mut ordering, &mut cyclic)?;
    }

    // 4. 按拓扑序生成（结构体字段布局先记录，供槽零值构造引用）
    let mut generated: HashMap<String, Vec<AstItem>> = HashMap::new();
    let mut layout: HashMap<String, Vec<generate::FieldSpec>> = HashMap::new();
    for name in &ordering {
        let a = analyzed
            .iter()
            .find(|a| &a.decl.name == name)
            .expect("analyzed async fn");
        let (struct_item, spec) = generate::gen_struct(a, &layout, &cyclic);
        layout.insert(name.clone(), spec);
        generated.insert(name.clone(), vec![struct_item]);
    }
    for name in &ordering {
        let a = analyzed
            .iter()
            .find(|a| &a.decl.name == name)
            .expect("analyzed async fn");
        generated
            .get_mut(name)
            .expect("generated")
            .push(generate::gen_impl(a, &cyclic));
        generated
            .get_mut(name)
            .expect("generated")
            .push(generate::gen_ctor(a, &layout, &cyclic));
    }

    // 5. 替换原 async fn 项（保持原位置，展开为 3 项）
    let mut new_items: Vec<AstItem> = Vec::with_capacity(program.items.len() + 3 * async_names.len());
    for item in std::mem::take(&mut program.items) {
        match &item {
            AstItem::FnDecl(f) if f.is_async && !f.is_extern => {
                new_items.extend(generated.remove(&f.name).expect("generated async fn"));
            }
            _ => new_items.push(item),
        }
    }
    program.items = new_items;
    Ok(())
}

/// PC-1/PC-3：把类型声明的「声明点一致性 + 类型体内联成员」归一为既有 `impl` 块
/// （见 `docs/rfc/protocol-syntax.md` §3.3 / §6）：
///
/// - `struct C { fields; members }`（无协议）→ `struct C { fields }` + `impl C { members }`
/// - `struct C: P { fields; members }` → 按「成员名是否属于 P 的需求集」拆分：属于 P 的成员入
///   `impl P for C`，其余入固有 `impl C`；协议无匹配成员时仍发空 impl（用于默认方法一致性声明）。
/// - 多协议 `struct C: A, B { .. }` → 逐协议展开；成员按「首个接受它的协议」归属，未匹配者入固有 impl。
/// - `enum E: P { .. }` 同理。内置 protocol（`Drop`/`Any`）或预扫描未命中的协议视为「接受全部成员」。
///
/// 递归处理模块项。
fn lower_type_conformance(program: &mut AstProgram) -> Result<(), DesugarError> {
    // 预扫描全程序协议成员名（simple name 与 `mod::Name` 两种键），供成员归属裁决（§3.3）。
    let mut proto_members: HashMap<String, HashSet<String>> = HashMap::new();
    collect_protocol_members(&program.items, "", &mut proto_members);
    lower_items(&mut program.items, &proto_members)
}

/// 递归收集协议（`protocol`/`protocol`）的成员名集合（方法 + 关联类型），供裁决拆分使用。
fn collect_protocol_members(
    items: &[AstItem],
    prefix: &str,
    out: &mut HashMap<String, HashSet<String>>,
) {
    for item in items {
        match item {
            AstItem::ProtocolDecl(t) => {
                let mut names: HashSet<String> = HashSet::new();
                for m in &t.methods {
                    names.insert(m.name.clone());
                }
                for ty in &t.types {
                    names.insert(ty.clone());
                }
                out.insert(t.name.clone(), names.clone());
                if !prefix.is_empty() {
                    out.insert(format!("{prefix}::{}", t.name), names);
                }
            }
            AstItem::ModDecl(m) => {
                let child = if prefix.is_empty() {
                    m.name.clone()
                } else {
                    format!("{prefix}::{}", m.name)
                };
                collect_protocol_members(&m.items, &child, out);
            }
            _ => {}
        }
    }
}

/// 构造一个归一后的 `AstItem::ImplBlock`。
#[allow(clippy::too_many_arguments)]
fn make_impl(
    protocol_name: Option<String>,
    protocol_type_args: Vec<AstType>,
    type_name: String,
    span: Span,
    generics: Vec<AstTypeParam>,
    methods: Vec<AstFnDecl>,
    types: Vec<(String, AstType)>,
) -> AstItem {
    AstItem::ImplBlock(Box::new(AstImplBlock {
        protocol_name,
        type_name,
        generics,
        protocol_type_args,
        extra_protocols: Vec::new(),
        types,
        methods,
        span,
    }))
}

/// PC-9：把 `impl T: A, B { .. }` 的多协议一致性拆分为多个独立 impl 块。
///
/// 第 0 个协议（原 `protocol_name`）保留在原块（位置不变），其余协议追加到 `synthesized`。
/// 成员按「首个接受它的协议」归属（与声明点一致性 `struct C: A, B` 同一规则）；无处归属者
/// 并入首个协议块——保证 `impl T: A, B { .. }` 与「拆成多个 `impl` 分别书写」在成员全属
/// 首协议时等价。
fn lower_impl_conformance(
    i: &mut AstImplBlock,
    synthesized: &mut Vec<AstItem>,
    proto_members: &HashMap<String, HashSet<String>>,
) {
    let type_name = i.type_name.clone();
    let span = i.span;
    let generics = i.generics.clone();
    let first = i.protocol_name.take();
    let first_args = std::mem::take(&mut i.protocol_type_args);
    let mut protocols: Vec<(String, Vec<AstType>)> = Vec::new();
    if let Some(f) = first {
        protocols.push((f, first_args));
    }
    protocols.extend(std::mem::take(&mut i.extra_protocols));
    if protocols.len() < 2 {
        // 防御：无额外协议时不应进入此处；回填后原样返回。
        if let Some((p, a)) = protocols.pop() {
            i.protocol_name = Some(p);
            i.protocol_type_args = a;
        }
        return;
    }
    let methods = std::mem::take(&mut i.methods);
    let assoc_types = std::mem::take(&mut i.types);
    // 协议 `p` 是否「接受」成员名 `name`：已预扫描者查其需求集；
    // 未命中（内置协议 / 跨编译单元协议）者视为接受全部。
    let accepts = |p: &str, name: &str| -> bool {
        proto_members.get(p).map_or(true, |s| s.contains(name))
    };
    let mut buckets: Vec<(Vec<AstFnDecl>, Vec<(String, AstType)>)> =
        protocols.iter().map(|_| (Vec::new(), Vec::new())).collect();
    for m in methods {
        match protocols.iter().position(|(n, _)| accepts(n, &m.name)) {
            Some(k) => buckets[k].0.push(m),
            None => buckets[0].0.push(m),
        }
    }
    for (tn, ty) in assoc_types {
        match protocols.iter().position(|(n, _)| accepts(n, &tn)) {
            Some(k) => buckets[k].1.push((tn, ty)),
            None => buckets[0].1.push((tn, ty)),
        }
    }
    let (m0, t0) = buckets.remove(0);
    let (p0, a0) = protocols.remove(0);
    i.protocol_name = Some(p0);
    i.protocol_type_args = a0;
    i.methods = m0;
    i.types = t0;
    for ((pn, pargs), (pm, pt)) in protocols.into_iter().zip(buckets) {
        synthesized.push(make_impl(
            Some(pn),
            pargs,
            type_name.clone(),
            span,
            generics.clone(),
            pm,
            pt,
        ));
    }
}

/// 递归处理一层 items（模块内递归 + 本层 struct/enum/impl 归一）。
fn lower_items(
    items: &mut Vec<AstItem>,
    proto_members: &HashMap<String, HashSet<String>>,
) -> Result<(), DesugarError> {
    // 先递归模块（外部模块 `module m;` 的 items 为空，天然跳过）
    for item in items.iter_mut() {
        if let AstItem::ModDecl(m) = item {
            lower_items(&mut m.items, proto_members)?;
        }
    }
    // 再处理本层：合成的 impl 追加到末尾（typecheck 先收集全部再检查，顺序无关）。
    let mut synthesized: Vec<AstItem> = Vec::new();
    for item in items.iter_mut() {
        // PC-9：`impl T: A, B { .. }`（多协议一致性）→ 按协议成员名裁决拆分为多个 impl 块。
        if let AstItem::ImplBlock(i) = item {
            if !i.extra_protocols.is_empty() {
                lower_impl_conformance(i, &mut synthesized, proto_members);
            }
            continue;
        }
        let (type_name, span, generics, conformances, methods, assoc_types) = match item {
            AstItem::StructDecl(s) => (
                s.name.clone(),
                s.span,
                s.generics.clone(),
                std::mem::take(&mut s.conformances),
                std::mem::take(&mut s.methods),
                std::mem::take(&mut s.assoc_types),
            ),
            AstItem::EnumDecl(e) => (
                e.name.clone(),
                e.span,
                e.generics.clone(),
                std::mem::take(&mut e.conformances),
                std::mem::take(&mut e.methods),
                std::mem::take(&mut e.assoc_types),
            ),
            _ => continue,
        };
        if methods.is_empty() && assoc_types.is_empty() && conformances.is_empty() {
            continue;
        }
        // 无协议：单一固有 impl。
        if conformances.is_empty() {
            synthesized.push(make_impl(
                None,
                Vec::new(),
                type_name,
                span,
                generics,
                methods,
                assoc_types,
            ));
            continue;
        }
        // 每个声明协议一个成员桶；未匹配任何协议需求的成员入固有 impl。
        let mut per_proto: Vec<(String, Vec<AstType>, Vec<AstFnDecl>, Vec<(String, AstType)>)> =
            conformances
                .iter()
                .map(|(n, a)| (n.clone(), a.clone(), Vec::new(), Vec::new()))
                .collect();
        let mut inherent_methods: Vec<AstFnDecl> = Vec::new();
        let mut inherent_types: Vec<(String, AstType)> = Vec::new();
        // 协议 `p` 是否「接受」成员名 `name`：已预扫描者查其需求集；
        // 未命中（内置 protocol / 跨编译单元协议）者视为接受全部。
        let accepts = |p: &str, name: &str| -> bool {
            proto_members.get(p).map_or(true, |s| s.contains(name))
        };
        for m in methods {
            match per_proto.iter().position(|(n, ..)| accepts(n, &m.name)) {
                Some(i) => per_proto[i].2.push(m),
                None => inherent_methods.push(m),
            }
        }
        for (tn, ty) in assoc_types {
            match per_proto.iter().position(|(n, ..)| accepts(n, &tn)) {
                Some(i) => per_proto[i].3.push((tn, ty)),
                None => inherent_types.push((tn, ty)),
            }
        }
        for (pn, pargs, pm, pt) in per_proto {
            synthesized.push(make_impl(
                Some(pn),
                pargs,
                type_name.clone(),
                span,
                generics.clone(),
                pm,
                pt,
            ));
        }
        if !inherent_methods.is_empty() || !inherent_types.is_empty() {
            synthesized.push(make_impl(
                None,
                Vec::new(),
                type_name,
                span,
                generics,
                inherent_methods,
                inherent_types,
            ));
        }
    }
    items.extend(synthesized);
    Ok(())
}

/// DFS 拓扑排序：`name` 及其依赖（被依赖者先入 `ordering`）。
///
/// W6 递归 async fn 支持：依赖环不再报错。当 DFS 回溯遇到仍在访问栈中的
/// 节点（`visited` 已含但 `ordering` 未含）即构成环，将其全部成员记入
/// `cyclic`（供生成层对递归子 future 槽用 `Box<__Fut_>` 打破无限大小）。
/// 环内节点因 `visited` 去重仍会全部进入 `ordering`（类型层两遍收集地基
/// 保证 `struct __Fut_f` 可前向自引用）。
fn topo_sort(
    name: &str,
    analyzed: &[AnalyzedAsync],
    visited: &mut HashSet<String>,
    ordering: &mut Vec<String>,
    cyclic: &mut HashSet<String>,
) -> Result<(), DesugarError> {
    if ordering.contains(&name.to_string()) {
        return Ok(());
    }
    if visited.contains(name) {
        // 环：所有当前访问栈中尚未完成排序的成员均属递归环。
        for a in analyzed {
            if visited.contains(&a.decl.name) && !ordering.contains(&a.decl.name) {
                cyclic.insert(a.decl.name.clone());
            }
        }
        return Ok(());
    }
    visited.insert(name.to_string());
    let a = analyzed
        .iter()
        .find(|a| a.decl.name == name)
        .expect("analyzed async fn");
    for dep in &a.deps {
        topo_sort(dep, analyzed, visited, ordering, cyclic)?;
    }
    ordering.push(name.to_string());
    Ok(())
}
