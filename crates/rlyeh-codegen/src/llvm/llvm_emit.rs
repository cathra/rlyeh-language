//! llvm_emit：LLVM 发射子模块。
//! （由 llvm.rs 的 `impl LlvmEmitter` 拆分而来，保持语义等价）

use super::*;

impl LlvmEmitter {
    pub(super) fn emit_terminator(
        &mut self,
        term: &LirTerminator,
        body: &mut String,
        is_main: bool,
        f: &LirFunction,
    ) -> Result<(), CodegenError> {
        match term {
            LirTerminator::Return(v) => {
                if is_main {
                    body.push_str("  ret i32 0\n");
                    return Ok(());
                }
                match v {
                    Some(x) => {
                        let ty = local_type(f, x);
                        if ty == LirType::Unit {
                            body.push_str("  ret void\n");
                        } else if self
                            .by_value_locals
                            .get(&f.name)
                            .map(|s| s.contains(x))
                            .unwrap_or(false)
                        {
                            // 标量聚合按值返回：load 对象两槽打包 `{i64, i64}`
                            // 按寄存器返回（对象本体在调用方栈槽/被 SROA）。
                            let p = self.reg();
                            body.push_str(&format!("  %{p} = load i8*, i8** %{x}.addr\n"));
                            let a = self.reg();
                            body.push_str(&format!("  %{a} = bitcast i8* %{p} to [2 x i64]*\n"));
                            // 同上：Apple clang 21 不接受聚合类型 GEP 引用局部值，
                            // 改用 i8 字节偏移访问槽 0 / 槽 1。
                            let b0 = self.reg();
                            body.push_str(&format!("  %{b0} = bitcast [2 x i64]* %{a} to i8*\n"));
                            let g0 = self.reg();
                            body.push_str(&format!("  %{g0} = getelementptr i8, i8* %{b0}, i64 0\n"));
                            let p0 = self.reg();
                            body.push_str(&format!("  %{p0} = bitcast i8* %{g0} to i64*\n"));
                            let s0 = self.reg();
                            body.push_str(&format!("  %{s0} = load i64, i64* %{p0}\n"));
                            let b1 = self.reg();
                            body.push_str(&format!("  %{b1} = bitcast [2 x i64]* %{a} to i8*\n"));
                            let g1 = self.reg();
                            body.push_str(&format!("  %{g1} = getelementptr i8, i8* %{b1}, i64 8\n"));
                            let p1 = self.reg();
                            body.push_str(&format!("  %{p1} = bitcast i8* %{g1} to i64*\n"));
                            let s1 = self.reg();
                            body.push_str(&format!("  %{s1} = load i64, i64* %{p1}\n"));
                            let v0 = self.reg();
                            body.push_str(&format!(
                                "  %{v0} = insertvalue {{ i64, i64 }} poison, i64 %{s0}, 0\n"
                            ));
                            let v1 = self.reg();
                            body.push_str(&format!(
                                "  %{v1} = insertvalue {{ i64, i64 }} %{v0}, i64 %{s1}, 1\n"
                            ));
                            body.push_str(&format!("  ret {{ i64, i64 }} %{v1}\n"));
                        } else {
                            let val =
                                self.operand_value(&LirOperand::Local(x.clone()), ty, body, f)?;
                            body.push_str(&format!("  ret {} {val}\n", llvm_type(ty)?));
                        }
                    }
                    None => body.push_str("  ret void\n"),
                }
            }
            LirTerminator::Jump(b) => {
                body.push_str(&format!("  br label %b{b}\n"));
            }
            LirTerminator::CondJump {
                cond,
                then,
                otherwise,
            } => {
                let v =
                    self.operand_value(&LirOperand::Local(cond.clone()), LirType::Bool, body, f)?;
                body.push_str(&format!(
                    "  br i1 {v}, label %b{then}, label %b{otherwise}\n"
                ));
            }
        }
        Ok(())
    }

    pub(super) fn emit_cast(&mut self, v: &str, src: LirType, to: &str, body: &mut String) -> String {
        let (bits, signed, store) = parse_cast_target(to);
        // bool 目标的源规整一律按有符号（fptosi）——icmp ne 只关心非零，
        // 避免 fptoui 对负浮点的 UB
        let signed = signed || store == LirType::Bool;
        // 1) 源规整到 i64（浮点源例外：若目标为浮点则恒等透传，否则 fptosi/fptoui）
        // 注意：`i64v` 统一为带 `%` 的操作数文本（后续指令直接内插）
        let i64v: String = match src {
            LirType::F64 => {
                if store == LirType::F64 {
                    // 浮点恒等（f32/f64 存储统一 64 位槽）：fadd 0.0 制造裸寄存器
                    return self.bare_reg(v, LirType::F64, body);
                }
                let r = self.reg();
                if signed {
                    body.push_str(&format!("  %{r} = fptosi double {v} to i64\n"));
                } else {
                    body.push_str(&format!("  %{r} = fptoui double {v} to i64\n"));
                }
                format!("%{r}")
            }
            LirType::Bool => {
                let r = self.reg();
                body.push_str(&format!("  %{r} = zext i1 {v} to i64\n"));
                format!("%{r}")
            }
            LirType::Char => {
                let r = self.reg();
                body.push_str(&format!("  %{r} = zext i32 {v} to i64\n"));
                format!("%{r}")
            }
            // 整数源直通（64 位槽值已保持符号扩展不变量）
            _ => v.to_string(),
        };
        // 2) 按目标语义类型发射
        match store {
            LirType::F64 => {
                let r = self.reg();
                if signed {
                    body.push_str(&format!("  %{r} = sitofp i64 {i64v} to double\n"));
                } else {
                    body.push_str(&format!("  %{r} = uitofp i64 {i64v} to double\n"));
                }
                r
            }
            LirType::Bool => {
                let r = self.reg();
                body.push_str(&format!("  %{r} = icmp ne i64 {i64v}, 0\n"));
                r
            }
            LirType::Char => {
                let r = self.reg();
                body.push_str(&format!("  %{r} = trunc i64 {i64v} to i32\n"));
                r
            }
            LirType::I64 => {
                if bits >= 64 {
                    // 恒等（目标位宽 ≥64 或同存储）：add 0 制造裸寄存器
                    self.bare_reg(&i64v, LirType::I64, body)
                } else {
                    // 窄化：trunc to iN + sext/zext 回 i64（保持符号扩展不变量）
                    let t = self.reg();
                    let e = self.reg();
                    body.push_str(&format!("  %{t} = trunc i64 {i64v} to i{bits}\n"));
                    if signed {
                        body.push_str(&format!("  %{e} = sext i{bits} %{t} to i64\n"));
                    } else {
                        body.push_str(&format!("  %{e} = zext i{bits} %{t} to i64\n"));
                    }
                    e
                }
            }
            _ => i64v,
        }
    }

    pub(super) fn bare_reg(&mut self, v: &str, ty: LirType, body: &mut String) -> String {
        if let Some(rest) = v.strip_prefix('%') {
            return rest.to_string();
        }
        let r = self.reg();
        match ty {
            LirType::F64 => {
                body.push_str(&format!("  %{r} = fadd double {v}, 0.000000e+00\n"));
            }
            _ => {
                body.push_str(&format!("  %{r} = add i64 {v}, 0\n"));
            }
        }
        r
    }

    pub(super) fn store_to(
        &mut self,
        target: &str,
        reg: &str,
        body: &mut String,
        f: &LirFunction,
    ) -> Result<(), CodegenError> {
        let t_ty = local_type(f, target);
        if t_ty == LirType::Unit {
            return Ok(());
        }
        let t_lt = llvm_type(t_ty)?;
        body.push_str(&format!("  store {t_lt} %{reg}, {t_lt}* %{target}.addr\n"));
        Ok(())
    }

    pub(super) fn operand_value(
        &mut self,
        op: &LirOperand,
        ty: LirType,
        body: &mut String,
        _f: &LirFunction,
    ) -> Result<String, CodegenError> {
        match op {
            LirOperand::Local(name) => {
                let lt = llvm_type(ty)?;
                let r = self.reg();
                body.push_str(&format!("  %{r} = load {lt}, {lt}* %{name}.addr\n"));
                Ok(format!("%{r}"))
            }
            lit => self.literal(lit, ty, body),
        }
    }

    pub(super) fn literal(
        &mut self,
        lit: &LirOperand,
        ty: LirType,
        _body: &mut String,
    ) -> Result<String, CodegenError> {
        match lit {
            LirOperand::Int(i) => Ok(i.to_string()),
            LirOperand::Float(x) => Ok(format!("0x{:016X}", x.to_bits())),
            LirOperand::Char(c) => {
                // char 为 32 位 Unicode 码点（Rust char，0..=0x10FFFF），
                // 允许全部 Unicode（V2 拓宽，不再限 ASCII）。
                Ok((*c as u32).to_string())
            }
            LirOperand::Bool(b) => Ok(if *b { "true" } else { "false" }.to_string()),
            LirOperand::String(s) => self.emit_string_global(s),
            LirOperand::Unit => Err(CodegenError::UnsupportedType {
                ty: LirType::Unit,
                context: "单元值不能作为存储操作数".to_string(),
            }),
            LirOperand::Local(_) => Err(CodegenError::UnsupportedType {
                ty,
                context: "意外的局部变量操作数".to_string(),
            }),
            // 函数地址仅在 Assign 中处理（需被调函数签名做 bitcast）
            LirOperand::FnPtr(_) => Err(CodegenError::UnsupportedType {
                ty,
                context: "意外的函数地址操作数".to_string(),
            }),
        }
    }

    /// 局部值在**类型不同的存储槽之间搬运**时插入转换（当前仅 bool ↔ i64）。
    ///
    /// 背景：`LirStmt::Assign` 按**源**槽类型宽度直接 store，而源槽与目标槽的
    /// LLVM 宽度可能不同——bool 形参与具名 bool 局部的槽是 `i64`（8 字节），
    /// 按值推断出的 bool 临时槽却是 `i1`（1 字节），`store i64 %v, i1* %t` 会
    /// **写穿 1 字节槽破坏相邻栈**（match 守卫值并入 bool 临时槽即经此路径，
    /// 表现为守卫恒假，见 SH-P0-7 P-M1）。
    ///
    /// 返回 `Some(新寄存器名)` 表示已发射转换指令（结果即为 `dst` 类型）；
    /// `None` 表示无需转换，调用方按源类型原样 store。
    pub(super) fn coerce_local_slot(
        &mut self,
        reg: &str,
        src: LirType,
        dst: LirType,
        body: &mut String,
    ) -> Option<String> {
        let c = match (src, dst) {
            (LirType::I64, LirType::Bool) => {
                let r = self.reg();
                body.push_str(&format!("  %{r} = icmp ne i64 %{reg}, 0\n"));
                r
            }
            (LirType::Bool, LirType::I64) => {
                let r = self.reg();
                body.push_str(&format!("  %{r} = zext i1 %{reg} to i64\n"));
                r
            }
            // 其余组合保持原样（现状行为），避免波及既有路径
            _ => return None,
        };
        Some(c)
    }

    pub(super) fn emit_string_global(&mut self, s: &str) -> Result<String, CodegenError> {
        let idx = self.global_counter;
        self.global_counter += 1;
        let name = format!("@.str.{idx}");
        let (escaped, bytes) = escape_llvm_string(s);
        self.globals.push(format!(
            "{name} = private unnamed_addr constant [{len} x i8] c\"{escaped}\\00\"",
            len = bytes + 1
        ));
        Ok(format!(
            "getelementptr inbounds ([{len} x i8], [{len} x i8]* {name}, i64 0, i64 0)",
            len = bytes + 1
        ))
    }

    pub(super) fn emit_string_global_ptr(
        &mut self,
        body: &mut String,
        s: &str,
    ) -> Result<String, CodegenError> {
        let idx = self.global_counter;
        self.global_counter += 1;
        let name = format!("@.str.{idx}");
        let (escaped, bytes) = escape_llvm_string(s);
        self.globals.push(format!(
            "{name} = private unnamed_addr constant [{len} x i8] c\"{escaped}\\00\"",
            len = bytes + 1
        ));
        let reg = self.reg();
        body.push_str(&format!(
            "  %{reg} = bitcast [{len} x i8]* {name} to i8*\n",
            len = bytes + 1
        ));
        Ok(format!("%{reg}"))
    }

    pub(super) fn emit_fmt_global(&mut self, s: &str) -> Result<String, CodegenError> {
        let idx = self.global_counter;
        self.global_counter += 1;
        let name = format!("@.fmt.{idx}");
        let (escaped, bytes) = escape_llvm_string(s);
        self.globals.push(format!(
            "{name} = private unnamed_addr constant [{len} x i8] c\"{escaped}\\00\"",
            len = bytes + 1
        ));
        Ok(format!(
            "getelementptr inbounds ([{len} x i8], [{len} x i8]* {name}, i64 0, i64 0)",
            len = bytes + 1
        ))
    }

    pub(super) fn emit_bool_strings(&mut self) -> Result<(String, String), CodegenError> {
        Ok((
            self.emit_string_global("true")?,
            self.emit_string_global("false")?,
        ))
    }}
