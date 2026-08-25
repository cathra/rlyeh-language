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
    Local,
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
fn touches_any(s: &LirStmt, aliases: &[&str]) -> bool {
    use LirStmt::*;
    match s {
        FieldSet { base, value, .. } => aliases.contains(&base.as_str()) || aliases.contains(&value.as_str()),
        FieldGet { target, base, .. } => {
            aliases.contains(&target.as_str()) || aliases.contains(&base.as_str())
        }
        IndexGet {
            target, base, index, ..
        } => {
            aliases.contains(&target.as_str())
                || aliases.contains(&base.as_str())
                || aliases.contains(&index.as_str())
        }
        IndexSet {
            base, index, value, ..
        } => {
            aliases.contains(&base.as_str())
                || aliases.contains(&index.as_str())
                || aliases.contains(&value.as_str())
        }
        Assign { target, value } => {
            aliases.contains(&target.as_str())
                || value
                    .as_local()
                    .map(|l| aliases.contains(&l.as_str()))
                    .unwrap_or(false)
        }
        Binary {
            target, lhs, rhs, ..
        } => {
            aliases.contains(&target.as_str())
                || lhs.as_local().map(|l| aliases.contains(&l.as_str())).unwrap_or(false)
                || rhs.as_local().map(|l| aliases.contains(&l.as_str())).unwrap_or(false)
        }
        Unary {
            target, operand, ..
        } => {
            aliases.contains(&target.as_str())
                || operand.as_local().map(|l| aliases.contains(&l.as_str())).unwrap_or(false)
        }
        Call { target, args, .. } => {
            target.as_deref().map(|t| aliases.contains(&t)).unwrap_or(false)
                || args.iter().any(|a| aliases.contains(&a.as_str()))
        }
        CallIndirect {
            target,
            callee,
            args,
            ..
        } => {
            target.as_deref().map(|t| aliases.contains(&t)).unwrap_or(false)
                || aliases.contains(&callee.as_str())
                || args.iter().any(|a| aliases.contains(&a.as_str()))
        }
        AddrOf { target, operand, .. } => {
            aliases.contains(&target.as_str()) || aliases.contains(&operand.as_str())
        }
        DerefRead { target, base, .. } => {
            aliases.contains(&target.as_str()) || aliases.contains(&base.as_str())
        }
        DerefWrite { base, value, .. } => {
            aliases.contains(&base.as_str()) || aliases.contains(&value.as_str())
        }
        Alloc { target, .. } => aliases.contains(&target.as_str()),
        RegionEnter { .. }
        | RegionExit { .. }
        | Transfer { .. }
        | AllocInRegion { .. }
        | AllocInRegionDirect { .. } => false,
    }
}

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
fn inline_region_literal(f: &mut LirFunction) {
    use LirStmt::*;
    for block in &mut f.blocks {
        let stmts = std::mem::take(&mut block.stmts);
        let mut out: Vec<LirStmt> = Vec::with_capacity(stmts.len());
        let mut i = 0;
        while i < stmts.len() {
            let s = &stmts[i];
            if let AllocInRegion { target, region, size } = s {
                // 向前回看窗口：[k, i) = `Alloc(t)` + 别名链 + 字段构造
                // （允许字段值求值等不读写目标对象的指令穿插）。
                let mut aliases: Vec<&str> = vec![target.as_str()];
                let mut k = i;
                let mut alloc_hit = false;
                let mut alias_assign: Option<usize> = None; // 待删除的别名 Assign
                while k > 0 {
                    match &stmts[k - 1] {
                        FieldSet { base, .. } if aliases.contains(&base.as_str()) => k -= 1,
                        Alloc {
                            target: t,
                            slots,
                            by_value: false,
                        } if aliases.contains(&t.as_str()) && *slots * 8 == *size =>
                        {
                            alloc_hit = true;
                            k -= 1;
                            break;
                        }
                        Assign { target: at, value }
                            if aliases.contains(&at.as_str())
                                && matches!(value.as_local(), Some(y) if !aliases.contains(&y.as_str())) =>
                        {
                            // 别名扩张：`x = y`（y 为分配 target），该 Assign 待删除
                            if let Some(y) = value.as_local() {
                                alias_assign = Some(k - 1);
                                aliases.push(y.as_str());
                            }
                            k -= 1;
                        }
                        Assign { target: at, .. } if aliases.contains(&at.as_str()) => break,
                        other if !touches_any(other, &aliases) => k -= 1,
                        _ => break,
                    }
                }
                if alloc_hit && k < i && matches!(&stmts[k], Alloc { .. }) {
                    // 命中：截断 out 到窗口前，重排窗口内容
                    let win_len = i - k;
                    out.truncate(out.len() - win_len);
                    // 删除别名 Assign；非 FieldSet(别名) 指令（纯计算）保留原位；
                    // FieldSet(别名) 收集到 Direct 之后（原序）
                    for idx in (k + 1)..i {
                        if alias_assign == Some(idx) {
                            continue; // 别名赋值被 Direct 取代（target 即区域指针）
                        }
                        if !matches!(
                            &stmts[idx],
                            FieldSet { base, .. } if aliases.contains(&base.as_str())
                        ) {
                            out.push(stmts[idx].clone());
                        }
                    }
                    // Direct 分配（区域指针）
                    out.push(AllocInRegionDirect {
                        target: target.clone(),
                        region: region.clone(),
                        size: *size,
                    });
                    for idx in (k + 1)..i {
                        if matches!(
                            &stmts[idx],
                            FieldSet { base, .. } if aliases.contains(&base.as_str())
                        ) {
                            out.push(stmts[idx].clone());
                        }
                    }
                    i += 1; // 跳过 AllocInRegion（其 memcpy 语义由 Direct 取代）
                    continue;
                }
            }
            out.push(s.clone());
            i += 1;
        }
        block.stmts = out;
    }
}

/// 生成 LLVM IR 文本。
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
fn block_succs(b: &LirBlock) -> Vec<usize> {
    match &b.terminator {
        LirTerminator::Jump(t) => vec![*t],
        LirTerminator::CondJump { then, otherwise, .. } => vec![*then, *otherwise],
        LirTerminator::Return(_) => Vec::new(),
    }
}

/// 迭代式支配集合计算（O(n²·E) 数据流；LIR 块数少，够用）。
fn compute_dominators(f: &LirFunction) -> Vec<HashSet<usize>> {
    let n = f.blocks.len();
    let mut pred: Vec<Vec<usize>> = (0..n).map(|_| Vec::new()).collect();
    for (i, b) in f.blocks.iter().enumerate() {
        for s in block_succs(b) {
            pred[s].push(i);
        }
    }
    let all: HashSet<usize> = (0..n).collect();
    let mut dom: Vec<HashSet<usize>> = (0..n)
        .map(|i| {
            if i == 0 {
                let mut s = HashSet::new();
                s.insert(0);
                s
            } else {
                all.clone()
            }
        })
        .collect();
    loop {
        let mut changed = false;
        for b in 0..n {
            if b == 0 {
                continue;
            }
            // dom[b] = {b} ∪ (∩ dom[p] for p in pred[b])
            let mut inter: HashSet<usize> = if let Some(p0) = pred[b].first() {
                dom[*p0].clone()
            } else {
                HashSet::new()
            };
            for p in pred[b].iter().skip(1) {
                inter = inter.intersection(&dom[*p]).cloned().collect();
            }
            inter.insert(b);
            if inter != dom[b] {
                dom[b] = inter;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    dom
}

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
fn find_loop_promo(f: &LirFunction) -> Option<PromoCtx> {
    let n = f.blocks.len();
    let dom = compute_dominators(f);
    // 全部回边：`(src → dst)` 且 `dst` 支配 `src`（`dom[src] ∋ dst`）
    let mut back_edges: Vec<(usize, usize)> = Vec::new();
    for (i, b) in f.blocks.iter().enumerate() {
        for s in block_succs(b) {
            if dom[i].contains(&s) {
                back_edges.push((i, s));
            }
        }
    }
    if back_edges.len() != 1 {
        return None;
    }
    let (latch, header) = back_edges[0];
    // 单块自环循环（latch == header）同样可提升：header phi、join/cont
    // 标签与回边值结构不依赖块分离，preheader 快照与 exit 写回不变。
    // preheader：header 的非回边前驱，须恰一个
    let preds_of_header: Vec<usize> = (0..n)
        .filter(|p| block_succs(&f.blocks[*p]).contains(&header))
        .collect();
    let non_back: Vec<usize> = preds_of_header.iter().copied().filter(|p| *p != latch).collect();
    if non_back.len() != 1 {
        return None;
    }
    let preheader = non_back[0];
    // 循环体：header 支配且可经前驱边回到 header 的块集合
    // （退出目标块虽被 header 支配，但不在回 header 的路径上，排除）。
    let mut body: HashSet<usize> = HashSet::new();
    body.insert(header);
    let mut stack = vec![header];
    while let Some(b) = stack.pop() {
        for (pi, pb) in f.blocks.iter().enumerate() {
            if !dom[pi].contains(&header) {
                continue;
            }
            if block_succs(pb).contains(&b) && !body.contains(&pi) {
                body.insert(pi);
                stack.push(pi);
            }
        }
    }
    // 条件 4：循环体内无嵌套回边
    for (src, dst) in &back_edges {
        if *src == latch && *dst == header {
            continue;
        }
        if body.contains(src) && body.contains(dst) {
            return None;
        }
    }
    // 条件 3：循环体内无 region 生命周期指令
    for &b in &body {
        for s in &f.blocks[b].stmts {
            if matches!(s, LirStmt::RegionEnter { .. } | LirStmt::RegionExit { .. }) {
                return None;
            }
        }
    }
    // 条件 5：同 region 的 Ptr bump 点（可多个，批量提升），全部位于 latch。
    // 仅收集 `AllocInRegionDirect`（直接构造）：值镜像 `AllocInRegion` 需
    // memcpy 发射，无法窗口聚合（散落 bump 会与单发提升的回边 phi 重复
    // 定义冲突）。
    let mut bumps: Vec<(String, String, usize, usize)> = Vec::new(); // (region, target, size, block)
    for &b in &body {
        for s in &f.blocks[b].stmts {
            let (target, region, size) = match s {
                LirStmt::AllocInRegionDirect {
                    target, region, size, ..
                } => (target.as_str(), region.clone(), *size),
                _ => continue,
            };
            if size == 0 || local_type(f, target) != LirType::Ptr {
                continue;
            }
            if let Some((prev_region, _, _, _)) = bumps.first() {
                if prev_region != &region {
                    return None; // 不同 region 的分配不可聚合提升
                }
            }
            bumps.push((region, target.to_string(), size, b));
        }
    }
    if bumps.is_empty() {
        return None;
    }
    let region = bumps[0].0.clone();
    if bumps.iter().any(|(_, _, _, blk)| *blk != latch) {
        return None; // 提升 phi 结构依赖 bump 位于 latch
    }
    // 条件 5b（批量提升窗口兼容预检）：latch 内**首个 bump 到末个 bump 之间**
    // 仅允许无副作用穿插（`FieldSet`/`Assign`/`Binary`），保证发射期
    // `try_emit_region_promo_batch` 的窗口必能聚合**全部** bump——否则散落
    // 的 bump 走单发提升会产生重复 `%ncur_{hdr}` 回边 phi（IR 非法）。
    // 注意 span 必须限定在 bump 区间内：末个 bump 之后的 `FieldGet`（如
    // `sum + x1.a + x2.a...` 读取分配结果）属合法跟随，不得触发拒绝。
    {
        let stmts = &f.blocks[latch].stmts;
        let mut bump_idx: Vec<usize> = Vec::new();
        for (i, s) in stmts.iter().enumerate() {
            if matches!(
                s,
                LirStmt::AllocInRegionDirect { target, .. }
                    if bumps.iter().any(|(_, t, _, _)| t == target)
            ) {
                bump_idx.push(i);
            }
        }
        if bump_idx.len() != bumps.len() {
            return None; // 部分 bump 缺失（异常），保守拒绝
        }
        if let (Some(&first), Some(&last)) = (bump_idx.first(), bump_idx.last()) {
            for (i, s) in stmts.iter().enumerate().take(last).skip(first + 1) {
                let is_bump = bump_idx.contains(&i);
                if !is_bump
                    && !matches!(
                        s,
                        LirStmt::FieldSet { .. }
                            | LirStmt::Assign { .. }
                            | LirStmt::Binary { .. }
                    )
                {
                    return None; // 窗口会中断：bump 散落
                }
            }
        }
    }
    // 逃逸检查：bump target 及其指针拷贝不得逃逸循环
    if !no_promo_escape(f, &body, &bumps) {
        return None;
    }
    // 条件 6/7：恰一条退出边，源为 header，目标唯一前驱为 header
    let mut exits: Vec<(usize, usize)> = Vec::new(); // (src, dst)
    for &b in &body {
        for s in block_succs(&f.blocks[b]) {
            if !body.contains(&s) {
                exits.push((b, s));
            }
        }
    }
    if exits.len() != 1 {
        return None;
    }
    let (exit_src, exit_dst) = exits[0];
    if exit_src != header {
        return None;
    }
    let preds_of_exit: Vec<usize> = (0..n)
        .filter(|p| block_succs(&f.blocks[*p]).contains(&exit_dst))
        .collect();
    if preds_of_exit != vec![header] {
        return None;
    }
    Some(PromoCtx {
        header,
        preheader,
        latch,
        exit_dst,
        region,
        bumps: bumps.iter().map(|(_, t, s, _)| (t.clone(), *s)).collect(),
        // label 在 emit_function 发射 header 时分配并回填
        join_label: String::new(),
        cont_label: String::new(),
    })
}

/// 提升安全（逃逸）检查：bump target 及其指针拷贝（别名闭包）不得逃逸循环
/// ——不得作调用实参、不得嵌入其他对象/引用、不得被循环外代码引用。
/// 提升后每次迭代复用循环外分配的内存（批量提升为一次聚合分配 +
/// 偏移派生），指针若逃逸（外部持有/循环后使用）复用会互相覆盖，故必须拒绝。
/// 保守检查：宁可误报拒绝，不可漏报。
fn no_promo_escape(
    f: &LirFunction,
    body: &HashSet<usize>,
    bumps: &[(String, String, usize, usize)],
) -> bool {
    // 别名闭包：bump target + 经 Assign 拷贝传播的指针变量（不动点）
    let mut aliases: HashSet<String> = HashSet::new();
    for (_, t, _, _) in bumps {
        aliases.insert(t.clone());
    }
    loop {
        let mut changed = false;
        for &b in body {
            for s in &f.blocks[b].stmts {
                if let LirStmt::Assign { target, value } = s {
                    if let Some(x) = value.as_local() {
                        if aliases.contains(x) && aliases.insert(target.clone()) {
                            changed = true;
                        }
                        if aliases.contains(target) && aliases.insert(x.clone()) {
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    // 循环内逃逸点
    for &b in body {
        for s in &f.blocks[b].stmts {
            match s {
                LirStmt::Call { args, .. } | LirStmt::CallIndirect { args, .. } => {
                    if args.iter().any(|a| aliases.contains(a)) {
                        return false; // 对象指针作调用实参（外部可持有）
                    }
                }
                LirStmt::FieldSet { value, .. } | LirStmt::IndexSet { value, .. } => {
                    if aliases.contains(value) {
                        return false; // 指针写入其他对象（嵌入）
                    }
                }
                LirStmt::DerefWrite { value, .. } => {
                    if aliases.contains(value) {
                        return false; // 写入引用指向处（外部可持有）
                    }
                }
                LirStmt::AddrOf { operand, .. } => {
                    if aliases.contains(operand) {
                        return false; // 引用化（可逃逸）
                    }
                }
                LirStmt::Transfer { place, .. } => {
                    if aliases.contains(place) {
                        return false; // 所有权转移出循环
                    }
                }
                _ => {}
            }
        }
    }
    // 循环外引用：任何非 body 块的操作数/返回值含别名 → 逃逸
    for (bi, blk) in f.blocks.iter().enumerate() {
        if body.contains(&bi) {
            continue;
        }
        for s in &blk.stmts {
            if stmt_used_locals(s).iter().any(|x| aliases.contains(*x)) {
                return false;
            }
        }
        if let LirTerminator::Return(Some(v)) = &blk.terminator {
            if aliases.contains(v) {
                return false;
            }
        }
    }
    true
}

/// 枚举指令中**被使用**的局部变量（不含定义的 target）。
fn stmt_used_locals(st: &LirStmt) -> Vec<&String> {
    let mut out = Vec::new();
    fn push_local<'a>(out: &mut Vec<&'a String>, o: &'a LirOperand) {
        if let LirOperand::Local(l) = o {
            out.push(l);
        }
    }
    match st {
        LirStmt::Assign { value, .. } => push_local(&mut out, value),
        LirStmt::Binary { lhs, rhs, .. } => {
            push_local(&mut out, lhs);
            push_local(&mut out, rhs);
        }
        LirStmt::Unary { operand, .. } => push_local(&mut out, operand),
        LirStmt::Call { args, .. } => out.extend(args),
        LirStmt::CallIndirect { callee, args, .. } => {
            out.push(callee);
            out.extend(args);
        }
        LirStmt::FieldGet { base, .. } => out.push(base),
        LirStmt::FieldSet { base, value, .. } => {
            out.push(base);
            out.push(value);
        }
        LirStmt::IndexGet { base, index, .. } => {
            out.push(base);
            out.push(index);
        }
        LirStmt::IndexSet {
            base, index, value, ..
        } => {
            out.push(base);
            out.push(index);
            out.push(value);
        }
        LirStmt::AddrOf { operand, .. } => out.push(operand),
        LirStmt::DerefRead { base, .. } => out.push(base),
        LirStmt::DerefWrite { base, value, .. } => {
            out.push(base);
            out.push(value);
        }
        LirStmt::AllocInRegion { .. }
        | LirStmt::AllocInRegionDirect { .. }
        | LirStmt::Alloc { .. }
        | LirStmt::RegionEnter { .. }
        | LirStmt::RegionExit { .. }
        | LirStmt::Transfer { .. } => {}
    }
    out
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
fn propagate_by_value_aliases(bvs: &mut HashSet<String>, f: &LirFunction) {
    loop {
        let mut changed = false;
        for b in &f.blocks {
            for s in &b.stmts {
                if let LirStmt::Assign {
                    target,
                    value: LirOperand::Local(rhs),
                } = s
                {
                    if bvs.contains(rhs) && bvs.insert(target.clone()) {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
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

        // —— 标量聚合按值优化：预扫描 ——
        // 1) 收集被取址的函数（函数值绑定 / vtable 槽 / 函数实参中的
        //    `FnPtr`）：这些函数可能被 CallIndirect 按旧签名（i8* 返回）
        //    bitcast 间接调用，返回类型不能改为 `{i64, i64}`。
        let mut taken: HashSet<String> = HashSet::new();
        for f in &program.functions {
            for b in &f.blocks {
                for s in &b.stmts {
                    // `FnPtr` 仅出现在 `Assign{value}` 位置（函数值绑定 /
                    // vtable 槽先 `let __m = f` 再 `FieldSet`），其余语句的
                    // value 均为 Local，不携带函数名。
                    if let LirStmt::Assign {
                        value: LirOperand::FnPtr(n),
                        ..
                    } = s
                    {
                        taken.insert(n.clone());
                    }
                }
            }
        }
        // 2) 第一遍：收集每个函数内 `Alloc{by_value}` 的目标局部变量
        let mut by_value_locals: HashMap<String, HashSet<String>> = HashMap::new();
        for f in &program.functions {
            let mut bvs: HashSet<String> = HashSet::new();
            for b in &f.blocks {
                for s in &b.stmts {
                    if let LirStmt::Alloc {
                        target,
                        by_value: true,
                        ..
                    } = s
                    {
                        bvs.insert(target.clone());
                    }
                }
            }
            by_value_locals.insert(f.name.clone(), bvs);
        }
        // 3) 指针拷贝别名闭包传播：把 `Alloc{by_value}` 目标的「指针拷贝别名」
        //    也纳入按值集合。`Option::Some(v)` / `None`、`Result::Ok/Err` 等
        //    构造先 Alloc 到内部临时（如 `_t4`），再拷贝指针到 `return` / 绑定
        //    目标（如 `__tmp486`）；若只记录 Alloc target，判定 Return 时会因
        //    名字不一致漏判，导致栈槽地址按 i8* 返回（悬垂指针）。
        for f in &program.functions {
            propagate_by_value_aliases(
                by_value_locals.get_mut(&f.name).expect("by_value_locals entry"),
                f,
            );
        }
        // 4) 判定「按值返回」函数 + 调用点 target 并入 + 逃逸剔除的不动点循环。
        //    调用「按值返回函数」的 target 会解包写入其栈槽，故并入按值集合；
        //    `fn f() { g() }`（直接返回调用结果）依赖 g 判定后 f 才可判定，
        //    因此二者需迭代至不动点。extern / main / 被取址函数不参与
        //    ret_by_value 判定（但逃逸剔除对所有函数生效——main 内同样可能
        //    构造 `Result::Ok(Thread { .. })` 嵌入逃逸对象）。
        //
        //    逃逸剔除：by_value 对象（栈槽 `[2 x i64]`）的「地址」若被存入
        //    其他聚合对象（FieldSet 的 value，base 非自身）或作实参传出
        //    （Call / CallIndirect，被调方可能持有该地址），该地址在函数返回
        //    后悬垂——被调方 / 堆对象持有指向已退出栈帧的指针（如
        //    `Result::Ok(Thread { tid })` 把 Thread 栈槽地址嵌入 Result 堆对象
        //    再返回，调用方解包读到的 tid 指向已释放栈帧）。此类对象必须改走
        //    calloc 堆分配（Alloc 发射处按 bvs 成员判定），不能使用栈槽。
        //    剔除是「永久」的（记录在 `escaped` 中）：否则 4a 每轮会把同一
        //    target 重新并入，与 4b 的剔除互相震荡。
        let mut ret_by_value: HashSet<String> = HashSet::new();
        // 每函数已确认逃逸（须改走 calloc）的 by_value 对象（永久剔除）
        let mut escaped: HashMap<String, HashSet<String>> = HashMap::new();
        for f in &program.functions {
            escaped.insert(f.name.clone(), HashSet::new());
        }
        loop {
            let mut changed = false;
            // 4a) 调用「按值返回函数」的 target 纳入调用方按值集合
            //     （已确认逃逸的 target 跳过：其本体改由 calloc 分配）
            for f in &program.functions {
                let bvs = by_value_locals.get_mut(&f.name).expect("by_value_locals entry");
                let esc = &escaped[&f.name];
                let mut grew = false;
                for b in &f.blocks {
                    for s in &b.stmts {
                        if let LirStmt::Call {
                            target: Some(t),
                            callee,
                            ..
                        } = s
                        {
                            if ret_by_value.contains(callee)
                                && !esc.contains(t)
                                && bvs.insert(t.clone())
                            {
                                grew = true;
                            }
                        }
                    }
                }
                if grew {
                    propagate_by_value_aliases(bvs, f);
                    changed = true;
                }
            }
            // 4b) 逃逸诊断 + 剔除（所有函数，含 main / extern / 被取址函数）
            for f in &program.functions {
                let bvs = by_value_locals.get_mut(&f.name).expect("by_value_locals entry");
                // owned 拷贝：避免持 `escaped` 借用时更新它（E0502）
                let esc = escaped.get(&f.name).cloned().unwrap_or_default();
                // 4b-i) 逃逸种子：by_value 对象的地址被写入其他聚合对象
                //       （FieldSet 的 value，base 非自身）或作实参传出
                //       （Call / CallIndirect，被调方可能持有该地址）。
                let mut esc2 = esc.clone();
                for b in &f.blocks {
                    for s in &b.stmts {
                        match s {
                            LirStmt::FieldSet { base, value, .. } => {
                                if value != base && bvs.contains(value) {
                                    esc2.insert(value.clone());
                                }
                            }
                            LirStmt::Call { args, .. } | LirStmt::CallIndirect { args, .. } => {
                                for a in args {
                                    if bvs.contains(a) {
                                        esc2.insert(a.clone());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                // 4b-ii) 逃逸双向别名闭包：`T = S` 指针拷贝后同地址，
                //        任一侧逃逸则另一侧也逃逸。
                loop {
                    let mut c2 = false;
                    for b in &f.blocks {
                        for s in &b.stmts {
                            if let LirStmt::Assign {
                                target,
                                value: LirOperand::Local(rhs),
                            } = s
                            {
                                if esc2.contains(target) && esc2.insert(rhs.clone()) {
                                    c2 = true;
                                }
                                if esc2.contains(rhs) && esc2.insert(target.clone()) {
                                    c2 = true;
                                }
                            }
                        }
                    }
                    if !c2 {
                        break;
                    }
                }
                // 4b-v) 剔除逃逸成员（永久记录，防 4a 重插震荡）。
                //       Alloc 发射处对不在 bvs 的 by_value 目标改走 calloc，
                //       Return / 解包对不在 bvs 的对象走 i8* / calloc。
                for e in &esc2 {
                    if bvs.remove(e) {
                        changed = true;
                    }
                }
                // 连带剔除 & ret_by_value 判定仅对非 extern / main / 被取址函数
                if f.is_extern || f.name == "main" || taken.contains(&f.name) {
                    if esc2 != esc {
                        escaped.insert(f.name.clone(), esc2);
                        changed = true;
                    }
                    continue;
                }
                // 4b-iii) 判定：所有非 Unit 的 Return 都返回 by_value 对象，
                //         且至少一个（保持「f ∈ ret_by_value ⟺ 全部 Return
                //         ∈ bvs」不变量，否则 emit_terminator 对残留成员打包
                //         而调用点按 i8* 接收，签名不一致）。
                let mut ret_vals: Vec<Local> = Vec::new();
                for b in &f.blocks {
                    if let LirTerminator::Return(Some(x)) = &b.terminator {
                        ret_vals.push(x.clone());
                    }
                }
                let mut ok = !ret_vals.is_empty();
                for x in &ret_vals {
                    if !bvs.contains(x) {
                        ok = false;
                        break;
                    }
                }
                if ok && !ret_by_value.contains(&f.name) {
                    ret_by_value.insert(f.name.clone());
                    changed = true;
                } else if !ok && ret_by_value.contains(&f.name) {
                    ret_by_value.remove(&f.name);
                    changed = true;
                }
                // 4b-iv) 连带剔除：非 by_value 返回函数的全部 Return 值
                //        也须剔除（连同别名），保证上述不变量。
                if !ok {
                    for x in &ret_vals {
                        esc2.insert(x.clone());
                    }
                    loop {
                        let mut c3 = false;
                        for b in &f.blocks {
                            for s in &b.stmts {
                                if let LirStmt::Assign {
                                    target,
                                    value: LirOperand::Local(rhs),
                                } = s
                                {
                                    if esc2.contains(target) && esc2.insert(rhs.clone()) {
                                        c3 = true;
                                    }
                                    if esc2.contains(rhs) && esc2.insert(target.clone()) {
                                        c3 = true;
                                    }
                                }
                            }
                        }
                        if !c3 {
                            break;
                        }
                    }
                    for e in &esc2 {
                        if bvs.remove(e) {
                            changed = true;
                        }
                    }
                }
                // 更新 escaped（本函数处理完统一落账）
                if esc2 != esc {
                    escaped.insert(f.name.clone(), esc2);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        Self {
            sigs,
            globals: Vec::new(),
            global_counter: 0,
            reg_counter: 0,
            label_counter: 0,
            alloc_slots: Vec::new(),
            alloc_slot_cursor: 0,
            body: String::new(),
            by_value_locals,
            ret_by_value,
            promo: None,
            current_block: 0,
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

    /// 分配一个新的基本块 label `%lN`（label 与寄存器命名空间独立，
    /// 用于内联 bump 快路径的 fast/slow/join/cont 分支）。
    fn label(&mut self) -> String {
        let l = self.label_counter;
        self.label_counter += 1;
        format!("l{l}")
    }

    /// 生成单个函数定义（extern 声明生成 `declare`）。
    fn emit_function(&mut self, f: &LirFunction) -> Result<(), CodegenError> {
        if f.is_extern {
            // `__rlyeh_` 前缀为驱动注入的平台内建（如 `__rlyeh_target_os`）：
            // 跳过 declare——driver 在汇编阶段追加 `define internal`（同符号 declare+define 冲突）。
            if f.name.starts_with("__rlyeh_") {
                return Ok(());
            }
            // calloc 已在模块头（preamble）预置 `declare i64 @calloc(i64, i64)`，
            // 且必须早于所有调用点（避免 LLVM IR parser 隐式声明 + 显式声明冲突）；
            // 标准库 `extern fn calloc` 的声明在此跳过，防止重复 declare。
            if f.name == "calloc" {
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

        // 每次生成函数前重置内联 bump 快路径的跨块状态
        // （槽名在函数内消费，label 跨函数递增亦可，重置保证可复现）。
        self.alloc_slots.clear();
        self.alloc_slot_cursor = 0;
        self.label_counter = 0;

        let is_main = f.name == "main";
        if is_main && !f.params.is_empty() {
            return Err(CodegenError::InvalidMain);
        }

        let ret_ty = if is_main {
            "i32".to_string()
        } else if f.return_type == LirType::Unit {
            "void".to_string()
        } else if self.ret_by_value.contains(&f.name) {
            // 标量聚合按值返回：`{tag, payload}` 两槽按寄存器返回
            "{ i64, i64 }".to_string()
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
        // 标量聚合按值：为按值局部变量预分配 16 字节对象本体槽
        // （`%{x}.obj = alloca [2 x i64]`；`%{x}.addr` 存其 i8* 地址，
        // FieldGet/FieldSet/IndexGet 等既有代码零改动即可直接读写）。
        if let Some(bvs) = self.by_value_locals.get(&f.name) {
            for name in bvs {
                body.push_str(&format!("  %{name}.obj = alloca [2 x i64]\n"));
            }
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

        // 内联 bump 快路径：预扫描全部 `in 'r` 聚合分配，为每个分配在
        // entry 统一预分配一个 `i8*` 结果槽（非入口块不允许 alloca，
        // 快路径/慢路径分支分别 store，join 后 load 汇合）。
        for b in &f.blocks {
            for s in &b.stmts {
                let region_alloc = match s {
                    LirStmt::AllocInRegion { target, size, .. }
                    | LirStmt::AllocInRegionDirect { target, size, .. } => Some((target, size)),
                    _ => None,
                };
                if let Some((target, size)) = region_alloc {
                    if *size > 0 && local_type(f, target) == LirType::Ptr {
                        let slot = self.reg();
                        self.alloc_slots.push(slot);
                    }
                }
            }
        }
        for slot in &self.alloc_slots {
            body.push_str(&format!("  %{slot} = alloca i8*\n"));
        }

        // 基本块
        // —— 循环级 region 状态提升（热路径优化）——
        // 命名约定统一以 header 块号作后缀：`%p{cur,base,lim}_{h}`（快照）、
        // `%h{cur,base,lim}_{h}`（header phi）、`%n{cur,base,lim}_{h}`
        // （latch 内 bump 汇合后的回边值）。header phi 引用 `%n{..}_{h}`
        // 是文本 IR 前向引用（latch 在 header 之后发射），LLVM 解析合法。
        let promo = find_loop_promo(f).map(|mut p| {
            // 预分配 bump 的 join/cont label：header phi 文本上引用
            // cont_label（LLVM CFG 中 latch 的真实回边块），故必须在
            // 发射 header 前确定；join_label 供 bump 的 fast/slow 汇合点。
            p.join_label = self.label();
            p.cont_label = self.label();
            p
        });
        self.promo = promo.clone();
        for (i, block) in f.blocks.iter().enumerate() {
            self.current_block = i;
            let is_promo_header = promo.as_ref().map(|p| p.header == i).unwrap_or(false);
            let is_promo_preheader = promo.as_ref().map(|p| p.preheader == i).unwrap_or(false);
            let is_promo_exit = promo.as_ref().map(|p| p.exit_dst == i).unwrap_or(false);
            if i > 0 {
                body.push_str(&format!("b{i}:\n"));
            }
            if is_promo_header {
                let p = promo.as_ref().expect("promo header");
                // 块 0（entry）的 label 是 `entry`，其余块是 `b{i}`
                let pre_l = if p.preheader == 0 {
                    "entry".to_string()
                } else {
                    format!("b{}", p.preheader)
                };
                // 回边 entry 必须引用 latch 实际跳回 header 的块
                // （bump 提升后为预分配的 cont label，而非 LIR 块号）。
                let latch_l = p.cont_label.clone();
                body.push_str(&format!(
                    "  %hcur_{i} = phi i64 [ %pcur_{i}, %{pre_l} ], [ %ncur_{i}, %{latch_l} ]\n"
                ));
                body.push_str(&format!(
                    "  %hbase_{i} = phi i8* [ %pbase_{i}, %{pre_l} ], [ %nbase_{i}, %{latch_l} ]\n"
                ));
                body.push_str(&format!(
                    "  %hlim_{i} = phi i64 [ %plim_{i}, %{pre_l} ], [ %nlim_{i}, %{latch_l} ]\n"
                ));
            }
            if is_promo_exit {
                // 循环退出：把 cursor 写回 Region 头。header phi 值
                // `%hcur_{hdr}` 即"最近一次迭代后的 cursor"（header 入口
                // 的 phi 值；零次迭代时为快照值，写回无害）。
                let p = promo.as_ref().expect("promo exit");
                let hdr = p.header;
                let rh = self.reg();
                body.push_str(&format!("  %{rh} = load i8*, i8** %{}.rh\n", p.region));
                let g2 = self.reg();
                body.push_str(&format!("  %{g2} = getelementptr i8, i8* %{rh}, i64 8\n"));
                let cv = self.reg();
                body.push_str(&format!("  %{cv} = bitcast i8* %{g2} to i64*\n"));
                body.push_str(&format!("  store i64 %hcur_{hdr}, i64* %{cv}\n"));
            }
            let mut si = 0;
            while si < block.stmts.len() {
                // 提升路径批量聚合优先（多 bump 循环提升），其次未提升批量，
                // 最后单发
                if let Some(n) = self.try_emit_region_promo_batch(&block.stmts, si, &mut body, f)?
                {
                    si += n;
                } else if let Some(n) = self.try_emit_region_batch(&block.stmts, si, &mut body, f)?
                {
                    si += n;
                } else {
                    self.emit_stmt(&block.stmts[si], &mut body, f)?;
                    si += 1;
                }
            }
            if is_promo_preheader {
                // 循环前快照：base/cursor/limit → `%p{..}_{hdr}`（terminator 之前）
                let p = promo.as_ref().expect("promo preheader");
                let hdr = p.header;
                let rh = self.reg();
                body.push_str(&format!("  %{rh} = load i8*, i8** %{}.rh\n", p.region));
                body.push_str(&format!("  %pbase_{hdr} = load i8*, i8** %{rh}\n"));
                let g2 = self.reg();
                body.push_str(&format!("  %{g2} = getelementptr i8, i8* %{rh}, i64 8\n"));
                let cv = self.reg();
                body.push_str(&format!("  %{cv} = bitcast i8* %{g2} to i64*\n"));
                body.push_str(&format!("  %pcur_{hdr} = load i64, i64* %{cv}\n"));
                let g3 = self.reg();
                body.push_str(&format!("  %{g3} = getelementptr i8, i8* %{rh}, i64 16\n"));
                let lv = self.reg();
                body.push_str(&format!("  %{lv} = bitcast i8* %{g3} to i64*\n"));
                body.push_str(&format!("  %plim_{hdr} = load i64, i64* %{lv}\n"));
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
                let in_bvs = *by_value
                    && self
                        .by_value_locals
                        .get(&f.name)
                        .map(|s| s.contains(target))
                        .unwrap_or(false);
                if in_bvs {
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

    /// 提升路径的批量 bump（P4）：提升 latch 内**多个**同 region 定长分配
    /// 合成**一次**提升 bump——快路径 1 次溢出检查 + 纯寄存器划出 Σsize，
    /// 各对象指针由 `gep hbase, (al+offset)` 派生；慢路径 1 次扩容调用；
    /// 回边 phi 仅一组（ncur/nbase/nlim）。对齐等价性同 `try_emit_region_batch`。
    ///
    /// 返回消费的 stmt 数；promo 未命中或窗口不足 2 个时返回 `None`
    /// （调用方回落批量 bump / 单发）。
    fn try_emit_region_promo_batch(
        &mut self,
        stmts: &[LirStmt],
        start: usize,
        body: &mut String,
        f: &LirFunction,
    ) -> Result<Option<usize>, CodegenError> {
        // 窗口收集：同 `try_emit_region_batch`（`FieldSet`/`Assign`/`Binary`
        // 无副作用穿插可跳过），但**不排除** promo 命中。
        let mut allocs: Vec<(usize, String, String, usize)> = Vec::new();
        let mut window_end = start;
        for (j, st) in stmts.iter().enumerate().skip(start) {
            match st {
                LirStmt::AllocInRegionDirect { target, region, size } => {
                    if *size == 0 || local_type(f, target) != LirType::Ptr {
                        break;
                    }
                    if let Some((_, _, prev_region, _)) = allocs.first() {
                        if prev_region != region {
                            break;
                        }
                    }
                    allocs.push((j, target.clone(), region.clone(), *size));
                    window_end = j + 1;
                }
                LirStmt::FieldSet { .. } | LirStmt::Assign { .. } | LirStmt::Binary { .. } => {
                    window_end = j + 1; // 无副作用穿插，不打断窗口
                }
                _ => break,
            }
        }
        if allocs.len() < 2 {
            return Ok(None);
        }
        let region = allocs[0].2.clone();
        let hdr = match self.promo_header_for(&region) {
            Some(h) => h,
            None => return Ok(None),
        };
        let handle = format!("{region}.rh");
        let total: usize = allocs.iter().map(|(_, _, _, s)| s).sum();
        // 聚合提升 bump：各对象指针直接写各 target 槽
        self.emit_region_bump_promoted_batch(hdr, &handle, &allocs, total, body);
        // cont 为预分配 label（header phi 引用它）；穿插 stmt 落在 cont 内
        let cont = self.promo.as_ref().unwrap().cont_label.clone();
        body.push_str(&format!("  br label %{cont}\n"));
        body.push_str(&format!("{cont}:\n"));
        for j in start..window_end {
            if allocs.iter().any(|(k, _, _, _)| *k == j) {
                continue;
            }
            self.emit_stmt(&stmts[j], body, f)?;
        }
        Ok(Some(window_end - start))
    }

    /// 块级批量 bump（P3）：同一基本块内**连续出现**的同 region 定长分配
    /// （`AllocInRegionDirect`，size 均为 8 的倍数）聚合为
    /// **一次** bump——快路径 1 次溢出检查 + 1 次 cursor 写回，慢路径 1 次
    /// 扩容调用；各对象指针由 `gep base, offset` 派生。仅对未提升路径生效
    /// （提升路径已零访存，且其 cont label / header phi 结构不容跨 bump 聚合）。
    ///
    /// 语义等价性：size 为 `slot_count × 8`（8 的倍数），每次 bump 对齐后
    /// cursor 恒保持 8 对齐，故「一次 bump(Σsize) + 偏移派生」与「N 次独立
    /// bump(size_i)」逐字节一致；慢路径一次 `rlyeh_region_alloc(total)` 分配
    /// 连续整块，与 N 次调用等价（仅 alloc_count 统计差异——内联快路径
    /// 本就不计数，统计语义见 region.rs）。
    ///
    /// 返回消费的 stmt 数；起点非定长分配或窗口不足 2 个时返回 `None`
    /// （调用方按单发处理）。
    fn try_emit_region_batch(
        &mut self,
        stmts: &[LirStmt],
        start: usize,
        body: &mut String,
        f: &LirFunction,
    ) -> Result<Option<usize>, CodegenError> {
        // (stmt_idx, target, region, size)：收集连续 `AllocInRegionDirect`；
        // 窗口内允许穿插**无副作用** stmt（`FieldSet`/`Assign`/`Binary`，
        // 只读/写 LIR 槽与对象字段，不碰 Region 头）——聚合后原样发射，
        // 因其读的 target 槽已在聚合循环首部填充，顺序重排不改变语义。
        // 其他 stmt（`Call`、`AllocInRegion` 值镜像等）一律断开窗口：
        // 值镜像的 memcpy 源栈临时可能在窗口内才构造，提前 memcpy 会
        // 读到未初始化槽；Call 可能触发慢路径/写 Region 头。
        let mut allocs: Vec<(usize, String, String, usize)> = Vec::new();
        let mut window_end = start; // 窗口末端（不含）
        for (j, st) in stmts.iter().enumerate().skip(start) {
            match st {
                LirStmt::AllocInRegionDirect { target, region, size } => {
                    if *size == 0 || local_type(f, target) != LirType::Ptr {
                        break;
                    }
                    if self.promo_header_for(region).is_some() {
                        break; // 提升路径零访存，无聚合收益且结构不允许
                    }
                    if let Some((_, _, prev_region, _)) = allocs.first() {
                        if prev_region != region {
                            break;
                        }
                    }
                    allocs.push((j, target.clone(), region.clone(), *size));
                    window_end = j + 1;
                }
                LirStmt::FieldSet { .. } | LirStmt::Assign { .. } | LirStmt::Binary { .. } => {
                    window_end = j + 1; // 无副作用穿插，不打断窗口
                }
                _ => break,
            }
        }
        if allocs.len() < 2 {
            return Ok(None);
        }
        let region = allocs[0].2.clone();
        let handle = format!("{region}.rh");
        let total: usize = allocs.iter().map(|(_, _, _, s)| s).sum();
        // 首个分配消费 1 个汇合槽；其余对象指针由 base gep 派生，无需槽
        let slot0 = self.alloc_slots[self.alloc_slot_cursor].clone();
        self.alloc_slot_cursor += 1;
        let base = self.emit_inline_region_bump(&handle, total, &slot0, body);
        let mut offset = 0usize;
        for (idx, (_, target, _, size)) in allocs.iter().enumerate() {
            let p = if idx == 0 {
                base.clone() // offset 0：bump 返回指针即首对象
            } else {
                let r = self.reg();
                body.push_str(&format!(
                    "  %{r} = getelementptr i8, i8* %{base}, i64 {offset}\n"
                ));
                r
            };
            body.push_str(&format!("  store i8* %{p}, i8** %{target}.addr\n"));
            offset += *size;
        }
        // 原样发射窗口内穿插的无副作用 stmt（target 槽已在聚合循环首部填充）
        for j in start..window_end {
            if allocs.iter().any(|(k, _, _, _)| *k == j) {
                continue;
            }
            self.emit_stmt(&stmts[j], body, f)?;
        }
        Ok(Some(window_end - start))
    }

    /// 生成内联 bump 快路径（`AllocInRegion` / `AllocInRegionDirect` 共用）：
    /// 直接读写 repr(C) `Region` 首部固定偏移字段（base+0 / cursor+8 /
    /// limit+16 / alloc_count+24，见 rlyeh-region-alloc/src/region.rs
    /// FAST_*_OFF 常量），仅当前块空间不足时分支到慢路径
    /// （`call @rlyeh_region_alloc` 扩容）。
    ///
    /// 与 `Region::try_bump` 公式逐位一致：
    /// `aligned = (cursor+align-1) & !(align-1)`，`new = aligned+size`；
    /// 对齐下限 8（`MemoryBlock::BASE_ALIGN`，align 恒为 2 的幂）。
    ///
    /// 快路径/慢路径结果经 `slot`（entry 预分配的 `i8*` 槽）汇合，
    /// 返回汇合后的区域指针寄存器 `%p`。生成序列以 `br %cont` + `{cont}:`
    /// 结尾，原 block 的后续指令物理上落在 cont 内（emit_terminator 追加
    /// 的 br/ret 亦在其后），保证终结指令后紧跟 label 的 IR 合法。
    fn emit_inline_region_bump(
        &mut self,
        handle: &str,
        size: usize,
        slot: &str,
        body: &mut String,
    ) -> String {
        let align = 8u64; // 与 rlyeh-region-alloc MemoryBlock::BASE_ALIGN 一致
        let amask = align - 1; // 7
        let not_amask = !amask; // 18446744073709551607
        let b = self.reg();
        body.push_str(&format!("  %{b} = load i8*, i8** %{handle}\n"));
        let base = self.reg();
        body.push_str(&format!("  %{base} = load i8*, i8** %{b}, !tbaa !2\n"));
        let (cg, cv) = (self.reg(), self.reg());
        let (cur, lg, lv, lim) = (self.reg(), self.reg(), self.reg(), self.reg());
        body.push_str(&format!("  %{cg} = getelementptr i8, i8* %{b}, i64 8\n"));
        body.push_str(&format!("  %{cv} = bitcast i8* %{cg} to i64*\n"));
        body.push_str(&format!("  %{cur} = load i64, i64* %{cv}, !tbaa !2\n"));
        body.push_str(&format!("  %{lg} = getelementptr i8, i8* %{b}, i64 16\n"));
        body.push_str(&format!("  %{lv} = bitcast i8* %{lg} to i64*\n"));
        body.push_str(&format!("  %{lim} = load i64, i64* %{lv}, !tbaa !2\n"));
        let (m, al, new, ok) = (self.reg(), self.reg(), self.reg(), self.reg());
        body.push_str(&format!("  %{m} = add i64 %{cur}, {amask}\n"));
        body.push_str(&format!("  %{al} = and i64 %{m}, {not_amask}\n"));
        body.push_str(&format!("  %{new} = add i64 %{al}, {size}\n"));
        body.push_str(&format!("  %{ok} = icmp ule i64 %{new}, %{lim}\n"));
        let (fast, slow, join) = (self.label(), self.label(), self.label());
        body.push_str(&format!("  br i1 %{ok}, label %{fast}, label %{slow}\n"));
        // 注意：LLVM IR 中 label 定义不带 `%`（引用时才带 `%`）。
        body.push_str(&format!("{fast}:\n"));
        // 快路径：更新 cursor。alloc_count 不在此递增——为换取热路径
        // 性能，内联快路径不计数（统计语义见 rlyeh-region-alloc/src/region.rs）。
        body.push_str(&format!("  store i64 %{new}, i64* %{cv}, !tbaa !2\n"));
        let fp = self.reg();
        body.push_str(&format!("  %{fp} = getelementptr i8, i8* %{base}, i64 %{al}\n"));
        body.push_str(&format!("  store i8* %{fp}, i8** %{slot}\n"));
        body.push_str(&format!("  br label %{join}\n"));
        // 慢路径：调用运行时扩容并分配（cursor 未被快路径修改）
        body.push_str(&format!("{slow}:\n"));
        let sp = self.reg();
        body.push_str(&format!(
            "  %{sp} = call i8* @rlyeh_region_alloc(i8* %{b}, i64 {size}, i64 8) nounwind\n"
        ));
        body.push_str(&format!("  store i8* %{sp}, i8** %{slot}\n"));
        body.push_str(&format!("  br label %{join}\n"));
        // 汇合：取区域指针
        body.push_str(&format!("{join}:\n"));
        let p = self.reg();
        body.push_str(&format!("  %{p} = load i8*, i8** %{slot}\n"));
        p
    }

    /// 当前 bump 是否处于循环提升上下文（当前块 = 提升 latch 且 region
    /// 匹配），返回 header 块号（命名后缀）。
    fn promo_header_for(&self, region: &str) -> Option<usize> {
        let p = self.promo.as_ref()?;
        if p.latch == self.current_block && p.region == region {
            Some(p.header)
        } else {
            None
        }
    }

    /// 循环提升模式的 bump（热路径零访存，见 `PromoCtx` 注释）：
    /// 直接用 header phi 值 `%h{cur,base,lim}_{hdr}` 计算划出，快路径
    /// 无任何 Region 头访存；慢路径才 load handle、call 扩容并 reload
    /// base/cursor/limit；join 处生成回边值 phi（`%n{cur,base,lim}_{hdr}`）
    /// 供 header phi 的下一次迭代引用。
    ///
    /// 与 `emit_inline_region_bump` 相同：结果经 `slot` 汇合，返回 `%p`；
    /// 序列以 `br %cont` + `{cont}:` 结尾由调用方补齐。
    fn emit_region_bump_promoted(
        &mut self,
        hdr: usize,
        handle: &str,
        size: usize,
        slot: &str,
        body: &mut String,
    ) -> String {
        let align = 8u64; // 与 MemoryBlock::BASE_ALIGN 一致
        let amask = align - 1; // 7
        let not_amask = !amask; // 18446744073709551607
        let (m, al, new, ok) = (self.reg(), self.reg(), self.reg(), self.reg());
        body.push_str(&format!("  %{m} = add i64 %hcur_{hdr}, {amask}\n"));
        body.push_str(&format!("  %{al} = and i64 %{m}, {not_amask}\n"));
        body.push_str(&format!("  %{new} = add i64 %{al}, {size}\n"));
        body.push_str(&format!("  %{ok} = icmp ule i64 %{new}, %hlim_{hdr}\n"));
        // join 为预分配 label（header phi 已引用它）；fast/slow 新建
        debug_assert!(
            self.promo
                .as_ref()
                .is_some_and(|p| p.bumps.iter().any(|(_, s)| *s == size)),
            "promo bump size mismatch"
        );
        let join = self
            .promo
            .as_ref()
            .map(|p| p.join_label.clone())
            .expect("promo join label");
        let (fast, slow) = (self.label(), self.label());
        body.push_str(&format!("  br i1 %{ok}, label %{fast}, label %{slow}\n"));
        // 快路径：纯寄存器运算（cursor 更新经 join phi，不写 Region 头）
        body.push_str(&format!("{fast}:\n"));
        let fp = self.reg();
        body.push_str(&format!("  %{fp} = getelementptr i8, i8* %hbase_{hdr}, i64 %{al}\n"));
        body.push_str(&format!("  store i8* %{fp}, i8** %{slot}\n"));
        body.push_str(&format!("  br label %{join}\n"));
        // 慢路径：先把快路径寄存器 cursor 写回 Region 头 cursor 槽——
        // 提升后循环内 cursor 存于寄存器（phi），Region 头 cursor 保持进入
        // 循环时的旧值；若不写回，运行时 try_bump 会基于旧 cursor 判断，
        // 在「已分配区」重复返回指针（永远不扩容、快路径越界写）。
        // 写回后 try_bump 从正确位置判断越界 → grow 扩容 → 新块。
        // 扩容可能改写 base/limit/cursor（grow 三字段均更新），
        // 故 call 后全部 reload 并经 join phi 修正回边值。
        body.push_str(&format!("{slow}:\n"));
        let h = self.reg();
        body.push_str(&format!("  %{h} = load i8*, i8** %{handle}\n"));
        let (cg, cv) = (self.reg(), self.reg());
        body.push_str(&format!("  %{cg} = getelementptr i8, i8* %{h}, i64 8\n"));
        body.push_str(&format!("  %{cv} = bitcast i8* %{cg} to i64*\n"));
        body.push_str(&format!("  store i64 %hcur_{hdr}, i64* %{cv}, !tbaa !2\n"));
        let sp = self.reg();
        body.push_str(&format!(
            "  %{sp} = call i8* @rlyeh_region_alloc(i8* %{h}, i64 {size}, i64 8) nounwind\n"
        ));
        let rc = self.reg();
        body.push_str(&format!("  %{rc} = load i64, i64* %{cv}, !tbaa !2\n"));
        let rb = self.reg();
        body.push_str(&format!("  %{rb} = load i8*, i8** %{h}, !tbaa !2\n"));
        let (lg, lv) = (self.reg(), self.reg());
        body.push_str(&format!("  %{lg} = getelementptr i8, i8* %{h}, i64 16\n"));
        body.push_str(&format!("  %{lv} = bitcast i8* %{lg} to i64*\n"));
        let rl = self.reg();
        body.push_str(&format!("  %{rl} = load i64, i64* %{lv}, !tbaa !2\n"));
        body.push_str(&format!("  store i8* %{sp}, i8** %{slot}\n"));
        body.push_str(&format!("  br label %{join}\n"));
        // 汇合：phi 必须位于块首（先 phi 后 load slot）
        body.push_str(&format!("{join}:\n"));
        body.push_str(&format!(
            "  %ncur_{hdr} = phi i64 [ %{new}, %{fast} ], [ %{rc}, %{slow} ]\n"
        ));
        body.push_str(&format!(
            "  %nbase_{hdr} = phi i8* [ %hbase_{hdr}, %{fast} ], [ %{rb}, %{slow} ]\n"
        ));
        body.push_str(&format!(
            "  %nlim_{hdr} = phi i64 [ %hlim_{hdr}, %{fast} ], [ %{rl}, %{slow} ]\n"
        ));
        let p = self.reg();
        body.push_str(&format!("  %{p} = load i8*, i8** %{slot}\n"));
        p
    }

    /// 批量提升 bump 发射（fast/slow/join）：一次溢出检查划出 Σsize，
    /// 各对象指针由 `gep hbase, (al+offset)`（快）/ `gep sp, offset`（慢）
    /// 派生并写入各自 target 槽。快路径不写 Region 头（cursor 经 join phi
    /// 回传）；慢路径先写回寄存器 cursor 再扩容（grow 改写 base/cursor/
    /// limit，call 后全量 reload），join 处一组 ncur/nbase/nlim phi。
    /// 对齐等价性同 `try_emit_region_batch`（size 均为 8 的倍数）。
    fn emit_region_bump_promoted_batch(
        &mut self,
        hdr: usize,
        handle: &str,
        allocs: &[(usize, String, String, usize)],
        total: usize,
        body: &mut String,
    ) {
        let align = 8u64; // 与 MemoryBlock::BASE_ALIGN 一致
        let amask = align - 1;
        let not_amask = !amask;
        let (m, al, new, ok) = (self.reg(), self.reg(), self.reg(), self.reg());
        body.push_str(&format!("  %{m} = add i64 %hcur_{hdr}, {amask}\n"));
        body.push_str(&format!("  %{al} = and i64 %{m}, {not_amask}\n"));
        body.push_str(&format!("  %{new} = add i64 %{al}, {total}\n"));
        body.push_str(&format!("  %{ok} = icmp ule i64 %{new}, %hlim_{hdr}\n"));
        let join = self
            .promo
            .as_ref()
            .map(|p| p.join_label.clone())
            .expect("promo join label");
        let (fast, slow) = (self.label(), self.label());
        body.push_str(&format!("  br i1 %{ok}, label %{fast}, label %{slow}\n"));
        // 快路径：纯寄存器运算，各对象指针由 hbase 偏移派生
        body.push_str(&format!("{fast}:\n"));
        let mut offset = 0usize;
        for (_, target, _, size) in allocs {
            let fp = self.reg();
            body.push_str(&format!(
                "  %{fp} = getelementptr i8, i8* %hbase_{hdr}, i64 %{al}\n"
            ));
            let p = if offset == 0 {
                fp
            } else {
                let r = self.reg();
                body.push_str(&format!("  %{r} = getelementptr i8, i8* %{fp}, i64 {offset}\n"));
                r
            };
            body.push_str(&format!("  store i8* %{p}, i8** %{target}.addr\n"));
            offset += *size;
        }
        body.push_str(&format!("  br label %{join}\n"));
        // 慢路径：写回寄存器 cursor → 扩容 → reload → 各对象指针
        body.push_str(&format!("{slow}:\n"));
        let h = self.reg();
        body.push_str(&format!("  %{h} = load i8*, i8** %{handle}\n"));
        let (cg, cv) = (self.reg(), self.reg());
        body.push_str(&format!("  %{cg} = getelementptr i8, i8* %{h}, i64 8\n"));
        body.push_str(&format!("  %{cv} = bitcast i8* %{cg} to i64*\n"));
        body.push_str(&format!("  store i64 %hcur_{hdr}, i64* %{cv}, !tbaa !2\n"));
        let sp = self.reg();
        body.push_str(&format!(
            "  %{sp} = call i8* @rlyeh_region_alloc(i8* %{h}, i64 {total}, i64 8) nounwind\n"
        ));
        let rc = self.reg();
        body.push_str(&format!("  %{rc} = load i64, i64* %{cv}, !tbaa !2\n"));
        let rb = self.reg();
        body.push_str(&format!("  %{rb} = load i8*, i8** %{h}, !tbaa !2\n"));
        let (lg, lv) = (self.reg(), self.reg());
        body.push_str(&format!("  %{lg} = getelementptr i8, i8* %{h}, i64 16\n"));
        body.push_str(&format!("  %{lv} = bitcast i8* %{lg} to i64*\n"));
        let rl = self.reg();
        body.push_str(&format!("  %{rl} = load i64, i64* %{lv}, !tbaa !2\n"));
        let mut offset = 0usize;
        for (_, target, _, size) in allocs {
            let fp = self.reg();
            body.push_str(&format!("  %{fp} = getelementptr i8, i8* %{sp}, i64 {offset}\n"));
            body.push_str(&format!("  store i8* %{fp}, i8** %{target}.addr\n"));
            offset += *size;
        }
        body.push_str(&format!("  br label %{join}\n"));
        // 汇合：phi 位于块首，一组回边值
        body.push_str(&format!("{join}:\n"));
        body.push_str(&format!(
            "  %ncur_{hdr} = phi i64 [ %{new}, %{fast} ], [ %{rc}, %{slow} ]\n"
        ));
        body.push_str(&format!(
            "  %nbase_{hdr} = phi i8* [ %hbase_{hdr}, %{fast} ], [ %{rb}, %{slow} ]\n"
        ));
        body.push_str(&format!(
            "  %nlim_{hdr} = phi i64 [ %hlim_{hdr}, %{fast} ], [ %{rl}, %{slow} ]\n"
        ));
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
                // 函数指针值（i8* 槽）→ i64 形参（S0 线程入口 `__rlyeh_thread_spawn(f, 0)`）：
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
        } else if self.ret_by_value.contains(callee) {
            // 标量聚合按值返回：`call {i64, i64}`，解包两槽写入 target 栈槽
            // （`{tag, payload}` 寄存器返回；target.obj 已在 entry 预分配）。
            let r = self.reg();
            body.push_str(&format!(
                "  %{r} = call {{ i64, i64 }} @{callee_name}({arg_str})\n"
            ));
            if let Some(t) = target {
                let e0 = self.reg();
                body.push_str(&format!("  %{e0} = extractvalue {{ i64, i64 }} %{r}, 0\n"));
                let e1 = self.reg();
                body.push_str(&format!("  %{e1} = extractvalue {{ i64, i64 }} %{r}, 1\n"));
                // target 被预扫描判定为逃逸（地址将被存入堆对象 / 传出）：
                // 本体改由 calloc 堆承载两槽（by_value 约定 ≤2 槽，固定
                // 16 字节），不能写 entry 栈槽（会悬垂）。
                let in_bvs = self
                    .by_value_locals
                    .get(&f.name)
                    .map(|s| s.contains(t))
                    .unwrap_or(false);
                if in_bvs {
                    // Apple clang 21 不接受聚合类型 GEP 引用函数局部值
                    // （`getelementptr inbounds ([2 x i64], [2 x i64]* %x, ...)`
                    // 报 `invalid use of function-local name`），按项目惯例改
                    // i8 字节偏移：bitcast 为 i8* → `getelementptr i8` →
                    // bitcast 回 i64*。
                    let b0 = self.reg();
                    body.push_str(&format!("  %{b0} = bitcast [2 x i64]* %{t}.obj to i8*\n"));
                    let g0 = self.reg();
                    body.push_str(&format!("  %{g0} = getelementptr i8, i8* %{b0}, i64 0\n"));
                    let p0 = self.reg();
                    body.push_str(&format!("  %{p0} = bitcast i8* %{g0} to i64*\n"));
                    body.push_str(&format!("  store i64 %{e0}, i64* %{p0}\n"));
                    let b1 = self.reg();
                    body.push_str(&format!("  %{b1} = bitcast [2 x i64]* %{t}.obj to i8*\n"));
                    let g1 = self.reg();
                    body.push_str(&format!("  %{g1} = getelementptr i8, i8* %{b1}, i64 8\n"));
                    let p1 = self.reg();
                    body.push_str(&format!("  %{p1} = bitcast i8* %{g1} to i64*\n"));
                    body.push_str(&format!("  store i64 %{e1}, i64* %{p1}\n"));
                    let p = self.reg();
                    body.push_str(&format!("  %{p} = bitcast [2 x i64]* %{t}.obj to i8*\n"));
                    body.push_str(&format!("  store i8* %{p}, i8** %{t}.addr\n"));
                } else {
                    let r64 = self.reg();
                    body.push_str(&format!("  %{r64} = call i64 @calloc(i64 1, i64 16)\n"));
                    let p = self.reg();
                    body.push_str(&format!("  %{p} = inttoptr i64 %{r64} to i8*\n"));
                    let b0 = self.reg();
                    body.push_str(&format!("  %{b0} = bitcast i8* %{p} to i64*\n"));
                    body.push_str(&format!("  store i64 %{e0}, i64* %{b0}\n"));
                    let g1 = self.reg();
                    body.push_str(&format!("  %{g1} = getelementptr i8, i8* %{p}, i64 8\n"));
                    let b1 = self.reg();
                    body.push_str(&format!("  %{b1} = bitcast i8* %{g1} to i64*\n"));
                    body.push_str(&format!("  store i64 %{e1}, i64* %{b1}\n"));
                    body.push_str(&format!("  store i8* %{p}, i8** %{t}.addr\n"));
                }
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
        // 实参（不特判 extern String：间接调用目标为 Rlyeh 函数，
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
        // 动态数组分配：target = calloc(n, 8)（每槽 8 字节，与数组元素步长一致）。
        // 使用 calloc 清零：读未初始化内存是 LLVM 层面的 poison/UB，O2 优化管线
        // 可能据此产生未定义行为（如空 HashMap 迭代死循环）；清零保证确定语义。
        if callee == "alloc_array" {
            let n =
                self.operand_value(&LirOperand::Local(args[0].clone()), LirType::I64, body, f)?;
            let r64 = self.reg();
            body.push_str(&format!("  %{r64} = call i64 @calloc(i64 {n}, i64 8)\n"));
            let r = self.reg();
            body.push_str(&format!("  %{r} = inttoptr i64 %{r64} to i8*\n"));
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
        // 字节缓冲分配（String 动态缓冲）：target = calloc(n, 1)（按字节，无步长缩放）。
        // calloc 清零：与 alloc_array 一致，杜绝未初始化内存 UB。
        if callee == "alloc_bytes" {
            let n =
                self.operand_value(&LirOperand::Local(args[0].clone()), LirType::I64, body, f)?;
            let r64 = self.reg();
            body.push_str(&format!("  %{r64} = call i64 @calloc(i64 {n}, i64 1)\n"));
            let r = self.reg();
            body.push_str(&format!("  %{r} = inttoptr i64 %{r64} to i8*\n"));
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
    // 返回 `()` 的函数指针：void 返回（函数定义侧 `define void`，见 gen_function）
    let ret = if ret_ty == LirType::Unit {
        "void".to_string()
    } else {
        llvm_type(ret_ty)?.to_string()
    };
    Ok(format!("{ret}({params})*"))
}

/// 二元运算指令映射。
fn binary_instr(op: &rlyeh_lir::HirBinaryOp, ty: LirType) -> Result<&'static str, CodegenError> {
    use rlyeh_lir::HirBinaryOp::*;
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
