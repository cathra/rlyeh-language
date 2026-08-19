//! zeta-std：Zeta 标准库（绑定层阶段）
//!
//! 依据 P009 策略，本阶段先用 Rust 实现「绑定层」，暴露给后续的 Zeta 标准库源码
//! （`*.zeta`）使用。编译器具备运行 Zeta 标准库能力后，本 crate 将逐步替换为
//! 薄封装。

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

/// 非阻塞 IO（NIO）与零拷贝传输（sendfile）
pub mod nio;
