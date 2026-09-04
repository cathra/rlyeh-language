//! # rlyeh-typecheck
//!
//! Rlyeh 语言类型检查器（MVP）。
//!
//! 将 rlyeh-ast 抽象语法树检查并展开为 rlyeh-hir 高级中间表示，
//! 核心语义：
//!
//! - **比较链**（`0 < x < 10`）：方向检查（正向 / 反向 / 混合报错），
//!   展开为低层比较运算。反向链 `0 > x > 10` 表示区间外。
//! - **`in` 集合成员判断**（`x in (0..<10)`）：集合内范围元素展开为
//!   离散成员（`x == 0 || x == 1 || ... || x == 9`），要求上下界为
//!   编译期整数常量；成员较多时保留为集合查找。
//! - **`in` 裸范围区间判断**（`x in 0..<10`）：展开为区间比较
//!   （`x >= 0 && x < 10`），端点运行时求值。
//!
//! 入口见 [`typecheck`]（AST → HIR）与 [`typecheck_source`]（源码 → HIR）。

#![warn(missing_docs)]
#![warn(unsafe_code)]

mod check_expr;
mod check_item;
mod check_stmt;
mod comparison;
mod context;
mod error;
mod in_expr;
mod types;

pub use error::TypeError;
pub use types::{FnSignature, Mutability, StructDef, Type};

use rlyeh_hir::HirProgram;
use rlyeh_lexer::Span;

/// 合成源码位置：用于编译器生成项（单态化实例等），其无对应源文件位置。
/// 这类项的 `span.start` 为 0，< `prelude_len`，故在「用户代码过滤」时被排除。
pub(crate) const DUMMY_SPAN: Span = Span {
    start: 0,
    end: 0,
    line: 0,
    col: 0,
};

pub use crate::check_item::{collect_fn_signatures, typecheck, typecheck_with_region_hints};

/// 便捷函数：解析源码并类型检查，返回 HIR。
pub fn typecheck_source(source: &str) -> Result<HirProgram, TypeError> {
    typecheck_source_with_region_hints(source, &Default::default(), 0)
}

/// 便捷函数：解析源码并类型检查，注入 L3 PGO 回灌提示（区域名 → 推荐初始容量）。
pub fn typecheck_source_with_region_hints(
    source: &str,
    region_hints: &std::collections::HashMap<String, usize>,
    prelude_len: usize,
) -> Result<HirProgram, TypeError> {
    let mut program = rlyeh_parser::parse(source).map_err(|e| TypeError::Unsupported {
        what: format!("语法错误: {e}"),
        span: e.span(),
    })?;
    // S1c：async/await 状态机 desugar（parse 后、typecheck 前，AST → AST）
    rlyeh_desugar::desugar_program(&mut program).map_err(|e| TypeError::Unsupported {
        what: e.to_string(),
        span: e.span(),
    })?;
    typecheck_with_region_hints(&program, region_hints, prelude_len)
}
