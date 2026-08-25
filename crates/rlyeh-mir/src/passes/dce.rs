//! 死代码消除 pass。
//!
//! 1. **不可达块删除**：从入口块（index 0）做图遍历，移除不可达基本块并重映射跳转目标；
//! 2. **死赋值删除**：后向活跃扫描，删除目标为临时变量（`_tN`）且此后未被使用的赋值。

use crate::{BasicBlock, Local, MirFunction, MirProgram, MirStmt, MirTerminator, MirValue};
use std::collections::HashSet;

/// 对程序执行死代码消除。
pub fn dead_code_elimination(program: &mut MirProgram) {
    for f in &mut program.functions {
        if f.is_extern {
            // extern 声明无函数体，跳过（避免空 blocks 索引越界）
            continue;
        }
        remove_unreachable_blocks(f);
        remove_dead_assignments(f);
    }
}

/// 基本块的后继块（可达性遍历用）。
fn successors(block: &BasicBlock) -> Vec<usize> {
    match &block.terminator {
        Some(MirTerminator::Jump(t)) => vec![*t],
        Some(MirTerminator::CondJump {
            then, otherwise, ..
        }) => vec![*then, *otherwise],
        Some(MirTerminator::Return(_)) | None => vec![],
    }
}

/// 删除不可达基本块并重映射跳转目标。
fn remove_unreachable_blocks(f: &mut MirFunction) {
    let n = f.blocks.len();
    let mut reachable = vec![false; n];
    let mut stack = vec![0usize];
    reachable[0] = true;
    while let Some(id) = stack.pop() {
        for succ in successors(&f.blocks[id]) {
            if !reachable[succ] {
                reachable[succ] = true;
                stack.push(succ);
            }
        }
    }

    // 旧 id -> 新 id
    let mut map = vec![usize::MAX; n];
    let mut new_blocks = Vec::new();
    for (old, ok) in reachable.iter().enumerate() {
        if *ok {
            map[old] = new_blocks.len();
            new_blocks.push(f.blocks[old].clone());
        }
    }

    // 重映射跳转目标
    for b in &mut new_blocks {
        match &mut b.terminator {
            Some(MirTerminator::Jump(t)) => *t = map[*t],
            Some(MirTerminator::CondJump {
                then, otherwise, ..
            }) => {
                *then = map[*then];
                *otherwise = map[*otherwise];
            }
            _ => {}
        }
    }
    f.blocks = new_blocks;
}

/// 收集右值中读取的局部变量。
fn collect_used(value: &MirValue, used: &mut HashSet<Local>) {
    match value {
        MirValue::Place(p) => {
            used.insert(p.clone());
        }
        MirValue::Binary { lhs, rhs, .. } => {
            collect_used(lhs, used);
            collect_used(rhs, used);
        }
        MirValue::Unary { operand, .. } => collect_used(operand, used),
        MirValue::Int(_)
        | MirValue::Float(_)
        | MirValue::String(_)
        | MirValue::Char(_)
        | MirValue::Bool(_)
        | MirValue::Unit
        | MirValue::FnRef(_) => {}
    }
}

/// 删除目标为临时变量且此后未被使用的赋值。
///
/// 活跃信息为**函数级全局**收集（而非块内局部）：if-else 合并等场景中，
/// 结果变量在 then/else 块产生、在 merge 块被读取，块内扫描会误删跨块
/// 使用的临时变量，因此先遍历整个函数收集所有被读取的变量，再逐块删除
/// 全函数未使用的临时赋值。临时变量（`_tN`）作用域不跨函数，全局收集安全。
fn remove_dead_assignments(f: &mut MirFunction) {
    // 第一遍：收集整个函数内所有被读取的变量（跨块活跃信息）
    let mut used: HashSet<Local> = HashSet::new();
    for block in &f.blocks {
        match &block.terminator {
            Some(MirTerminator::Return(Some(p))) => {
                used.insert(p.clone());
            }
            Some(MirTerminator::CondJump { cond, .. }) => {
                used.insert(cond.clone());
            }
            _ => {}
        }
        for stmt in &block.stmts {
            match stmt {
                MirStmt::Assign { value, .. } => collect_used(value, &mut used),
                MirStmt::Call { args, .. } => {
                    // 调用可能有副作用（如 `print` / `println`），参数必活跃
                    used.extend(args.iter().cloned());
                }
                MirStmt::CallIndirect { callee, args, .. } => {
                    // 间接调用可能调用任意函数（副作用未知），
                    // 函数指针与实参必活跃，否则实参赋值会被误删
                    used.insert(callee.clone());
                    used.extend(args.iter().cloned());
                }
                MirStmt::FieldGet { base, .. } => {
                    used.insert(base.clone());
                }
                MirStmt::FieldSet { base, value, .. } => {
                    used.insert(base.clone());
                    used.insert(value.clone());
                }
                MirStmt::IndexGet { base, index, .. } => {
                    used.insert(base.clone());
                    used.insert(index.clone());
                }
                MirStmt::IndexSet { base, index, value, .. } => {
                    used.insert(base.clone());
                    used.insert(index.clone());
                    used.insert(value.clone());
                }
                MirStmt::AddrOf { operand, .. } => {
                    used.insert(operand.clone());
                }
                MirStmt::DerefRead { base, .. } => {
                    used.insert(base.clone());
                }
                MirStmt::DerefWrite { base, value, .. } => {
                    used.insert(base.clone());
                    used.insert(value.clone());
                }
                _ => {}
            }
        }
    }

    // 第二遍：逐块删除目标为临时变量且全函数未使用的赋值。
    // Call / Alloc / FieldGet / FieldSet / IndexGet / IndexSet 有副作用或产出对象，永不删除。
    for block in &mut f.blocks {
        let mut new_stmts = Vec::with_capacity(block.stmts.len());
        for stmt in block.stmts.drain(..) {
            let dead = matches!(
                &stmt,
                MirStmt::Assign { target, .. } if is_temp(target) && !used.contains(target)
            );
            if !dead {
                new_stmts.push(stmt);
            }
        }
        block.stmts = new_stmts;
    }
}

/// 是否为 lowering 生成的临时变量（`_tN`）。
fn is_temp(name: &str) -> bool {
    name.starts_with("_t")
}
