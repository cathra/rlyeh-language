//! 基础内联 pass。
//!
//! 将**单块、无区域操作、终止符为 return** 的小函数在调用点展开：
//! 参数按位置替换为实参，返回变量替换为调用点目标。
//!
//! 限制（MVP）：
//! - 不内联递归函数（直接递归通过 `callee != 当前函数` 排除）；
//! - 含 `RegionEnter` / `RegionExit` / `AllocInRegion` / `Transfer` 的函数不内联；
//! - 单遍展开（内联副本内部的调用不继续展开）。

use crate::{Local, MirFunction, MirProgram, MirStmt, MirTerminator, MirValue};
use std::collections::{HashMap, HashSet};

/// 对程序执行基础内联。
pub fn inline_small_functions(program: &mut MirProgram) {
    // 收集可内联函数名
    let inlineable: HashSet<String> = program
        .functions
        .iter()
        .filter(|f| is_inlineable(f))
        .map(|f| f.name.clone())
        .collect();
    if inlineable.is_empty() {
        return;
    }

    // 预取可内联函数体（克隆），避免遍历 `&mut program.functions` 时再借用 program
    let inline_bodies: Vec<(String, MirFunction)> = program
        .functions
        .iter()
        .filter(|cf| inlineable.contains(&cf.name))
        .map(|cf| (cf.name.clone(), cf.clone()))
        .collect();

    let mut counter = 0usize;
    for f in &mut program.functions {
        let fname = f.name.clone();
        for block in &mut f.blocks {
            let mut new_stmts = Vec::with_capacity(block.stmts.len());
            for stmt in &block.stmts {
                if let MirStmt::Call { callee, args, .. } = stmt {
                    if inlineable.contains(callee) && callee != &fname {
                        let callee_fn = inline_bodies
                            .iter()
                            .find(|(n, _)| n == callee)
                            .expect("inlineable 集合来自同一程序")
                            .1
                            .clone();
                        // 展开函数体
                        let mut subst: HashMap<String, String> = HashMap::new();
                        for (p, a) in callee_fn.params.iter().zip(args.iter()) {
                            subst.insert(p.clone(), a.clone());
                        }
                        for s in &callee_fn.blocks[0].stmts {
                            if let Some(inlined) = inline_stmt(s, &mut subst, &mut counter) {
                                new_stmts.push(inlined);
                            }
                        }
                        // 返回值绑定到调用点目标
                        match &callee_fn.blocks[0].terminator {
                            Some(MirTerminator::Return(Some(p))) => {
                                let mapped = map_local(p, &mut subst, &mut counter);
                                bind_call_result(stmt, &mut new_stmts, &mapped);
                            }
                            // 无返回值（`return;`）：目标绑定为 Unit
                            _ => {
                                if let MirStmt::Call {
                                    target: Some(t), ..
                                } = stmt
                                {
                                    new_stmts.push(MirStmt::Assign {
                                        target: t.clone(),
                                        value: MirValue::Unit,
                                    });
                                }
                            }
                        }
                        continue;
                    }
                }
                new_stmts.push(stmt.clone());
            }
            block.stmts = new_stmts;
        }
    }
}

/// 是否可内联：单块、无区域操作、终止符为 return。
fn is_inlineable(f: &MirFunction) -> bool {
    if f.blocks.len() != 1 {
        return false;
    }
    let block = &f.blocks[0];
    if !matches!(block.terminator, Some(MirTerminator::Return(_))) {
        return false;
    }
    block.stmts.iter().all(|s| !has_region_op(s))
}

/// 指令是否涉及区域操作。
fn has_region_op(s: &MirStmt) -> bool {
    matches!(
        s,
        MirStmt::RegionEnter { .. }
            | MirStmt::RegionExit { .. }
            | MirStmt::AllocInRegion { .. }
            | MirStmt::Transfer { .. }
    )
}

/// 生成内联副本中的新临时变量名。
fn fresh_inline_temp(counter: &mut usize) -> Local {
    let name = format!("_i{}", counter);
    *counter += 1;
    name
}

/// 生成一条内联指令（参数/临时变量按 `subst` 重命名）。
fn inline_stmt(
    s: &MirStmt,
    subst: &mut HashMap<String, String>,
    counter: &mut usize,
) -> Option<MirStmt> {
    match s {
        MirStmt::Assign { target, value } => Some(MirStmt::Assign {
            target: map_local(target, subst, counter),
            value: map_value(value, subst, counter),
        }),
        MirStmt::Call {
            target,
            callee,
            args,
        } => Some(MirStmt::Call {
            target: target.as_ref().map(|t| map_local(t, subst, counter)),
            callee: callee.clone(),
            args: args.iter().map(|a| map_local(a, subst, counter)).collect(),
        }),
        MirStmt::Alloc { target, slots } => Some(MirStmt::Alloc {
            target: map_local(target, subst, counter),
            slots: *slots,
        }),
        MirStmt::FieldGet {
            target,
            base,
            index,
            ty,
        } => Some(MirStmt::FieldGet {
            target: map_local(target, subst, counter),
            base: map_local(base, subst, counter),
            index: *index,
            ty: *ty,
        }),
        MirStmt::FieldSet {
            base,
            index,
            value,
            ty,
        } => Some(MirStmt::FieldSet {
            base: map_local(base, subst, counter),
            index: *index,
            value: map_local(value, subst, counter),
            ty: *ty,
        }),
        MirStmt::IndexGet {
            target,
            base,
            index,
            ty,
            is_str,
        } => Some(MirStmt::IndexGet {
            target: map_local(target, subst, counter),
            base: map_local(base, subst, counter),
            index: map_local(index, subst, counter),
            ty: *ty,
            is_str: *is_str,
        }),
        MirStmt::IndexSet {
            base,
            index,
            value,
            ty,
            is_str,
        } => Some(MirStmt::IndexSet {
            base: map_local(base, subst, counter),
            index: map_local(index, subst, counter),
            value: map_local(value, subst, counter),
            ty: *ty,
            is_str: *is_str,
        }),
        // 引用指令：参数/局部变量按 subst 重命名后保留。
        // 实参 `&x` 的 AddrOf 指令在调用点已生成，内联副本直接复用其临时值。
        MirStmt::AddrOf {
            target,
            operand,
            pointee,
        } => Some(MirStmt::AddrOf {
            target: map_local(target, subst, counter),
            operand: map_local(operand, subst, counter),
            pointee: *pointee,
        }),
        MirStmt::DerefRead { target, base, ty } => Some(MirStmt::DerefRead {
            target: map_local(target, subst, counter),
            base: map_local(base, subst, counter),
            ty: *ty,
        }),
        MirStmt::DerefWrite { base, value, ty } => Some(MirStmt::DerefWrite {
            base: map_local(base, subst, counter),
            value: map_local(value, subst, counter),
            ty: *ty,
        }),
        // 间接调用：函数指针参数映射到实参临时（`f` → 调用点已构造的 `add` 指针槽），
        // 签名类型名透传；此前缺失此分支导致内联后整个 CallIndirect 被丢弃，
        // 返回值临时从未被写入，调用点读到未初始化栈垃圾值。
        MirStmt::CallIndirect {
            target,
            callee,
            args,
            param_names,
            ret_name,
        } => Some(MirStmt::CallIndirect {
            target: target.as_ref().map(|t| map_local(t, subst, counter)),
            callee: map_local(callee, subst, counter),
            args: args.iter().map(|a| map_local(a, subst, counter)).collect(),
            param_names: param_names.clone(),
            ret_name: ret_name.clone(),
        }),
        _ => None, // 区域操作已在上层排除
    }
}

/// 重命名局部变量：参数映射到实参，其余名字（函数内部局部变量）一律
/// 映射为内联副本新名。
///
/// 修复：此前仅对 `_` 开头的临时变量重命名，用户命名的局部变量（如
/// `let r = ...`）被原样保留，内联进调用者后与调用者同名变量合并为同一
/// 槽位，导致语义错误（如 `r.try_read_lock()` 的返回值写回调用者 `r`）。
fn map_local(name: &str, subst: &mut HashMap<String, String>, counter: &mut usize) -> Local {
    if let Some(mapped) = subst.get(name) {
        return mapped.clone();
    }
    // 非参数的名字必然属于被内联函数内部（参数已由 subst 映射到实参），
    // 全部重命名以避免与调用者作用域中的同名变量冲突。
    let fresh = fresh_inline_temp(counter);
    subst.insert(name.to_string(), fresh.clone());
    fresh
}

/// 递归重命名右值中的局部变量。
fn map_value(v: &MirValue, subst: &mut HashMap<String, String>, counter: &mut usize) -> MirValue {
    match v {
        MirValue::Place(p) => MirValue::Place(map_local(p, subst, counter)),
        MirValue::Binary { op, lhs, rhs } => MirValue::Binary {
            op: *op,
            lhs: Box::new(map_value(lhs, subst, counter)),
            rhs: Box::new(map_value(rhs, subst, counter)),
        },
        MirValue::Unary { op, operand } => MirValue::Unary {
            op: *op,
            operand: Box::new(map_value(operand, subst, counter)),
        },
        other => other.clone(),
    }
}

/// 将内联函数返回值绑定到原调用点目标。
fn bind_call_result(call: &MirStmt, new_stmts: &mut Vec<MirStmt>, ret_place: &str) {
    if let MirStmt::Call {
        target: Some(t), ..
    } = call
    {
        new_stmts.push(MirStmt::Assign {
            target: t.clone(),
            value: MirValue::Place(ret_place.to_string()),
        });
    }
    // 调用点不关心返回值：不绑定（未使用的临时变量由 DCE 清理）
}
