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
//! - **`main` 特化**：Rlyeh 的 `main` 生成 `define i32 @main()`（返回 0）。

use std::collections::{BTreeSet, HashMap, HashSet};

use rlyeh_lir::{
    FieldScalar, LirBlock, LirConst, LirFunction, LirOperand, LirProgram, LirStmt, LirTerminator,
    LirType, Local, ReprConv,
};

use crate::error::CodegenError;

/// 内建函数名（与 `rlyeh-lir::lower::BUILTIN_FUNCTIONS` 保持一致）。
/// libc 变参函数白名单（R 阶段，2026-08）：这些符号在 libc 中按
/// `(int, int, ...)` 等**变参原型**声明，而 Rlyeh extern 只能表达固定参数。
/// 元组第二项 = libc 原型的**固定形参个数**（fcntl = 2：fd、cmd）。
///
/// 若把 declare 写成固定参数（`(i64, i64, i64)`），LLVM 调用点不会生成 SysV
/// ABI 的寄存器保存区（reg_save_area）复制与 `%al` 设置；libc 内部 va_start
/// 仍按变参从保存区读第 3 个参数，读到残留垃圾值（fcntl F_SETFL 的 arg 曾
/// 读到随机 flags 导致行为不稳定）。若把声明写成 `(i64, i64, i64, ...)`
/// （3 个"固定"参数），LLVM 会认为 3 个参数全部走寄存器而不写保存区，
/// 同样错位。
///
/// 正确做法：只保留 libc 的固定形参个数（fcntl = 2），其余 Rlyeh 参数并入
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
    "panic",
    "mem_swap",
];

/// 指令是否读/写了 `aliases` 中任一对象（用于判定 `AllocInRegion`
/// 回看窗口是否可安全重排）。

/// LIR 变换（region 字面量直接构造）：
/// 把同块内 `Alloc(t)` + `FieldSet(t,..)*` + `AllocInRegion(t)` 的字面量构造
/// 重写为 `AllocInRegionDirect(t)` + `FieldSet(t,..)*`——聚合字段直接在区域
/// 指针上写入，消除中间堆临时分配与值镜像 memcpy（region_alloc 基准热路径
/// 的关键优化；同时修复 `in 'r` 变量仍指向堆临时对象的低效）。
///
/// 匹配规则（保守）：从 `AllocInRegion` 向前回看，窗口内允许
/// `FieldSet(t,..)` 与不读写 `t` 的纯计算指令（字段值求值）；
/// 窗口起点须为 `Alloc(t)` 且槽数对应（`slots × 8 == size`）。
/// 命中后：窗口内的 `FieldSet(t,..)` 全部移到 `AllocInRegionDirect` 之后
/// （直接写区域指针），其余指令保持原位置；否则保持原样（走 memcpy 路径）。

/// 生成 LLVM IR 文本。

/// 循环级 region 状态提升（热路径优化，见 `find_loop_promo`）：
///
/// 识别「规范 while 循环内恰一个区域 bump 点」的模式，把 region 的
/// base/cursor/limit 在循环 preheader 快照为 SSA 值（`%pcur`/`%pbase`/
/// `%plim`），循环 header 用 phi（`%hcur`/`%hbase`/`%hlim`）维护，
/// bump 快路径仅寄存器运算（零访存），慢路径 call 后 reload 并经 join
/// 的 phi（`%ncur`/`%nbase`/`%nlim`）作为回边值，循环退出（header
/// 条件为假跳出）时把 `%hcur` 写回 Region 头 cursor。
///
/// 背景：内联 bump 慢路径 `call @rlyeh_region_alloc` 会改写
/// base/cursor/limit（grow 时 `self.base/limit/cursor` 均更新），LLVM
/// 无法证明调用不写 Region 头，因此 LICM 无法把 base/limit/cursor 的
/// load 提升出循环——每次分配都重复 3 次访存。本提升在 IR 层自行维护
/// 状态（SSA phi），不依赖 LLVM 别名分析，热路径访存归零。

pub fn generate_llvm(program: &LirProgram) -> Result<String, CodegenError> {
    // region 字面量直接构造变换（须在 emit 前完成：预扫描/emit 按变换后指令消费）
    let mut functions: Vec<LirFunction> = Vec::with_capacity(program.functions.len());
    for f in &program.functions {
        let mut f2 = f.clone();
        inline_region_literal(&mut f2);
        functions.push(f2);
    }
    let program2 = LirProgram {
        functions,
        globals: program.globals.clone(),
    };
    let mut emitter = LlvmEmitter::new(&program2);
    for f in &program2.functions {
        emitter.emit_function(f)?;
    }

    let mut out = String::new();
    out.push_str("; ModuleID = 'rlyeh'\n");
    out.push_str("declare i32 @printf(i8*, ...)\n");
    // POSIX `dprintf(fd, fmt, ...)`：eprint/eprintln 直写 fd 2（stderr）。
    // 不引用 `stderr` 符号（macOS 为 `__stderrp`，不可移植）；WASI 亦提供 dprintf。
    out.push_str("declare i32 @dprintf(i32, i8*, ...)\n");
    out.push_str("declare void @abort()\n");
    out.push_str("declare i8* @malloc(i64)\n");
    // calloc 预置声明必须**早于所有调用点**（LLVM IR parser 对 call 自动创建的
    // 隐式声明与后续显式 declare 视为 redefinition 报错）。标准库的
    // `extern fn calloc -> i64` 声明由 emit_function 跳过（见下），避免重复。
    // 返回值为 i64，内置分配点经 inttoptr 转 i8*。
    out.push_str("declare i64 @calloc(i64, i64)\n");
    out.push_str("declare void @free(i8*)\n");
    out.push_str("declare void @llvm.memcpy.p0i8.p0i8.i64(i8*, i8*, i64, i1)\n");
    out.push_str("declare i32 @memcmp(i8*, i8*, i64)\n");
    // L3 region 接线：rlyeh-region-alloc 运行时（driver 链接 librlyeh_region_alloc.a）
    // noalias：enter/alloc 返回新分配内存（Box<Region> / 新块），与入参不 alias；
    // nounwind：extern "C" 运行时不 unwind（C ABI 默认，显式标注供优化器推理）。
    // 注意：不可标 argmemonly/readnone——运行时内部走 std::alloc（系统堆）。
    out.push_str("declare noalias i8* @rlyeh_region_enter(i8*, i64, i64, i8, double, i8, i8, i8) nounwind\n");
    out.push_str("declare noalias i8* @rlyeh_region_alloc(i8*, i64, i64) nounwind\n");
    out.push_str("declare void @rlyeh_region_transfer(i8*, i8*) nounwind\n");
    out.push_str("declare void @rlyeh_region_exit(i8*) nounwind\n");
    // —— TBAA 访问域（module 级 metadata）——
    // !0 根 → !1 内存域 → !2 Region 头、!3 Region 体。
    // 打标规则（保守，防 UB）：仅 Region 头字段 load/store 标 !2、值镜像
    // memcpy 标 !3；用户内存（栈/全局/堆对象）一律不标——Rlyeh 数组索引 /
    // `&` 引用存在别名共享，用户对象间打标才是别名逃逸源头。Region 头
    // 物理上独立于 bump 块与用户内存（Box<Region> ≠ std::alloc 块），
    // 标 !2/!3 后 LLVM 可证「Region 头与用户数据不重叠」，重排/消除 reload。
    out.push_str("!0 = !{!\"rlyeh_root\"}\n");
    out.push_str("!1 = !{!\"rlyeh_mem\", !0}\n");
    out.push_str("!2 = !{!\"rlyeh_region_header\", !1}\n");
    out.push_str("!3 = !{!\"rlyeh_region_body\", !1}\n");
    // 全局变量（`static` / `static mut`）定义：发射为 data 段符号。
    // 不可变发射为 `constant`，可变发射为 `global`；均带对齐以满足 LLVM 要求。
    for g in &program.globals {
        let lt = llvm_type(g.type_)?;
        let init = match &g.init {
            LirConst::I64(v) => format!("{lt} {v}"),
            LirConst::F64(v) => format!("double 0x{:016X}", v.to_bits()),
            LirConst::Bool(v) => format!("i1 {}", if *v { "true" } else { "false" }),
            LirConst::Char(v) => format!("i32 {}", *v as u32),
        };
        let kind = if g.is_mut { "global" } else { "constant" };
        out.push_str(&format!("@{name} = {kind} {init}, align 8\n", name = g.name));
    }
    for g in &emitter.globals {
        out.push_str(g);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&emitter.body);
    Ok(out)
}

/// 循环级 region 状态提升（热路径优化，见 `find_loop_promo`）：
///
/// 识别「规范 while 循环内恰一个区域 bump 点」的模式，把 region 的
/// base/cursor/limit 在循环 preheader 快照为 SSA 值（`%pcur`/`%pbase`/
/// `%plim`），循环 header 用 phi（`%hcur`/`%hbase`/`%hlim`）维护，
/// bump 快路径仅寄存器运算（零访存），慢路径 call 后 reload 并经 join
/// 的 phi（`%ncur`/`%nbase`/`%nlim`）作为回边值，循环退出（header
/// 条件为假跳出）时把 `%hcur` 写回 Region 头 cursor。
///
/// 背景：内联 bump 慢路径 `call @rlyeh_region_alloc` 会改写
/// base/cursor/limit（grow 时 `self.base/limit/cursor` 均更新），LLVM
/// 无法证明调用不写 Region 头，因此 LICM 无法把 base/limit/cursor 的
/// load 提升出循环——每次分配都重复 3 次访存。本提升在 IR 层自行维护
/// 状态（SSA phi），不依赖 LLVM 别名分析，热路径访存归零。
#[derive(Clone)]

struct PromoCtx {
    /// 循环头块（被回边指向；条件检查所在）
    header: usize,
    /// 循环前驱块（header 的唯一非回边前驱；快照注入点）
    preheader: usize,
    /// 回边源块（循环体末尾；bump 点所在）
    latch: usize,
    /// 循环退出目标块（退出边源恒为 header，条件为假跳出）
    exit_dst: usize,
    /// 被提升的 region 名
    region: String,
    /// 提升的 bump 列表：(target, size)。循环内同 region 的多个定长
    /// 分配批量提升：循环外一次聚合分配（Σsize），迭代内各对象指针
    /// 由 hbase 偏移派生（对齐等价性见 `try_emit_region_batch`）。
    bumps: Vec<(String, usize)>,
    /// 预分配的 bump join label（回边值 `%n{cur,base,lim}_{h}` phi 所在块）。
    /// 发射 header 时分配（`emit_function` 用 `find_loop_promo` 结果填充），
    /// `emit_region_bump_promoted` 消费。join 是 latch 内 fast/slow 的汇合点。
    join_label: String,
    /// 预分配的 bump 后续 cont label（`AllocInRegion` 的 join 之后块）。
    /// join 经 cont 才到达回边跳转，**cont 才是 header 在 LLVM CFG 中的
    /// 真实前驱**，header phi 的回边 entry 必须引用它（而非 LIR latch 块号）。
    cont_label: String,
}

/// 块的后继块集合。

/// 迭代式支配集合计算（O(n²·E) 数据流；LIR 块数少，够用）。

/// 保守的循环提升分析：全部条件满足才返回 `Some`，否则维持原逻辑
/// （每次 bump 从 Region 头 load，正确性不受影响）。
///
/// 条件：
/// 1. 恰一条回边 `(latch → header)`（`header` 支配 `latch`），非自循环；
/// 2. `header` 恰一个非回边前驱（preheader）；
/// 3. 循环体内无 `RegionEnter`/`RegionExit`（region 生命周期不跨循环）；
/// 4. 循环体内无嵌套循环（嵌套回边）；
/// 5. 循环体内同 region 的 Ptr bump 点（可多个，批量提升）全部为
///    `AllocInRegionDirect`（`size > 0`）且位于 latch 块内，并满足
///    批量提升窗口兼容（bump 之间仅无副作用穿插）；
/// 6. 恰一条退出边，且退出边源为 header（条件为假跳出）；
/// 7. 退出目标块唯一前驱为 header。

/// 提升安全（逃逸）检查：bump target 及其指针拷贝（别名闭包）不得逃逸循环
/// ——不得作调用实参、不得嵌入其他对象/引用、不得被循环外代码引用。
/// 提升后每次迭代复用循环外分配的内存（批量提升为一次聚合分配 +
/// 偏移派生），指针若逃逸（外部持有/循环后使用）复用会互相覆盖，故必须拒绝。
/// 保守检查：宁可误报拒绝，不可漏报。

/// 枚举指令中**被使用**的局部变量（不含定义的 target）。

/// LLVM IR 生成器状态。
struct LlvmEmitter {
    /// 函数签名表：名称 → (参数类型, 返回类型, 是否 extern, extern 返回 i32)
    sigs: HashMap<String, (Vec<LirType>, LirType, bool, bool)>,
    /// 收集的全局常量定义
    globals: Vec<String>,
    /// 全局变量表（`static` / `static mut`，名称 → 类型）：供 codegen 将
    /// 全局名引用发射为 `@name` 而非局部栈槽 `%name.addr`。
    global_types: HashMap<String, LirType>,
    /// 全局常量 / 格式串计数器
    global_counter: usize,
    /// 临时寄存器计数器
    reg_counter: usize,
    /// 分支 label 计数器（label 与寄存器命名空间独立）
    label_counter: usize,
    /// `in 'r` 聚合分配的结果槽名（entry 统一 alloca，内联快路径 store/load；
    /// emit_stmt 按 `alloc_slot_cursor` 顺序消费）
    alloc_slots: Vec<String>,
    /// 消费 `alloc_slots` 的游标
    alloc_slot_cursor: usize,
    /// 生成的函数体（累积）
    body: String,
    /// 标量聚合按值优化：函数名 → 该函数内按值分配的局部变量集合
    /// （Alloc{by_value} 的目标 + 调用按值返回函数的 target）。
    by_value_locals: HashMap<String, BTreeSet<String>>,
    /// 标量聚合按值优化：按值返回（返回 `{i64, i64}`）的函数名集合。
    ret_by_value: HashSet<String>,
    /// 当前函数的循环提升上下文（`find_loop_promo` 结果；None = 不提升）
    promo: Option<PromoCtx>,
    /// 当前正在发射的基本块索引（emit_stmt 判断 bump 是否位于提升 latch）
    current_block: usize,
}

/// 指针拷贝别名闭包传播：把函数内「从 `bvs` 中对象拷贝」的 `Assign` 左值
/// （如 `let __tmp = _t4;` 的指针拷贝）也纳入 `bvs`，迭代至不动点。

impl LlvmEmitter {

    /// 分配一个新的命名寄存器 `%rN`（跨函数递增，保证唯一）。
    ///
    /// 不使用裸数字 `%N`：LLVM 中命名寄存器也占用函数内的数值序列，
    /// 混用数字编号会触发 "instruction expected to be numbered" 错误。
    fn reg(&mut self) -> String {
        let r = self.reg_counter;
        self.reg_counter += 1;
        format!("r{r}")
    }

    /// 分配一个新的基本块 label `%lN`（label 与寄存器命名空间独立，
    /// 用于内联 bump 快路径的 fast/slow/join/cont 分支）。
    fn label(&mut self) -> String {
        let l = self.label_counter;
        self.label_counter += 1;
        format!("l{l}")
    }

    /// 生成单个函数定义（extern 声明生成 `declare`）。

    /// 翻译单条 LIR 指令。
    fn emit_stmt(
        &mut self,
        stmt: &LirStmt,
        body: &mut String,
        f: &LirFunction,
    ) -> Result<(), CodegenError> {
        match stmt {
            LirStmt::Assign { target, value } => {
                // 全局变量赋值：写入 `@target` 而非局部栈槽 `%target.addr`。
                if let Some(&gt) = self.global_types.get(target) {
                    let lt = llvm_type(gt)?;
                    match value {
                        LirOperand::Local(src) => {
                            let src_ty = local_type(f, src);
                            let lt_src = llvm_type(src_ty)?;
                            let r = self.reg();
                            body.push_str(&format!(
                                "  %{r} = load {lt_src}, {lt_src}* %{src}.addr\n"
                            ));
                            body.push_str(&format!("  store {lt_src} %{r}, {lt}* @{target}\n"));
                        }
                        LirOperand::FnPtr(name) => {
                            let r = self.reg();
                            let (pt, rt, _, _) = self
                                .sigs
                                .get(name)
                                .cloned()
                                .ok_or_else(|| CodegenError::UndefinedFunction {
                                    name: name.clone(),
                                })?;
                            let fnty = fn_llvm_type(&pt, rt)?;
                            let gn = llvm_global_name(name);
                            body.push_str(&format!("  %{r} = bitcast {fnty} @{gn} to i8*\n"));
                            body.push_str(&format!("  store i8* %{r}, {lt}* @{target}\n"));
                        }
                        lit => {
                            let v = self.literal(lit, gt, body)?;
                            body.push_str(&format!("  store {lt} {v}, {lt}* @{target}\n"));
                        }
                    }
                    return Ok(());
                }
                let ty = local_type(f, target);
                if ty == LirType::Unit {
                    return Ok(());
                }
                let lt = llvm_type(ty)?;
                if let LirOperand::Local(src) = value {
                    // 源是全局变量（`static` / `static mut`）：从 `@src` 加载而非局部栈槽。
                    if let Some(&sgt) = self.global_types.get(src) {
                        let slt = llvm_type(sgt)?;
                        let r = self.reg();
                        body.push_str(&format!("  %{r} = load {slt}, {slt}* @{src}\n"));
                        let (val, val_ty) = match self.coerce_local_slot(&r, sgt, ty, body) {
                            Some(c) => (c, ty),
                            None => (r.clone(), sgt),
                        };
                        body.push_str(&format!(
                            "  store {} %{val}, {lt}* %{target}.addr\n",
                            llvm_type(val_ty)?
                        ));
                        return Ok(());
                    }
                    let src_ty = local_type(f, src);
                    if src_ty == LirType::Unit {
                        return Ok(());
                    }
                    let lt_src = llvm_type(src_ty)?;
                    let r = self.reg();
                    body.push_str(&format!("  %{r} = load {lt_src}, {lt_src}* %{src}.addr\n"));
                    // 源槽与目标槽类型不同时须插入转换，否则会按源类型的宽度
                    // 写入目标槽（bool 值 ↔ i64 槽会写穿 1 字节槽破坏相邻栈，
                    // 详见 `llvm_emit::coerce_local_slot`）。
                    let (val, val_ty) = match self.coerce_local_slot(&r, src_ty, ty, body) {
                        Some(c) => (c, ty),
                        None => (r.clone(), src_ty),
                    };
                    body.push_str(&format!(
                        "  store {} %{val}, {lt}* %{target}.addr\n",
                        llvm_type(val_ty)?
                    ));
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
                    rlyeh_lir::HirUnaryOp::Neg => {
                        if *ty == LirType::F64 {
                            body.push_str(&format!("  %{r} = fneg double {v}\n"));
                        } else {
                            body.push_str(&format!("  %{r} = sub i64 0, {v}\n"));
                        }
                    }
                    rlyeh_lir::HirUnaryOp::Not => {
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
            LirStmt::Alloc {
                target,
                slots,
                by_value,
            } => {
                // 标量聚合按值：直接落到 entry 预分配的 `[2 x i64]` 栈槽，
                // 免 calloc（tag/字段写入由后续 FieldSet 完成，槽 0 未写
                // 时为 poison——与普通聚合一致，读取须先写；按值聚合的
                // 构造序列 `Alloc → FieldSet(tag) → FieldSet(payload)`
                // 保证返回前两槽均已写入）。
                //
                // 注意：预扫描（`LlvmEmitter::new` 步骤 4）已做逃逸诊断——
                // 若对象地址被存入其他聚合 / 作实参传出（跨函数生命周期），
                // 该对象已从 `by_value_locals` 剔除，此处必须回退 calloc
                // 堆分配；否则栈槽地址逃逸为悬垂指针。
                // V2-C（2026-08-26，方案 A）：StrFat 标量双字值——不作为 by_value
                // `.obj` 对象，也不走 calloc 堆分配，而是清零 `.addr` 值槽
                // （`{i8*,i64}`），后续 FieldSet 直接写 `.addr` 槽，Assign/打印经
                // `.addr` 值槽读写。避免 by_value `.obj` 污染 data 槽（根因）。
                let in_bvs = *by_value
                    && self
                        .by_value_locals
                        .get(&f.name)
                        .map(|s| s.contains(target))
                        .unwrap_or(false);
                let is_strfat = f
                    .locals
                    .iter()
                    .any(|(n, t)| n == target && *t == LirType::StrFat);
                if is_strfat {
                    body.push_str(&format!(
                        "  store {{ i8*, i64 }} zeroinitializer, {{ i8*, i64 }}* %{target}.addr\n"
                    ));
                } else if in_bvs {
                    let r = self.reg();
                    body.push_str(&format!(
                        "  %{r} = bitcast [2 x i64]* %{target}.obj to i8*\n"
                    ));
                    body.push_str(&format!("  store i8* %{r}, i8** %{target}.addr\n"));
                } else {
                    // 堆上分配 slots*8 字节（槽 0 为枚举 tag），返回 i8*
                    // calloc 清零：未写字段（如枚举 tag）读取时不再触发 LLVM poison/UB。
                    let r64 = self.reg();
                    body.push_str(&format!(
                        "  %{r64} = call i64 @calloc(i64 1, i64 {})\n",
                        slots * 8
                    ));
                    let r = self.reg();
                    body.push_str(&format!("  %{r} = inttoptr i64 %{r64} to i8*\n"));
                    let t_lt = llvm_type(LirType::Ptr)?;
                    body.push_str(&format!("  store {t_lt} %{r}, {t_lt}* %{target}.addr\n"));
                }
            }
            // 字段 / 索引 / 指针 / 解引用：实现见 `llvm_field`
            LirStmt::FieldGet { .. }
            | LirStmt::FieldSet { .. }
            | LirStmt::IndexGet { .. }
            | LirStmt::IndexSet { .. }
            | LirStmt::FieldAddr { .. }
            | LirStmt::PtrAdd { .. }
            | LirStmt::Cast { .. }
            | LirStmt::AddrOf { .. }
            | LirStmt::DerefRead { .. }
            | LirStmt::DerefWrite { .. } => self.emit_field_stmt(stmt, f, body)?,
            // L3 region 接线：region 指令 → rlyeh-region-alloc 运行时调用
            // （区域句柄槽 `%{key}.rh` 已在入口块预分配；实现见 `llvm_region`）
            LirStmt::RegionEnter { .. }
            | LirStmt::RegionExit { .. }
            | LirStmt::AllocInRegion { .. }
            | LirStmt::AllocInRegionDirect { .. }
            | LirStmt::Transfer { .. } => self.emit_region_stmt(stmt, f, body)?,
        }
        Ok(())
    }
}

/// 将符号名转为合法的 LLVM 全局标识符。
///
mod llvm_region;
mod llvm_call;
mod llvm_emit;
mod llvm_func;
mod llvm_util;
mod llvm_ctor;
mod llvm_field;

use llvm_util::*;
