//! 表达式检查子模块：二元运算符检查。
//! （由 check_expr/mod.rs 拆分而来，保持语义等价）

use super::*;

pub(super) fn check_binary(
    op: BinaryOp,
    left: &Type,
    right: &Type,
    span: Span,
) -> Result<(HirBinaryOp, Type), TypeError> {
    let hir_op = match op {
        BinaryOp::Add => HirBinaryOp::Add,
        BinaryOp::Sub => HirBinaryOp::Sub,
        BinaryOp::Mul => HirBinaryOp::Mul,
        BinaryOp::Div => HirBinaryOp::Div,
        BinaryOp::Mod => HirBinaryOp::Mod,
        BinaryOp::And | BinaryOp::Or => {
            if !left.is_bool() || !right.is_bool() {
                return Err(TypeError::ExpectedBool {
                    found: left.to_string(),
                    span,
                });
            }
            let hir_op = if op == BinaryOp::And {
                HirBinaryOp::And
            } else {
                HirBinaryOp::Or
            };
            return Ok((hir_op, Type::Bool));
        }
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
            // 位运算：要求整数操作数，结果为整数。U3 核心项（2026-08-30）：
            // 标量枚举值即 tag（整数），可作位运算操作数；结果按整数（i64）处理
            // （位掩码 / 移位结果未必对应合法枚举变体）。
            let is_int_like = |t: &Type| t.is_integer() || matches!(t, Type::ScalarEnum(_));
            if !is_int_like(left) || !is_int_like(right) {
                return Err(TypeError::ExpectedInt {
                    found: left.to_string(),
                    span,
                });
            }
            let hir_op = match op {
                BinaryOp::BitAnd => HirBinaryOp::BitAnd,
                BinaryOp::BitOr => HirBinaryOp::BitOr,
                BinaryOp::BitXor => HirBinaryOp::BitXor,
                BinaryOp::Shl => HirBinaryOp::Shl,
                _ => HirBinaryOp::Shr,
            };
            let result_ty = if matches!(left, Type::ScalarEnum(_))
                || matches!(right, Type::ScalarEnum(_))
            {
                Type::I64
            } else {
                left.clone()
            };
            return Ok((hir_op, result_ty));
        }
    };

    // 算术运算：要求数值类型
    if !left.is_numeric() || !right.is_numeric() {
        return Err(TypeError::ExpectedNumeric {
            found: left.to_string(),
            span,
        });
    }
    if !left.compatible_with(right) {
        return Err(TypeError::WrongType {
            expected: left.to_string(),
            found: right.to_string(),
            span,
        });
    }
    Ok((hir_op, merge_numeric(left.clone(), right.clone())))
}

/// V5d（2026-09-02）：可重载的二元运算符 → 运算符 trait 方法名。
/// 逻辑 &&/|| 短路语义不可重载；比较链 < > 等走 CompareOp 独立路径，本期不重载。
pub(super) fn overload_method(op: BinaryOp) -> Option<&'static str> {
    let name = match op {
        BinaryOp::Add => "add",
        BinaryOp::Sub => "sub",
        BinaryOp::Mul => "mul",
        BinaryOp::Div => "div",
        BinaryOp::Mod => "rem",
        BinaryOp::BitAnd => "bitand",
        BinaryOp::BitOr => "bitor",
        BinaryOp::BitXor => "bitxor",
        BinaryOp::Shl => "shl",
        BinaryOp::Shr => "shr",
        BinaryOp::And | BinaryOp::Or => return None,
    };
    Some(name)
}

pub(super) fn is_castable_scalar(t: &Type) -> bool {
    matches!(
        t,
        Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::ISize
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::USize
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::Char
    )
}

pub(super) fn merge_numeric(a: Type, b: Type) -> Type {
    if a.is_float() || b.is_float() {
        Type::F64
    } else {
        Type::I64
    }
}
