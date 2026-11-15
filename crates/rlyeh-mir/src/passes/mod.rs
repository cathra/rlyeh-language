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

use crate::{MirProgram, OptLevel};

/// 按优化级别执行 MIR 流水线。
///
/// - O0：不执行任何 MIR pass；
/// - O1：常量折叠 → 死代码消除；
/// - O2/O3：O1 后执行基础内联，再 DCE 清理内联产物（O3 的额外强度由 clang -O3 承担）；
/// - Os：仅常量折叠 + DCE（大小优先）。
pub fn optimize(program: &mut MirProgram, level: OptLevel) {
    if !level.mir_basic() {
        return;
    }
    constant_fold(program);
    dead_code_elimination(program);
    if level.mir_inline() {
        inline_small_functions(program);
        dead_code_elimination(program);
    }
}
