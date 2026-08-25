//! # rlyeh-borrowck
//!
//! Rlyeh 语言借用检查器（L0 静态所有权层）。
//!
//! 对类型检查后的 HIR 进行所有权验证：
//! - **use-after-move**：变量被 `transfer` 转移所有权后再次使用
//!   （Rust E0382 对应，P005 遗留缺口）；
//! - **不可变绑定赋值**：对非 `let mut` 绑定的变量赋值
//!   （Rust E0384 对应，typecheck 符号表不记录可变性，此前无人检查）；
//! - **借用冲突**（Rust E0502 / E0499 对应）：`&mut` 与任何活跃借用互斥、
//!   多个 `&mut` 互斥、活跃可变借用期间写入被借用变量，按 NLL 近似
//!   （借用存活范围 = 语句粒度 `born..=last_use`，`last_use` 由预扫描
//!   阶段按同一遍历顺序预先收集）；
//! - **对不可变绑定取 `&mut`**（Rust E0596 对应）：`&mut` 要求 `let mut`；
//! - **悬垂引用**（Rust E0597 对应）：局部引用逃逸出函数（尾表达式 /
//!   `return` 直接返回 `&x` 或绑定引用变量；参数来源引用允许返回）；
//! - 区域块值传递例外：`transfer x out of 'r; x` 中 `x` 作为区域块尾值
//!   是所有权转出，不视为"使用"。
//!
//! ## 设计约定
//!
//! - HIR 节点不携带源码位置，错误中的 `line`/`col` 当前为占位 0，
//!   待 Span 传播后填充（与 rlyeh-regionck 一致）。
//! - 语义有意比 Rust 宽松：读取被借用变量允许（裸指针别名是合法模式），
//!   经 `*p` 写入视为"借用使用"；仅直接赋值被借用变量触发冲突。
//!   共享借用（多个 `&`）可共存。
//! - `MoveWhileBorrowed` 为防御性变体，保留构造与 Display。
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
