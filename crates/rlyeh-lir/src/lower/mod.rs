//! MIR → LIR lowering：三地址码拆平与类型推断。

use std::collections::HashMap;

use rlyeh_hir::{FieldScalar, HirBinaryOp, HirUnaryOp, ReprConv};
use rlyeh_mir::{BasicBlock, MirFunction, MirProgram, MirStmt, MirTerminator, MirValue};

use crate::error::LirError;
use crate::{
    LirBlock, LirFunction, LirOperand, LirProgram, LirStmt, LirTerminator, LirType, Local,
};

/// 内建函数（由代码生成层映射到运行时，LLVM 后端实现为 `printf` / `malloc` /
/// `llvm.memcpy` / `free` 调用）。
///
/// 这里集中登记，供类型推断阶段区分"内建调用返回单元值"与"用户函数调用"。
pub const BUILTIN_FUNCTIONS: &[&str] = &[
    "print",
    "println",
    "eprint",
    "eprintln",
    "alloc_array",
    "array_copy",
    "array_free",
    "alloc_bytes",
    "copy_bytes",
    "bytes_eq",
    "bytes_cmp",
    "print_string",
    "println_string",
    "eprint_string",
    "eprintln_string",
    "hash_value",
    "panic",
    "mem_swap",
];

/// 将优化后的 MIR 程序降低为 LIR 程序。
///
/// 步骤：
/// 1. 对每个函数做多遍类型推断，得到局部变量类型表与函数返回类型；
/// 2. 将 MIR 的赋值树拆平为三地址码指令（嵌套运算引入临时变量）；
/// 3. 解析跨函数调用目标变量的类型。
pub fn lower_program(program: &MirProgram) -> Result<LirProgram, LirError> {
    // 第一遍：每个函数的局部类型表 + 返回类型
    // extern 声明无函数体，直接由 extern_sig 构造签名（不参与类型推断）
    let mut infer_results: Vec<(HashMap<Local, LirType>, LirType)> =
        Vec::with_capacity(program.functions.len());
    for f in &program.functions {
        if f.is_extern {
            let ret = extern_return_type(f);
            infer_results.push((HashMap::new(), ret));
        } else {
            infer_results.push(infer_function_types(f)?);
        }
    }

    // V2-D（2026-08-26）：全程序函数参数类型表（函数名 → 各参数 LirType）。
    // 用户函数参数默认 i64（LIR 无参数类型信息）；经调用点实参传播——实参为
    // StrFat（`&str`）时对应参数标记为 StrFat（`str_len(v)` 传 `{data,len}` 双字）。
    let mut param_types: HashMap<String, Vec<LirType>> = program
        .functions
        .iter()
        .map(|f| {
            if f.is_extern {
                let ptys = f
                    .extern_sig
                    .as_ref()
                    .map(|(ps, _)| ps.iter().map(|p| parse_extern_type(p)).collect())
                    .unwrap_or_default();
                (f.name.clone(), ptys)
            } else {
                (f.name.clone(), vec![LirType::I64; f.params.len()])
            }
        })
        .collect();

    // 全程序函数返回类型表（供跨函数调用目标解析）
    let mut ret_types: HashMap<String, LirType> = program
        .functions
        .iter()
        .zip(&infer_results)
        .map(|(f, (_, ret))| {
            // V2 StrFat 胖指针：`as_str_range` / `trim`（返回 `&str`）的返回类型
            // 显式标注为 `StrFat`（双槽 {data, len}）。其返回值为 by_value StrFat
            // 对象（LIR 推断为 `Ptr`），若不纠正，调用点 target 被标注为 `Ptr`，
            // `println(&str)` 无法走 `%.*s` 长度限定打印（退化为 `%s` 读到 \0 才停）。
            let ty = if f.name.contains("as_str_range") || f.name.contains("String::trim") {
                LirType::StrFat
            } else {
                *ret
            };
            (f.name.clone(), ty)
        })
        .collect();

    // 跨函数返回类型重算：第一遍推断在函数内独立进行，用户函数调用的 target
    // 类型取不到（占位 i64），导致「返回用户函数调用结果」的函数（如
    // `String::trim` 返回 `String::substring` 调用）返回类型被误判为 i64。
    // 此处用全程序 ret_types 解析调用目标后再重算返回类型，并迭代至稳定以
    // 处理函数间链式依赖；最终 ret_types 供第二遍与 `lower_function` 使用。
    for _ in 0..8 {
        let mut changed = false;
        for (f, (ty, ret)) in program.functions.iter().zip(&mut infer_results) {
            if f.is_extern {
                continue;
            }
            // V2-C：直接更新 `ty`（而非克隆 `resolved`）——否则调用点
            // `r = as_str_range(...)` 的 `ty[r]` 不会标记为 `StrFat`，codegen
            // 打印 `&str` 走 Ptr 分支（输出指针地址而非子区间）。
            resolve_call_target_types(f, ty, &ret_types);
            // V2-D：参数类型传播——本函数内 Call 的实参为 StrFat（`&str`）时，
            // 被调函数对应参数标记为 StrFat（跨函数 `{data,len}` 双字传递）。
            for block in &f.blocks {
                for stmt in &block.stmts {
                    if let MirStmt::Call { callee, args, .. } = stmt {
                        if BUILTIN_FUNCTIONS.contains(&callee.as_str()) {
                            continue;
                        }
                        if let Some(pt) = param_types.get_mut(callee) {
                            for (i, arg) in args.iter().enumerate() {
                                // S2：切片胖指针 `&[T]`（SliceFat）与 `&str`（StrFat）
                                // 同为 `{data, len}` 双字，均须跨函数传播到被调函数参数
                                let want = ty.get(arg).copied();
                                if i < pt.len()
                                    && matches!(want, Some(LirType::StrFat | LirType::SliceFat))
                                {
                                    let w = want.unwrap();
                                    if pt[i] != w {
                                        pt[i] = w;
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let new_ret = infer_return_type(f, ty);
            if new_ret != *ret {
                *ret = new_ret;
                changed = true;
            }
        }
        let rebuilt: HashMap<String, LirType> = program
            .functions
            .iter()
            .zip(&infer_results)
            .map(|(f, (_, ret))| {
                let ty = if f.name.contains("as_str_range") || f.name.contains("String::trim") {
                    LirType::StrFat
                } else {
                    *ret
                };
                (f.name.clone(), ty)
            })
            .collect();
        if rebuilt != ret_types {
            ret_types = rebuilt;
            changed = true;
        }
        if !changed {
            break;
        }
    }

    // 第二遍：生成 LIR 函数（返回类型取重算后的全程序表）
    let mut functions = Vec::with_capacity(program.functions.len());
    for (f, (ty, _)) in program.functions.iter().zip(&infer_results) {
        let ret = ret_types.get(&f.name).copied().unwrap_or(LirType::Unit);
        let ptys = param_types.get(&f.name).cloned().unwrap_or_default();
        functions.push(lower_function(f, ty, ret, &ret_types, ptys)?);
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
                        // `alloc_array` / `alloc_bytes` 返回动态缓冲指针，
                        // `hash_value` / `bytes_cmp` 返回 i64（散列值 / memcmp 结果），
                        // `bytes_eq` 返回布尔，其余内建返回单元值
                        let rt = if callee == "alloc_array" || callee == "alloc_bytes" {
                            LirType::Ptr
                        } else if callee == "hash_value" || callee == "bytes_cmp" {
                            LirType::I64
                        } else if callee == "bytes_eq" {
                            LirType::Bool
                        } else {
                            LirType::Unit
                        };
                        changed |= set_type(&mut ty, t, rt)?;
                    }
                    // 用户函数调用：跨函数类型在全部推断完成后解析
                    MirStmt::Call { .. } => {}
                    // 间接调用：签名类型名已显式携带，直接解析登记
                    MirStmt::CallIndirect {
                        target,
                        ret_name,
                        callee,
                        ..
                    } => {
                        if let Some(t) = target {
                            changed |= set_type(&mut ty, t, parse_extern_type(ret_name))?;
                        }
                        // 函数指针变量统一为指针槽
                        changed |= set_type(&mut ty, callee, LirType::Ptr)?;
                    }
                    MirStmt::Alloc {
                        target,
                        by_value,
                        slots,
                        is_strfat,
                        ..
                    } => {
                        // V2-C（方案 A）：StrFat 双槽推断——仅 typecheck 标记的
                        // `is_strfat`（`as_str`/`as_str_range` 构造的 `{data,len}`）
                        // 推断为 StrFat。普通双槽 by_value 结构体（如 `Point{x,y}`）
                        // 不标记，保持 Ptr（避免 addr_of_field_index 误判）。
                        let lir = if *is_strfat && *by_value && *slots == 2 {
                            LirType::StrFat
                        } else {
                            LirType::Ptr
                        };
                        changed |= set_type(&mut ty, target, lir)?;
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
                    MirStmt::AddrOf { target, .. } => {
                        // 引用值统一为指针
                        changed |= set_type(&mut ty, target, LirType::Ptr)?;
                    }
                    MirStmt::DerefRead {
                        target,
                        ty: scalar_ty,
                        base,
                        ..
                    } => {
                        changed |= set_type(&mut ty, target, field_scalar_to_lir(*scalar_ty))?;
                        // 解引用操作数必须是引用 / 指针（typecheck 保证），
                        // 登记为指针槽：引用参数 / 引用返回值经 `*r` 处补全类型，
                        // 避免返回引用的函数被误推断为 i64 导致调用点槽类型错乱。
                        changed |= set_type(&mut ty, base, LirType::Ptr)?;
                    }
                    MirStmt::DerefWrite { base, .. } => {
                        // 同 DerefRead：解引用写入的基址为引用 / 指针槽
                        changed |= set_type(&mut ty, base, LirType::Ptr)?;
                    }
                    MirStmt::AddrOfField { target, base, .. } => {
                        // 取字段地址：对象为指针槽，目标为指针槽
                        changed |= set_type(&mut ty, base, LirType::Ptr)?;
                        changed |= set_type(&mut ty, target, LirType::Ptr)?;
                    }
                    MirStmt::PtrAdd { target, base, offset, .. } => {
                        // 指针算术：基址/目标为指针槽，偏移为 i64
                        changed |= set_type(&mut ty, base, LirType::Ptr)?;
                        changed |= set_type(&mut ty, offset, LirType::I64)?;
                        changed |= set_type(&mut ty, target, LirType::Ptr)?;
                    }
                    MirStmt::Cast { target, value, to } => {
                        // U6 Cast IR：目标类型按 `to` 名解析（整族→I64 槽、
                        // 浮点→F64 槽、bool→Bool、char→Char）；源值类型由源定义处推断
                        changed |= set_type(&mut ty, target, cast_target_lir_type(to))?;
                        let _ = value;
                    }
                    // 区域指令不产生类型信息
                    MirStmt::RegionEnter { .. }
                    | MirStmt::RegionExit { .. }
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
                MirStmt::CallIndirect {
                    target,
                    callee,
                    args,
                    ..
                } => {
                    if let Some(t) = target {
                        names.push(t.clone());
                    }
                    names.push(callee.clone());
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
                MirStmt::AddrOf { target, operand, .. } => {
                    names.push(target.clone());
                    names.push(operand.clone());
                }
                MirStmt::DerefRead { target, base, .. } => {
                    names.push(target.clone());
                    names.push(base.clone());
                }
                MirStmt::DerefWrite { base, value, .. } => {
                    names.push(base.clone());
                    names.push(value.clone());
                }
                MirStmt::AddrOfField { target, base, .. } => {
                    names.push(target.clone());
                    names.push(base.clone());
                }
                MirStmt::PtrAdd { target, base, offset, .. } => {
                    names.push(target.clone());
                    names.push(base.clone());
                    names.push(offset.clone());
                }
                MirStmt::Cast { target, value, .. } => {
                    names.push(target.clone());
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

/// extern 声明的返回类型（由 extern_sig 解析；缺省为单元类型）。
fn extern_return_type(f: &MirFunction) -> LirType {
    match &f.extern_sig {
        Some((_, ret)) => parse_extern_type(ret),
        None => LirType::Unit,
    }
}

/// 将 extern 签名类型名解析为 `LirType`（与 typecheck `type_to_extern_name` 对应）。
fn parse_extern_type(name: &str) -> LirType {
    match name {
        "i64" | "isize" | "u64" | "usize" => LirType::I64,
        // 32 位整数返回值：ABI 上按 i64 承载（LIR 无 i32 槽），
        // 由 codegen 的 `extern_ret32` 标记生成 i32 declare + sext 清洗。
        "i32" | "u32" => LirType::I64,
        "i8" | "u8" => LirType::Char,
        "f64" => LirType::F64,
        "bool" => LirType::Bool,
        "char" => LirType::Char,
        "string" => LirType::Str,
        // 标准库 `String` 结构体参数：ABI 上按 C 字符串（data 指针）传递，
        // 登记为 `Str` 以便 codegen 在调用点取 data 指针（而非结构体指针）。
        "String" => LirType::Str,
        "()" => LirType::Unit,
        // 引用 / 聚合等一律按指针处理（extern ABI 按值传指针数字）
        _ => LirType::Ptr,
    }
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
        // 函数地址：指针（统一为 i8* 槽）
        MirValue::FnRef(_) => Some(LirType::Ptr),
    }
}

/// 槽值标量种类 → LIR 类型。
/// U6 Cast IR 目标类型解析：`to` 类型名 → LIR 存储类型。
/// 整族（≤64 位）存储统一 64 位槽（I64）；`f32`/`f64` 存储 F64 槽；
/// `bool`→Bool（i1）；`char`→Char（i8）。
fn cast_target_lir_type(to: &str) -> LirType {
    match to {
        "bool" => LirType::Bool,
        "char" => LirType::Char,
        "f32" | "f64" => LirType::F64,
        // i8/u8/i16/u16/i32/u32/i64/u64/isize/usize
        _ => LirType::I64,
    }
}

fn field_scalar_to_lir(ty: FieldScalar) -> LirType {
    match ty {
        FieldScalar::Int => LirType::I64,
        FieldScalar::Float => LirType::F64,
        FieldScalar::Bool => LirType::Bool,
        FieldScalar::Char => LirType::Char,
        FieldScalar::Str => LirType::Str,
        FieldScalar::StrFat => LirType::StrFat,
        FieldScalar::SliceFat => LirType::SliceFat,
        FieldScalar::Ptr => LirType::Ptr,
        // repr(C) 真布局：目标 local 仍为 Rlyeh 宽类型（i64/f64/...），
        // 读取时经 conv 提升、写入时经逆转换降窄。
        FieldScalar::ReprCField { field_ty, conv, .. } => match conv {
            ReprConv::Zext | ReprConv::Sext => LirType::I64,
            ReprConv::Fpext => LirType::F64,
            ReprConv::None => match field_ty {
                "i64" => LirType::I64,
                "double" => LirType::F64,
                "i1" => LirType::Bool,
                "i32" => LirType::Char,
                "i8*" => LirType::Ptr,
                _ => LirType::I64,
            },
        },
        // repr(C) 嵌套聚合子对象：结果是指针（i8*）。
        FieldScalar::ReprCSubPtr { .. } => LirType::Ptr,
    }
}

/// 二元运算结果类型：算术/位运算按操作数（浮点优先），比较 / 逻辑为 `Bool`。
fn binary_result_type(op: HirBinaryOp, lt: Option<LirType>, rt: Option<LirType>) -> LirType {
    match op {
        HirBinaryOp::Add
        | HirBinaryOp::Sub
        | HirBinaryOp::Mul
        | HirBinaryOp::Div
        | HirBinaryOp::Mod
        | HirBinaryOp::BitAnd
        | HirBinaryOp::BitOr
        | HirBinaryOp::BitXor
        | HirBinaryOp::Shl
        | HirBinaryOp::Shr => {
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

/// 用全程序返回类型表解析跨函数调用目标类型。
///
/// 注意：推断阶段对所有未推断变量默认填充 i64（`infer_function_types`
/// 的占位策略），因此这里使用**覆盖**语义而非冲突检测——用户调用 target
/// 的真实类型由 callee 返回类型决定。同时做多遍**副本类型传播**
/// （`x = y` 且 `y` 为 Place 时，`x` 跟随 `y` 的类型），保证
/// `let v = obj.method();` 这类别名变量的类型与调用结果一致。
fn resolve_call_target_types(
    f: &MirFunction,
    ty: &mut HashMap<Local, LirType>,
    ret_types: &HashMap<String, LirType>,
) {
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
}

/// 将单个 MIR 函数降低为 LIR 函数。
fn lower_function(
    f: &MirFunction,
    ty: &HashMap<Local, LirType>,
    return_type: LirType,
    ret_types: &HashMap<String, LirType>,
    param_types: Vec<LirType>,
) -> Result<LirFunction, LirError> {
    if f.is_extern {
        // extern 声明：无函数体，参数类型由 extern_sig 解析，codegen 生成 declare
        let mut params = Vec::with_capacity(f.params.len());
        if let Some((ptys, ret)) = &f.extern_sig {
            for (p, pt) in f.params.iter().zip(ptys) {
                params.push((p.clone(), parse_extern_type(pt)));
            }
            // 返回 i32 的 extern（pthread trylock 等）：标记以便 codegen 生成
            // `declare i32` + 调用后 sext 存槽（规避 int 返回值高位未定义）。
            let extern_ret32 = matches!(ret.as_str(), "i32");
            return Ok(LirFunction {
                name: f.name.clone(),
                params,
                return_type,
                locals: Vec::new(),
                blocks: Vec::new(),
                is_extern: true,
                extern_ret32,
            });
        } else {
            for p in &f.params {
                params.push((p.clone(), LirType::I64));
            }
        }
        return Ok(LirFunction {
            name: f.name.clone(),
            params,
            return_type,
            locals: Vec::new(),
            blocks: Vec::new(),
            is_extern: true,
            extern_ret32: false,
        });
    }

    let mut ty = ty.clone();
    resolve_call_target_types(f, &mut ty, ret_types);

    // V2-D：参数类型优先取跨函数传播的 `param_types`（实参为 StrFat 时参数为
    // StrFat），否则回退局部推断 / i64。同步写回 `ty`（locals），确保函数体内
    // 参数局部变量的 codegen 类型与函数签名一致（否则 `String::len({i8*,i64} %self)`
    // 的 `self.addr` 被标为 `i64`，`self.len` 走通用 FieldGet 读到错误偏移）。
    let params = f
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let pt = param_types
                .get(i)
                .copied()
                .unwrap_or_else(|| ty.get(p).copied().unwrap_or(LirType::I64));
            ty.insert(p.clone(), pt);
            (p.clone(), pt)
        })
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
        is_extern: false,
        extern_ret32: false,
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


mod lower_stmts;
