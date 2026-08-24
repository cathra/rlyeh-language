//! LIR → LLVM IR 文本生成。
//!
//! ## 后端策略（MVP）
//!
//! - **非 SSA 槽式存储**：每个局部变量在入口块 `alloca` 一个槽，访问时
//!   `load` / `store`。这天然支持分支合并（各分支写同一变量、合并块读取），
//!   无需构造 φ 节点；`-O1` 及以上的 `mem2reg` 可将其提升为 SSA；
//! - **内建函数**：`print` / `println` 映射为 `printf` 调用（参数按类型分派
//!   格式串）；`true` / `false` 通过 `select` 选择字符串指针；
//! - **区域标注指令**：MVP 后端直接忽略（分配语义由后续后端 / 运行时提供）；
//! - **`main` 特化**：Zeta 的 `main` 生成 `define i32 @main()`（返回 0）。

use std::collections::HashMap;

use zeta_lir::{FieldScalar, LirFunction, LirOperand, LirProgram, LirStmt, LirTerminator, LirType, Local};

use crate::error::CodegenError;

/// 内建函数名（与 `zeta-lir::lower::BUILTIN_FUNCTIONS` 保持一致）。
/// libc 变参函数白名单（R 阶段，2026-08）：这些符号在 libc 中按
/// `(int, int, ...)` 等**变参原型**声明，而 Zeta extern 只能表达固定参数。
/// 元组第二项 = libc 原型的**固定形参个数**（fcntl = 2：fd、cmd）。
///
/// 若把 declare 写成固定参数（`(i64, i64, i64)`），LLVM 调用点不会生成 SysV
/// ABI 的寄存器保存区（reg_save_area）复制与 `%al` 设置；libc 内部 va_start
/// 仍按变参从保存区读第 3 个参数，读到残留垃圾值（fcntl F_SETFL 的 arg 曾
/// 读到随机 flags 导致行为不稳定）。若把声明写成 `(i64, i64, i64, ...)`
/// （3 个"固定"参数），LLVM 会认为 3 个参数全部走寄存器而不写保存区，
/// 同样错位。
///
/// 正确做法：只保留 libc 的固定形参个数（fcntl = 2），其余 Zeta 参数并入
/// `...` 变参——LLVM 生成变参调用序（复制到保存区 + 设 %al），libc 的
/// va_arg 从保存区读到正确值。
const VARIADIC_EXTERNS: &[(&str, usize)] = &[("fcntl", 2)];

const BUILTIN_FUNCTIONS: &[&str] = &[
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
];

/// 生成 LLVM IR 文本。
pub fn generate_llvm(program: &LirProgram) -> Result<String, CodegenError> {
    let mut emitter = LlvmEmitter::new(program);
    for f in &program.functions {
        emitter.emit_function(f)?;
    }

    let mut out = String::new();
    out.push_str("; ModuleID = 'zeta'\n");
    out.push_str("declare i32 @printf(i8*, ...)\n");
    // POSIX `dprintf(fd, fmt, ...)`：eprint/eprintln 直写 fd 2（stderr）。
    // 不引用 `stderr` 符号（macOS 为 `__stderrp`，不可移植）；WASI 亦提供 dprintf。
    out.push_str("declare i32 @dprintf(i32, i8*, ...)\n");
    out.push_str("declare i8* @malloc(i64)\n");
    out.push_str("declare void @free(i8*)\n");
    out.push_str("declare void @llvm.memcpy.p0i8.p0i8.i64(i8*, i8*, i64, i1)\n");
    out.push_str("declare i32 @memcmp(i8*, i8*, i64)\n");
    // L3 region 接线：zeta-region-alloc 运行时（driver 链接 libzeta_region_alloc.a）
    out.push_str("declare i8* @zeta_region_enter(i8*, i64, i64, i8, double, i8, i8, i8)\n");
    out.push_str("declare i8* @zeta_region_alloc(i8*, i64, i64)\n");
    out.push_str("declare void @zeta_region_transfer(i8*, i8*)\n");
    out.push_str("declare void @zeta_region_exit(i8*)\n");
    for g in &emitter.globals {
        out.push_str(g);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&emitter.body);
    Ok(out)
}

/// LLVM IR 生成器状态。
struct LlvmEmitter {
    /// 函数签名表：名称 → (参数类型, 返回类型, 是否 extern, extern 返回 i32)
    sigs: HashMap<String, (Vec<LirType>, LirType, bool, bool)>,
    /// 收集的全局常量定义
    globals: Vec<String>,
    /// 全局常量 / 格式串计数器
    global_counter: usize,
    /// 临时寄存器计数器
    reg_counter: usize,
    /// 生成的函数体（累积）
    body: String,
}

impl LlvmEmitter {
    fn new(program: &LirProgram) -> Self {
        let sigs = program
            .functions
            .iter()
            .map(|f| {
                let params = f.params.iter().map(|(_, t)| *t).collect::<Vec<_>>();
                (
                    f.name.clone(),
                    (params, f.return_type, f.is_extern, f.extern_ret32),
                )
            })
            .collect();
        Self {
            sigs,
            globals: Vec::new(),
            global_counter: 0,
            reg_counter: 0,
            body: String::new(),
        }
    }

    /// 分配一个新的命名寄存器 `%rN`（跨函数递增，保证唯一）。
    ///
    /// 不使用裸数字 `%N`：LLVM 中命名寄存器也占用函数内的数值序列，
    /// 混用数字编号会触发 "instruction expected to be numbered" 错误。
    fn reg(&mut self) -> String {
        let r = self.reg_counter;
        self.reg_counter += 1;
        format!("r{r}")
    }

    /// 生成单个函数定义（extern 声明生成 `declare`）。
    fn emit_function(&mut self, f: &LirFunction) -> Result<(), CodegenError> {
        if f.is_extern {
            // `__zeta_` 前缀为驱动注入的平台内建（如 `__zeta_target_os`）：
            // 跳过 declare——driver 在汇编阶段追加 `define internal`（同符号 declare+define 冲突）。
            if f.name.starts_with("__zeta_") {
                return Ok(());
            }
            // extern 声明：返回 i32 的（pthread trylock 等）按 i32 声明，
            // 调用点经 `sext i32` 清洗后存入 i64 槽（规避 int 返回值高位未定义）。
            let ret_ty = if f.extern_ret32 {
                "i32".to_string()
            } else if f.return_type == LirType::Unit {
                "void".to_string()
            } else {
                llvm_type(f.return_type)?.to_string()
            };
            let mut params = f
                .params
                .iter()
                .map(|(name, ty)| Ok(format!("{} %{name}", llvm_type(*ty)?)))
                .collect::<Result<Vec<_>, CodegenError>>()?;
            // libc 变参函数（fcntl 等）：只保留 libc 原型的固定形参个数，
            // 其余参数并入 `...` 变参（见 VARIADIC_EXTERNS 注释：
            // 固定声明会导致变参 ABI 错位，libc va_arg 读到垃圾值）。
            let mut variadic = "";
            if let Some((_, fixed)) = VARIADIC_EXTERNS
                .iter()
                .find(|(n, _)| *n == f.name.as_str())
            {
                params.truncate(*fixed);
                variadic = if params.is_empty() { "..." } else { ", ..." };
            }
            let params_str = params.join(", ");
            self.body.push_str(&format!(
                "declare {ret_ty} @{}({params_str}{variadic})\n",
                llvm_global_name(&f.name)
            ));
            return Ok(());
        }

        let is_main = f.name == "main";
        if is_main && !f.params.is_empty() {
            return Err(CodegenError::InvalidMain);
        }

        let ret_ty = if is_main {
            "i32".to_string()
        } else if f.return_type == LirType::Unit {
            "void".to_string()
        } else {
            llvm_type(f.return_type)?.to_string()
        };
        let params = f
            .params
            .iter()
            .map(|(name, ty)| Ok(format!("{} %{name}", llvm_type(*ty)?)))
            .collect::<Result<Vec<_>, CodegenError>>()?
            .join(", ");

        let mut body = String::new();
        body.push_str(&format!(
            "define {ret_ty} @{}({params}) {{\n",
            llvm_global_name(&f.name)
        ));

        // 入口块：为全部局部变量分配槽，存储参数
        body.push_str("entry:\n");
        for (name, ty) in &f.locals {
            if *ty == LirType::Unit {
                continue;
            }
            let lt = llvm_type(*ty)?;
            body.push_str(&format!("  %{name}.addr = alloca {lt}\n"));
        }
        for (name, ty) in &f.params {
            let lt = llvm_type(*ty)?;
            body.push_str(&format!("  store {lt} %{name}, {lt}* %{name}.addr\n"));
        }

        // L3 region 接线：按区域名预分配区域句柄槽（匿名区域已由 MIR lower
        // 赋唯一内部名；同名嵌套区域 MVP 不支持，区域名须在函数内唯一）。
        let mut region_keys: Vec<String> = Vec::new();
        for b in &f.blocks {
            for s in &b.stmts {
                if let LirStmt::RegionEnter { name: Some(n), .. } = s {
                    if !region_keys.contains(n) {
                        region_keys.push(n.clone());
                    }
                }
            }
        }
        for key in &region_keys {
            body.push_str(&format!("  %{key}.rh = alloca i8*\n"));
        }

        // 基本块
        for (i, block) in f.blocks.iter().enumerate() {
            if i > 0 {
                body.push_str(&format!("b{i}:\n"));
            }
            for stmt in &block.stmts {
                self.emit_stmt(stmt, &mut body, f)?;
            }
            self.emit_terminator(&block.terminator, &mut body, is_main, f)?;
        }
        body.push_str("}\n\n");
        self.body.push_str(&body);
        Ok(())
    }

    /// 翻译单条 LIR 指令。
    fn emit_stmt(
        &mut self,
        stmt: &LirStmt,
        body: &mut String,
        f: &LirFunction,
    ) -> Result<(), CodegenError> {
        match stmt {
            LirStmt::Assign { target, value } => {
                let ty = local_type(f, target);
                if ty == LirType::Unit {
                    return Ok(());
                }
                let lt = llvm_type(ty)?;
                if let LirOperand::Local(src) = value {
                    let src_ty = local_type(f, src);
                    if src_ty == LirType::Unit {
                        return Ok(());
                    }
                    let lt_src = llvm_type(src_ty)?;
                    let r = self.reg();
                    body.push_str(&format!("  %{r} = load {lt_src}, {lt_src}* %{src}.addr\n"));
                    body.push_str(&format!("  store {lt_src} %{r}, {lt}* %{target}.addr\n"));
                } else if let LirOperand::FnPtr(name) = value {
                    // 函数地址：按被调函数签名 bitcast 为 i8* 存入指针槽（统一函数指针表示）
                    let r = self.reg();
                    let (pt, rt, _, _) =
                        self.sigs
                            .get(name)
                            .cloned()
                            .ok_or_else(|| CodegenError::UndefinedFunction {
                                name: name.clone(),
                            })?;
                    let fnty = fn_llvm_type(&pt, rt)?;
                    let gn = llvm_global_name(name);
                    body.push_str(&format!("  %{r} = bitcast {fnty} @{gn} to i8*\n"));
                    body.push_str(&format!("  store i8* %{r}, {lt}* %{target}.addr\n"));
                } else {
                    let lit = self.literal(value, ty, body)?;
                    body.push_str(&format!("  store {lt} {lit}, {lt}* %{target}.addr\n"));
                }
            }
            LirStmt::Binary {
                target,
                op,
                ty,
                lhs,
                rhs,
            } => {
                // ty 为操作数类型；比较 / 逻辑的结果类型为 bool
                let lt = llvm_type(*ty)?;
                let lhs_v = self.operand_value(lhs, *ty, body, f)?;
                let rhs_v = self.operand_value(rhs, *ty, body, f)?;
                let instr = binary_instr(op, *ty)?;
                let r = self.reg();
                body.push_str(&format!("  %{r} = {instr} {lt} {lhs_v}, {rhs_v}\n"));
                self.store_to(target, &r, body, f)?;
            }
            LirStmt::Unary {
                target,
                op,
                ty,
                operand,
            } => {
                let v = self.operand_value(operand, *ty, body, f)?;
                let r = self.reg();
                match op {
                    zeta_lir::HirUnaryOp::Neg => {
                        if *ty == LirType::F64 {
                            body.push_str(&format!("  %{r} = fneg double {v}\n"));
                        } else {
                            body.push_str(&format!("  %{r} = sub i64 0, {v}\n"));
                        }
                    }
                    zeta_lir::HirUnaryOp::Not => {
                        body.push_str(&format!("  %{r} = xor i1 true, {v}\n"));
                    }
                }
                self.store_to(target, &r, body, f)?;
            }
            LirStmt::Call {
                target,
                callee,
                args,
            } => {
                self.emit_call(target.as_ref(), callee, args, body, f)?;
            }
            LirStmt::CallIndirect {
                target,
                callee,
                args,
                param_tys,
                ret_ty,
            } => {
                self.emit_call_indirect(target.as_ref(), callee, args, param_tys, *ret_ty, body, f)?;
            }
            LirStmt::Alloc { target, slots } => {
                // 堆上分配 slots*8 字节（槽 0 为枚举 tag），返回 i8*
                let r = self.reg();
                body.push_str(&format!(
                    "  %{r} = call i8* @malloc(i64 {})\n",
                    slots * 8
                ));
                let t_lt = llvm_type(LirType::Ptr)?;
                body.push_str(&format!("  store {t_lt} %{r}, {t_lt}* %{target}.addr\n"));
            }
            LirStmt::FieldGet {
                target,
                base,
                index,
                ty,
            } => {
                // 槽偏移 = index*8 字节；GEP 后按槽值标量类型 load
                let lt = field_scalar_llvm(*ty)?;
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
            LirStmt::FieldSet {
                base,
                index,
                value,
                ty,
            } => {
                let lt = field_scalar_llvm(*ty)?;
                let vty = field_scalar_lir(*ty);
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
                let v = self.operand_value(&LirOperand::Local(value.clone()), vty, body, f)?;
                body.push_str(&format!("  store {lt} {v}, {lt}* %{c}\n"));
            }
            LirStmt::IndexGet {
                target,
                base,
                index,
                ty,
                is_str,
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
                        self.store_to(target, &r8, body, f)?;
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
                        body.push_str(&format!("  store i8 {v}, i8* %{c}\n"));
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
                    // 标量：取变量存储槽地址，统一为 i8*
                    let lt = llvm_type(local_type(f, operand))?;
                    let r = self.reg();
                    body.push_str(&format!("  %{r} = bitcast {lt}* %{operand}.addr to i8*\n"));
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
            // L3 region 接线：region 指令 → zeta-region-alloc 运行时调用
            // （区域句柄槽 `%{key}.rh` 已在入口块预分配）
            LirStmt::RegionEnter { name, options } => {
                let key = match name {
                    Some(n) if !n.is_empty() => n.clone(),
                    _ => "__anon".to_string(),
                };
                let handle = format!("{key}.rh");
                // 区域名指针：Apple clang 21 不认 `%r = getelementptr inbounds
                // ([N x i8], ...)` 独立指令（报 "expected type"），改用 bitcast
                // 指令形式取字符串首地址，再以 `i8* %r` 作实参引用。
                let nptr_arg = if key == "__anon" {
                    "i8* null".to_string()
                } else {
                    let nptr_reg = self.emit_string_global_ptr(body, &key)?;
                    format!("i8* {nptr_reg}")
                };
                let nlen = key.len();
                let initial = options.size.unwrap_or(0) as u64;
                let allow_growth = if options.allow_growth { 1 } else { 0 };
                let factor = options.growth_factor.unwrap_or(2.0);
                let factor_s = if factor.fract() == 0.0 {
                    format!("{factor:.1}")
                } else {
                    format!("{factor}")
                };
                let exact = if options.exact { 1 } else { 0 };
                let adaptive = if options.adaptive { 1 } else { 0 };
                let strategy = if options.strategy.is_some() { 1 } else { 0 };
                let r = self.reg();
                body.push_str(&format!(
                    "  %{r} = call i8* @zeta_region_enter({nptr_arg}, i64 {nlen}, i64 {initial}, i8 {allow_growth}, double {factor_s}, i8 {exact}, i8 {adaptive}, i8 {strategy})\n"
                ));
                body.push_str(&format!("  store i8* %{r}, i8** %{handle}\n"));
            }
            LirStmt::RegionExit { name } => {
                let key = match name {
                    Some(n) if !n.is_empty() => n.clone(),
                    _ => "__anon".to_string(),
                };
                let handle = format!("{key}.rh");
                let r = self.reg();
                body.push_str(&format!("  %{r} = load i8*, i8** %{handle}\n"));
                body.push_str(&format!("  call void @zeta_region_exit(i8* %{r})\n"));
            }
            LirStmt::AllocInRegion { target, region, size } => {
                // 仅聚合对象（Ptr 槽）接线：区域内 bump 分配 + 值镜像；
                // 标量 `in 'r` 无区域分配语义（MVP 保持栈上副本）。
                if *size > 0 && local_type(f, target) == LirType::Ptr {
                    let handle = format!("{region}.rh");
                    let r = self.reg();
                    body.push_str(&format!("  %{r} = load i8*, i8** %{handle}\n"));
                    let p = self.reg();
                    body.push_str(&format!(
                        "  %{p} = call i8* @zeta_region_alloc(i8* %{r}, i64 {size}, i64 8)\n"
                    ));
                    let v = self.operand_value(&LirOperand::Local(target.clone()), LirType::Ptr, body, f)?;
                    body.push_str(&format!(
                        "  call void @llvm.memcpy.p0i8.p0i8.i64(i8* %{p}, i8* {v}, i64 {size}, i1 false)\n"
                    ));
                }
            }
            LirStmt::Transfer { place, region } => {
                let handle = format!("{region}.rh");
                let r = self.reg();
                body.push_str(&format!("  %{r} = load i8*, i8** %{handle}\n"));
                let v = self.operand_value(&LirOperand::Local(place.clone()), LirType::Ptr, body, f)?;
                body.push_str(&format!(
                    "  call void @zeta_region_transfer(i8* %{r}, i8* {v})\n"
                ));
            }
        }
        Ok(())
    }

    /// 生成函数调用（内建 → `printf`；用户函数 → `call`）。
    fn emit_call(
        &mut self,
        target: Option<&Local>,
        callee: &str,
        args: &[Local],
        body: &mut String,
        f: &LirFunction,
    ) -> Result<(), CodegenError> {
        if BUILTIN_FUNCTIONS.contains(&callee) {
            return self.emit_builtin_call(target, callee, args, body, f);
        }

        let (param_tys, ret_ty, is_extern, extern_ret32) =
            self.sigs
                .get(callee)
                .cloned()
                .ok_or_else(|| CodegenError::UndefinedFunction {
                    name: callee.to_string(),
                })?;

        let mut arg_v = Vec::with_capacity(args.len());
        for (arg, pt) in args.iter().zip(&param_tys) {
            if is_extern && *pt == LirType::Str {
                // extern 的 `String` 参数：实参是 String 结构体指针（3 槽 data/len/cap），
                // 需取其 data 指针（槽 0）作为 C 字符串/缓冲传给 libc。
                let p = self.operand_value(&LirOperand::Local(arg.clone()), LirType::Ptr, body, f)?;
                let addr = self.reg();
                body.push_str(&format!("  %{addr} = bitcast i8* {p} to i8**\n"));
                let d = self.reg();
                body.push_str(&format!("  %{d} = load i8*, i8** %{addr}\n"));
                arg_v.push(format!("%{d}"));
            } else if *pt == LirType::I64 && local_type(f, arg.as_str()) == LirType::Ptr {
                // 函数指针值（i8* 槽）→ i64 形参（S0 线程入口 `__zeta_thread_spawn(f, 0)`）：
                // 函数指针按地址整数传递，取地址后 ptrtoint 为 i64。
                let p = self.operand_value(&LirOperand::Local(arg.clone()), LirType::Ptr, body, f)?;
                let r = self.reg();
                body.push_str(&format!("  %{r} = ptrtoint i8* {p} to i64\n"));
                arg_v.push(format!("%{r}"));
            } else {
                arg_v.push(self.operand_value(&LirOperand::Local(arg.clone()), *pt, body, f)?);
            }
        }
        let arg_str = arg_v
            .iter()
            .zip(&param_tys)
            .map(|(v, pt)| format!("{} {v}", llvm_type(*pt).unwrap()))
            .collect::<Vec<_>>()
            .join(", ");

        let callee_name = llvm_global_name(callee);
        // libc 变参函数（fcntl 等）：调用点需带**显式变参函数类型**
        // `({固定参数类型}, ...)`。仅靠变参 declare 不够——LLVM 对
        // `call i32 @fcntl(...)`（隐式类型）不会触发变参调用序，ARM64
        // AAPCS64 会把变参留在寄存器 x2，而 libc va_start 从调用者栈的
        // 参数区读取，得到垃圾值；显式类型才生成「变参复制到栈」。
        let variadic_ty: String =
            VARIADIC_EXTERNS
                .iter()
                .find(|(n, _)| *n == callee)
                .map(|(_, fixed)| {
                    let fixed_tys = param_tys
                        .iter()
                        .take(*fixed)
                        .map(|t| llvm_type(*t).map(|s| s.to_string()))
                        .collect::<Result<Vec<_>, _>>()?;
                    let fp = fixed_tys.join(", ");
                    let sep = if fp.is_empty() { "" } else { ", " };
                    Ok::<_, CodegenError>(format!(" ({fp}{sep}...)"))
                })
                .transpose()?
                .unwrap_or_default();
        if ret_ty == LirType::Unit {
            body.push_str(&format!("  call void{variadic_ty} @{callee_name}({arg_str})\n"));
        } else if is_extern && extern_ret32 {
            // extern 返回 i32（pthread trylock 等）：call i32 后 sext 到 i64
            // 存入 i64 槽，得到干净的 32 位符号扩展值（规避高位未定义）。
            let r = self.reg();
            body.push_str(&format!(
                "  %{r} = call i32{variadic_ty} @{callee_name}({arg_str})\n"
            ));
            let e = self.reg();
            body.push_str(&format!("  %{e} = sext i32 %{r} to i64\n"));
            if let Some(t) = target {
                body.push_str(&format!("  store i64 %{e}, i64* %{t}.addr\n"));
            }
        } else {
            let r = self.reg();
            body.push_str(&format!(
                "  %{r} = call {}{variadic_ty} @{callee_name}({arg_str})\n",
                llvm_type(ret_ty)?
            ));
            if let Some(t) = target {
                let lt = llvm_type(ret_ty)?;
                body.push_str(&format!("  store {lt} %{r}, {lt}* %{t}.addr\n"));
            }
        }
        Ok(())
    }

    /// 生成函数指针间接调用：`call {ret} %cast(args)`。
    ///
    /// 函数指针统一存储为 `i8*`（槽式存储），调用前按被调函数签名
    /// `bitcast` 为 `{ret}({params})*` 再做间接调用。
    #[allow(clippy::too_many_arguments)]
    fn emit_call_indirect(
        &mut self,
        target: Option<&Local>,
        callee: &Local,
        args: &[Local],
        param_tys: &[LirType],
        ret_ty: LirType,
        body: &mut String,
        f: &LirFunction,
    ) -> Result<(), CodegenError> {
        // 函数指针变量：load i8*
        let fp = self.operand_value(
            &LirOperand::Local(callee.clone()),
            LirType::Ptr,
            body,
            f,
        )?;
        // bitcast i8* → {ret}({params})*
        let fnty = fn_llvm_type(param_tys, ret_ty)?;
        let cast = self.reg();
        body.push_str(&format!("  %{cast} = bitcast i8* {fp} to {fnty}\n"));
        // 实参（不特判 extern String：间接调用目标为 Zeta 函数，
        // String 参数是结构体指针，签名类型名解析一致）
        let mut arg_v = Vec::with_capacity(args.len());
        for (arg, pt) in args.iter().zip(param_tys) {
            arg_v.push(self.operand_value(&LirOperand::Local(arg.clone()), *pt, body, f)?);
        }
        let arg_str = arg_v
            .iter()
            .zip(param_tys)
            .map(|(v, pt)| format!("{} {v}", llvm_type(*pt).unwrap()))
            .collect::<Vec<_>>()
            .join(", ");
        if ret_ty == LirType::Unit {
            body.push_str(&format!("  call void %{cast}({arg_str})\n"));
        } else {
            let r = self.reg();
            body.push_str(&format!(
                "  %{r} = call {} %{cast}({arg_str})\n",
                llvm_type(ret_ty)?
            ));
            if let Some(t) = target {
                let lt = llvm_type(ret_ty)?;
                body.push_str(&format!("  store {lt} %{r}, {lt}* %{t}.addr\n"));
            }
        }
        Ok(())
    }

    /// 生成内建调用：
    /// `print` / `println` → `printf`；`alloc_array` → `malloc`；
    /// `array_copy` → `llvm.memcpy`；`array_free` → `free`。
    fn emit_builtin_call(
        &mut self,
        target: Option<&Local>,
        callee: &str,
        args: &[Local],
        body: &mut String,
        f: &LirFunction,
    ) -> Result<(), CodegenError> {
        // 动态数组分配：target = malloc(n * 8)（每槽 8 字节，与数组元素步长一致）
        if callee == "alloc_array" {
            let n =
                self.operand_value(&LirOperand::Local(args[0].clone()), LirType::I64, body, f)?;
            let bytes = self.reg();
            body.push_str(&format!("  %{bytes} = mul i64 {n}, 8\n"));
            let r = self.reg();
            body.push_str(&format!("  %{r} = call i8* @malloc(i64 %{bytes})\n"));
            if let Some(t) = target {
                body.push_str(&format!("  store i8* %{r}, i8** %{t}.addr\n"));
            }
            return Ok(());
        }
        // 动态数组拷贝：memcpy(dst, src, n * 8)
        if callee == "array_copy" {
            let dst =
                self.operand_value(&LirOperand::Local(args[0].clone()), LirType::Ptr, body, f)?;
            let src =
                self.operand_value(&LirOperand::Local(args[1].clone()), LirType::Ptr, body, f)?;
            let n =
                self.operand_value(&LirOperand::Local(args[2].clone()), LirType::I64, body, f)?;
            let r = self.reg();
            body.push_str(&format!("  %{r} = mul i64 {n}, 8\n"));
            body.push_str(&format!(
                "  call void @llvm.memcpy.p0i8.p0i8.i64(i8* {dst}, i8* {src}, i64 %{r}, i1 false)\n"
            ));
            return Ok(());
        }
        // 动态数组释放：free(p)
        if callee == "array_free" {
            let p = self.operand_value(&LirOperand::Local(args[0].clone()), LirType::Ptr, body, f)?;
            body.push_str(&format!("  call void @free(i8* {p})\n"));
            return Ok(());
        }
        // 字节缓冲分配（String 动态缓冲）：target = malloc(n)（按字节，无步长缩放）
        if callee == "alloc_bytes" {
            let n =
                self.operand_value(&LirOperand::Local(args[0].clone()), LirType::I64, body, f)?;
            let r = self.reg();
            body.push_str(&format!("  %{r} = call i8* @malloc(i64 {n})\n"));
            if let Some(t) = target {
                body.push_str(&format!("  store i8* %{r}, i8** %{t}.addr\n"));
            }
            return Ok(());
        }
        // HashMap 键散列：Knuth 乘法混合散列（wrapping 乘法，MVP 仅支持整数键）
        if callee == "hash_value" {
            let v =
                self.operand_value(&LirOperand::Local(args[0].clone()), LirType::I64, body, f)?;
            let r = self.reg();
            body.push_str(&format!("  %{r} = mul i64 {v}, 2654435761\n"));
            if let Some(t) = target {
                body.push_str(&format!("  store i64 %{r}, i64* %{t}.addr\n"));
            }
            return Ok(());
        }
        // 字节缓冲拷贝：memcpy(dst, src, n)（不经 8 倍缩放）
        if callee == "copy_bytes" {
            let dst =
                self.operand_value(&LirOperand::Local(args[0].clone()), LirType::Ptr, body, f)?;
            let src =
                self.operand_value(&LirOperand::Local(args[1].clone()), LirType::Ptr, body, f)?;
            let n =
                self.operand_value(&LirOperand::Local(args[2].clone()), LirType::I64, body, f)?;
            body.push_str(&format!(
                "  call void @llvm.memcpy.p0i8.p0i8.i64(i8* {dst}, i8* {src}, i64 {n}, i1 false)\n"
            ));
            return Ok(());
        }
        // 字节缓冲相等：`memcmp(a, b, n) == 0`（String 内容比较；
        // 长度相等性由调用方先比较，n 恒为同一长度）
        if callee == "bytes_eq" {
            let a =
                self.operand_value(&LirOperand::Local(args[0].clone()), LirType::Ptr, body, f)?;
            let b =
                self.operand_value(&LirOperand::Local(args[1].clone()), LirType::Ptr, body, f)?;
            let n =
                self.operand_value(&LirOperand::Local(args[2].clone()), LirType::I64, body, f)?;
            let r = self.reg();
            body.push_str(&format!("  %{r} = call i32 @memcmp(i8* {a}, i8* {b}, i64 {n})\n"));
            if let Some(t) = target {
                let eq = self.reg();
                body.push_str(&format!("  %{eq} = icmp eq i32 %{r}, 0\n"));
                body.push_str(&format!("  store i1 %{eq}, i1* %{t}.addr\n"));
            }
            return Ok(());
        }
        // 字节缓冲字典序：`memcmp(a, b, n)` 有符号扩展为 i64（String 排序；
        // 负/零/正 → 小于/等于/大于；前缀相等时长度兜底由 desugar 层处理）
        if callee == "bytes_cmp" {
            let a =
                self.operand_value(&LirOperand::Local(args[0].clone()), LirType::Ptr, body, f)?;
            let b =
                self.operand_value(&LirOperand::Local(args[1].clone()), LirType::Ptr, body, f)?;
            let n =
                self.operand_value(&LirOperand::Local(args[2].clone()), LirType::I64, body, f)?;
            let r = self.reg();
            body.push_str(&format!("  %{r} = call i32 @memcmp(i8* {a}, i8* {b}, i64 {n})\n"));
            if let Some(t) = target {
                let sext = self.reg();
                body.push_str(&format!("  %{sext} = sext i32 %{r} to i64\n"));
                body.push_str(&format!("  store i64 %{sext}, i64* %{t}.addr\n"));
            }
            return Ok(());
        }
        // String 打印：读 String 对象槽 0（data 指针）与槽 1（字节长度），
        // 以 `printf("%.*s", len, data)` 输出（支持任意字节内容，遇 \0 截断）；
        // eprint*_string 输出到 stderr（`fprintf(@stderr, ...)`）。
        let to_stderr = callee == "eprint"
            || callee == "eprintln"
            || callee == "eprint_string"
            || callee == "eprintln_string";
        // 输出调用辅助：`printf(fmt, args...)` / `dprintf(2, fmt, args...)`
        let emit_out = |body: &mut String, fmt: &str, rest: &str| {
            if to_stderr {
                body.push_str(&format!(
                    "  call i32 (i32, i8*, ...) @dprintf(i32 2, i8* {fmt}{rest})\n"
                ));
            } else {
                body.push_str(&format!("  call i32 (i8*, ...) @printf(i8* {fmt}{rest})\n"));
            }
        };
        if callee == "print_string"
            || callee == "println_string"
            || callee == "eprint_string"
            || callee == "eprintln_string"
        {
            let p = self.operand_value(&LirOperand::Local(args[0].clone()), LirType::Ptr, body, f)?;
            let len_a = self.reg();
            body.push_str(&format!(
                "  %{len_a} = getelementptr i8, i8* {p}, i64 8\n"
            ));
            let len_v = self.reg();
            body.push_str(&format!("  %{len_v} = load i64, i64* %{len_a}\n"));
            let len32 = self.reg();
            body.push_str(&format!("  %{len32} = trunc i64 %{len_v} to i32\n"));
            let data_a = self.reg();
            body.push_str(&format!("  %{data_a} = bitcast i8* {p} to i8**\n"));
            let data_v = self.reg();
            body.push_str(&format!("  %{data_v} = load i8*, i8** %{data_a}\n"));
            let nl = if callee == "println_string" || callee == "eprintln_string" {
                "\n"
            } else {
                ""
            };
            let fmt = self.emit_fmt_global(&format!("%.*s{nl}"))?;
            emit_out(body, &fmt, &format!(", i32 %{len32}, i8* %{data_v}"));
            return Ok(());
        }
        let newline = callee == "println" || callee == "eprintln";
        if args.is_empty() {
            // 仅换行
            let fmt = self.emit_fmt_global("\n")?;
            emit_out(body, &fmt, "");
            return Ok(());
        }

        let arg = &args[0];
        let aty = local_type(f, arg);
        let fmt = self.emit_fmt_global(&builtin_fmt(aty, newline))?;
        match aty {
            LirType::Str | LirType::I64 | LirType::F64 | LirType::Ptr => {
                let v = self.operand_value(&LirOperand::Local(arg.clone()), aty, body, f)?;
                emit_out(body, &fmt, &format!(", {} {v}", llvm_type(aty)?));
            }
            LirType::Char => {
                let v = self.operand_value(&LirOperand::Local(arg.clone()), aty, body, f)?;
                let r = self.reg();
                // printf 变参整型提升：i8 → i32
                body.push_str(&format!("  %{r} = zext i8 {v} to i32\n"));
                emit_out(body, &fmt, &format!(", i32 %{r}"));
            }
            LirType::Bool => {
                let v = self.operand_value(&LirOperand::Local(arg.clone()), aty, body, f)?;
                let (t_ptr, f_ptr) = self.emit_bool_strings()?;
                let r = self.reg();
                body.push_str(&format!(
                    "  %{r} = select i1 {v}, i8* {t_ptr}, i8* {f_ptr}\n"
                ));
                emit_out(body, &fmt, &format!(", i8* %{r}"));
            }
            LirType::Unit => {
                emit_out(body, &fmt, "");
            }
        }
        Ok(())
    }

    /// 生成终止符。
    fn emit_terminator(
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

    /// 将寄存器值按目标变量的类型存储到其槽（`target` 为 `Unit` 时忽略）。
    fn store_to(
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

    /// 加载 / 取立即数，返回可内联进指令的操作数字符串。
    fn operand_value(
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

    /// 生成立即数常量表达式（`String` 产生全局常量 + GEP）。
    fn literal(
        &mut self,
        lit: &LirOperand,
        ty: LirType,
        _body: &mut String,
    ) -> Result<String, CodegenError> {
        match lit {
            LirOperand::Int(i) => Ok(i.to_string()),
            LirOperand::Float(x) => Ok(format!("0x{:016X}", x.to_bits())),
            LirOperand::Char(c) => {
                let b = u8::try_from(*c as u32).map_err(|_| CodegenError::UnsupportedType {
                    ty: LirType::Char,
                    context: "MVP 仅支持 ASCII 字符".to_string(),
                })?;
                Ok(b.to_string())
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

    /// 生成字符串全局常量并返回 GEP 表达式。
    fn emit_string_global(&mut self, s: &str) -> Result<String, CodegenError> {
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

    /// 生成字符串全局常量，并以 `bitcast` **指令**形式取其 `i8*` 首地址，
    /// 返回结果寄存器名。
    ///
    /// Apple clang 21 不接受 `%r = getelementptr inbounds ([N x i8], ...)`
    /// 的独立指令形式（报 `expected type`，但同一表达式内联在 `store` 中
    /// 合法），故需独立取地址值的场景（如区域名指针实参）改用 bitcast。
    fn emit_string_global_ptr(
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

    /// 生成格式串全局常量并返回 GEP 表达式。
    fn emit_fmt_global(&mut self, s: &str) -> Result<String, CodegenError> {
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

    /// 生成布尔值的 `true` / `false` 字符串常量，返回（真串 GEP，假串 GEP）。
    fn emit_bool_strings(&mut self) -> Result<(String, String), CodegenError> {
        Ok((
            self.emit_string_global("true")?,
            self.emit_string_global("false")?,
        ))
    }
}

/// 将符号名转为合法的 LLVM 全局标识符。
///
/// 模块扁平化后的符号名（如 `math::add`）含 `::`，不在 LLVM 非引号
/// 标识符字符集内，需使用引号形式 `@"math::add"`。
fn llvm_global_name(name: &str) -> String {
    if name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'$' | b'.' | b'_'))
    {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// 槽值标量种类 → LLVM IR 类型（槽按 8 字节对齐，存取按标量宽度）。
fn field_scalar_llvm(ty: FieldScalar) -> Result<&'static str, CodegenError> {
    Ok(match ty {
        FieldScalar::Int => "i64",
        FieldScalar::Float => "double",
        FieldScalar::Bool => "i1",
        FieldScalar::Char => "i8",
        FieldScalar::Str => "i8*",
        FieldScalar::Ptr => "i8*",
    })
}

/// 槽值标量种类 → LIR 类型。
fn field_scalar_lir(ty: FieldScalar) -> LirType {
    match ty {
        FieldScalar::Int => LirType::I64,
        FieldScalar::Float => LirType::F64,
        FieldScalar::Bool => LirType::Bool,
        FieldScalar::Char => LirType::Char,
        FieldScalar::Str => LirType::Str,
        FieldScalar::Ptr => LirType::Ptr,
    }
}

/// 将 LIR 类型映射为 LLVM IR 类型。
fn llvm_type(ty: LirType) -> Result<&'static str, CodegenError> {
    match ty {
        LirType::I64 => Ok("i64"),
        LirType::F64 => Ok("double"),
        LirType::Bool => Ok("i1"),
        LirType::Char => Ok("i8"),
        LirType::Str => Ok("i8*"),
        LirType::Ptr => Ok("i8*"),
        LirType::Unit => Err(CodegenError::UnsupportedType {
            ty,
            context: "单元类型不能作为存储 / 参数 / 运算类型".to_string(),
        }),
    }
}

/// 生成 LLVM 函数指针类型：`{ret}({param})*`。
fn fn_llvm_type(param_tys: &[LirType], ret_ty: LirType) -> Result<String, CodegenError> {
    let params = param_tys
        .iter()
        .map(|t| llvm_type(*t).map(ToString::to_string))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let ret = llvm_type(ret_ty)?;
    Ok(format!("{ret}({params})*"))
}

/// 二元运算指令映射。
fn binary_instr(op: &zeta_lir::HirBinaryOp, ty: LirType) -> Result<&'static str, CodegenError> {
    use zeta_lir::HirBinaryOp::*;
    let ok = |s: &'static str| Ok(s);
    match (op, ty) {
        (Add, LirType::I64) => ok("add"),
        (Sub, LirType::I64) => ok("sub"),
        (Mul, LirType::I64) => ok("mul"),
        (Div, LirType::I64) => ok("sdiv"),
        (Mod, LirType::I64) => ok("srem"),
        (Add, LirType::F64) => ok("fadd"),
        (Sub, LirType::F64) => ok("fsub"),
        (Mul, LirType::F64) => ok("fmul"),
        (Div, LirType::F64) => ok("fdiv"),
        (Mod, LirType::F64) => ok("frem"),
        // 位运算（整数语义；Shr 用 ashr 算术右移保持有符号语义）
        (BitAnd, LirType::I64) => ok("and"),
        (BitOr, LirType::I64) => ok("or"),
        (BitXor, LirType::I64) => ok("xor"),
        (Shl, LirType::I64) => ok("shl"),
        (Shr, LirType::I64) => ok("ashr"),
        (And, LirType::Bool) => ok("and"),
        (Or, LirType::Bool) => ok("or"),
        (Eq, LirType::I64 | LirType::Bool | LirType::Char) => ok("icmp eq"),
        (Ne, LirType::I64 | LirType::Bool | LirType::Char) => ok("icmp ne"),
        (Lt, LirType::I64 | LirType::Char) => ok("icmp slt"),
        (Le, LirType::I64 | LirType::Char) => ok("icmp sle"),
        (Gt, LirType::I64 | LirType::Char) => ok("icmp sgt"),
        (Ge, LirType::I64 | LirType::Char) => ok("icmp sge"),
        (Eq, LirType::F64) => ok("fcmp oeq"),
        (Ne, LirType::F64) => ok("fcmp one"),
        (Lt, LirType::F64) => ok("fcmp olt"),
        (Le, LirType::F64) => ok("fcmp ole"),
        (Gt, LirType::F64) => ok("fcmp ogt"),
        (Ge, LirType::F64) => ok("fcmp oge"),
        _ => Err(CodegenError::UnsupportedType {
            ty,
            context: format!("运算符 {op:?} 不支持该操作数类型"),
        }),
    }
}

/// 内建打印格式串。
fn builtin_fmt(ty: LirType, newline: bool) -> String {
    let base = match ty {
        LirType::Str => "%s",
        LirType::I64 => "%lld",
        LirType::F64 => "%f",
        LirType::Char => "%c",
        LirType::Bool => "%s",
        LirType::Ptr => "%p",
        LirType::Unit => "",
    };
    if newline {
        format!("{base}\n")
    } else {
        base.to_string()
    }
}

/// 转义 LLVM IR 字符串常量，返回（转义内容，原始字节数）。
fn escape_llvm_string(s: &str) -> (String, usize) {
    let mut out = String::new();
    let mut bytes = 0;
    for b in s.bytes() {
        bytes += 1;
        match b {
            b'\\' => out.push_str("\\\\"),
            // `"` 用十六进制转义（`\"` 在 LLVM 字符串常量中不可靠：`c"\"\00"` 被
            // clang 解析为 `[1 x i8]`，报长度不匹配；`\22` 无歧义）
            b'"' => out.push_str("\\22"),
            b'\n' => out.push_str("\\0A"),
            b'\r' => out.push_str("\\0D"),
            b'\t' => out.push_str("\\09"),
            0x20..=0x7E => out.push(b as char),
            _ => out.push_str(&format!("\\{b:02X}")),
        }
    }
    (out, bytes)
}

/// 查询局部变量类型（未找到时按 i64 兜底，LIR 推断阶段已保证存在）。
fn local_type(f: &LirFunction, name: &str) -> LirType {
    f.locals
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, t)| *t)
        .unwrap_or(LirType::I64)
}
