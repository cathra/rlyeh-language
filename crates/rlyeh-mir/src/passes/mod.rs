//! 基础优化 passes。
//!
//! 当前提供的优化（按执行顺序）：
//! 1. [`constant_fold`]：常量折叠（含常量条件终止符折叠）；
//! 2. [`dead_code_elimination`]：死代码消除（不可达块 + 死赋值）；
//! 3. [`inline_small_functions`]：基础内联（单块小函数）。
//!
//! 完整流水线见 [`optimize`]。

pub mod const_fold;
pub mod dce;
pub mod inline;

pub use const_fold::constant_fold;
pub use dce::dead_code_elimination;
pub use inline::inline_small_functions;

use crate::MirProgram;

/// 标准优化流水线：常量折叠 → 死代码消除 → 基础内联 → 死代码消除（清理内联产物）。
pub fn optimize(program: &mut MirProgram) {
    constant_fold(program);
    dead_code_elimination(program);
    inline_small_functions(program);
    dead_code_elimination(program);
}
