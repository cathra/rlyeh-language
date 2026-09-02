//! llvm_region：LLVM 发射子模块。
//! （由 llvm.rs 的 `impl LlvmEmitter` 拆分而来，保持语义等价）

use super::*;

impl LlvmEmitter {
    pub(super) fn try_emit_region_promo_batch(
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

    pub(super) fn try_emit_region_batch(
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

    pub(super) fn emit_inline_region_bump(
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

    pub(super) fn promo_header_for(&self, region: &str) -> Option<usize> {
        let p = self.promo.as_ref()?;
        if p.latch == self.current_block && p.region == region {
            Some(p.header)
        } else {
            None
        }
    }

    pub(super) fn emit_region_bump_promoted(
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

    pub(super) fn emit_region_bump_promoted_batch(
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

    /// L3 region 接线：`emit_stmt` 的 region 分支下沉。
    ///
    /// `llvm.rs` 的 `emit_stmt` 以 or-pattern 把这 5 种 region 语句**整体**分派到
    /// 此处，故其余变体不可达（违反即报 `CodegenError::Internal`，而非静默跳过
    /// 发射）。区域句柄槽 `%{key}.rh` 已在入口块预分配。
    pub(super) fn emit_region_stmt(
        &mut self,
        stmt: &LirStmt,
        f: &LirFunction,
        body: &mut String,
    ) -> Result<(), CodegenError> {
        match stmt {
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
            _ => {
                return Err(CodegenError::Internal(format!(
                    "emit_region_stmt 收到非 region 语句：{stmt:?}"
                )));
            }
        }
        Ok(())
    }
}
