//! 增量编译引擎（P007 务实版）。
//!
//! 单文件编译模型下的落地范围：
//! - [`hash`]：源码哈希 + 模块接口哈希（函数签名）
//! - [`depgraph`]：依赖图（拓扑 / 受影响传播 / 循环检测），为多模块铺路
//! - [`cache`]：`.rlyeh_cache` 产物缓存（LLVM IR）+ 损坏恢复 + 过期清理
//!
//! 命中语义：源码哈希一致 → 跳过完整流水线（parse / typecheck /
//! borrowck / regionck / MIR / LIR / codegen），直接复用缓存 LLVM IR。

pub mod cache;
pub mod depgraph;
pub mod hash;
