//! 常量折叠 pass：编译期计算常量表达式。
//!
//! - 折叠 `Assign` 右值中的常量运算树（整数四则 / 比较、布尔逻辑、字符比较）；
//!   除零 / 取模零保守不折叠（保留运行时语义）；
//! - 折叠块内"常量条件"的 `CondJump` 为无条件 `Jump`（配合 DCE 清理不可达块）。
//!
//! 浮点运算不折叠（`NaN` / 舍入语义由运行时保证）。

use crate::{MirFunction, MirProgram, MirStmt, MirTerminator, MirValue};
use std::collections::HashMap;
use zeta_hir::{HirBinaryOp, HirUnaryOp};

/// 对程序执行常量折叠。
pub fn constant_fold(program: &mut MirProgram) {
    for f in &mut program.functions {
        fold_assigns(f);
        fold_cond_terms(f);
    }
}

/// 折叠所有 `Assign` 右值中的常量子表达式。
fn fold_assigns(f: &mut MirFunction) {
    for block in &mut f.blocks {
        for stmt in &mut block.stmts {
            if let MirStmt::Assign { value, .. } = stmt {
                *value = fold_value(std::mem::replace(value, MirValue::Unit));
            }
        }
    }
}

/// 递归折叠运算树。
pub(crate) fn fold_value(v: MirValue) -> MirValue {
    match v {
        MirValue::Binary { op, lhs, rhs } => {
            let l = fold_value(*lhs);
            let r = fold_value(*rhs);
            fold_binary(op, &l, &r).unwrap_or(MirValue::Binary {
                op,
                lhs: Box::new(l),
                rhs: Box::new(r),
            })
        }
        MirValue::Unary { op, operand } => {
            let o = fold_value(*operand);
            match (op, &o) {
                (HirUnaryOp::Not, MirValue::Bool(b)) => MirValue::Bool(!b),
                (HirUnaryOp::Neg, MirValue::Int(i)) => MirValue::Int(i.wrapping_neg()),
                _ => MirValue::Unary {
                    op,
                    operand: Box::new(o),
                },
            }
        }
        other => other,
    }
}

/// 折叠二元运算（操作数均为常量时返回 `Some`）。
fn fold_binary(op: HirBinaryOp, lhs: &MirValue, rhs: &MirValue) -> Option<MirValue> {
    use HirBinaryOp::*;
    match (lhs, rhs) {
        (MirValue::Int(a), MirValue::Int(b)) => match op {
            Add => Some(MirValue::Int(a.wrapping_add(*b))),
            Sub => Some(MirValue::Int(a.wrapping_sub(*b))),
            Mul => Some(MirValue::Int(a.wrapping_mul(*b))),
            Div => (*b != 0).then(|| MirValue::Int(a.wrapping_div(*b))),
            Mod => (*b != 0).then(|| MirValue::Int(a.wrapping_rem(*b))),
            Eq => Some(MirValue::Bool(a == b)),
            Ne => Some(MirValue::Bool(a != b)),
            Lt => Some(MirValue::Bool(a < b)),
            Le => Some(MirValue::Bool(a <= b)),
            Gt => Some(MirValue::Bool(a > b)),
            Ge => Some(MirValue::Bool(a >= b)),
            And | Or => None, // 类型不匹配，保守不折叠
        },
        (MirValue::Bool(a), MirValue::Bool(b)) => match op {
            And => Some(MirValue::Bool(*a && *b)),
            Or => Some(MirValue::Bool(*a || *b)),
            Eq => Some(MirValue::Bool(a == b)),
            Ne => Some(MirValue::Bool(a != b)),
            _ => None,
        },
        (MirValue::Char(a), MirValue::Char(b)) => match op {
            Eq => Some(MirValue::Bool(a == b)),
            Ne => Some(MirValue::Bool(a != b)),
            _ => None,
        },
        _ => None,
    }
}

/// 折叠块内由常量赋值的条件终止符：`CondJump(cond=const)` → `Jump`。
fn fold_cond_terms(f: &mut MirFunction) {
    for block in &mut f.blocks {
        // 收集块内常量：target -> bool
        let mut consts: HashMap<&str, bool> = HashMap::new();
        for stmt in &block.stmts {
            if let MirStmt::Assign {
                target,
                value: MirValue::Bool(b),
            } = stmt
            {
                consts.insert(target.as_str(), *b);
            }
        }
        let replacement = match &block.terminator {
            Some(MirTerminator::CondJump {
                cond,
                then,
                otherwise,
            }) => consts
                .get(cond.as_str())
                .map(|b| if *b { *then } else { *otherwise }),
            _ => None,
        };
        if let Some(target) = replacement {
            block.terminator = Some(MirTerminator::Jump(target));
        }
    }
}
