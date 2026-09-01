//! 表达式检查子模块：llvm_util。
//! （由 llvm.rs 二次拆分而来，保持语义等价）

use super::*;
use rlyeh_lir::ReprConv;

pub(crate) fn touches_any(s: &LirStmt, aliases: &[&str]) -> bool {
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
        FieldAddr { target, base, .. } => {
            aliases.contains(&target.as_str()) || aliases.contains(&base.as_str())
        }
        PtrAdd {
            target, base, offset, ..
        } => {
            aliases.contains(&target.as_str())
                || aliases.contains(&base.as_str())
                || aliases.contains(&offset.as_str())
        }
        Cast { target, value, .. } => {
            aliases.contains(&target.as_str())
                || value.as_local().map(|l| aliases.contains(&l.as_str())).unwrap_or(false)
        }
        Alloc { target, .. } => aliases.contains(&target.as_str()),
        RegionEnter { .. }
        | RegionExit { .. }
        | Transfer { .. }
        | AllocInRegion { .. }
        | AllocInRegionDirect { .. } => false,
    }
}

pub(crate) fn inline_region_literal(f: &mut LirFunction) {
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

pub(crate) fn block_succs(b: &LirBlock) -> Vec<usize> {
    match &b.terminator {
        LirTerminator::Jump(t) => vec![*t],
        LirTerminator::CondJump { then, otherwise, .. } => vec![*then, *otherwise],
        LirTerminator::Return(_) => Vec::new(),
    }
}

pub(crate) fn compute_dominators(f: &LirFunction) -> Vec<HashSet<usize>> {
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

pub(crate) fn find_loop_promo(f: &LirFunction) -> Option<PromoCtx> {
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

pub(crate) fn no_promo_escape(
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

pub(crate) fn stmt_used_locals(st: &LirStmt) -> Vec<&String> {
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
        LirStmt::FieldAddr { target, base, .. } => {
            out.push(target);
            out.push(base);
        }
        LirStmt::PtrAdd { target, base, offset, .. } => {
            out.push(target);
            out.push(base);
            out.push(offset);
        }
        LirStmt::Cast { value, .. } => push_local(&mut out, value),
        LirStmt::AllocInRegion { .. }
        | LirStmt::AllocInRegionDirect { .. }
        | LirStmt::Alloc { .. }
        | LirStmt::RegionEnter { .. }
        | LirStmt::RegionExit { .. }
        | LirStmt::Transfer { .. } => {}
    }
    out
}

pub(crate) fn propagate_by_value_aliases(bvs: &mut HashSet<String>, f: &LirFunction) {
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

pub(crate) fn llvm_global_name(name: &str) -> String {
    if name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'$' | b'.' | b'_'))
    {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

pub(crate) fn field_scalar_llvm(ty: FieldScalar) -> Result<&'static str, CodegenError> {
    Ok(match ty {
        FieldScalar::Int => "i64",
        FieldScalar::Float => "double",
        FieldScalar::Bool => "i1",
        // char：32 位 Unicode 码点（V2 拓宽：原为 i8 仅 ASCII，现支持全部
        // Unicode 码点 0..=0x10FFFF，解锁 Chars::next() -> Option<char>）
        FieldScalar::Char => "i32",
        FieldScalar::Str => "i8*",
        // &str 胖指针：`{ i8*, i64 }`（data 指针 + 长度）双槽
        FieldScalar::StrFat => "{ i8*, i64 }",
        // 切片胖指针：`&[T]`（data 指针 + 长度）双槽，与 StrFat 同布局
        FieldScalar::SliceFat => "{ i8*, i64 }",
        FieldScalar::Ptr => "i8*",
        // repr(C) 真布局：返回字段在内存中的窄 LLVM 类型（load/store 按此类型）。
        FieldScalar::ReprCField { field_ty, .. } => field_ty,
        // repr(C) 嵌套聚合子对象：结果是指向内联子对象的指针（i8*）。
        FieldScalar::ReprCSubPtr { .. } => "i8*",
    })
}

pub(crate) fn field_scalar_lir(ty: FieldScalar) -> LirType {
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

pub(crate) fn llvm_type(ty: LirType) -> Result<&'static str, CodegenError> {
    match ty {
        LirType::I64 => Ok("i64"),
        LirType::F64 => Ok("double"),
        LirType::Bool => Ok("i1"),
        LirType::Char => Ok("i32"),
        LirType::Str => Ok("i8*"),
        // &str 胖指针：`{ i8*, i64 }`（data 指针 + 长度）双槽
        LirType::StrFat => Ok("{ i8*, i64 }"),
        // 切片胖指针：与 StrFat 同布局
        LirType::SliceFat => Ok("{ i8*, i64 }"),
        LirType::Ptr => Ok("i8*"),
        LirType::Unit => Err(CodegenError::UnsupportedType {
            ty,
            context: "单元类型不能作为存储 / 参数 / 运算类型".to_string(),
        }),
    }
}

pub(crate) fn fn_llvm_type(param_tys: &[LirType], ret_ty: LirType) -> Result<String, CodegenError> {
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

pub(crate) fn binary_instr(op: &rlyeh_lir::HirBinaryOp, ty: LirType) -> Result<&'static str, CodegenError> {
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

pub(crate) fn builtin_fmt(ty: LirType, newline: bool) -> String {
    let base = match ty {
        LirType::Str => "%s",
        LirType::StrFat => "%p",
        // 切片 MVP 不直接打印（与 Unit 同处理：空格式串）
        LirType::SliceFat => "",
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

pub(crate) fn escape_llvm_string(s: &str) -> (String, usize) {
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

pub(crate) fn local_type(f: &LirFunction, name: &str) -> LirType {
    f.locals
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, t)| *t)
        .unwrap_or(LirType::I64)
}

pub(crate) fn parse_cast_target(to: &str) -> (u32, bool, LirType) {
    match to {
        "bool" => (1, false, LirType::Bool),
        "char" => (32, false, LirType::Char),
        "f32" | "f64" => (64, true, LirType::F64),
        "u8" => (8, false, LirType::I64),
        "u16" => (16, false, LirType::I64),
        "u32" => (32, false, LirType::I64),
        "u64" | "usize" => (64, false, LirType::I64),
        "i8" => (8, true, LirType::I64),
        "i16" => (16, true, LirType::I64),
        "i32" => (32, true, LirType::I64),
        // i64 / isize / 其余整数目标
        _ => (64, true, LirType::I64),
    }
}