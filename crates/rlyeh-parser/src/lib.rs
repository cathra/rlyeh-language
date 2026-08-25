//! # rlyeh-parser
//!
//! Rlyeh 语言语法分析器：将 rlyeh-lexer 产生的 Token 流解析为
//! rlyeh-ast 抽象语法树。
//!
//! ## 设计要点
//!
//! - 表达式采用 Pratt 解析（优先级自高到低：路径 → 后缀/前缀 →
//!   `as` → `*` `/` `%` → `+` `-` → 移位 → 比较链 →
//!   位运算 → 逻辑运算 → 赋值）。
//! - 比较链（`0 < x < 10`）、`in` 集合/裸范围/区域归属
//!   （`x in (0..<10)` 为成员判断 `InSet`，`x in 0..<10` 为区间判断 `InRange`）、
//!   `region`、`transfer` 等 Rlyeh 核心语法均有专属节点。
//! - 嵌套泛型 `Vec<Vec<u32>>` 通过 `>>` 拆分（`pending_gt`）支持。

#![warn(missing_docs)]
#![warn(unsafe_code)]

mod actor;
mod error;
mod expr;
mod item;
mod parser;
mod pattern;
mod region;
mod stmt;
mod ty;

#[cfg(test)]
mod tests;

pub use error::ParseError;
pub use parser::Parser;

use rlyeh_ast::AstProgram;

/// 便捷函数：解析源码为完整 AST 程序
pub fn parse(source: &str) -> Result<AstProgram, ParseError> {
    Parser::new(source)?.parse_program()
}
