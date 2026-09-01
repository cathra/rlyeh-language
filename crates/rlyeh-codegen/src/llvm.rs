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

use std::collections::{HashMap, HashSet};

use rlyeh_lir::{
    FieldScalar, LirBlock, LirFunction, LirOperand, LirProgram, LirStmt, LirTerminator, LirType,
    Local, ReprConv,
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
    let program2 = LirProgram { functions };
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
    out.push_str("declare i8* @malloc(i64)\n");
    // calloc 预置声明必须**早于所有调用点**（LLVM IR parser 对 call 自动创建的
    // 隐式声明与后续显式 declare 视为 redefinition 报错）。标准库 core.rl 的
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
    by_value_locals: HashMap<String, HashSet<String>>,
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
            // L3 region 接线：region 指令 → rlyeh-region-alloc 运行时调用
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
                    "  %{r} = call i8* @rlyeh_region_enter({nptr_arg}, i64 {nlen}, i64 {initial}, i8 {allow_growth}, double {factor_s}, i8 {exact}, i8 {adaptive}, i8 {strategy}) nounwind\n"
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
                body.push_str(&format!("  call void @rlyeh_region_exit(i8* %{r}) nounwind\n"));
            }
            LirStmt::AllocInRegion { target, region, size } => {
                // 仅聚合对象（Ptr 槽）接线：区域内 bump 分配 + 值镜像；
                // 标量 `in 'r` 无区域分配语义（MVP 保持栈上副本）。
                if *size > 0 && local_type(f, target) == LirType::Ptr {
                    let slot = self.alloc_slots[self.alloc_slot_cursor].clone();
                    self.alloc_slot_cursor += 1;
                    let handle = format!("{region}.rh");
                    let p = if let Some(hdr) = self.promo_header_for(region) {
                        self.emit_region_bump_promoted(hdr, &handle, *size, &slot, body)
                    } else {
                        self.emit_inline_region_bump(&handle, *size, &slot, body)
                    };
                    let v = self.operand_value(
                        &LirOperand::Local(target.clone()),
                        LirType::Ptr,
                        body,
                        f,
                    )?;
                    body.push_str(&format!(
                        "  call void @llvm.memcpy.p0i8.p0i8.i64(i8* %{p}, i8* {v}, i64 {size}, i1 false), !tbaa !3\n"
                    ));
                    // 提升模式下 cont 为预分配 label（header phi 引用它）
                    let cont = if self.promo_header_for(region).is_some() {
                        self.promo.as_ref().unwrap().cont_label.clone()
                    } else {
                        self.label()
                    };
                    body.push_str(&format!("  br label %{cont}\n"));
                    body.push_str(&format!("{cont}:\n"));
                }
            }
            LirStmt::AllocInRegionDirect { target, region, size } => {
                // 字面量直接构造（inline_region_literal 变换产物）：
                // 聚合字段在区域指针上直接写入，无中间堆临时、无值镜像。
                if *size > 0 && local_type(f, target) == LirType::Ptr {
                    let slot = self.alloc_slots[self.alloc_slot_cursor].clone();
                    self.alloc_slot_cursor += 1;
                    let handle = format!("{region}.rh");
                    let p = if let Some(hdr) = self.promo_header_for(region) {
                        self.emit_region_bump_promoted(hdr, &handle, *size, &slot, body)
                    } else {
                        self.emit_inline_region_bump(&handle, *size, &slot, body)
                    };
                    // target 槽 = 区域指针（后续 FieldSet 直接写区域内存）
                    body.push_str(&format!("  store i8* %{p}, i8** %{target}.addr\n"));
                    // 提升模式下 cont 为预分配 label（header phi 引用它）
                    let cont = if self.promo_header_for(region).is_some() {
                        self.promo.as_ref().unwrap().cont_label.clone()
                    } else {
                        self.label()
                    };
                    body.push_str(&format!("  br label %{cont}\n"));
                    body.push_str(&format!("{cont}:\n"));
                }
            }
            LirStmt::Transfer { place, region } => {
                let handle = format!("{region}.rh");
                let r = self.reg();
                body.push_str(&format!("  %{r} = load i8*, i8** %{handle}\n"));
                let v = self.operand_value(&LirOperand::Local(place.clone()), LirType::Ptr, body, f)?;
                body.push_str(&format!(
                    "  call void @rlyeh_region_transfer(i8* %{r}, i8* {v}) nounwind\n"
                ));
            }
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

use llvm_util::*;
