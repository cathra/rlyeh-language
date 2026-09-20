//! llvm_field：LLVM 发射子模块——字段 / 索引 / 指针 / 解引用语句。
//! （由 llvm.rs 的 `impl LlvmEmitter` 拆分而来，保持语义等价）

use super::*;

impl LlvmEmitter {
    /// `emit_stmt` 的字段 / 索引 / 指针 / 解引用分支下沉。
    ///
    /// `llvm.rs` 的 `emit_stmt` 以 or-pattern 把这 10 种语句**整体**分派到此处，
    /// 故其余变体不可达（违反即报 `CodegenError::Internal`，而非静默跳过发射）。
    pub(super) fn emit_field_stmt(
        &mut self,
        stmt: &LirStmt,
        f: &LirFunction,
        body: &mut String,
    ) -> Result<(), CodegenError> {
        match stmt {
            LirStmt::FieldGet {
                target,
                base,
                index,
                ty,
            } => {
                // 槽偏移 = index*8 字节；GEP 后按槽值标量类型 load
                let lt = field_scalar_llvm(*ty)?;
                // V2-C（2026-08-26，方案 A）：StrFat 值槽直接 GEP `.addr` 读槽
                //（与 FieldSet 对称，不读 `.addr` 值当指针）。
                let base_is_strfat = f
                    .locals
                    .iter()
                    .any(|(n, t)| n == base && *t == LirType::StrFat);
                if base_is_strfat {
                    let c = self.reg();
                    body.push_str(&format!(
                        "  %{c} = getelementptr {{ i8*, i64 }}, {{ i8*, i64 }}* %{base}.addr, i32 0, i32 {index}\n"
                    ));
                    let r = self.reg();
                    body.push_str(&format!("  %{r} = load {lt}, {lt}* %{c}\n"));
                    self.store_to(target, &r, body, f)?;
                } else if let FieldScalar::ReprCField { offset, field_ty, conv } = *ty {
                    // SH-P0-1 E2（repr(C) 真布局）：按 C 字节偏移 GEP，load 窄字段后
                    // 经 conv 提升为 Rlyeh 宽值（目标 local 仍为宽类型，无需改 LirType）。
                    let b = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let s = self.reg();
                    body.push_str(&format!(
                        "  %{s} = getelementptr i8, i8* {b}, i64 {offset}\n"
                    ));
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* %{s} to {field_ty}*\n"));
                    let rn = self.reg();
                    body.push_str(&format!("  %{rn} = load {field_ty}, {field_ty}* %{c}\n"));
                    let rv = match conv {
                        ReprConv::None => rn,
                        ReprConv::Zext | ReprConv::Sext => {
                            let r = self.reg();
                            let ext = if matches!(conv, ReprConv::Zext) { "zext" } else { "sext" };
                            body.push_str(&format!("  %{r} = {ext} {field_ty} %{rn} to i64\n"));
                            r
                        }
                        ReprConv::Fpext => {
                            let r = self.reg();
                            body.push_str(&format!("  %{r} = fpext {field_ty} %{rn} to double\n"));
                            r
                        }
                    };
                    self.store_to(target, &rv, body, f)?;
                } else if let FieldScalar::ReprCSubPtr { offset, .. } = *ty {
                    // repr(C) 嵌套聚合子对象：返回指向 base+offset 的子指针（i8*）
                    let b = self.operand_value(&LirOperand::Local(base.clone()), LirType::Ptr, body, f)?;
                    let s = self.reg();
                    body.push_str(&format!("  %{s} = getelementptr i8, i8* {b}, i64 {offset}\n"));
                    self.store_to(target, &s, body, f)?;
                } else {
                    let b = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let s = self.reg();
                    body.push_str(&format!(
                        "  %{s} = getelementptr i8, i8* {b}, i64 {}\n",
                        index * 8
                    ));
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* %{s} to {lt}*\n"));
                    let r = self.reg();
                    body.push_str(&format!("  %{r} = load {lt}, {lt}* %{c}\n"));
                    self.store_to(target, &r, body, f)?;
                }
            }
            LirStmt::FieldSet {
                base,
                index,
                value,
                ty,
            } => {
                let lt = field_scalar_llvm(*ty)?;
                let vty = field_scalar_lir(*ty);
                // X4：`()` 单元值作为字段值（`Wrapper::Empty(())` / `Result::Ok(())`
                // 的 payload）——`()` 是 0 大小，槽写 0 占位，不 load（Unit 类型
                // 局部变量未分配槽，load `%x.addr` 会未定义）。
                let val_is_unit = f
                    .locals
                    .iter()
                    .any(|(n, t)| n == value && *t == LirType::Unit);
                let v = if val_is_unit {
                    "0".to_string()
                } else {
                    self.operand_value(&LirOperand::Local(value.clone()), vty, body, f)?
                };
                // V2-C（2026-08-26，方案 A）：StrFat 值槽（`.addr` = `{i8*,i64}`）
                // 直接 GEP 写槽——**不读 `.addr` 值当对象指针**（否则 base 的 data
                // 槽被当作指针解引用，写到错误地址，根因见 task-v2.md）。
                let base_is_strfat = f
                    .locals
                    .iter()
                    .any(|(n, t)| n == base && *t == LirType::StrFat);
                if base_is_strfat {
                    let c = self.reg();
                    body.push_str(&format!(
                        "  %{c} = getelementptr {{ i8*, i64 }}, {{ i8*, i64 }}* %{base}.addr, i32 0, i32 {index}\n"
                    ));
                    body.push_str(&format!("  store {lt} {v}, {lt}* %{c}\n"));
                } else if let FieldScalar::ReprCField { offset, field_ty, conv } = *ty {
                    // SH-P0-1 E2（repr(C) 真布局）：按 C 字节偏移 GEP，宽值经逆转换降窄后落内存。
                    let b = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let s = self.reg();
                    body.push_str(&format!("  %{s} = getelementptr i8, i8* {b}, i64 {offset}\n"));
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* %{s} to {field_ty}*\n"));
                    let vn = match conv {
                        ReprConv::None => v,
                        ReprConv::Zext | ReprConv::Sext => {
                            let r = self.reg();
                            body.push_str(&format!("  %{r} = trunc i64 {v} to {field_ty}\n"));
                            format!("%{r}")
                        }
                        ReprConv::Fpext => {
                            let r = self.reg();
                            body.push_str(&format!("  %{r} = fptrunc double {v} to {field_ty}\n"));
                            format!("%{r}")
                        }
                    };
                    body.push_str(&format!("  store {field_ty} {vn}, {field_ty}* %{c}\n"));
                } else if let FieldScalar::ReprCSubPtr { offset, size } = *ty {
                    // SH-P0-1 E2（嵌套聚合内联）：memcpy value（子对象指针）到 base+offset，长度 size 字节
                    let b = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let d = self.reg();
                    body.push_str(&format!("  %{d} = getelementptr i8, i8* {b}, i64 {offset}\n"));
                    let src = self.operand_value(
                        &LirOperand::Local(value.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    body.push_str(&format!(
                        "  call void @llvm.memcpy.p0i8.p0i8.i64(i8* %{d}, i8* {src}, i64 {size}, i1 false)\n"
                    ));
                } else {
                    let b = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let s = self.reg();
                    body.push_str(&format!(
                        "  %{s} = getelementptr i8, i8* {b}, i64 {}\n",
                        index * 8
                    ));
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* %{s} to {lt}*\n"));
                    body.push_str(&format!("  store {lt} {v}, {lt}* %{c}\n"));
                }
            }
            LirStmt::IndexGet {
                target,
                base,
                index,
                ty,
                is_str,
                len,
            } => {
                // 数组元素步长 8 字节；字符串字符步长 1 字节。GEP 后按元素标量类型 load
                let lt = field_scalar_llvm(*ty)?;
                let b = self.operand_value(
                    &LirOperand::Local(base.clone()),
                    LirType::Ptr,
                    body,
                    f,
                )?;
                let i = self.operand_value(
                    &LirOperand::Local(index.clone()),
                    LirType::I64,
                    body,
                    f,
                )?;
                // 边界检查（TCL2/ASIL：消除越界 UB，越界确定性 abort）
                if let Some(len_local) = len {
                    let ln = self.operand_value(
                        &LirOperand::Local(len_local.clone()),
                        LirType::I64,
                        body,
                        f,
                    )?;
                    let neg = self.reg();
                    let over = self.reg();
                    let bad = self.reg();
                    let ok = self.reg();
                    let fail = self.reg();
                    body.push_str(&format!("  %{neg} = icmp slt i64 {i}, 0\n"));
                    body.push_str(&format!("  %{over} = icmp sge i64 {i}, {ln}\n"));
                    body.push_str(&format!("  %{bad} = or i1 %{neg}, %{over}\n"));
                    body.push_str(&format!(
                        "  br i1 %{bad}, label %idx_fail_{fail}, label %idx_ok_{ok}\n"
                    ));
                    body.push_str(&format!("idx_fail_{fail}:\n"));
                    body.push_str("  call void @abort()\n");
                    body.push_str("  unreachable\n");
                    body.push_str(&format!("idx_ok_{ok}:\n"));
                }
                let addr = self.reg();
                if *is_str {
                    body.push_str(&format!("  %{addr} = getelementptr i8, i8* {b}, i64 {i}\n"));
                    // 1 字节元素：字符串字符 / `u8` 字节缓冲。load i8 后按需零扩展
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* %{addr} to i8*\n"));
                    let r8 = self.reg();
                    body.push_str(&format!("  %{r8} = load i8, i8* %{c}\n"));
                    if *ty == FieldScalar::Int {
                        let r = self.reg();
                        body.push_str(&format!("  %{r} = zext i8 %{r8} to i64\n"));
                        self.store_to(target, &r, body, f)?;
                    } else {
                        // 非 Int 元素（如 char）：load 出 i8 字节后零扩展为目标标量
                        // 类型（char 在 LLVM 中以 i32 表示，见 field_scalar_llvm）
                        let r = self.reg();
                        body.push_str(&format!("  %{r} = zext i8 %{r8} to {lt}\n"));
                        self.store_to(target, &r, body, f)?;
                    }
                } else {
                    let scaled = self.reg();
                    body.push_str(&format!("  %{scaled} = mul i64 {i}, 8\n"));
                    body.push_str(&format!(
                        "  %{addr} = getelementptr i8, i8* {b}, i64 %{scaled}\n"
                    ));
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* %{addr} to {lt}*\n"));
                    let r = self.reg();
                    body.push_str(&format!("  %{r} = load {lt}, {lt}* %{c}\n"));
                    self.store_to(target, &r, body, f)?;
                }
            }
            LirStmt::IndexSet {
                base,
                index,
                value,
                ty,
                is_str,
                len,
            } => {
                let lt = field_scalar_llvm(*ty)?;
                let vty = field_scalar_lir(*ty);
                let b = self.operand_value(
                    &LirOperand::Local(base.clone()),
                    LirType::Ptr,
                    body,
                    f,
                )?;
                let i = self.operand_value(
                    &LirOperand::Local(index.clone()),
                    LirType::I64,
                    body,
                    f,
                )?;
                // 边界检查（TCL2/ASIL：消除越界 UB，越界确定性 abort）
                if let Some(len_local) = len {
                    let ln = self.operand_value(
                        &LirOperand::Local(len_local.clone()),
                        LirType::I64,
                        body,
                        f,
                    )?;
                    let neg = self.reg();
                    let over = self.reg();
                    let bad = self.reg();
                    let ok = self.reg();
                    let fail = self.reg();
                    body.push_str(&format!("  %{neg} = icmp slt i64 {i}, 0\n"));
                    body.push_str(&format!("  %{over} = icmp sge i64 {i}, {ln}\n"));
                    body.push_str(&format!("  %{bad} = or i1 %{neg}, %{over}\n"));
                    body.push_str(&format!(
                        "  br i1 %{bad}, label %idx_fail_{fail}, label %idx_ok_{ok}\n"
                    ));
                    body.push_str(&format!("idx_fail_{fail}:\n"));
                    body.push_str("  call void @abort()\n");
                    body.push_str("  unreachable\n");
                    body.push_str(&format!("idx_ok_{ok}:\n"));
                }
                let addr = self.reg();
                if *is_str {
                    body.push_str(&format!("  %{addr} = getelementptr i8, i8* {b}, i64 {i}\n"));
                    // 1 字节元素：store 前按需截断为 i8
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* %{addr} to i8*\n"));
                    let v = self.operand_value(&LirOperand::Local(value.clone()), vty, body, f)?;
                    if *ty == FieldScalar::Int {
                        let v8 = self.reg();
                        body.push_str(&format!("  %{v8} = trunc i64 {v} to i8\n"));
                        body.push_str(&format!("  store i8 %{v8}, i8* %{c}\n"));
                    } else {
                        // 非 Int 元素（如 char）：store 前把目标标量类型的值截断为 i8
                        let v8 = self.reg();
                        body.push_str(&format!("  %{v8} = trunc {lt} {v} to i8\n"));
                        body.push_str(&format!("  store i8 %{v8}, i8* %{c}\n"));
                    }
                } else {
                    let scaled = self.reg();
                    body.push_str(&format!("  %{scaled} = mul i64 {i}, 8\n"));
                    body.push_str(&format!(
                        "  %{addr} = getelementptr i8, i8* {b}, i64 %{scaled}\n"
                    ));
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* %{addr} to {lt}*\n"));
                    let v = self.operand_value(&LirOperand::Local(value.clone()), vty, body, f)?;
                    body.push_str(&format!("  store {lt} {v}, {lt}* %{c}\n"));
                }
            }
            LirStmt::FieldAddr { target, base, index, ty } => {
                // `&obj.field`（V1）：GEP 到字段地址（i8* 槽）——
                // 写回经 DerefWrite(base=target) 直达原字段。repr(C) 用 C 字节偏移。
                let off = if let FieldScalar::ReprCField { offset, .. } = *ty {
                    offset as i64
                } else if let FieldScalar::ReprCSubPtr { offset, .. } = *ty {
                    offset as i64
                } else {
                    (index * 8) as i64
                };
                let b = self.operand_value(
                    &LirOperand::Local(base.clone()),
                    LirType::Ptr,
                    body,
                    f,
                )?;
                let s = self.reg();
                body.push_str(&format!(
                    "  %{s} = getelementptr i8, i8* {b}, i64 {off}\n"
                ));
                self.store_to(target, &s, body, f)?;
            }
            LirStmt::PtrAdd {
                target,
                base,
                offset,
                elem,
                is_str,
            } => {
                // `ptr + n`（V1）：指针推进（元素步长 8 字节 / 字符串 1 字节）
                let b = self.operand_value(
                    &LirOperand::Local(base.clone()),
                    LirType::Ptr,
                    body,
                    f,
                )?;
                let i = self.operand_value(
                    &LirOperand::Local(offset.clone()),
                    LirType::I64,
                    body,
                    f,
                )?;
                let addr = self.reg();
                if *is_str {
                    body.push_str(&format!("  %{addr} = getelementptr i8, i8* {b}, i64 {i}\n"));
                } else {
                    let scaled = self.reg();
                    body.push_str(&format!("  %{scaled} = mul i64 {i}, 8\n"));
                    body.push_str(&format!(
                        "  %{addr} = getelementptr i8, i8* {b}, i64 %{scaled}\n"
                    ));
                }
                let _ = elem;
                self.store_to(target, &addr, body, f)?;
            }
            LirStmt::Cast { target, value, to } => {
                // U6 Cast IR：数值→数值类型转换（`expr as T`）
                // 源存储类型从类型表查询（I64/F64/Bool/Char）
                let src = match value {
                    LirOperand::Local(l) => local_type(f, l),
                    LirOperand::Int(_) => LirType::I64,
                    LirOperand::Float(_) => LirType::F64,
                    LirOperand::Bool(_) => LirType::Bool,
                    _ => LirType::I64,
                };
                let v = self.operand_value(value, src, body, f)?;
                let r = self.emit_cast(&v, src, to, body);
                self.store_to(target, &r, body, f)?;
            }
            LirStmt::AddrOf {
                target,
                operand,
                pointee,
            } => {
                if *pointee == FieldScalar::Ptr {
                    // 聚合对象：对象指针即"地址"，拷贝值
                    let v = self.operand_value(
                        &LirOperand::Local(operand.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    // operand_value 返回带 `%` 前缀，store_to 内部会补 `%`
                    self.store_to(target, &v[1..], body, f)?;
                } else {
                    // 标量：取变量存储槽地址，统一为 i8*。
                    // 全局变量（`static` / `static mut`）无局部栈槽，地址即
                    // data 段符号 `@operand` 本身（M3，0.2.0-V `&GLOBAL`）。
                    let is_global = self.global_types.contains_key(operand);
                    let lt = if is_global {
                        llvm_type(*self.global_types.get(operand).unwrap())?
                    } else {
                        llvm_type(local_type(f, operand))?
                    };
                    let r = self.reg();
                    if is_global {
                        body.push_str(&format!("  %{r} = bitcast {lt}* @{operand} to i8*\n"));
                    } else {
                        body.push_str(&format!("  %{r} = bitcast {lt}* %{operand}.addr to i8*\n"));
                    }
                    self.store_to(target, &r, body, f)?;
                }
            }
            LirStmt::DerefRead { target, base, ty } => {
                if *ty == FieldScalar::Ptr {
                    // 聚合引用：指针拷贝
                    let v = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    // operand_value 返回带 `%` 前缀，store_to 内部会补 `%`
                    self.store_to(target, &v[1..], body, f)?;
                } else if let FieldScalar::ReprCField { field_ty, conv, .. } = *ty {
                    // SH-P0-1：裸指针 u8 解引用——按窄内存类型 load 后提升为宽值
                    // （目标 local 仍为宽类型，无需改 LirType）。
                    let b = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* {b} to {field_ty}*\n"));
                    let rn = self.reg();
                    body.push_str(&format!("  %{rn} = load {field_ty}, {field_ty}* %{c}\n"));
                    let rv = match conv {
                        ReprConv::None => rn,
                        ReprConv::Zext | ReprConv::Sext => {
                            let r = self.reg();
                            let ext = if matches!(conv, ReprConv::Zext) { "zext" } else { "sext" };
                            body.push_str(&format!("  %{r} = {ext} {field_ty} %{rn} to i64\n"));
                            r
                        }
                        ReprConv::Fpext => {
                            let r = self.reg();
                            body.push_str(&format!("  %{r} = fpext {field_ty} %{rn} to double\n"));
                            r
                        }
                    };
                    self.store_to(target, &rv, body, f)?;
                } else if let FieldScalar::ReprCSubPtr { offset, .. } = *ty {
                    // repr(C) 嵌套聚合子对象：返回指向 base+offset 的子指针（i8*）
                    let b = self.operand_value(&LirOperand::Local(base.clone()), LirType::Ptr, body, f)?;
                    let s = self.reg();
                    body.push_str(&format!("  %{s} = getelementptr i8, i8* {b}, i64 {offset}\n"));
                    self.store_to(target, &s, body, f)?;
                } else {
                    // 标量引用：bitcast 后 load
                    let lt = field_scalar_llvm(*ty)?;
                    let b = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* {b} to {lt}*\n"));
                    let r = self.reg();
                    body.push_str(&format!("  %{r} = load {lt}, {lt}* %{c}\n"));
                    self.store_to(target, &r, body, f)?;
                }
            }
            LirStmt::DerefWrite { base, value, ty } => {
                if *ty == FieldScalar::Ptr {
                    // 聚合引用写入：写入对象指针
                    let b = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let v = self.operand_value(
                        &LirOperand::Local(value.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* {b} to i8**\n"));
                    body.push_str(&format!("  store i8* {v}, i8** %{c}\n"));
                } else if let FieldScalar::ReprCField { field_ty, conv, .. } = *ty {
                    // SH-P0-1：裸指针 u8 解引用写入——宽值经逆转换降窄后落内存。
                    let b = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let v = self.operand_value(
                        &LirOperand::Local(value.clone()),
                        field_scalar_lir(*ty),
                        body,
                        f,
                    )?;
                    let vn = match conv {
                        ReprConv::None => v,
                        ReprConv::Zext | ReprConv::Sext => {
                            let r = self.reg();
                            body.push_str(&format!("  %{r} = trunc i64 {v} to {field_ty}\n"));
                            format!("%{r}")
                        }
                        ReprConv::Fpext => {
                            let r = self.reg();
                            body.push_str(&format!("  %{r} = fptrunc double {v} to {field_ty}\n"));
                            format!("%{r}")
                        }
                    };
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* {b} to {field_ty}*\n"));
                    body.push_str(&format!("  store {field_ty} {vn}, {field_ty}* %{c}\n"));
                } else if let FieldScalar::ReprCSubPtr { offset, size } = *ty {
                    // SH-P0-1 E2（嵌套聚合内联）：memcpy value（子对象指针）到 base+offset，长度 size 字节
                    let b = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let d = self.reg();
                    body.push_str(&format!("  %{d} = getelementptr i8, i8* {b}, i64 {offset}\n"));
                    let src = self.operand_value(
                        &LirOperand::Local(value.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    body.push_str(&format!(
                        "  call void @llvm.memcpy.p0i8.p0i8.i64(i8* %{d}, i8* {src}, i64 {size}, i1 false)\n"
                    ));
                } else {
                    // 标量引用写入：bitcast 后 store
                    let lt = field_scalar_llvm(*ty)?;
                    let b = self.operand_value(
                        &LirOperand::Local(base.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    let v = self.operand_value(
                        &LirOperand::Local(value.clone()),
                        field_scalar_lir(*ty),
                        body,
                        f,
                    )?;
                    let c = self.reg();
                    body.push_str(&format!("  %{c} = bitcast i8* {b} to {lt}*\n"));
                    body.push_str(&format!("  store {lt} {v}, {lt}* %{c}\n"));
                }
            }
            _ => {
                return Err(CodegenError::Internal(format!(
                    "emit_field_stmt 收到非字段/索引/指针/解引用语句：{stmt:?}"
                )));
            }
        }
        Ok(())
    }
}
