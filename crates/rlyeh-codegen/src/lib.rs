//! # zeta-codegen
//!
//! Zeta 代码生成：LIR → 后端指令。
//!
//! MVP 提供 **LLVM IR 文本后端**（见 [`llvm::generate_llvm`]）：
//! 输出人类可读的 `.ll` 模块，经系统 `clang` 汇编 / 链接为可执行文件。
//! 开发期也可配合 `clang -S` 检查生成质量。后续可在此扩展
//! Cranelift / WASM 后端。

#![warn(missing_docs)]
#![warn(unsafe_code)]

pub mod error;
mod llvm;

pub use error::CodegenError;
pub use llvm::generate_llvm;
