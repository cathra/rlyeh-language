//! llvm_func：LLVM 发射子模块。
//! （由 llvm.rs 的 `impl LlvmEmitter` 拆分而来，保持语义等价）

use super::*;

impl LlvmEmitter {
    pub(super) fn emit_function(&mut self, f: &LirFunction) -> Result<(), CodegenError> {
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
    }}
