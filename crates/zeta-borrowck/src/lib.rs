//! # zeta-borrowck
//!
//! Zeta 语言借用检查器（L0 静态所有权层）。
//!
//! 对类型检查后的 HIR 进行所有权验证：
//! - **use-after-move**：变量被 `transfer` 转移所有权后再次使用
//!   （Rust E0382 对应，P005 遗留缺口）；
//! - **不可变绑定赋值**：对非 `let mut` 绑定的变量赋值
//!   （Rust E0384 对应，typecheck 符号表不记录可变性，此前无人检查）；
//! - 区域块值传递例外：`transfer x out of 'r; x` 中 `x` 作为区域块尾值
//!   是所有权转出，不视为"使用"。
//!
//! ## 设计约定
//!
//! - HIR 节点不携带源码位置，错误中的 `line`/`col` 当前为占位 0，
//!   待 Span 传播后填充（与 zeta-regionck 一致）。
//! - 借用互斥规则（`&` / `&mut`）与生命周期推断依赖引用语法与类型标注，
//!   当前 MVP 阶段 typecheck 对 `&` 返回 `Unsupported`，相关检查暂不可达；
//!   `BorrowConflict` / `MoveWhileBorrowed` 为防御性变体，保留构造与 Display。
//! - 一般 move 语义（`let y = x;` 后 `x` 不可用）依赖 Copy/非 Copy 类型区分，
//!   而 HIR 无类型标注，MVP 阶段仅检查 transfer 这一明确的所有权转移路径。
//!
//! 编译流水线位置：Type Checker → **Borrow Checker** → Region Checker。

#![warn(missing_docs)]
#![warn(unsafe_code)]

mod checker;
mod error;

pub use checker::BorrowChecker;
pub use error::BorrowError;
