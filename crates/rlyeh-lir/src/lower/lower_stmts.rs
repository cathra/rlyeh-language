//! lower_stmts：LLVM 发射子模块。
//! （由 mod.rs 的 `impl FunctionLowerer` 拆分而来，保持语义等价）

use super::*;

impl FunctionLowerer {
    pub(super) fn lower_stmt(&mut self, stmt: &MirStmt, out: &mut Vec<LirStmt>) -> Result<(), LirError> {
        match stmt {
            MirStmt::Assign { target, value } => {
                self.lower_assign(target, value, out)?;
            }
            MirStmt::Call {
                target,
                callee,
                args,
            } => {
                out.push(LirStmt::Call {
                    target: target.clone(),
                    callee: callee.clone(),
                    args: args.clone(),
                });
            }
            MirStmt::CallIndirect {
                target,
                callee,
                args,
                param_names,
                ret_name,
            } => {
                let param_tys = param_names.iter().map(|n| parse_extern_type(n)).collect();
                out.push(LirStmt::CallIndirect {
                    target: target.clone(),
                    callee: callee.clone(),
                    args: args.clone(),
                    param_tys,
                    ret_ty: parse_extern_type(ret_name),
                });
            }
            MirStmt::RegionEnter { name, options } => {
                out.push(LirStmt::RegionEnter {
                    name: name.clone(),
                    options: *options,
                });
            }
            MirStmt::RegionExit { name } => out.push(LirStmt::RegionExit { name: name.clone() }),
            MirStmt::AllocInRegion {
                target,
                region,
                size,
            } => {
                out.push(LirStmt::AllocInRegion {
                    target: target.clone(),
                    region: region.clone(),
                    size: *size,
                });
            }
            MirStmt::Transfer { place, region } => {
                out.push(LirStmt::Transfer {
                    place: place.clone(),
                    region: region.clone(),
                });
            }
            MirStmt::Alloc {
                target,
                slots,
                by_value,
                is_strfat: _,
            } => {
                out.push(LirStmt::Alloc {
                    target: target.clone(),
                    slots: *slots,
                    by_value: *by_value,
                });
            }
            MirStmt::FieldGet {
                target,
                base,
                index,
                ty,
            } => {
                out.push(LirStmt::FieldGet {
                    target: target.clone(),
                    base: base.clone(),
                    index: *index,
                    ty: *ty,
                });
            }
            MirStmt::FieldSet {
                base,
                index,
                value,
                ty,
            } => {
                out.push(LirStmt::FieldSet {
                    base: base.clone(),
                    index: *index,
                    value: value.clone(),
                    ty: *ty,
                });
            }
            MirStmt::IndexGet {
                target,
                base,
                index,
                ty,
                is_str,
            } => {
                out.push(LirStmt::IndexGet {
                    target: target.clone(),
                    base: base.clone(),
                    index: index.clone(),
                    ty: *ty,
                    is_str: *is_str,
                });
            }
            MirStmt::IndexSet {
                base,
                index,
                value,
                ty,
                is_str,
            } => {
                out.push(LirStmt::IndexSet {
                    base: base.clone(),
                    index: index.clone(),
                    value: value.clone(),
                    ty: *ty,
                    is_str: *is_str,
                });
            }
            MirStmt::AddrOf {
                target,
                operand,
                pointee,
            } => {
                out.push(LirStmt::AddrOf {
                    target: target.clone(),
                    operand: operand.clone(),
                    pointee: *pointee,
                });
            }
            MirStmt::DerefRead { target, base, ty } => {
                out.push(LirStmt::DerefRead {
                    target: target.clone(),
                    base: base.clone(),
                    ty: *ty,
                });
            }
            MirStmt::DerefWrite { base, value, ty } => {
                out.push(LirStmt::DerefWrite {
                    base: base.clone(),
                    value: value.clone(),
                    ty: *ty,
                });
            }
            MirStmt::AddrOfField { target, base, index, ty } => {
                out.push(LirStmt::FieldAddr {
                    target: target.clone(),
                    base: base.clone(),
                    index: *index,
                    ty: *ty,
                });
            }
            MirStmt::PtrAdd { target, base, offset, elem, is_str } => {
                out.push(LirStmt::PtrAdd {
                    target: target.clone(),
                    base: base.clone(),
                    offset: offset.clone(),
                    elem: *elem,
                    is_str: *is_str,
                });
            }
            MirStmt::Cast { target, value, to } => {
                // U6 Cast IR：数值→数值类型转换透传（目标类型名）
                out.push(LirStmt::Cast {
                    target: target.clone(),
                    value: LirOperand::Local(value.clone()),
                    to: to.clone(),
                });
            }
        }
        Ok(())
    }

    pub(super) fn lower_assign(
        &mut self,
        target: &Local,
        value: &MirValue,
        out: &mut Vec<LirStmt>,
    ) -> Result<(), LirError> {
        match value {
            MirValue::Binary { op, lhs, rhs } => {
                let lhs_op = self.lower_operand(lhs, out)?;
                let rhs_op = self.lower_operand(rhs, out)?;
                let ty = self.binary_operand_type(*op, &lhs_op, &rhs_op);
                out.push(LirStmt::Binary {
                    target: target.clone(),
                    op: *op,
                    ty,
                    lhs: lhs_op,
                    rhs: rhs_op,
                });
            }
            MirValue::Unary { op, operand } => {
                let operand_op = self.lower_operand(operand, out)?;
                let ty = self.unary_operand_type(*op, &operand_op);
                out.push(LirStmt::Unary {
                    target: target.clone(),
                    op: *op,
                    ty,
                    operand: operand_op,
                });
            }
            _ => {
                let op = self.lower_operand(value, out)?;
                out.push(LirStmt::Assign {
                    target: target.clone(),
                    value: op,
                });
            }
        }
        Ok(())
    }

    pub(super) fn operand_type(&self, op: &LirOperand) -> Option<LirType> {
        op.literal_type().or_else(|| match op {
            LirOperand::Local(l) => self
                .ty
                .get(l)
                .copied()
                // 嵌套二元/一元拆平的临时变量登记在 extra_locals 中，
                // 查不到会误判 i64（如 `x*x + y*y` 外层加法结果类型错乱）
                .or_else(|| {
                    self.extra_locals
                        .iter()
                        .find(|(n, _)| n == l)
                        .map(|(_, t)| *t)
                }),
            _ => None,
        })
    }

    pub(super) fn binary_operand_type(&self, op: HirBinaryOp, lhs: &LirOperand, rhs: &LirOperand) -> LirType {
        if matches!(op, HirBinaryOp::And | HirBinaryOp::Or) {
            return LirType::Bool;
        }
        let lt = self.operand_type(lhs);
        let rt = self.operand_type(rhs);
        if lt == Some(LirType::F64) || rt == Some(LirType::F64) {
            LirType::F64
        } else if lt == Some(LirType::Char) || rt == Some(LirType::Char) {
            LirType::Char
        } else {
            LirType::I64
        }
    }

    pub(super) fn binary_result_type(&self, op: HirBinaryOp, oty: LirType) -> LirType {
        if matches!(
            op,
            HirBinaryOp::Eq
                | HirBinaryOp::Ne
                | HirBinaryOp::Lt
                | HirBinaryOp::Le
                | HirBinaryOp::Gt
                | HirBinaryOp::Ge
                | HirBinaryOp::And
                | HirBinaryOp::Or
        ) {
            LirType::Bool
        } else {
            oty
        }
    }

    pub(super) fn unary_operand_type(&self, op: HirUnaryOp, operand: &LirOperand) -> LirType {
        match op {
            HirUnaryOp::Neg => self.operand_type(operand).unwrap_or(LirType::I64),
            HirUnaryOp::Not => LirType::Bool,
        }
    }

    pub(super) fn lower_operand(
        &mut self,
        v: &MirValue,
        out: &mut Vec<LirStmt>,
    ) -> Result<LirOperand, LirError> {
        match v {
            MirValue::Int(i) => {
                // LLVM 后端 MVP 仅支持 i64；i128 校验范围
                let _ = i64::try_from(*i).map_err(|_| LirError::IntOverflow { value: *i })?;
                Ok(LirOperand::Int(*i))
            }
            MirValue::Float(x) => Ok(LirOperand::Float(*x)),
            MirValue::String(s) => Ok(LirOperand::String(s.clone())),
            MirValue::Char(c) => Ok(LirOperand::Char(*c)),
            MirValue::Bool(b) => Ok(LirOperand::Bool(*b)),
            MirValue::Unit => Ok(LirOperand::Unit),
            MirValue::Place(l) => Ok(LirOperand::Local(l.clone())),
            MirValue::FnRef(name) => Ok(LirOperand::FnPtr(name.clone())),
            MirValue::Binary { op, lhs, rhs } => {
                // 嵌套二元 → 拆平到临时变量；
                // ty 字段为操作数类型，临时变量槽登记为结果类型（比较 / 逻辑 → bool）
                let lhs_op = self.lower_operand(lhs, out)?;
                let rhs_op = self.lower_operand(rhs, out)?;
                let oty = self.binary_operand_type(*op, &lhs_op, &rhs_op);
                let tmp = self.fresh_temp(self.binary_result_type(*op, oty));
                out.push(LirStmt::Binary {
                    target: tmp.clone(),
                    op: *op,
                    ty: oty,
                    lhs: lhs_op,
                    rhs: rhs_op,
                });
                Ok(LirOperand::Local(tmp))
            }
            MirValue::Unary { op, operand } => {
                let operand_op = self.lower_operand(operand, out)?;
                let ty = self.unary_operand_type(*op, &operand_op);
                let tmp = self.fresh_temp(ty);
                out.push(LirStmt::Unary {
                    target: tmp.clone(),
                    op: *op,
                    ty,
                    operand: operand_op,
                });
                Ok(LirOperand::Local(tmp))
            }
        }
    }}
