//! MIR → LIR lowering：三地址码拆平与类型推断。

use std::collections::HashMap;

use zeta_hir::{FieldScalar, HirBinaryOp, HirUnaryOp};
use zeta_mir::{BasicBlock, MirFunction, MirProgram, MirStmt, MirTerminator, MirValue};

use crate::error::LirError;
use crate::{
    LirBlock, LirFunction, LirOperand, LirProgram, LirStmt, LirTerminator, LirType, Local,
};

/// 内建函数（由代码生成层映射到运行时，LLVM 后端实现为 `printf` 调用）。
///
/// 这里集中登记，供类型推断阶段区分"内建调用返回单元值"与"用户函数调用"。
pub const BUILTIN_FUNCTIONS: &[&str] = &["print", "println"];

/// 将优化后的 MIR 程序降低为 LIR 程序。
///
/// 步骤：
/// 1. 对每个函数做多遍类型推断，得到局部变量类型表与函数返回类型；
/// 2. 将 MIR 的赋值树拆平为三地址码指令（嵌套运算引入临时变量）；
/// 3. 解析跨函数调用目标变量的类型。
pub fn lower_program(program: &MirProgram) -> Result<LirProgram, LirError> {
    // 第一遍：每个函数的局部类型表 + 返回类型
    let mut infer_results: Vec<(HashMap<Local, LirType>, LirType)> =
        Vec::with_capacity(program.functions.len());
    for f in &program.functions {
        infer_results.push(infer_function_types(f)?);
    }

    // 全程序函数返回类型表（供跨函数调用目标解析）
    let ret_types: HashMap<String, LirType> = program
        .functions
        .iter()
        .zip(&infer_results)
        .map(|(f, (_, ret))| (f.name.clone(), *ret))
        .collect();
    // 第二遍：生成 LIR 函数
    let mut functions = Vec::with_capacity(program.functions.len());
    for (f, (ty, ret)) in program.functions.iter().zip(&infer_results) {
        functions.push(lower_function(f, ty, *ret, &ret_types)?);
    }

    Ok(LirProgram { functions })
}

/// 对单个函数做类型推断，返回（局部变量类型表，返回类型）。
fn infer_function_types(f: &MirFunction) -> Result<(HashMap<Local, LirType>, LirType), LirError> {
    let mut ty: HashMap<Local, LirType> = HashMap::new();

    // 多遍迭代至稳定：Binary 操作数为 Place 时，其类型可能由后续赋值确定
    for _ in 0..8 {
        let mut changed = false;
        for block in &f.blocks {
            for stmt in &block.stmts {
                match stmt {
                    MirStmt::Assign { target, value } => {
                        if let Some(vt) = infer_value_type(value, &ty) {
                            changed |= set_type(&mut ty, target, vt)?;
                        }
                    }
                    MirStmt::Call {
                        target: Some(t),
                        callee,
                        ..
                    } if BUILTIN_FUNCTIONS.contains(&callee.as_str()) => {
                        changed |= set_type(&mut ty, t, LirType::Unit)?;
                    }
                    // 用户函数调用：跨函数类型在全部推断完成后解析
                    MirStmt::Call { .. } => {}
                    MirStmt::Alloc { target, .. } => {
                        changed |= set_type(&mut ty, target, LirType::Ptr)?;
                    }
                    MirStmt::FieldGet { target, ty: fty, .. } => {
                        changed |= set_type(&mut ty, target, field_scalar_to_lir(*fty))?;
                    }
                    // 写槽不产生新类型信息
                    MirStmt::FieldSet { .. } => {}
                    MirStmt::IndexGet { target, ty: ety, .. } => {
                        changed |= set_type(&mut ty, target, field_scalar_to_lir(*ety))?;
                    }
                    // 索引写入不产生新类型信息
                    MirStmt::IndexSet { .. } => {}
                    // 区域指令不产生类型信息
                    MirStmt::RegionEnter { .. }
                    | MirStmt::RegionExit
                    | MirStmt::AllocInRegion { .. }
                    | MirStmt::Transfer { .. } => {}
                }
            }
        }
        if !changed {
            break;
        }
    }

    // 未推断类型的变量（参数、用户调用目标等）默认 i64
    for name in collect_locals(f) {
        ty.entry(name).or_insert(LirType::I64);
    }

    let ret = infer_return_type(f, &ty);
    Ok((ty, ret))
}

/// 收集函数内出现的全部局部变量名（参数 + 指令目标 + 操作数 + 返回操作数）。
fn collect_locals(f: &MirFunction) -> Vec<Local> {
    let mut names: Vec<String> = f.params.clone();
    for block in &f.blocks {
        for stmt in &block.stmts {
            match stmt {
                MirStmt::Assign { target, value } => {
                    names.push(target.clone());
                    collect_value_locals(value, &mut names);
                }
                MirStmt::Call {
                    target,
                    callee: _,
                    args,
                } => {
                    if let Some(t) = target {
                        names.push(t.clone());
                    }
                    names.extend(args.iter().cloned());
                }
                MirStmt::AllocInRegion { target, .. } => names.push(target.clone()),
                MirStmt::Transfer { place, .. } => names.push(place.clone()),
                MirStmt::Alloc { target, .. } => names.push(target.clone()),
                MirStmt::FieldGet { target, base, .. } => {
                    names.push(target.clone());
                    names.push(base.clone());
                }
                MirStmt::FieldSet { base, value, .. } => {
                    names.push(base.clone());
                    names.push(value.clone());
                }
                MirStmt::IndexGet { target, base, index, .. } => {
                    names.push(target.clone());
                    names.push(base.clone());
                    names.push(index.clone());
                }
                MirStmt::IndexSet { base, index, value, .. } => {
                    names.push(base.clone());
                    names.push(index.clone());
                    names.push(value.clone());
                }
                _ => {}
            }
        }
        if let Some(MirTerminator::Return(Some(x))) = &block.terminator {
            names.push(x.clone());
        }
        if let Some(MirTerminator::CondJump { cond, .. }) = &block.terminator {
            names.push(cond.clone());
        }
    }
    names
}

/// 收集 MIR 值树中出现的全部变量名。
fn collect_value_locals(v: &MirValue, names: &mut Vec<Local>) {
    match v {
        MirValue::Place(l) => names.push(l.clone()),
        MirValue::Binary { lhs, rhs, .. } => {
            collect_value_locals(lhs, names);
            collect_value_locals(rhs, names);
        }
        MirValue::Unary { operand, .. } => collect_value_locals(operand, names),
        _ => {}
    }
}

/// 推断 MIR 值的类型（`None` 表示暂无法确定）。
fn infer_value_type(v: &MirValue, ty: &HashMap<Local, LirType>) -> Option<LirType> {
    match v {
        MirValue::Int(_) => Some(LirType::I64),
        MirValue::Float(_) => Some(LirType::F64),
        MirValue::String(_) => Some(LirType::Str),
        MirValue::Char(_) => Some(LirType::Char),
        MirValue::Bool(_) => Some(LirType::Bool),
        MirValue::Unit => Some(LirType::Unit),
        MirValue::Place(l) => ty.get(l).copied(),
        MirValue::Binary { op, lhs, rhs } => {
            let lt = infer_value_type(lhs, ty);
            let rt = infer_value_type(rhs, ty);
            Some(binary_result_type(*op, lt, rt))
        }
        MirValue::Unary { op, operand } => {
            let ot = infer_value_type(operand, ty);
            Some(match op {
                HirUnaryOp::Neg => ot.unwrap_or(LirType::I64),
                HirUnaryOp::Not => LirType::Bool,
            })
        }
    }
}

/// 槽值标量种类 → LIR 类型。
fn field_scalar_to_lir(ty: FieldScalar) -> LirType {
    match ty {
        FieldScalar::Int => LirType::I64,
        FieldScalar::Float => LirType::F64,
        FieldScalar::Bool => LirType::Bool,
        FieldScalar::Char => LirType::Char,
        FieldScalar::Str => LirType::Str,
        FieldScalar::Ptr => LirType::Ptr,
    }
}

/// 二元运算结果类型：算术按操作数（浮点优先），比较 / 逻辑为 `Bool`。
fn binary_result_type(op: HirBinaryOp, lt: Option<LirType>, rt: Option<LirType>) -> LirType {
    match op {
        HirBinaryOp::Add
        | HirBinaryOp::Sub
        | HirBinaryOp::Mul
        | HirBinaryOp::Div
        | HirBinaryOp::Mod => {
            if lt == Some(LirType::F64) || rt == Some(LirType::F64) {
                LirType::F64
            } else {
                LirType::I64
            }
        }
        HirBinaryOp::Eq
        | HirBinaryOp::Ne
        | HirBinaryOp::Lt
        | HirBinaryOp::Le
        | HirBinaryOp::Gt
        | HirBinaryOp::Ge
        | HirBinaryOp::And
        | HirBinaryOp::Or => LirType::Bool,
    }
}

/// 登记变量类型；类型冲突时报错，成功且发生变更返回 `true`。
fn set_type(ty: &mut HashMap<Local, LirType>, name: &Local, t: LirType) -> Result<bool, LirError> {
    match ty.get(name) {
        Some(&old) if old != t => Err(LirError::TypeConflict {
            variable: name.clone(),
            expected: old,
            found: t,
        }),
        Some(_) => Ok(false),
        None => {
            ty.insert(name.clone(), t);
            Ok(true)
        }
    }
}

/// 推断函数返回类型：取第一个非 `Unit` 的 `Return` 操作数类型。
fn infer_return_type(f: &MirFunction, ty: &HashMap<Local, LirType>) -> LirType {
    for block in &f.blocks {
        if let Some(MirTerminator::Return(Some(x))) = &block.terminator {
            if let Some(t) = ty.get(x) {
                if *t != LirType::Unit {
                    return *t;
                }
            }
        }
    }
    LirType::Unit
}

/// 将单个 MIR 函数降低为 LIR 函数。
fn lower_function(
    f: &MirFunction,
    ty: &HashMap<Local, LirType>,
    return_type: LirType,
    ret_types: &HashMap<String, LirType>,
) -> Result<LirFunction, LirError> {
    // 解析跨函数调用目标类型：用户函数 → 全程序返回类型表。
    //
    // 注意：推断阶段对所有未推断变量默认填充 i64（`infer_function_types`
    // 的占位策略），因此这里使用**覆盖**语义而非冲突检测——用户调用 target
    // 的真实类型由 callee 返回类型决定。同时做多遍**副本类型传播**
    // （`x = y` 且 `y` 为 Place 时，`x` 跟随 `y` 的类型），保证
    // `let v = obj.method();` 这类别名变量的类型与调用结果一致。
    let mut ty = ty.clone();
    for _ in 0..8 {
        let mut changed = false;
        for block in &f.blocks {
            for stmt in &block.stmts {
                match stmt {
                    MirStmt::Call {
                        target: Some(t),
                        callee,
                        ..
                    } if !BUILTIN_FUNCTIONS.contains(&callee.as_str()) => {
                        if let Some(rt) = ret_types.get(callee).copied() {
                            if ty.insert(t.clone(), rt) != Some(rt) {
                                changed = true;
                            }
                        }
                        // 未知 callee（外部扩展）：保持占位默认 i64
                    }
                    MirStmt::Assign {
                        target,
                        value: MirValue::Place(src),
                    } => {
                        if let Some(st) = ty.get(src).copied() {
                            if ty.insert(target.clone(), st) != Some(st) {
                                changed = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if !changed {
            break;
        }
    }

    let params = f
        .params
        .iter()
        .map(|p| (p.clone(), ty.get(p).copied().unwrap_or(LirType::I64)))
        .collect::<Vec<_>>();

    // 全部局部变量类型表（排序保证确定性输出）
    let mut locals: Vec<(Local, LirType)> = ty.clone().into_iter().collect();
    locals.sort_by(|a, b| a.0.cmp(&b.0));

    let mut lowerer = FunctionLowerer::new(&ty);
    let mut blocks = Vec::with_capacity(f.blocks.len());
    for (i, block) in f.blocks.iter().enumerate() {
        blocks.push(lower_block(block, i, &f.name, &mut lowerer)?);
    }

    // 并入 LIR 新生成的临时变量（sort 保持确定性输出，重复名字取其一）
    locals.extend(lowerer.extra_locals);
    locals.sort_by(|a, b| a.0.cmp(&b.0));
    locals.dedup_by(|a, b| a.0 == b.0);

    Ok(LirFunction {
        name: f.name.clone(),
        params,
        return_type,
        locals,
        blocks,
    })
}

/// 降低单个基本块。
fn lower_block(
    block: &BasicBlock,
    block_idx: usize,
    fn_name: &str,
    lowerer: &mut FunctionLowerer,
) -> Result<LirBlock, LirError> {
    let mut stmts = Vec::new();
    for stmt in &block.stmts {
        lowerer.lower_stmt(stmt, &mut stmts)?;
    }
    let terminator = block
        .terminator
        .as_ref()
        .map(lower_terminator)
        .ok_or_else(|| LirError::MissingTerminator {
            function: fn_name.to_string(),
            block: block_idx,
        })?;
    Ok(LirBlock { stmts, terminator })
}

/// 降低终止符（结构与 MIR 一致）。
fn lower_terminator(t: &MirTerminator) -> LirTerminator {
    match t {
        MirTerminator::Return(v) => LirTerminator::Return(v.clone()),
        MirTerminator::Jump(b) => LirTerminator::Jump(*b),
        MirTerminator::CondJump {
            cond,
            then,
            otherwise,
        } => LirTerminator::CondJump {
            cond: cond.clone(),
            then: *then,
            otherwise: *otherwise,
        },
    }
}

/// 单个函数 lowering 的状态：临时变量计数器与局部变量类型表。
struct FunctionLowerer {
    temp_counter: usize,
    /// 局部变量类型表（变量引用操作数解析类型用）
    ty: HashMap<Local, LirType>,
    /// LIR 降低期间新生成的临时变量（如 Binary/Unary 拆平的 target）
    extra_locals: Vec<(Local, LirType)>,
}

impl FunctionLowerer {
    fn new(ty: &HashMap<Local, LirType>) -> Self {
        // 临时变量与 MIR 局部变量共用 `_tN` 命名空间：从既有最大编号 +1 开始，
        // 防止 LIR 生成的临时寄存器覆盖 MIR 的局部（如 Alloc 的 target `_t0`）
        let start = ty
            .keys()
            .filter_map(|k| {
                k.strip_prefix("_t")
                    .and_then(|s| s.parse::<usize>().ok())
            })
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);
        Self {
            temp_counter: start,
            ty: ty.clone(),
            extra_locals: Vec::new(),
        }
    }

    /// 生成新的临时变量名 `_tN` 并注册其类型（供 codegen 分配 alloca）。
    fn fresh_temp(&mut self, ty: LirType) -> Local {
        let name = format!("_t{}", self.temp_counter);
        self.temp_counter += 1;
        self.extra_locals.push((name.clone(), ty));
        name
    }

    /// 降低单条 MIR 指令。
    fn lower_stmt(&mut self, stmt: &MirStmt, out: &mut Vec<LirStmt>) -> Result<(), LirError> {
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
            MirStmt::RegionEnter { name, .. } => {
                out.push(LirStmt::RegionEnter { name: name.clone() });
            }
            MirStmt::RegionExit => out.push(LirStmt::RegionExit),
            MirStmt::AllocInRegion { target, region } => {
                out.push(LirStmt::AllocInRegion {
                    target: target.clone(),
                    region: region.clone(),
                });
            }
            MirStmt::Transfer { place, region } => {
                out.push(LirStmt::Transfer {
                    place: place.clone(),
                    region: region.clone(),
                });
            }
            MirStmt::Alloc { target, slots } => {
                out.push(LirStmt::Alloc {
                    target: target.clone(),
                    slots: *slots,
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
        }
        Ok(())
    }

    /// 降低赋值：将值树拆平为三地址码。
    fn lower_assign(
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

    /// 解析操作数类型（立即数直接取，变量查局部类型表）。
    fn operand_type(&self, op: &LirOperand) -> Option<LirType> {
        op.literal_type().or_else(|| match op {
            LirOperand::Local(l) => self.ty.get(l).copied(),
            _ => None,
        })
    }

    /// 二元运算的操作数类型：
    ///
    /// - 逻辑 `and` / `or`：`bool`
    /// - 其余（算术 / 比较）：按操作数类型，浮点优先，其次字符，默认整数
    fn binary_operand_type(&self, op: HirBinaryOp, lhs: &LirOperand, rhs: &LirOperand) -> LirType {
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

    /// 一元运算的操作数类型：`neg` 取操作数数值类型（默认整数），`not` 为 `bool`。
    fn unary_operand_type(&self, op: HirUnaryOp, operand: &LirOperand) -> LirType {
        match op {
            HirUnaryOp::Neg => self.operand_type(operand).unwrap_or(LirType::I64),
            HirUnaryOp::Not => LirType::Bool,
        }
    }

    /// 降低操作数：字面量 / 变量直接返回；嵌套运算拆平到临时变量。
    fn lower_operand(
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
            MirValue::Binary { op, lhs, rhs } => {
                // 嵌套二元 → 拆平到临时变量
                let lhs_op = self.lower_operand(lhs, out)?;
                let rhs_op = self.lower_operand(rhs, out)?;
                let ty = self.binary_operand_type(*op, &lhs_op, &rhs_op);
                let tmp = self.fresh_temp(ty);
                out.push(LirStmt::Binary {
                    target: tmp.clone(),
                    op: *op,
                    ty,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 通过 `FunctionLowerer` 间接测试操作数类型推断规则。
    fn op_type(op: HirBinaryOp, lhs: LirOperand, rhs: LirOperand) -> LirType {
        let lowerer = FunctionLowerer::new(&HashMap::new());
        lowerer.binary_operand_type(op, &lhs, &rhs)
    }

    #[test]
    fn binary_operand_type_rules() {
        // 算术：浮点优先，否则整数
        assert_eq!(
            op_type(
                HirBinaryOp::Add,
                LirOperand::Int(1),
                LirOperand::Local("x".into())
            ),
            LirType::I64
        );
        assert_eq!(
            op_type(HirBinaryOp::Add, LirOperand::Float(1.0), LirOperand::Int(1)),
            LirType::F64
        );
        // 字符比较：按字符操作数
        assert_eq!(
            op_type(
                HirBinaryOp::Lt,
                LirOperand::Char('a'),
                LirOperand::Char('b')
            ),
            LirType::Char
        );
        // 比较：操作数类型，结果类型为 bool（codegen 负责）
        assert_eq!(
            op_type(HirBinaryOp::Gt, LirOperand::Int(1), LirOperand::Int(2)),
            LirType::I64
        );
        // 逻辑：bool
        assert_eq!(
            op_type(
                HirBinaryOp::And,
                LirOperand::Bool(true),
                LirOperand::Bool(false)
            ),
            LirType::Bool
        );
    }

    #[test]
    fn operand_literal_types() {
        assert_eq!(LirOperand::Int(42).literal_type(), Some(LirType::I64));
        assert_eq!(LirOperand::Float(1.5).literal_type(), Some(LirType::F64));
        assert_eq!(
            LirOperand::String("s".into()).literal_type(),
            Some(LirType::Str)
        );
        assert_eq!(LirOperand::Char('x').literal_type(), Some(LirType::Char));
        assert_eq!(LirOperand::Bool(true).literal_type(), Some(LirType::Bool));
        assert_eq!(LirOperand::Unit.literal_type(), Some(LirType::Unit));
        assert_eq!(LirOperand::Local("x".into()).literal_type(), None);
    }
}
