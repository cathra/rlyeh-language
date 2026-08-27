//! llvm_ctor：LLVM 发射子模块。
//! （由 llvm.rs 的 `impl LlvmEmitter` 拆分而来，保持语义等价）

use super::*;

impl LlvmEmitter {
    pub(super) fn new(program: &LirProgram) -> Self {
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
        // V2-C（2026-08-26，方案 A）：排除 `StrFat`（`&str` 双槽 `{data, len}`）。
        // StrFat 是标量双字值，按方案 A 直接存于 `.addr` 值槽（`{i8*,i64}*`），
        // **不作为 by_value `.obj` 对象**——否则 by_value `Alloc` 会把 `.addr`
        // bitcast 成 `i8**` 写入 `.obj` 地址，污染 data 槽（根因，见 task-v2.md）。
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
                        let is_strfat = f
                            .locals
                            .iter()
                            .any(|(n, t)| n == target && *t == LirType::StrFat);
                        if !is_strfat {
                            bvs.insert(target.clone());
                        }
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
    }}
