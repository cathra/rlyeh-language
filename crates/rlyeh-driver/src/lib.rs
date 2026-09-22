//! # rlyeh-driver
//!
//! Rlyeh 编译器驱动：串联完整编译流水线
//!
//! ```text
//! 源码 → typecheck（含 parse）→ borrowck → regionck
//!      → MIR lowering + 优化 → LIR lowering → LLVM IR 文本 → clang → 可执行文件
//! ```
//!
//! 提供库级 API（[`compile_to_llvm`] / [`build_executable`] / [`run_source`]）
//! 与增量编译入口（[`IncrementalDriver`]），以及 CLI 二进制
//! （`rlyeh run` / `rlyeh build`，见 `main.rs`）。

#![warn(missing_docs)]
#![warn(unsafe_code)]

pub mod error;
pub mod incremental;
mod module;
mod stdlib;
pub mod test_runner;

use std::path::{Path, PathBuf};
use std::process::Command;

use error::DriverError;
pub use incremental::cache::CacheStats;
use incremental::cache::IncrementalCache;
use incremental::hash::{compute_interface_hash, compute_source_hash, extract_interface};
use rlyeh_borrowck::BorrowChecker;
use rlyeh_hir::{HirItem, HirProgram};
use rlyeh_lir::{LirConst, LirGlobal, LirType};
use rlyeh_regionck::RegionChecker;
use rlyeh_typecheck::{ConstValue, GlobalDecl, Type};

/// 一次编译的结果（含缓存状态，供 CLI 报告）。
#[derive(Debug, Clone)]
pub struct BuildOutcome {
    /// 生成的 LLVM IR 文本
    pub llvm: String,
    /// 是否命中增量缓存（跳过完整流水线）
    pub cache_hit: bool,
    /// 截至本次编译的缓存统计
    pub stats: CacheStats,
}

/// 执行完整流水线，返回 LLVM IR 文本（无缓存）。
pub fn compile_to_llvm(source: &str) -> Result<String, DriverError> {
    full_pipeline(source)
}

/// 编译源码为可执行文件（LLVM IR → clang 汇编 / 链接），无缓存。
///
/// `out_path` 指定产物路径（父目录需存在）。
pub fn build_executable(source: &str, out_path: &Path) -> Result<(), DriverError> {
    build_executable_with_target(source, out_path, None)
}

/// 指定 LLVM 目标 triple 构建可执行文件（`None` 为主机目标）。
pub fn build_executable_with_target(
    source: &str,
    out_path: &Path,
    target: Option<&str>,
) -> Result<(), DriverError> {
    let llvm = full_pipeline(source)?;
    assemble(&llvm, out_path, target)
}

/// 编译并运行源码，返回程序标准输出（UTF-8），无缓存。
///
/// 编译深递归程序（async/HTTP 状态机）时 Rlyeh 编译器（Rust 代码）递归较深，
/// macOS 上默认线程栈仅 ~2MB 会溢出；与 run-pass worker 线程一致（见
/// `test_runner.rs`），在 64MB 栈线程中执行编译+运行。Linux 默认 8MB 栈不受影响。
pub fn run_source(source: &str) -> Result<String, DriverError> {
    let source = source.to_string();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = temp_dir();
            let exe = dir.join("rlyeh-run");
            build_executable(&source, &exe)?;
            let out = run_exe(&exe)?;
            let _ = std::fs::remove_dir_all(&dir);
            Ok(out)
        })
        .map_err(|e| DriverError::Run(format!("无法启动编译线程: {e}")))?
        .join()
        .map_err(|_| DriverError::Run("编译线程 panic".to_string()))?
}

/// 编译入口文件（含 `module foo;` 外部模块）为 LLVM IR 文本，无缓存。
///
/// 入口文件所在目录下的 `<name>.rl` 或 `<name>/module.rl` 会被自动加载，
/// 所有模块文件合并为单一符号空间（扁平符号名 `模块名::item`）。
/// 标准库预置（`rlyeh-std/rlyeh/`，若存在）自动注入为前缀。
pub fn compile_file_to_llvm(entry: &Path) -> Result<String, DriverError> {
    let source = module::load_combined_source(entry)?;
    let (combined, prelude_len, prelude_lines) = source_with_std(source, false)?;
    full_pipeline_with_hints(
        &combined,
        &Default::default(),
        prelude_len,
        prelude_lines,
        rlyeh_typecheck::VisibilityMode::Off,
    )
}

/// 带可见性 / 标准库开关的编译入口（供测试 harness 注入 `--visibility` / `--no-std` 等
/// 用例级选项，见 `test_runner.rs` 的 `// flag:` 注释解析）。
pub fn compile_file_to_llvm_with_opts(
    entry: &Path,
    visibility: rlyeh_typecheck::VisibilityMode,
    no_std: bool,
) -> Result<String, DriverError> {
    let source = module::load_combined_source(entry)?;
    let (combined, prelude_len, prelude_lines) = source_with_std(source, no_std)?;
    full_pipeline_with_hints(
        &combined,
        &Default::default(),
        prelude_len,
        prelude_lines,
        visibility,
    )
}

/// 解析入口文件（含外部模块与标准库预置）为 AST 文本（`{:#?}` 格式化）。
///
/// 仅执行 lex + parse，不进入类型检查。用于快照测试 / 差分对拍（SH-P2-5 K4/K5）。
pub fn emit_ast(entry: &Path) -> Result<String, DriverError> {
    let source = module::load_combined_source(entry)?;
    let (combined, _, _) = source_with_std(source, false)?;
    let ast = rlyeh_parser::parse(&combined).map_err(|e| DriverError::Typecheck(e.to_string()))?;
    Ok(format!("{:#?}", ast))
}

/// 类型检查入口文件（含外部模块与标准库预置）为 HIR 文本（`{:#?}` 格式化）。
///
/// 仅执行 lex + parse + typecheck，不进入 borrowck / regionck / 代码生成。
/// 用于快照测试 / 差分对拍（SH-P2-5 K4/K5）。
pub fn emit_hir(entry: &Path) -> Result<String, DriverError> {
    let source = module::load_combined_source(entry)?;
    let (combined, prelude_len, prelude_lines) = source_with_std(source, false)?;
    let hir = rlyeh_typecheck::typecheck_source_with_region_hints(
        &combined,
        &std::collections::HashMap::new(),
        prelude_len,
        rlyeh_typecheck::VisibilityMode::Off,
    )
    .map_err(|e| DriverError::Typecheck(e.to_string_structured(prelude_len, prelude_lines)))?;
    Ok(format!("{:#?}", hir))
}

/// 解析入口文件（仅用户源码，不含标准库预置）为 AST 文本。
///
/// 与 [`emit_ast`] 不同：不拼接 std 预置，产物仅含用户源码顶层项，体积小、
/// 适合作为快照基线（SH-P2-5 K4/K5 加固）。用于差分 harness 的 `ast-user` 维度。
pub fn emit_ast_user(entry: &Path) -> Result<String, DriverError> {
    let source = module::load_combined_source(entry)?;
    let ast = rlyeh_parser::parse(&source).map_err(|e| DriverError::Typecheck(e.to_string()))?;
    Ok(format!("{:#?}", ast))
}

/// 类型检查入口文件为 HIR 文本，但仅保留用户源码项（排除 std 前缀）。
///
/// 复用 [`emit_hir`] 的完整类型检查（仍需 std 解析类型），再按 `HirItem.span`
/// 的字节偏移过滤掉落在 `prelude_len` 之前的 std 预置项，使产物体积可控、
/// 适合作为快照基线（SH-P2-5 K4/K5 加固）。用于差分 harness 的 `hir-user` 维度。
pub fn emit_hir_user(entry: &Path) -> Result<String, DriverError> {
    let source = module::load_combined_source(entry)?;
    let (combined, prelude_len, prelude_lines) = source_with_std(source, false)?;
    let hir = rlyeh_typecheck::typecheck_source_with_region_hints(
        &combined,
        &std::collections::HashMap::new(),
        prelude_len,
        rlyeh_typecheck::VisibilityMode::Off,
    )
    .map_err(|e| DriverError::Typecheck(e.to_string_structured(prelude_len, prelude_lines)))?;
    let items: Vec<HirItem> = hir
        .items
        .into_iter()
        .filter(|it| it.span.start >= prelude_len)
        .collect();
    Ok(format!("{:#?}", HirProgram { items }))
}

/// 解析源码为规范 S-表达式 AST 文本（M-M2 自举对拍 oracle）。
///
/// 与 [`emit_ast`] 的 `{:#?}` 调试 dump 不同：此处渲染为与 Rlyeh 版 parser
/// （`self-host/parser.rl`）可逐字节对齐的规范文本，便于差分对拍。
/// 当前切片（M-M2a）：整数/标识符/一元负号/括号/二元算术与比较/逻辑/位运算，
/// 括号透明（不产生节点）。
pub fn emit_ast_canonical_expr(source: &str) -> Result<String, DriverError> {
    let ast = rlyeh_parser::parse(source)
        .map_err(|e| DriverError::Typecheck(format!("parse error: {e}")))?;
    let item = ast
        .items
        .first()
        .ok_or_else(|| DriverError::Typecheck("空程序".to_string()))?;
    let expr = match item {
        rlyeh_ast::AstItem::Statement(s) => match &**s {
            rlyeh_ast::AstStmt::Expr(e) => e,
            rlyeh_ast::AstStmt::Semi(e) => e,
            _ => return Err(DriverError::Typecheck("顶层非表达式语句".to_string())),
        },
        _ => return Err(DriverError::Typecheck("顶层非语句项".to_string())),
    };
    Ok(render_expr_canonical(expr))
}

fn render_expr_canonical(e: &rlyeh_ast::AstExpr) -> String {
    match &*e.kind {
        rlyeh_ast::ExprKind::IntLiteral(v) => format!("(int {v})"),
        rlyeh_ast::ExprKind::Ident(name) => format!("(var {name})"),
        // M-M9：路径表达式 a::b::c -> (path a b c)
        rlyeh_ast::ExprKind::Path(segs) => {
            let mut s = String::from("(path");
            for seg in segs {
                s.push(' ');
                s.push_str(seg);
            }
            s.push(')');
            s
        }
        rlyeh_ast::ExprKind::BoolLiteral(b) => format!("(bool {b})"),
        rlyeh_ast::ExprKind::StringLiteral(s) => format!("(str {s})"),
        rlyeh_ast::ExprKind::Unary { op, operand } => match op {
            rlyeh_ast::UnaryOp::Neg => format!("(neg {})", render_expr_canonical(operand)),
            _ => format!("(unary-unsupported {})", render_expr_canonical(operand)),
        },
        rlyeh_ast::ExprKind::Binary { op, left, right } => {
            let sop = match op {
                rlyeh_ast::BinaryOp::Add => "add",
                rlyeh_ast::BinaryOp::Sub => "sub",
                rlyeh_ast::BinaryOp::Mul => "mul",
                rlyeh_ast::BinaryOp::Div => "div",
                rlyeh_ast::BinaryOp::Mod => "mod",
                rlyeh_ast::BinaryOp::And => "and",
                rlyeh_ast::BinaryOp::Or => "or",
                rlyeh_ast::BinaryOp::BitAnd => "bitand",
                rlyeh_ast::BinaryOp::BitOr => "bitor",
                rlyeh_ast::BinaryOp::BitXor => "bitxor",
                rlyeh_ast::BinaryOp::Shl => "shl",
                rlyeh_ast::BinaryOp::Shr => "shr",
            };
            format!(
                "(bin {sop} {} {})",
                render_expr_canonical(left),
                render_expr_canonical(right)
            )
        }
        // 比较在 AST 中为 ComparisonChain（单比较 = 2 元素 1 运算符；多比较链由后续切片处理）。
        // 切片1 仅覆盖单比较，规范格式统一为 `(bin OP ...)`，与 Rlyeh 版 parser 对齐。
        rlyeh_ast::ExprKind::ComparisonChain { elements, operators } => {
            if operators.len() == 1 {
                let name = match &operators[0] {
                    rlyeh_ast::CompareOp::Lt => "lt",
                    rlyeh_ast::CompareOp::Le => "le",
                    rlyeh_ast::CompareOp::Gt => "gt",
                    rlyeh_ast::CompareOp::Ge => "ge",
                    rlyeh_ast::CompareOp::Eq => "eq",
                    rlyeh_ast::CompareOp::Ne => "ne",
                };
                format!(
                    "(bin {name} {} {})",
                    render_expr_canonical(&elements[0]),
                    render_expr_canonical(&elements[1])
                )
            } else {
                "(cmpchain-unsupported)".to_string()
            }
        }
        rlyeh_ast::ExprKind::Return(opt) => match opt {
            Some(e) => format!("(return {})", render_expr_canonical(e)),
            None => "(return)".to_string(),
        },
        rlyeh_ast::ExprKind::Block(b) => render_block_canonical(b),
        // M-M2c：控制流（语句/表达式双形态）
        rlyeh_ast::ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            let mut s = format!(
                "(if {} {}",
                render_expr_canonical(cond),
                render_block_canonical(then_block)
            );
            if let Some(eb) = else_block {
                s.push(' ');
                s.push_str(&render_block_canonical(eb));
            }
            s.push(')');
            s
        }
        rlyeh_ast::ExprKind::While { cond, body } => {
            format!(
                "(while {} {})",
                render_expr_canonical(cond),
                render_block_canonical(body)
            )
        }
        rlyeh_ast::ExprKind::Loop { body } => {
            format!("(loop {})", render_block_canonical(body))
        }
        // M-M3a：字段访问 / 索引 / 调用 / 方法调用（表达式双形态之一）
        rlyeh_ast::ExprKind::FieldAccess { expr, field } => {
            format!("(field {} {field})", render_expr_canonical(expr))
        }
        rlyeh_ast::ExprKind::Index { expr, index } => {
            format!(
                "(index {} {})",
                render_expr_canonical(expr),
                render_expr_canonical(index)
            )
        }
        rlyeh_ast::ExprKind::Call { callee, args, .. } => {
            let mut s = format!("(call {}", render_expr_canonical(callee));
            for a in args {
                s = format!("{s} {}", render_expr_canonical(a));
            }
            s.push(')');
            s
        }
        rlyeh_ast::ExprKind::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            let mut s = format!(
                "(call (field {} {method})",
                render_expr_canonical(receiver)
            );
            for a in args {
                s = format!("{s} {}", render_expr_canonical(a));
            }
            s.push(')');
            s
        }
        // M-M3：元组字面量 / 数组字面量 / 单元类型
        rlyeh_ast::ExprKind::TupleLit(elements) => {
            let mut s = String::from("(tuple");
            for e in elements {
                s.push(' ');
                s.push_str(&render_expr_canonical(e));
            }
            s.push(')');
            s
        }
        rlyeh_ast::ExprKind::ArrayLit(elements) => {
            let mut s = String::from("(array-lit");
            for e in elements {
                s.push(' ');
                s.push_str(&render_expr_canonical(e));
            }
            s.push(')');
            s
        }
        rlyeh_ast::ExprKind::Unit => "(unit)".to_string(),
        // M-M4：范围表达式 a..<b / a...b / a<..b -> (range LOWER UPPER lower_inc upper_inc)
        rlyeh_ast::ExprKind::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => {
            let lo = match lower {
                Some(e) => render_expr_canonical(e),
                None => "?".to_string(),
            };
            let up = match upper {
                Some(e) => render_expr_canonical(e),
                None => "?".to_string(),
            };
            format!(
                "(range {} {} {} {})",
                lo,
                up,
                if *lower_inclusive { 1 } else { 0 },
                if *upper_inclusive { 1 } else { 0 },
            )
        }
        // M-M4：for 循环 -> (for PATTERN ITERATOR BODY)
        rlyeh_ast::ExprKind::For {
            pattern,
            iterator,
            body,
        } => format!(
            "(for {} {} {})",
            render_pattern_canonical(pattern),
            render_expr_canonical(iterator),
            render_block_canonical(body)
        ),
        // M-M5：match 表达式 -> (match SUBJECT (arm PATTERN [GUARD] BODY) ...)
        rlyeh_ast::ExprKind::Match { expr, arms } => {
            let mut s = format!("(match {}", render_expr_canonical(expr));
            for arm in arms {
                let pat = render_pattern_canonical(&arm.pattern);
                let mut arm_s = format!("(arm {pat}");
                if let Some(g) = &arm.guard {
                    arm_s = format!("{arm_s} {}", render_expr_canonical(g));
                }
                arm_s = format!("{arm_s} {})", render_expr_canonical(&arm.body));
                s = format!("{s} {arm_s}");
            }
            s.push(')');
            s
        }
        _ => "(unsupported)".to_string(),
    }
}

/// 解析源码为规范 S-表达式 AST 文本（M-M2b 自举对拍 oracle，程序级）。
///
/// 在 [`emit_ast_canonical_expr`]（M-M2a，单表达式）基础上扩展为整程序：
/// 渲染 `(program ...)` / `(block ...)` / `(let ...)` / `(semi ...)` / `(return ...)`
/// 等节点，与 Rlyeh 版 parser（`self-host/parser.rl` 的 `parse_program`）逐字节对齐。
/// 当前切片（M-M2b1）：块 + `let`/`let mut`/表达式语句/`return`，类型标注忽略。
pub fn emit_ast_canonical(source: &str) -> Result<String, DriverError> {
    let ast = rlyeh_parser::parse(source)
        .map_err(|e| DriverError::Typecheck(format!("parse error: {e}")))?;
    Ok(render_program_canonical(&ast))
}

fn render_program_canonical(ast: &rlyeh_ast::AstProgram) -> String {
    let mut s = String::from("(program");
    for item in &ast.items {
        match item {
            rlyeh_ast::AstItem::Statement(stmt) => {
                s.push(' ');
                s.push_str(&render_stmt_canonical(stmt));
            }
            // M-M6：函数项
            rlyeh_ast::AstItem::FnDecl(decl) => {
                s.push(' ');
                s.push_str(&render_fn_canonical(decl));
            }
            // M-M8：结构体 / 枚举声明
            rlyeh_ast::AstItem::StructDecl(decl) => {
                s.push(' ');
                s.push_str(&render_struct_decl_canonical(decl));
            }
            rlyeh_ast::AstItem::EnumDecl(decl) => {
                s.push(' ');
                s.push_str(&render_enum_decl_canonical(decl));
            }
            _ => {
                // M-M2b1 不覆盖项声明，渲染为占位以便对拍不崩溃
                s.push_str(" (item-unsupported)");
            }
        }
    }
    s.push(')');
    s
}

fn render_stmt_canonical(stmt: &rlyeh_ast::AstStmt) -> String {
    match stmt {
        rlyeh_ast::AstStmt::Let {
            pattern,
            type_anno,
            init,
            mutable,
        } => {
            let pat = render_pattern_canonical(pattern);
            let mut head = if *mutable {
                format!("(let mut {pat}")
            } else {
                format!("(let {pat}")
            };
            if let Some(ta) = type_anno {
                head = format!("{head} {}", render_type_canonical(&ta.ty));
            }
            format!("{head} {})", render_expr_canonical(init))
        }
        rlyeh_ast::AstStmt::Expr(e) => format!("(semi {})", render_expr_canonical(e)),
        rlyeh_ast::AstStmt::Semi(e) => format!("(semi {})", render_expr_canonical(e)),
        rlyeh_ast::AstStmt::Item(_) => "(item-unsupported)".to_string(),
    }
}

/// 渲染函数项为规范串（M-M6）：
/// `(fn [pub] NAME (params (param [mut] NAME TYPE) ...) [RET] BODY)`
/// - `params` 恒存在（空参数为 `(params)`）；
/// - `RET` 为返回类型节点（缺省省略）；
/// - `BODY` 为块节点（无体如 protocol 抽象方法则省略）。
fn render_fn_canonical(decl: &rlyeh_ast::AstFnDecl) -> String {
    let mut s = String::from("(fn ");
    if decl.is_pub {
        s.push_str("pub ");
    }
    s.push_str(&decl.name);
    if !decl.generics.is_empty() {
        s.push_str(" (generics");
        for g in &decl.generics {
            s.push(' ');
            s.push_str(&g.name);
        }
        s.push(')');
    }
    s.push_str(" (params");
    for p in &decl.params {
        s.push_str(" (param ");
        if p.is_mut {
            s.push_str("mut ");
        }
        s.push_str(&p.name);
        s.push(' ');
        s.push_str(&render_type_canonical(&p.type_));
        s.push(')');
    }
    s.push(')');
    if let Some(rt) = &decl.return_type {
        s.push(' ');
        s.push_str(&render_type_canonical(rt));
    }
    if let Some(b) = &decl.body {
        s.push(' ');
        s.push_str(&render_block_canonical(b));
    }
    s.push(')');
    s
}

/// 渲染结构体声明为规范串（M-M8）：`(struct [pub] NAME (field [pub] NAME TYPE) ...)`
fn render_struct_decl_canonical(decl: &rlyeh_ast::AstStructDecl) -> String {
    let mut s = String::from("(struct ");
    if decl.is_pub {
        s.push_str("pub ");
    }
    s.push_str(&decl.name);
    if !decl.generics.is_empty() {
        s.push_str(" (generics");
        for g in &decl.generics {
            s.push(' ');
            s.push_str(&g.name);
        }
        s.push(')');
    }
    for f in &decl.fields {
        s.push_str(" (field ");
        if f.is_pub {
            s.push_str("pub ");
        }
        s.push_str(&f.name);
        s.push(' ');
        s.push_str(&render_type_canonical(&f.type_));
        s.push(')');
    }
    s.push(')');
    s
}

/// 渲染枚举声明为规范串（M-M8）：`(enum [pub] NAME (variant NAME [TYPE...] | (field NAME TYPE)...) ...)`
fn render_enum_decl_canonical(decl: &rlyeh_ast::AstEnumDecl) -> String {
    let mut s = String::from("(enum ");
    if decl.is_pub {
        s.push_str("pub ");
    }
    s.push_str(&decl.name);
    if !decl.generics.is_empty() {
        s.push_str(" (generics");
        for g in &decl.generics {
            s.push(' ');
            s.push_str(&g.name);
        }
        s.push(')');
    }
    for v in &decl.variants {
        s.push_str(" (variant ");
        s.push_str(&v.name);
        if !v.tuple_fields.is_empty() {
            for t in &v.tuple_fields {
                s.push(' ');
                s.push_str(&render_type_canonical(t));
            }
        } else if !v.struct_fields.is_empty() {
            for f in &v.struct_fields {
                s.push_str(" (field ");
                if f.is_pub {
                    s.push_str("pub ");
                }
                s.push_str(&f.name);
                s.push(' ');
                s.push_str(&render_type_canonical(&f.type_));
                s.push(')');
            }
        }
        s.push(')');
    }
    s.push(')');
    s
}

/// 渲染模式为规范串：标识符 -> 裸名；`_` -> "_"；元组 -> `(tuple-pat ...)`；
/// 字面量 -> `(lit-int N)` / `(lit-bool b)` / `(lit-str s)` / `(lit-char c)`；
/// 范围 -> `(range-pat L U lower_inc upper_inc)`；或模式 -> `(or P1 P2 ...)`；
/// 剩余 -> `..`。结构体/枚举模式超出 M-M5 范围，渲染为 `(pat-unsupported)`。
fn render_pattern_canonical(p: &rlyeh_ast::AstPattern) -> String {
    match p {
        rlyeh_ast::AstPattern::Ident(n) => n.clone(),
        rlyeh_ast::AstPattern::Wildcard => "_".to_string(),
        rlyeh_ast::AstPattern::Literal(lv) => match lv {
            rlyeh_ast::LiteralValue::Int(v) => format!("(lit-int {v})"),
            rlyeh_ast::LiteralValue::Bool(b) => format!("(lit-bool {b})"),
            rlyeh_ast::LiteralValue::Str(s) => format!("(lit-str {s})"),
            rlyeh_ast::LiteralValue::Char(c) => format!("(lit-char {c})"),
            rlyeh_ast::LiteralValue::Float(f) => format!("(lit-float {f})"),
            rlyeh_ast::LiteralValue::Time { hour, minute, is_pm } => {
                format!("(lit-time {hour} {minute} {is_pm})")
            }
        },
        rlyeh_ast::AstPattern::Tuple(elems, _) => {
            let mut s = "(tuple-pat".to_string();
            for e in elems {
                s = format!("{s} {}", render_pattern_canonical(e));
            }
            s.push(')');
            s
        }
        rlyeh_ast::AstPattern::Range {
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        } => {
            let l = render_expr_canonical(lower);
            let u = render_expr_canonical(upper);
            format!(
                "(range-pat {} {} {} {})",
                l,
                u,
                if *lower_inclusive { 1 } else { 0 },
                if *upper_inclusive { 1 } else { 0 },
            )
        }
        rlyeh_ast::AstPattern::Or(alts) => {
            let mut s = "(or".to_string();
            for a in alts {
                s = format!("{s} {}", render_pattern_canonical(a));
            }
            s.push(')');
            s
        }
        rlyeh_ast::AstPattern::Rest => "..".to_string(),
        _ => "(pat-unsupported)".to_string(),
    }
}

/// 渲染类型为规范串（M-M2b2）：路径 `(type NAME ARG...)` / 引用 `(ref ...)` /
/// 元组类型 `(tuple-type ...)` / 数组 `(array T N)` / 推断 `(infer)`。
fn render_type_canonical(t: &rlyeh_ast::AstType) -> String {
    match t {
        rlyeh_ast::AstType::Path(name, args) => {
            let mut s = format!("(type {name}");
            for a in args {
                s = format!("{s} {}", render_type_canonical(a));
            }
            s.push(')');
            s
        }
        rlyeh_ast::AstType::Ref(inner, is_mut, _) => {
            if *is_mut {
                format!("(ref mut {})", render_type_canonical(inner))
            } else {
                format!("(ref {})", render_type_canonical(inner))
            }
        }
        rlyeh_ast::AstType::Tuple(ts) => {
            let mut s = "(tuple-type".to_string();
            for a in ts {
                s = format!("{s} {}", render_type_canonical(a));
            }
            s.push(')');
            s
        }
        rlyeh_ast::AstType::Array(inner, len_opt) => {
            let n = match len_opt {
                Some(e) => render_expr_canonical(e),
                None => "?".to_string(),
            };
            format!("(array {} {n})", render_type_canonical(inner))
        }
        rlyeh_ast::AstType::Infer => "(infer)".to_string(),
        _ => "(type-unsupported)".to_string(),
    }
}

fn render_block_canonical(block: &rlyeh_ast::AstBlock) -> String {
    let mut s = String::from("(block");
    for stmt in &block.stmts {
        s.push(' ');
        s.push_str(&render_stmt_canonical(stmt));
    }
    if let Some(fe) = &block.final_expr {
        s.push(' ');
        s.push_str(&render_expr_canonical(fe));
    }
    s.push(')');
    s
}

/// 词法分析入口文件为 Token 文本（每行一个规范化 token，供 M-M1 自举对拍）。
///
/// 仅 lex 用户源码（不含标准库预置），与 Rlyeh 版 lexer（`self-host/lexer.rl`）
/// 对拍时输入同源。规范格式见 `docs/tasks/self-hosting/self-host-lexer.md`。
pub fn emit_tokens(entry: &Path) -> Result<String, DriverError> {
    let source = module::load_combined_source(entry)?;
    emit_tokens_str(&source)
}

/// 直接对源码字符串做词法分析，返回规范化 token 文本（每行一个 token）。
///
/// 供 `tests/` 对拍 harness 直接对内存字符串做 oracle 比对，无需落盘。
pub fn emit_tokens_str(source: &str) -> Result<String, DriverError> {
    let mut lexer = rlyeh_lexer::Lexer::new(source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| DriverError::Typecheck(format!("lex error: {e}")))?;
    let lines: Vec<String> = tokens.iter().map(|t| token_to_canonical(&t.token)).collect();
    Ok(lines.join("\n"))
}

/// 将 [`rlyeh_lexer::Token`] 规范化为对拍用的单行文本。
///
/// 与 Rlyeh 版 `self-host/lexer.rl` 输出的格式必须逐行一致；新增 token 种类时
/// 两侧须同步（见 `docs/tasks/self-hosting/self-host-lexer.md` 的规范）。
fn token_to_canonical(t: &rlyeh_lexer::Token) -> String {
    use rlyeh_lexer::Token::*;
    match t {
        Let => "let".into(),
        Mut => "mut".into(),
        Const => "const".into(),
        Static => "static".into(),
        Fn => "fn".into(),
        Return => "return".into(),
        Pub => "pub".into(),
        Priv => "priv".into(),
        If => "if".into(),
        Else => "else".into(),
        Match => "match".into(),
        For => "for".into(),
        While => "while".into(),
        Loop => "loop".into(),
        Break => "break".into(),
        Continue => "continue".into(),
        Try => "try".into(),
        True => "true".into(),
        False => "false".into(),
        And => "and".into(),
        Or => "or".into(),
        Not => "not".into(),
        Struct => "struct".into(),
        Enum => "enum".into(),
        Impl => "impl".into(),
        Protocol => "protocol".into(),
        Type => "type".into(),
        Where => "where".into(),
        SelfKw => "Self".into(),
        Region => "region".into(),
        GcRegion => "gc_region".into(),
        In => "in".into(),
        Transfer => "transfer".into(),
        Out => "out".into(),
        Of => "of".into(),
        Unsafe => "unsafe".into(),
        Actor => "actor".into(),
        Async => "async".into(),
        Await => "await".into(),
        Spawn => "spawn".into(),
        Send => "send".into(),
        Recv => "recv".into(),
        Mod => "mod".into(),
        Use => "use".into(),
        As => "as".into(),
        Extern => "extern".into(),
        Dyn => "dyn".into(),
        Ident(s) => format!("IDENT {}", s),
        IntLiteral(v) => format!("INT {}", v),
        FloatLiteral { value: _, raw } => format!("FLOAT {}", raw),
        StringLiteral(s) => format!("STR {}", s),
        CharLiteral(c) => format!("CHAR {}", c),
        BoolLiteral(b) => format!("BOOL {}", b),
        Lifetime(s) => format!("LIFETIME {}", s),
        TimeLiteral { hour, minute, .. } => format!("TIME {}:{}", hour, minute),
        Plus => "+".into(),
        Minus => "-".into(),
        Star => "*".into(),
        Slash => "/".into(),
        Percent => "%".into(),
        Eq => "==".into(),
        Ne => "!=".into(),
        Lt => "<".into(),
        Le => "<=".into(),
        Gt => ">".into(),
        Ge => ">=".into(),
        AndAnd => "&&".into(),
        OrOr => "||".into(),
        NotNot => "!".into(),
        BitAnd => "&".into(),
        BitOr => "|".into(),
        BitXor => "^".into(),
        Shl => "<<".into(),
        Shr => ">>".into(),
        Assign => "=".into(),
        PlusEq => "+=".into(),
        MinusEq => "-=".into(),
        StarEq => "*=".into(),
        SlashEq => "/=".into(),
        PercentEq => "%=".into(),
        Range => "RANGE".into(),
        DotDotLt => "DOTDOTLT".into(),
        DotDotDot => "DOTDOTDOT".into(),
        LtDotDot => "LTDOTDOT".into(),
        Arrow => "->".into(),
        FatArrow => "=>".into(),
        LParen => "(".into(),
        RParen => ")".into(),
        LBrace => "{".into(),
        RBrace => "}".into(),
        LBracket => "[".into(),
        RBracket => "]".into(),
        Comma => ",".into(),
        Colon => ":".into(),
        Semicolon => ";".into(),
        Dot => ".".into(),
        At => "@".into(),
        Dollar => "$".into(),
        Pound => "#".into(),
        Question => "?".into(),
        NotIn => "NOTIN".into(),
        Eof => "EOF".into(),
    }
}

/// 编译入口文件（含外部模块）为可执行文件，无缓存。
pub fn build_executable_file(entry: &Path, out_path: &Path) -> Result<(), DriverError> {
    build_executable_file_with_target(entry, out_path, None)
}

/// 指定 LLVM 目标 triple 编译入口文件（含外部模块）并构建可执行文件。
pub fn build_executable_file_with_target(
    entry: &Path,
    out_path: &Path,
    target: Option<&str>,
) -> Result<(), DriverError> {
    let llvm = compile_file_to_llvm(entry)?;
    assemble(&llvm, out_path, target)
}

/// 编译并运行入口文件（含外部模块），返回程序标准输出（UTF-8），无缓存。
///
/// 编译深递归程序（async/HTTP 状态机）时 Rlyeh 编译器（Rust 代码）递归较深，
/// macOS 上默认线程栈仅 ~2MB 会溢出；与 run-pass worker 线程一致（见
/// `test_runner.rs`），在 64MB 栈线程中执行编译+运行。Linux 默认 8MB 栈不受影响。
pub fn run_source_file(entry: &Path) -> Result<String, DriverError> {
    let entry = entry.to_path_buf();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = temp_dir();
            let exe = dir.join("rlyeh-run");
            build_executable_file(&entry, &exe)?;
            let out = run_exe(&exe)?;
            let _ = std::fs::remove_dir_all(&dir);
            Ok(out)
        })
        .map_err(|e| DriverError::Run(format!("无法启动编译线程: {e}")))?
        .join()
        .map_err(|_| DriverError::Run("编译线程 panic".to_string()))?
}

/// 读取源码文件并执行静态检查（`rlyeh check`）。
///
/// 解析失败返回 `parse-error` 诊断；否则返回 lint 规则诊断。
/// 仅负责分析，不涉及完整编译流水线。
pub fn check_source_file(path: &Path) -> Result<Vec<rlyeh_check::Diagnostic>, DriverError> {
    let source = std::fs::read_to_string(path).map_err(DriverError::Io)?;
    Ok(rlyeh_check::check_source(&source))
}

/// 读取源码文件并生成 Markdown 文档（`rlyeh doc`）。
pub fn doc_source_file(path: &Path, options: &rlyeh_doc::DocOptions) -> Result<String, DriverError> {
    let source = std::fs::read_to_string(path).map_err(DriverError::Io)?;
    rlyeh_doc::doc_source(&source, options).map_err(DriverError::Doc)
}

/// 读取 PGO 画像文件并生成区域大小预测报告（`rlyeh profile`，阶段 F2 数据回灌消费侧）。
///
/// `.rl_profile` 为 JSON 格式（见 `rlyeh_region_alloc::profile`）：
/// 运行时按区域记录的分配量统计（p50/p90/p95/mean/max 等）。
/// 本函数加载画像 → 以 p95×安全系数预测初始区域大小 → 输出编译决策报告
/// （复用 `rlyeh_region_alloc::PgoAdvisor` / `CompilerInterface`）。
/// 语言级 region 接线后，该预测可直接回灌 `region 'r adaptive` 的初始容量。
pub fn region_profile_report(path: &Path) -> Result<String, DriverError> {
    let text = std::fs::read_to_string(path).map_err(DriverError::Io)?;
    let data: rlyeh_region_alloc::PgoData = serde_json::from_str(&text)
        .map_err(|e| DriverError::Profile(format!("解析 `.rl_profile` 失败: {e}")))?;
    Ok(build_region_report(&data))
}

/// 读取 `.rl_profile`，为每个区域生成 L3 PGO 回灌提示（区域名 → 推荐初始容量）。
///
/// 推荐容量 = `PgoAdvisor::recommend_size`（p95 × 安全系数，下限 64KiB）。
/// 注入到 `adaptive` 区域后，`rlyeh_region_enter` 以该容量创建初始块。
pub fn region_hints_from_profile(
    path: &Path,
) -> Result<std::collections::HashMap<String, usize>, DriverError> {
    let text = std::fs::read_to_string(path).map_err(DriverError::Io)?;
    let data: rlyeh_region_alloc::PgoData = serde_json::from_str(&text)
        .map_err(|e| DriverError::Profile(format!("解析 `.rl_profile` 失败: {e}")))?;
    let advisor = rlyeh_region_alloc::PgoAdvisor::from_data(data.clone());
    let mut hints = std::collections::HashMap::new();
    for id in data.region_ids() {
        if let Some(size) = advisor.recommend_size(&id) {
            hints.insert(id, size);
        }
    }
    Ok(hints)
}

/// 依据 PGO 数据生成区域分配决策报告（纯函数，便于单元测试）。
///
/// 每个区域：`estimated_size` = 历史平均分配量（静态基线）、
/// `initial_size` = PGO 推荐（p95×安全系数，下限 64KiB）、
/// `max_size` = 推荐×4 与历史峰值取大、`decision` 描述依据。
pub fn build_region_report(data: &rlyeh_region_alloc::PgoData) -> String {
    let advisor = rlyeh_region_alloc::PgoAdvisor::from_data(data.clone());
    let mut interface =
        rlyeh_region_alloc::CompilerInterface::new().with_pgo_data(data.clone());
    let mut ids = data.region_ids();
    ids.sort_unstable();
    for id in &ids {
        let region = data
            .region_profile(id)
            .expect("region id from region_ids() must exist");
        let initial = advisor
            .recommend_size(id)
            .unwrap_or(region.size_stats.mean);
        let max = (initial * 4).max(region.size_stats.max);
        let decision = format!(
            "PGO p95 {}B × safety 1.1（下限 64KiB）；p50 {}B / mean {}B / max {}B",
            region.size_stats.p95, region.size_stats.p50, region.size_stats.mean,
            region.size_stats.max
        );
        interface = interface.register_region(rlyeh_region_alloc::RegionCompileInfo {
            region_id: id.to_string(),
            estimated_size: region.size_stats.mean,
            initial_size: initial,
            max_size: max,
            decision,
        });
    }
    interface.generate_report()
}

/// 增量编译驱动：源码哈希命中时跳过完整流水线，直接复用缓存 LLVM IR。
///
/// 缓存目录为 `<cache_dir>/.rlyeh_cache`；`--force` 语义下强制全量重编译。
pub struct IncrementalDriver {
    /// 缓存根（`.rlyeh_cache` 的父目录）
    cache_dir: PathBuf,
    /// 强制全量重编译（忽略缓存）
    force: bool,
    /// 禁用标准库预置注入（`--no-std`）
    no_std: bool,
    /// LLVM 目标 triple（`--target` 交叉编译；`None` = 主机目标）
    target: Option<String>,
    /// 会话内缓存统计
    stats: CacheStats,
    /// L3 PGO 回灌：区域名 → 推荐初始容量（`rlyeh build --profile` 注入）
    region_hints: std::collections::HashMap<String, usize>,
    /// 标准库预置（prelude）字节长度（含末尾换行），用于 E3 extern 调用门禁豁免 std
    prelude_len: usize,
    /// 标准库预置（prelude）行数：用户源码前的偏移行数，用于 L1 诊断行号还原
    ///（`Span.line` 为合并源码行号，减去本值得到用户文件行号，SH-P2-6）。
    prelude_lines: usize,
    /// 可见性检查模式（P2）：`--visibility` 控制，默认 `Off`（兼容存量代码 / 标准库）。
    visibility: rlyeh_typecheck::VisibilityMode,
    /// 第三方依赖根（`--dep-root pkg=dir`，可多段）：dagon 包集成（P4）注入为扁平名字空间。
    dep_roots: Vec<(String, PathBuf)>,
}

impl IncrementalDriver {
    /// 创建增量编译驱动。
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            force: false,
            no_std: false,
            target: None,
            stats: CacheStats::default(),
            region_hints: std::collections::HashMap::new(),
            prelude_len: 0,
            prelude_lines: 0,
            visibility: rlyeh_typecheck::VisibilityMode::Off,
            dep_roots: Vec::new(),
        }
    }

    /// 注入 L3 PGO 回灌提示（区域名 → 推荐初始容量），
    /// 供 `adaptive` 区域在编译期采用推荐容量。
    pub fn with_region_hints(mut self, hints: std::collections::HashMap<String, usize>) -> Self {
        self.region_hints = hints;
        self
    }

    /// 强制全量重编译（忽略缓存）。
    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    /// 禁用标准库预置注入（文件入口 API 默认注入标准库）。
    pub fn with_no_std(mut self, no_std: bool) -> Self {
        self.no_std = no_std;
        self
    }

    /// 指定 LLVM 目标 triple（交叉编译；`None` 为主机目标）。
    pub fn with_target(mut self, target: Option<String>) -> Self {
        self.target = target;
        self
    }

    /// 设定可见性检查模式（P2，默认 `Off`）。
    pub fn with_visibility(mut self, visibility: rlyeh_typecheck::VisibilityMode) -> Self {
        self.visibility = visibility;
        self
    }

    /// 注入第三方依赖根（dagon 包集成 P4）：`--dep-root pkg=dir` 可多段。
    pub fn with_dep_roots(mut self, dep_roots: Vec<(String, PathBuf)>) -> Self {
        self.dep_roots = dep_roots;
        self
    }

    /// 会话内缓存统计。
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// 增量编译源码，返回 LLVM IR（命中缓存或全量编译后写入缓存）。
    ///
    /// `file` 是缓存键（通常为源文件路径）。
    pub fn compile_to_llvm(
        &mut self,
        file: &str,
        source: &str,
    ) -> Result<BuildOutcome, DriverError> {
        let mut cache = IncrementalCache::open(&self.cache_dir)?;
        if cache.was_recovered() {
            self.stats.recovered += 1;
        }

        let source_hash = compute_source_hash(source);

        // 1. 缓存查找：源码哈希一致 + 产物存在 → 直接复用
        if !self.force {
            if let Some(llvm) = cache.lookup_llvm(file, &source_hash)? {
                self.stats.hits += 1;
                return Ok(BuildOutcome {
                    llvm,
                    cache_hit: true,
                    stats: self.stats.clone(),
                });
            }
        }

        // 2. 全量编译 + 写入缓存
        self.stats.misses += 1;
        // D（P4）：字符串源码路径同样注入 `--dep-root` 依赖，使 `dep_roots`
        // 在所有编译路径一致生效（磁盘路径见 `compile_file_to_llvm`）。
        let source = module::inject_dep_roots(source, &self.dep_roots)?;
        let llvm = full_pipeline_with_hints(
            &source,
            &self.region_hints,
            self.prelude_len,
            self.prelude_lines,
            self.visibility,
        )?;
        let interface = extract_interface(&source)?;
        let interface_hash = compute_interface_hash(&interface);
        cache.store_llvm(file, &source_hash, &interface_hash, &llvm)?;
        let _ = cache.clean_stale();

        Ok(BuildOutcome {
            llvm,
            cache_hit: false,
            stats: self.stats.clone(),
        })
    }

    /// 增量编译并构建可执行文件。
    pub fn build_executable(
        &mut self,
        file: &str,
        source: &str,
        out_path: &Path,
    ) -> Result<BuildOutcome, DriverError> {
        let outcome = self.compile_to_llvm(file, source)?;
        assemble(&outcome.llvm, out_path, self.target.as_deref())?;
        Ok(outcome)
    }

    /// 增量编译、构建并运行，返回程序标准输出。
    pub fn run_source(
        &mut self,
        file: &str,
        source: &str,
    ) -> Result<(String, BuildOutcome), DriverError> {
        let dir = temp_dir();
        let exe = dir.join("rlyeh-run");
        let outcome = self.build_executable(file, source, &exe)?;
        let stdout = run_exe(&exe)?;
        let _ = std::fs::remove_dir_all(&dir);
        Ok((stdout, outcome))
    }

    /// 增量编译入口文件（含外部模块），返回 LLVM IR。
    ///
    /// 缓存键为入口文件路径；组合源码哈希覆盖全部模块文件与标准库预置，
    /// 任一模块/标准库变更都会触发重新编译。
    pub fn compile_file_to_llvm(&mut self, entry: &Path) -> Result<BuildOutcome, DriverError> {
        // 解析入口文件（含外部模块）为组合源码，D：注入 `--dep-root` 依赖
        let mut source = module::load_combined_source(entry)?;
        source = module::inject_dep_roots(&source, &self.dep_roots)?;
        let (combined, prelude_len, prelude_lines) =
            source_with_std(source, self.no_std)?;
        self.prelude_len = prelude_len;
        self.prelude_lines = prelude_lines;
        let key = entry.to_string_lossy().to_string();
        self.compile_to_llvm(&key, &combined)
    }

    /// 增量编译入口文件（含外部模块）并构建可执行文件。
    pub fn build_executable_file(
        &mut self,
        entry: &Path,
        out_path: &Path,
    ) -> Result<BuildOutcome, DriverError> {
        let outcome = self.compile_file_to_llvm(entry)?;
        assemble(&outcome.llvm, out_path, self.target.as_deref())?;
        Ok(outcome)
    }

    /// 增量编译入口文件（含外部模块）、构建并运行，返回程序标准输出。
    pub fn run_source_file(
        &mut self,
        entry: &Path,
    ) -> Result<(String, BuildOutcome), DriverError> {
        let dir = temp_dir();
        let exe = dir.join("rlyeh-run");
        let outcome = self.build_executable_file(entry, &exe)?;
        let stdout = run_exe(&exe)?;
        let _ = std::fs::remove_dir_all(&dir);
        Ok((stdout, outcome))
    }
}

/// 组合入口源码与标准库预置（`--no-std` 时原样返回，预置长度记 0）。
/// 返回 `(combined_source, prelude_len)`：`prelude_len` 为预置源码字节长度
/// （含拼接用的换行），用于 E3 extern 调用门禁豁免 std 内部 FFI。
fn source_with_std(source: String, no_std: bool) -> Result<(String, usize, usize), DriverError> {
    if no_std {
        return Ok((source, 0, 0));
    }
    Ok(match stdlib::load_std_prelude()? {
        Some(prelude) => {
            let len = prelude.len() + 1; // 含拼接用的 '\n'
            // 预置行数 = 预置内换行数 + 1（拼接的 '\n'）：用户源码从下一行第 1 列起，
            // 故用户行号 = 合并行号 - prelude_lines（L1 诊断对齐用，SH-P2-6）。
            let lines = prelude.matches('\n').count() + 1;
            (format!("{prelude}\n{source}"), len, lines)
        }
        None => (source, 0, 0),
    })
}

/// 完整流水线：typecheck → borrowck → regionck → MIR(+优化) → LIR → LLVM IR。
fn full_pipeline(source: &str) -> Result<String, DriverError> {
    full_pipeline_with_hints(
        source,
        &std::collections::HashMap::new(),
        0,
        0,
        rlyeh_typecheck::VisibilityMode::Off,
    )
}

/// 完整流水线，注入 L3 PGO 回灌提示（区域名 → 推荐初始容量）。
/// 将 typecheck 收集的 `GlobalDecl` 转换为 LIR 全局（类型 → LIR 类型）。
///
/// MVP 仅支持标量类型：整数族（≤128 位，含有/无符号与平台宽）按 ABI 统一承载于
/// `I64` 槽；浮点族 `f32`/`f64` 承载于 `F64` 槽；`bool`/`char` 对应 `Bool`/`Char`。
/// 字符串 / 引用 / 聚合类型等非常量数据段全局暂不支持，返回 `None`（跳过），
/// 后续可补常量数据段发射。
fn convert_global_decl(g: &GlobalDecl) -> Option<LirGlobal> {
    let type_ = match &g.type_ {
        Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::I128
        | Type::ISize | Type::U8 | Type::U16 | Type::U32 | Type::U64
        | Type::U128 | Type::USize => LirType::I64,
        Type::F32 | Type::F64 => LirType::F64,
        Type::Bool => LirType::Bool,
        Type::Char => LirType::Char,
        _ => return None,
    };
    let init = match &g.init {
        ConstValue::I64(v) => LirConst::I64(*v),
        ConstValue::F64(v) => LirConst::F64(*v),
        ConstValue::Bool(v) => LirConst::Bool(*v),
        ConstValue::Char(v) => LirConst::Char(*v),
    };
    Some(LirGlobal {
        name: g.name.clone(),
        type_,
        is_mut: g.is_mut,
        init,
    })
}

fn full_pipeline_with_hints(
    source: &str,
    region_hints: &std::collections::HashMap<String, usize>,
    prelude_len: usize,
    prelude_lines: usize,
    visibility: rlyeh_typecheck::VisibilityMode,
) -> Result<String, DriverError> {
    // 1. 类型检查（内部完成 lex + parse → HIR），并收集建议性警告与全局声明
    let (hir, warnings, globals) = rlyeh_typecheck::typecheck_source_with_warnings(
        source,
        region_hints,
        prelude_len,
        visibility,
    )
    .map_err(|e| DriverError::Typecheck(e.to_string_structured(prelude_len, prelude_lines)))?;

    // 打印建议性警告（借用简化 RFC B-1：冗余 `*` 等），不阻断编译
    for w in &warnings {
        eprintln!("{}", w.to_string_with_offset(prelude_len, prelude_lines));
    }

    // 2. 借用检查（L0 所有权）
    BorrowChecker::new()
        .check_program(&hir)
        .map_err(|errs| DriverError::Borrow(join_errors(errs, prelude_lines)))?;

    // 3. 区域检查（L1 区域系统）
    RegionChecker::new()
        .check_program(&hir)
        .map_err(|errs| DriverError::Region(join_errors(errs, prelude_lines)))?;

    // 4. MIR lowering + 优化
    let mut mir = rlyeh_mir::lower::lower_program(&hir);
    rlyeh_mir::passes::optimize(&mut mir);

    // 5. LIR lowering
    let mut lir = rlyeh_lir::lower::lower_program(&mir).map_err(|e| DriverError::Lir(e.to_string()))?;
    // 5b. 挂载全局变量（`static` / `static mut`）声明：typecheck 收集的 GlobalDecl
    //     经类型→LIR 类型转换后注入 LIR 程序，最终由 codegen 发射为 data 段符号。
    lir.globals = globals.iter().filter_map(convert_global_decl).collect();

    // 6. LLVM IR 文本生成
    rlyeh_codegen::generate_llvm(&lir).map_err(|e| DriverError::Codegen(e.to_string()))
}

/// LLVM IR 文本 → clang 汇编 / 链接 → 可执行文件。
///
/// `target` 为 LLVM 目标 triple（`--target` 交叉编译）；`None` 表示主机目标。
fn assemble(llvm: &str, out_path: &Path, target: Option<&str>) -> Result<(), DriverError> {
    // 输出路径的父目录必须存在（链接器无法创建）
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(DriverError::Io)?;
    }
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).map_err(DriverError::Io)?;
    let ll_path = dir.join("main.ll");
    // 注入平台内建（`__rlyeh_target_os`）后写盘——codegen 对 `__rlyeh_` 前缀 extern 不生成 declare。
    let llvm_with_builtins = format!("{llvm}\n{}", platform_builtin_ir(target, &llvm));
    // L4b: wasm 目标注入 actor 符号解析表（静态查表替代 dlsym，见 actor_resolve_ir）。
    let llvm_final = match target {
        Some(t) if is_wasm_triple(t) => {
            let actor_ir = actor_resolve_ir(&llvm_with_builtins);
            if actor_ir.is_empty() {
                llvm_with_builtins
            } else {
                format!("{llvm_with_builtins}\n{actor_ir}")
            }
        }
        _ => llvm_with_builtins,
    };
    std::fs::write(&ll_path, llvm_final).map_err(DriverError::Io)?;

    // WebAssembly 目标走独立汇编链路（wasi-libc sysroot + wasm-ld），其余走系统链接器。
    if let Some(t) = target {
        if is_wasm_triple(t) {
            return assemble_wasm(&ll_path, out_path, t, &dir);
        }
        // Y（2026-08-28）：跨 OS ELF 目标（Linux/RISC-V/LoongArch musl）——宿主
        // clang 的 ld 无法链接跨 OS 目标（macOS ld 不认识 `--hash-style` 等），
        // 改用 rust-lld（多目标链接器）+ Rust musl sysroot 的 crt/libc 链接。
        if is_elf_musl_target(t) && is_cross_target(target) {
            return assemble_cross_elf(&ll_path, out_path, t, &dir);
        }
    }

    let clang = clang_path();
    // 运行时 C ABI（staticlib）：Rlyeh 程序经 `extern fn rlyeh_actor_*` /
    // `extern fn rlyeh_gc_*` 调用。链接器按需提取对象——不含相应特性的程序不受影响
    // （库可缺失则跳过）。交叉编译到其他架构时本机 staticlib 无法链接，跳过并提示。
    let runtime_libs: Vec<PathBuf> = if is_cross_target(target) {
        eprintln!(
            "rlyeh: 提示: 交叉编译目标 `{}` 与主机架构不同，跳过 Actor / GC 运行时库（相关特性暂不支持交叉编译）",
            target.unwrap_or_default()
        );
        Vec::new()
    } else {
        [actor_runtime_lib_path(), gc_runtime_lib_path(), region_runtime_lib_path()]
            .into_iter()
            .flatten()
            .collect()
    };
    let mut cmd = Command::new(&clang);
    if let Some(t) = target {
        cmd.arg(format!("--target={t}"));
    }
    // 发布级优化：clang 汇编 LLVM IR 时启用 -O3 优化管线
    // （-O2 全部优化 + 更强的循环向量化 / 函数内联 / 循环变换）。
    // 生成的 IR 不含 noalias/nsw/nuw 标注，LLVM 的优化保守安全
    // （-O3 不依赖这些标注，无错误别名假设）。
    cmd.arg("-O3");
    cmd.arg(&ll_path);
    for lib in &runtime_libs {
        if let Some(parent) = lib.parent() {
            cmd.arg("-L").arg(parent);
        }
        cmd.arg("-l").arg(
            lib.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.trim_start_matches("lib").trim_end_matches(".a"))
                .unwrap_or("rlyeh_runtime"),
        );
    }
    cmd.arg("-o").arg(out_path);
    let result = cmd.output().map_err(|e| {
        DriverError::Clang(format!("无法启动 `{clang}`: {e}"))
    })?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        return Err(DriverError::Clang(stderr));
    }
    // 调试：设置 RLYEH_KEEP_TMP=1 时保留临时目录（含 main.ll 中间 IR）
    if std::env::var("RLYEH_KEEP_TMP").is_err() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    Ok(())
}

/// 指定目标 triple 是否为 WebAssembly（`wasm32` / `wasm64`）。
mod link;
mod platform_ir;
mod util;

use platform_ir::*;

// 汇编链接与工具链探测已下沉至 `link`（文件大小约束：单个文件 ≤1000 行）；
// 其中对外 API 在此显式 re-export，保证调用点路径不变。
pub use link::{
    host_triple, is_cross_target, is_elf_musl_target, is_wasm_triple, target_arch, target_os_code,
};
use link::*;
use util::*;