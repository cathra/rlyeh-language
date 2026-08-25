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
use rlyeh_ast::{AstFnDecl, AstItem, AstProgram};
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

/// 对程序执行 async/await 状态机 desugar（原地修改 items）。
///
/// 无 async fn 时原样返回；有则按依赖拓扑序分析并生成
/// struct/impl/构造器，替换原 async fn 项。
pub fn desugar_program(program: &mut AstProgram) -> Result<(), DesugarError> {
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
    for item in &program.items {
        if let AstItem::FnDecl(f) = item {
            if f.is_async && !f.is_extern {
                decls.insert(f.name.clone(), f.as_ref().clone());
            }
        }
    }

    // 2. 逐个分析（拿到依赖 + 完整状态机规划）
    let mut analyzed: Vec<AnalyzedAsync> = Vec::new();
    for (name, decl) in &decls {
        let a = analyze::analyze_async_fn(decl, &async_names)?;
        debug_assert_eq!(&a.decl.name, name);
        analyzed.push(a);
    }

    // 3. 依赖拓扑排序（被依赖者在前），检测环 = 递归 async fn
    let mut visited: HashSet<String> = HashSet::new();
    let mut ordering: Vec<String> = Vec::new();
    for a in &analyzed {
        topo_sort(&a.decl.name, &analyzed, &mut visited, &mut ordering)?;
    }

    // 4. 按拓扑序生成（结构体字段布局先记录，供槽零值构造引用）
    let mut generated: HashMap<String, Vec<AstItem>> = HashMap::new();
    let mut layout: HashMap<String, Vec<generate::FieldSpec>> = HashMap::new();
    for name in &ordering {
        let a = analyzed
            .iter()
            .find(|a| &a.decl.name == name)
            .expect("analyzed async fn");
        let (struct_item, spec) = generate::gen_struct(a, &layout);
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
            .push(generate::gen_impl(a));
        generated
            .get_mut(name)
            .expect("generated")
            .push(generate::gen_ctor(a, &layout));
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

/// DFS 拓扑排序：`name` 及其依赖（被依赖者先入 `ordering`）。
fn topo_sort(
    name: &str,
    analyzed: &[AnalyzedAsync],
    visited: &mut HashSet<String>,
    ordering: &mut Vec<String>,
) -> Result<(), DesugarError> {
    if ordering.contains(&name.to_string()) {
        return Ok(());
    }
    if visited.contains(name) {
        return Err(DesugarError::Unsupported {
            what: format!("递归 async fn 不支持（检测到依赖环，涉及 `{name}`）"),
            span: analyzed
                .iter()
                .find(|a| a.decl.name == name)
                .map(|a| a.decl.span)
                .unwrap_or(Span {
                    start: 0,
                    end: 0,
                    line: 0,
                    col: 0,
                }),
        });
    }
    visited.insert(name.to_string());
    let a = analyzed
        .iter()
        .find(|a| a.decl.name == name)
        .expect("analyzed async fn");
    for dep in &a.deps {
        topo_sort(dep, analyzed, visited, ordering)?;
    }
    ordering.push(name.to_string());
    Ok(())
}
